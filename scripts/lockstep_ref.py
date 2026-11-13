#!/usr/bin/env python3
"""Run lockstep scripts (scripts/lockstep_gen.py, scripts/export_streams.py) on ym2413-python
and print the trace that examples/ym2413-trace.rs prints for this crate.

    PYTHONPATH=REFERENCE/src pypy3 scripts/lockstep_ref.py SCRIPT [SCRIPT ...] [--out-dir DIR]

With one script and no --out-dir the trace goes to stdout; otherwise each script's trace is
written to DIR/<name>.trace. Trace lines:

    take COUNT m,r m,r ...          take_outputs() (full mode), then one line per traced sample:
    s m,r m,r ...                   its slot words, melody and rhythm, in the chip's own form
    take COUNT HASH words N W HASH  the same in digest mode (FNV-1a 64 over the low 16 bits)
    samples COUNT v v ...           take_samples()
    state k=v ... | name list ...   every attribute but the variant and the trace
    log_sin / exp / key_scale_steps / tremolo   the computed tables
"""

from __future__ import annotations

import sys
from pathlib import Path

from ym2413_python import DS1001, OPLL, YM2413
from ym2413_python.tables import EXP, KEY_SCALE_STEPS, LOG_SIN, TREMOLO

VARIANTS = {"YM2413": YM2413, "DS1001": DS1001}
FNV_OFFSET = 0xCBF29CE484222325
FNV_PRIME = 0x00000100000001B3
MASK = (1 << 64) - 1

LISTS = (
    "phase", "level", "state", "key", "previous", "before_previous", "held", "tremolo_on",
    "vibrato_on", "sustained", "rate_scaling", "multiplier", "key_scale", "base", "half_sine",
    "attack_rate", "decay_rate", "sustain_level", "release_rate", "releases", "feedback",
    "melody", "rhythm",
)  # fmt: skip


def fnv(h: int, value: int) -> int:
    return ((h ^ (value & 0xFFFF)) * FNV_PRIME) & MASK


def join(values) -> str:
    return " ".join(str(v) for v in values)


def state(chip: OPLL) -> str:
    out = (
        f"state clock_phase={chip.clock_phase} slot={chip.slot} melody_sum={chip.melody_sum} "
        f"rhythm_sum={chip.rhythm_sum} address={chip.address} test={chip.test} "
        f"counter={chip.counter} tremolo_time={chip.tremolo_time} "
        f"vibrato_time={chip.vibrato_time} tremolo={chip.tremolo} noise={chip.noise} "
        f"rhythm_mode={chip.rhythm_mode} registers={chip.registers.hex().upper()}"
    )
    for name in LISTS:
        out += f" | {name} {join(getattr(chip, name))}"
    return out


def trace(lines: list[str]) -> list[str]:
    out: list[str] = []
    chip = OPLL(VARIANTS[lines[0].split()[1]])
    digest = lines[1].split()[1] == "digest"
    for line in lines[2:]:
        op, *args = line.split() or [""]
        if op == "w":
            chip.write(int(args[0], 16), int(args[1], 16))
        elif op == "run":
            chip.run(int(args[0]))
        elif op == "reset":
            chip.reset()
        elif op == "trace":
            chip.trace = [] if args[0] == "1" else None
        elif op == "take":
            pairs = chip.take_outputs()
            samples = []
            if chip.trace is not None:
                keep = chip.trace.pop() if chip.trace and len(chip.trace[-1]) < 18 else None
                samples = chip.trace
                chip.trace = [] if keep is None else [keep]
            if digest:
                h = FNV_OFFSET
                for melody, rhythm in pairs:
                    h = fnv(fnv(h, melody), rhythm)
                words, count = FNV_OFFSET, 0
                for sample in samples:
                    for melody, rhythm in sample:
                        words = fnv(fnv(words, melody), rhythm)
                        count += 1
                out.append(f"take {len(pairs)} {h:016x} words {len(samples)} {count} {words:016x}")
            else:
                out.append(f"take {len(pairs)}" + "".join(f" {m},{r}" for m, r in pairs))
                for sample in samples:
                    out.append("s" + "".join(f" {m},{r}" for m, r in sample))
        elif op == "samples":
            values = chip.take_samples()
            out.append(f"samples {len(values)} {join(values)}")
        elif op == "state":
            out.append(state(chip))
        elif op == "tables":
            out.append(f"log_sin {join(LOG_SIN)}")
            out.append(f"exp {join(EXP)}")
            out.append(
                "key_scale_steps "
                + join(v for code in KEY_SCALE_STEPS for block in code for v in block)
            )
            out.append(f"tremolo {join(TREMOLO)}")
        elif op == "":
            pass
        else:
            raise SystemExit(f"unknown op {op}")
    return out


def main(argv: list[str]) -> None:
    out_dir = None
    scripts = []
    i = 0
    while i < len(argv):
        if argv[i] == "--out-dir":
            out_dir = Path(argv[i + 1])
            i += 2
        else:
            scripts.append(Path(argv[i]))
            i += 1
    for script in scripts:
        result = "\n".join(trace(script.read_text().splitlines())) + "\n"
        if out_dir is None and len(scripts) == 1:
            sys.stdout.write(result)
        else:
            (out_dir or Path(".")).joinpath(script.stem + ".trace").write_text(result)


if __name__ == "__main__":
    main(sys.argv[1:])
