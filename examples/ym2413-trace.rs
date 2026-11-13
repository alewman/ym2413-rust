//! Run lockstep scripts (rungs 2 and 3) on this crate and print the trace that
//! `scripts/lockstep_ref.py` prints for ym2413-python. The two are compared byte for byte.
//!
//! ```text
//! ym2413-trace SCRIPT [SCRIPT ...] [--out-dir DIR]
//! ```
//!
//! With one script and no `--out-dir` the trace goes to stdout; otherwise each script's trace is
//! written to `DIR/<name>.trace`. The script format is described in `scripts/lockstep_gen.py`.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use ym2413::tables::{tables, TREMOLO};
use ym2413::{OPLLVariant, DS1001, OPLL, YM2413};

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv(hash: u64, value: i32) -> u64 {
    (hash ^ (value as u64 & 0xFFFF)).wrapping_mul(FNV_PRIME)
}

fn join<T: std::fmt::Display>(values: &[T]) -> String {
    let mut out = String::new();
    for (i, value) in values.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        write!(out, "{value}").unwrap();
    }
    out
}

fn state(chip: &OPLL) -> String {
    let mut registers = String::new();
    for byte in chip.registers {
        write!(registers, "{byte:02X}").unwrap();
    }
    let lists: [(&str, &[i32]); 23] = [
        ("phase", &chip.phase),
        ("level", &chip.level),
        ("state", &chip.state),
        ("key", &chip.key),
        ("previous", &chip.previous),
        ("before_previous", &chip.before_previous),
        ("held", &chip.held),
        ("tremolo_on", &chip.tremolo_on),
        ("vibrato_on", &chip.vibrato_on),
        ("sustained", &chip.sustained),
        ("rate_scaling", &chip.rate_scaling),
        ("multiplier", &chip.multiplier),
        ("key_scale", &chip.key_scale),
        ("base", &chip.base),
        ("half_sine", &chip.half_sine),
        ("attack_rate", &chip.attack_rate),
        ("decay_rate", &chip.decay_rate),
        ("sustain_level", &chip.sustain_level),
        ("release_rate", &chip.release_rate),
        ("releases", &chip.releases),
        ("feedback", &chip.feedback),
        ("melody", &chip.melody),
        ("rhythm", &chip.rhythm),
    ];
    let mut out = format!(
        "state clock_phase={} slot={} melody_sum={} rhythm_sum={} address={} test={} counter={} \
         tremolo_time={} vibrato_time={} tremolo={} noise={} rhythm_mode={} registers={}",
        chip.clock_phase,
        chip.slot,
        chip.melody_sum,
        chip.rhythm_sum,
        chip.address,
        chip.test,
        chip.counter,
        chip.tremolo_time,
        chip.vibrato_time,
        chip.tremolo,
        chip.noise,
        chip.rhythm_mode,
        registers
    );
    for (name, values) in lists {
        write!(out, " | {name} {}", join(values)).unwrap();
    }
    out
}

fn words_line(sample: &[(i32, i32)]) -> String {
    let mut out = String::from("s");
    for (melody, rhythm) in sample {
        write!(out, " {melody},{rhythm}").unwrap();
    }
    out
}

fn trace(lines: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    let variant: OPLLVariant = match lines[0].split_whitespace().nth(1) {
        Some("YM2413") => YM2413,
        Some("DS1001") => DS1001,
        other => panic!("unknown variant {other:?}"),
    };
    let digest = lines[1].split_whitespace().nth(1) == Some("digest");
    let mut chip = OPLL::new(variant);
    for line in &lines[2..] {
        let mut parts = line.split_whitespace();
        let op = parts.next().unwrap_or("");
        let args: Vec<&str> = parts.collect();
        match op {
            "w" => chip.write(
                u8::from_str_radix(args[0], 16).unwrap(),
                u8::from_str_radix(args[1], 16).unwrap(),
            ),
            "run" => chip.run(args[0].parse().unwrap()),
            "reset" => chip.reset(),
            "trace" => {
                chip.trace = if args[0] == "1" {
                    Some(Vec::new())
                } else {
                    None
                };
            }
            "take" => {
                let pairs = chip.take_outputs();
                let samples: Vec<Vec<(i32, i32)>> = match &mut chip.trace {
                    Some(trace) => {
                        // keep a sample still being filled, so no slot word is lost
                        let keep = match trace.last() {
                            Some(last) if last.len() < 18 => trace.pop(),
                            _ => None,
                        };
                        let done = std::mem::take(trace);
                        if let Some(last) = keep {
                            trace.push(last);
                        }
                        done
                    }
                    None => Vec::new(),
                };
                if digest {
                    let mut hash = FNV_OFFSET;
                    for &(melody, rhythm) in &pairs {
                        hash = fnv(fnv(hash, melody), rhythm);
                    }
                    let mut words = FNV_OFFSET;
                    let mut count = 0;
                    for sample in &samples {
                        for &(melody, rhythm) in sample {
                            words = fnv(fnv(words, melody), rhythm);
                            count += 1;
                        }
                    }
                    out.push(format!(
                        "take {} {hash:016x} words {} {count} {words:016x}",
                        pairs.len(),
                        samples.len()
                    ));
                } else {
                    let mut line = format!("take {}", pairs.len());
                    for (melody, rhythm) in &pairs {
                        write!(line, " {melody},{rhythm}").unwrap();
                    }
                    out.push(line);
                    for sample in &samples {
                        out.push(words_line(sample));
                    }
                }
            }
            "samples" => {
                let values = chip.take_samples();
                out.push(format!("samples {} {}", values.len(), join(&values)));
            }
            "state" => out.push(state(&chip)),
            "tables" => {
                let t = tables();
                out.push(format!("log_sin {}", join(&t.log_sin)));
                out.push(format!("exp {}", join(&t.exp)));
                let mut steps = Vec::new();
                for code in &t.key_scale_steps {
                    for block in code {
                        steps.extend_from_slice(block);
                    }
                }
                out.push(format!("key_scale_steps {}", join(&steps)));
                out.push(format!("tremolo {}", join(&TREMOLO)));
            }
            "" => {}
            other => panic!("unknown op {other}"),
        }
    }
    out
}

fn main() {
    let mut scripts: Vec<PathBuf> = Vec::new();
    let mut out_dir: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--out-dir" {
            out_dir = Some(PathBuf::from(args.next().expect("--out-dir DIR")));
        } else {
            scripts.push(PathBuf::from(arg));
        }
    }
    for script in &scripts {
        let text = fs::read_to_string(script).expect("read script");
        let lines: Vec<&str> = text.lines().collect();
        let mut result = trace(&lines).join("\n");
        result.push('\n');
        match (&out_dir, scripts.len()) {
            (None, 1) => print!("{result}"),
            (dir, _) => {
                let dir = dir.as_deref().unwrap_or(Path::new("."));
                let name = script.file_stem().unwrap().to_string_lossy();
                fs::write(dir.join(format!("{name}.trace")), result).expect("write trace");
            }
        }
    }
}
