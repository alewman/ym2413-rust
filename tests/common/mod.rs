//! Shared by the tests: ways to drive the chip as a board would and to read it.
//!
//! A transcription of ym2413-python's `tests/helpers.py` (at f88dccf), plus two pieces the Rust
//! tests need that Python has built in or shares between its test files:
//!
//! - `PyRandom`: Python's `random.Random` (MT19937 with CPython's integer seeding,
//!   `randrange`, `choice`, `random`), so the contract tests draw the same streams as the
//!   reference's;
//! - `measured`: `rhythm_chip` and `two_operator_reference` of `tests/test_measured.py`, which
//!   `tests/test_provisional.py` imports from there.
//!
//! Python's keyword arguments become a `Tone` record (`tone_chip`) or extra functions (`note_k`,
//! `note_ks`). `attribute(chip, name, ...)` reads a field by name in Python; here it takes a
//! closure that reads the field.
//!
//! Tests read what `take_outputs()` returns, or the chip's own word train through
//! `trace`, wherever the statement under test is about sound. Where a statement is
//! about an envelope level, a phase or a counter (most of the measured ones are),
//! they read the plain field that holds it: `level`, `phase`, `counter`.
//!
//! `reference_value` is a second, separate writing of spec sections 6.2 to 6.5 from
//! the two ROM formulas, so that tests about modulation and feedback predict the
//! chip's numbers instead of repeating its code.

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::OnceLock;

use ym2413::variants::Row;
use ym2413::{OPLLVariant, CLOCKS_PER_CYCLE, CYCLES_PER_SAMPLE, OPLL, YM2413};

/// This repository's root (Python's `ROOT`).
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub const SAMPLE: u64 = CLOCKS_PER_CYCLE * CYCLES_PER_SAMPLE as u64; // 72 input clocks
pub const CLOCK_HZ: u64 = 3_579_545; // the reference clock of S1 section IV-1
pub const SAMPLE_MS: f64 = SAMPLE as f64 / CLOCK_HZ as f64 * 1000.0;

// Slots (spec 2.4).
pub const M: [usize; 9] = [0, 1, 2, 6, 7, 8, 12, 13, 14];
pub const C: [usize; 9] = [3, 4, 5, 9, 10, 11, 15, 16, 17];

/// A carrier that plays a plain sine at full level and a modulator that stays
/// silent: its attack rate is 0, so after the damp phase it never leaves the
/// quiet end of its envelope and outputs +0 (spec 6.8, 8.13).
pub const PLAIN_SINE: Row = [0x21, 0x21, 0x3F, 0x00, 0x00, 0xF0, 0x00, 0x00];

/// One register write with the waits Yamaha asks for (spec 3.3), then rounded
/// up to a whole number of samples so that tests can count samples.
pub fn write(chip: &mut OPLL, register: u8, value: u8) {
    chip.write(0, register);
    chip.run(12);
    chip.write(1, value);
    chip.run(84 + 48);
}

pub fn user_tone(chip: &mut OPLL, tone: &[u8]) {
    for (register, &value) in tone.iter().enumerate() {
        write(chip, register as u8, value);
    }
}

/// `note(chip, channel, f_number, block, key=1, sus=0)`
pub fn note(chip: &mut OPLL, channel: u8, f_number: i32, block: i32) {
    note_ks(chip, channel, f_number, block, 1, 0);
}

/// `note(chip, channel, f_number, block, key=key)`
pub fn note_k(chip: &mut OPLL, channel: u8, f_number: i32, block: i32, key: i32) {
    note_ks(chip, channel, f_number, block, key, 0);
}

pub fn note_ks(chip: &mut OPLL, channel: u8, f_number: i32, block: i32, key: i32, sus: i32) {
    write(chip, 0x10 + channel, (f_number & 0xFF) as u8);
    write(
        chip,
        0x20 + channel,
        (sus << 5 | key << 4 | block << 1 | f_number >> 8) as u8,
    );
}

/// Key a channel on without running the chip: the next sample is the first
/// in which the key is seen.
pub fn press(chip: &mut OPLL, channel: u8, f_number: i32, block: i32) {
    chip.write(0, 0x20 + channel);
    chip.write(1, (1 << 4 | block << 1 | f_number >> 8) as u8);
}

/// A chip with the user tone on channel 0, keyed on this instant.
pub fn struck(tone: &[u8], f_number: i32, block: i32) -> OPLL {
    let mut chip = tone_chip(
        tone,
        Tone {
            f_number,
            block,
            key: 0,
            ..Tone::default()
        },
    );
    press(&mut chip, 0, f_number, block);
    chip
}

/// The keyword arguments of `tone_chip`, with Python's defaults.
#[derive(Clone)]
pub struct Tone {
    pub f_number: i32,
    pub block: i32,
    pub volume: u8,
    pub channel: u8,
    pub variant: OPLLVariant,
    pub key: i32,
}

impl Default for Tone {
    fn default() -> Self {
        Tone {
            f_number: 0x100,
            block: 4,
            volume: 0,
            channel: 0,
            variant: YM2413,
            key: 1,
        }
    }
}

/// A chip playing the user tone on one channel.
pub fn tone_chip(tone: &[u8], options: Tone) -> OPLL {
    let mut chip = OPLL::new(options.variant);
    user_tone(&mut chip, tone);
    write(&mut chip, 0x30 + options.channel, options.volume);
    note_k(
        &mut chip,
        options.channel,
        options.f_number,
        options.block,
        options.key,
    );
    chip.take_outputs();
    chip
}

/// `tone_chip(tone)` with every default.
pub fn plain(tone: &[u8]) -> OPLL {
    tone_chip(tone, Tone::default())
}

/// `tone_chip(tone, f_number=..., block=...)`
pub fn pitched(tone: &[u8], f_number: i32, block: i32) -> OPLL {
    tone_chip(
        tone,
        Tone {
            f_number,
            block,
            ..Tone::default()
        },
    )
}

pub fn samples(chip: &mut OPLL, count: u64) -> Vec<i32> {
    chip.run(count * SAMPLE);
    chip.take_samples()
}

pub fn melody(chip: &mut OPLL, count: u64) -> Vec<i32> {
    chip.run(count * SAMPLE);
    chip.take_outputs().into_iter().map(|pair| pair.0).collect()
}

pub fn rhythm(chip: &mut OPLL, count: u64) -> Vec<i32> {
    chip.run(count * SAMPLE);
    chip.take_outputs().into_iter().map(|pair| pair.1).collect()
}

/// The operator's envelope level after each of `count` samples.
pub fn levels(chip: &mut OPLL, operator: usize, count: usize) -> Vec<i32> {
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        chip.run(SAMPLE);
        out.push(chip.level[operator]);
    }
    chip.take_outputs();
    out
}

/// `attribute(chip, name, index, count)`: the field `read` picks out, after each of `count`
/// samples.
pub fn attribute(chip: &mut OPLL, read: impl Fn(&OPLL) -> i32, count: usize) -> Vec<i32> {
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        chip.run(SAMPLE);
        out.push(read(chip));
    }
    chip.take_outputs();
    out
}

/// The eighteen (melody word, rhythm word) pairs of each of `count` samples.
///
/// Call it on a sample boundary. Words are in the chip's form: -1 is minus zero.
pub fn words(chip: &mut OPLL, count: u64) -> Vec<Vec<(i32, i32)>> {
    chip.trace = Some(Vec::new());
    chip.run(count * SAMPLE);
    let trace = chip.trace.take().unwrap();
    chip.take_outputs();
    trace
}

/// (value, length) for each run of equal values.
pub fn runs<T: Copy + PartialEq>(values: &[T]) -> Vec<(T, usize)> {
    let mut out: Vec<(T, usize)> = Vec::new();
    for &value in values {
        match out.last_mut() {
            Some(last) if last.0 == value => last.1 += 1,
            _ => out.push((value, 1)),
        }
    }
    out
}

/// Upward crossings of zero.
pub fn rises(values: &[i32]) -> usize {
    values.windows(2).filter(|w| w[0] <= 0 && 0 < w[1]).count()
}

// A second writing of the operator's arithmetic, from the formulas of spec 6.3 to
// 6.5 (S11's, as S4's prose restates them).
fn rom() -> &'static ([i32; 256], [i32; 256]) {
    static ROM: OnceLock<([i32; 256], [i32; 256])> = OnceLock::new();
    ROM.get_or_init(|| {
        let mut log_sin = [0; 256];
        let mut exp = [0; 256];
        for i in 0..256 {
            log_sin[i] = (-((i as f64 + 0.5) * std::f64::consts::PI / 512.0)
                .sin()
                .log2()
                * 256.0)
                .round_ties_even() as i32;
            exp[i] = ((2f64.powf(i as f64 / 256.0) - 1.0) * 1024.0).round_ties_even() as i32;
        }
        (log_sin, exp)
    })
}

/// Spec 6.2 to 6.9: the operator's eleven-bit value in one's complement, for a
/// phase in 1/1024 periods and a total attenuation in envelope steps.
pub fn reference_value(phase: i32, attenuation: i32, half_sine: bool) -> i32 {
    let (log_sin, exp) = rom();
    let phase = phase & 0x3FF;
    let mut index = phase & 0xFF;
    if phase & 0x100 != 0 {
        index = 255 - index;
    }
    let x = log_sin[index as usize] + 16 * attenuation.min(127);
    let value = (exp[(255 - (x & 0xFF)) as usize] + 1024) >> (x >> 8);
    if phase & 0x200 != 0 {
        return -1 - if half_sine { 0 } else { value };
    }
    value
}

/// The largest carrier word at a total attenuation (spec 6.10).
pub fn carrier_peak(attenuation: i32) -> i32 {
    (0..256)
        .map(|phase| reference_value(phase, attenuation, false))
        .max()
        .unwrap()
        >> 3
}

/// Python's `round(x, digits)`: correctly rounded, halves to even (as Rust's formatting).
pub fn py_round(x: f64, digits: usize) -> f64 {
    format!("{x:.digits$}").parse().unwrap()
}

/// Python's `random.Random`: MT19937 seeded as CPython seeds it from an integer, with the
/// methods the tests use. (CPython's `_randommodule.c` and `random.py` define these
/// algorithms; nothing here is from either chip model.)
pub struct PyRandom {
    state: [u32; 624],
    index: usize,
}

impl PyRandom {
    pub fn new(seed: u64) -> Self {
        let mut key: Vec<u32> = Vec::new();
        let mut n = seed;
        while n != 0 {
            key.push(n as u32);
            n >>= 32;
        }
        if key.is_empty() {
            key.push(0);
        }
        let mut random = PyRandom {
            state: [0; 624],
            index: 624,
        };
        random.init_genrand(19_650_218);
        let mt = &mut random.state;
        let (mut i, mut j) = (1usize, 0usize);
        for _ in 0..624.max(key.len()) {
            mt[i] = (mt[i] ^ (mt[i - 1] ^ (mt[i - 1] >> 30)).wrapping_mul(1_664_525))
                .wrapping_add(key[j])
                .wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= 624 {
                mt[0] = mt[623];
                i = 1;
            }
            if j >= key.len() {
                j = 0;
            }
        }
        for _ in 0..623 {
            mt[i] = (mt[i] ^ (mt[i - 1] ^ (mt[i - 1] >> 30)).wrapping_mul(1_566_083_941))
                .wrapping_sub(i as u32);
            i += 1;
            if i >= 624 {
                mt[0] = mt[623];
                i = 1;
            }
        }
        mt[0] = 0x8000_0000;
        random
    }

    fn init_genrand(&mut self, seed: u32) {
        self.state[0] = seed;
        for i in 1..624 {
            let previous = self.state[i - 1];
            self.state[i] = 1_812_433_253u32
                .wrapping_mul(previous ^ (previous >> 30))
                .wrapping_add(i as u32);
        }
        self.index = 624;
    }

    fn genrand(&mut self) -> u32 {
        if self.index >= 624 {
            let mt = &mut self.state;
            for k in 0..624 {
                let y = (mt[k] & 0x8000_0000) | (mt[(k + 1) % 624] & 0x7FFF_FFFF);
                let mut next = mt[(k + 397) % 624] ^ (y >> 1);
                if y & 1 != 0 {
                    next ^= 0x9908_B0DF;
                }
                mt[k] = next;
            }
            self.index = 0;
        }
        let mut y = self.state[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9D2C_5680;
        y ^= (y << 15) & 0xEFC6_0000;
        y ^ (y >> 18)
    }

    fn getrandbits(&mut self, k: u32) -> u64 {
        assert!((1..=32).contains(&k));
        (self.genrand() >> (32 - k)) as u64
    }

    fn randbelow(&mut self, n: u64) -> u64 {
        let k = 64 - n.leading_zeros();
        let mut r = self.getrandbits(k);
        while r >= n {
            r = self.getrandbits(k);
        }
        r
    }

    /// `randrange(start, stop)`
    pub fn randrange(&mut self, start: i64, stop: i64) -> i64 {
        start + self.randbelow((stop - start) as u64) as i64
    }

    pub fn choice<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.randbelow(items.len() as u64) as usize]
    }

    pub fn random(&mut self) -> f64 {
        let a = (self.genrand() >> 5) as f64;
        let b = (self.genrand() >> 6) as f64;
        (a * 67_108_864.0 + b) * (1.0 / 9_007_199_254_740_992.0)
    }
}

/// What `tests/test_provisional.py` imports from `tests/test_measured.py`.
pub mod measured {
    use super::*;

    /// Carrier words predicted from spec 6.5 to 6.13 with a separate writing of
    /// the operator: both operators at envelope level 0, the same pitch, starting
    /// from phase 0 with nothing remembered. The first word is that of the sample
    /// in which damp ends (spec 7.10, 8.13): the phases are reset, the level goes to
    /// 0, and the sample is computed, all in that sample (CHOICE, U20).
    pub fn two_operator_reference(
        total_level: i32,
        feedback: i32,
        count: usize,
        step: i32,
    ) -> Vec<i32> {
        let mut out = Vec::with_capacity(count);
        let (mut previous, mut before) = (0, 0);
        let mut phase = 0;
        for _ in 0..count {
            phase += step;
            let index = phase >> 9;
            let fed = if feedback != 0 {
                (previous + before) >> (8 - feedback)
            } else {
                0
            };
            before = previous;
            previous = reference_value(index + fed, 2 * total_level, false);
            out.push(reference_value(index + 2 * before, 0, false) >> 3);
        }
        out
    }

    /// `rhythm_chip(keys, f7=0x150, block7=2, f8=0x1C0, block8=0)`
    pub fn rhythm_chip(keys: u8) -> OPLL {
        rhythm_chip_at(keys, 0x150, 2, 0x1C0, 0)
    }

    pub fn rhythm_chip_at(keys: u8, f7: i32, block7: i32, f8: i32, block8: i32) -> OPLL {
        let mut chip = OPLL::new(YM2413);
        write(&mut chip, 0x16, 0x20);
        write(&mut chip, 0x26, 0x05);
        note_k(&mut chip, 7, f7, block7, 0);
        note_k(&mut chip, 8, f8, block8, 0);
        write(&mut chip, 0x0E, 0x20);
        write(&mut chip, 0x0E, 0x20 | keys);
        chip
    }
}
