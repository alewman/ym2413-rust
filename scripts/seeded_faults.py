#!/usr/bin/env python3
"""Rung 4: can each rung fail? The 30 seeded faults of ym2413-python's validation/faults.py,
the same rules broken in this crate's source, one textual change each.

    python3 scripts/seeded_faults.py [--jobs 4] [--only N,N,...]

Each fault is applied to a copy of the crate under the git-ignored external/faults/NN/; the copy
is judged by rung 1 (cargo test, by test file, as the reference lists its test files), rung 2
(the lockstep scripts against the reference traces already written by scripts/rung2.sh) and
rung 3 (the reference's streams, from scripts/rung3.sh). The unbroken copy (00) is judged first
and must pass all three. Exit status 1 if it fails or any fault goes unnoticed by every rung.
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
WORK = ROOT / "external" / "faults"
STABLE = Path.home() / ".rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin"

# Rung 1's test files, named as the reference's.
RUNG1 = {
    "contract": "rung 1 contract (test_contract.py)",
    "manual": "rung 1 manual (test_manual.py)",
    "measured": "rung 1 measured (test_measured.py)",
    "community": "rung 1 community (test_community.py)",
    "provisional": "rung 1 provisional (test_provisional.py)",
}

# (what is broken: the reference's own description, file, text to find, replacement, every?)
# Python's text in a comment where the Rust differs only by syntax.
FAULTS: tuple[tuple[str, str, str, str, bool], ...] = (
    ("log-sine ROM: no half-step offset (6.3)", "tables.rs", "(i as f64 + 0.5) * std::f64::consts::PI / 512.0", "(i as f64 + 1.0) * std::f64::consts::PI / 512.0", False),
    ("exponential ROM one step off (6.4)", "tables.rs", "2f64.powf(i as f64 / 256.0) - 1.0", "2f64.powf((i as f64 + 1.0) / 256.0) - 1.0", False),
    ("MULTI 11 is 11, not 10 (7.4)", "tables.rs", "18, 20, 20, 24", "18, 20, 22, 24", False),
    ("decay pattern for rate & 3 = 1 rotated (8.5)", "tables.rs", "[0, 1, 0, 1, 1, 1, 0, 1]", "[1, 0, 1, 1, 1, 0, 1, 0]", False),
    ("tremolo top held 4 x 64 samples (10.2)", "tables.rs", "if step < 114 {", "if step < 115 {", False),
    ("vibrato period 7,768 as S1 says (C2)", "tables.rs", "VIBRATO_PERIOD: i32 = 8_192", "VIBRATO_PERIOD: i32 = 7_768", False),
    ("mute at 120, not 124 (6.8)", "opll.rs", "QUIET: i32 = 124", "QUIET: i32 = 120", False),
    ("rate shift 12 - rate/4 (8.5)", "opll.rs", "let s = 13 - (rate >> 2);", "let s = 12 - (rate >> 2);", True),
    ("attack formula x - x/8 - 1 (8.10)", "opll.rs", "level - (level >> 4) - 1", "level - (level >> 3) - 1", False),
    ("noise taps x^23 + x^10 + 1 (11.7)", "opll.rs", "noise ^ (noise >> 14)", "noise ^ (noise >> 13)", False),
    ("no register aliases (4.3)", "opll.rs", "            address -= 9;\n            low -= 9;\n", "            return;\n", False),
    ("modulator TL 1.5 dB a step (6.7)", "opll.rs", "self.base[modulator] = 2 * (row[2] & 0x3F) as i32;", "self.base[modulator] = 4 * (row[2] & 0x3F) as i32;", False),
    ("volume 1.5 dB a step (6.7)", "opll.rs", "self.base[carrier] = 8 * volume;", "self.base[carrier] = 4 * volume;", False),
    ("SUS release at rate 7 (8.22)", "opll.rs", "self.variant.sustained_release_rate // the SUS bit: rate 5", "self.variant.percussive_release_rate", False),
    ("decay ends at or beyond SL (8.19)", "opll.rs", "if level >> 3 == self.sustain_level[operator] {", "if level >> 3 >= self.sustain_level[operator] {", False),
    ("phase counter 18 bits (7.2)", "opll.rs", "& 0x7FFFF", "& 0x3FFFF", False),
    ("phase step not halved (7.3)", "opll.rs", "            ((f_number * multiplier) << block) >> 1\n", "            (f_number * multiplier) << block\n", False),
    ("half sine keeps its size (6.9)", "opll.rs", "                value = 0; // spec 6.9 (measured): size zero, the sign still set\n", "", False),
    ("modulation not doubled (6.11)", "opll.rs", "phase += 2 * self.before_previous[channel];", "phase += self.before_previous[channel];", False),
    ("carrier word shifted by 4 (6.10)", "opll.rs", "self.operate(operator, channel, phase & 0x3FF) >> 3\n    }\n\n    /// The one bit", "self.operate(operator, channel, phase & 0x3FF) >> 4\n    }\n\n    /// The one bit", False),
    ("feedback one step weaker (6.12)", "opll.rs", "(8 - feedback)", "(9 - feedback)", False),
    # Python replaces the line with `pass`.
    ("total attenuation not clipped (6.7)", "opll.rs", "attenuation = attenuation.min(BOTTOM);", "", False),
    ("percussion not repeated (11.12)", "opll.rs", "rhythm_word = self.held[copy];", "rhythm_word = 0;", False),
    # Python tests `self.test & 0x100`, never set in a byte; a u8 cannot hold $100, so 0 here.
    ("test bit 0 ignored (13.2)", "opll.rs", "if self.test & 0x01 != 0 {", "if self.test & 0x00 != 0 {", False),
    ("damp at rate 11 (8.16)", "variants.rs", "damp_rate: 12,", "damp_rate: 11,", False),
    ("percussive release at rate 6 (8.22)", "variants.rs", "percussive_release_rate: 7,", "percussive_release_rate: 6,", False),
    ("DS1001 with nine channels (S6)", "variants.rs", "    variant.channels = 6;\n", "", False),
    ("VRC7 reset clears the vibrato (S6)", "variants.rs", "    variant.reset_clears_vibrato = false;\n", "", False),
    ("hi-hat phases swapped (provisional, U22)", "variants.rs", "[0x2D0, 0x234, 0x034, 0x0D0]", "[0x234, 0x2D0, 0x0D0, 0x034]", False),
    ("instant attack decided later (8.13)", "opll.rs", "                if rate >= 60 {\n", "                if rate >= 64 {\n", False),
)  # fmt: skip


def environment() -> dict[str, str]:
    env = dict(os.environ)
    if (STABLE / "cargo").exists():
        env["PATH"] = f"{STABLE}:{env['PATH']}"
    return env


def make_copy(index: int, file: str = "", find: str = "", replacement: str = "", every=False) -> Path:
    copy = WORK / f"{index:02d}"
    for part in ("src", "tests", "examples"):
        target = copy / part
        if target.exists():
            # the previous run's copy is set aside, never deleted (the working rules); the
            # build directory stays, so a rerun only recompiles this crate
            aside = ROOT / "trash" / "faults" / str(os.getpid()) / f"{index:02d}"
            aside.mkdir(parents=True, exist_ok=True)
            shutil.move(str(target), str(aside / part))
        shutil.copytree(ROOT / part, target)
    for name in ("Cargo.toml", "Cargo.lock"):
        shutil.copy2(ROOT / name, copy / name)
    if file:
        source = copy / "src" / file
        text = source.read_text()
        count = text.count(find)
        if count == 0 or (count > 1 and not every):
            sys.exit(f"fault {index}: {find!r} occurs {count} times in {file}")
        source.write_text(text.replace(find, replacement))
    return copy


def judge(copy: Path) -> tuple[bool, set[str]]:
    """(built, the rungs that fail)."""
    env = environment()
    env["CARGO_TARGET_DIR"] = str(copy / "target")
    # copytree keeps the sources' old modification times, and cargo judges freshness by them:
    # stamp every file now, so a reused build directory cannot serve a binary of other text
    for part in ("src", "tests", "examples"):
        for path in (copy / part).rglob("*"):
            if path.is_file():
                os.utime(path)
    for name in ("Cargo.toml", "Cargo.lock"):
        os.utime(copy / name)
    caught: set[str] = set()
    build = subprocess.run(
        ["nice", "cargo", "build", "--release", "-q", "--example", "ym2413-trace"],
        cwd=copy, env=env, capture_output=True, text=True, check=False,
    )  # fmt: skip
    if build.returncode:
        return False, set(RUNG1.values()) | {"rung 2", "rung 3"}
    # stderr (cargo's "Running tests/x.rs") merged into stdout in order, so each result line
    # follows the header of its file
    tests = subprocess.run(
        ["nice", "cargo", "test", "--release", "--no-fail-fast", "--tests"],
        cwd=copy, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
        check=False,
    )  # fmt: skip
    current = None
    for line in tests.stdout.splitlines():
        running = re.match(r"\s*Running tests/(\w+)\.rs", line)
        if running:
            current = running.group(1)
        elif current in RUNG1 and re.match(r"test result: FAILED", line):
            caught.add(RUNG1[current])
    if tests.returncode and not caught:
        caught |= set(RUNG1.values())  # a test target did not build
    binary = copy / "target" / "release" / "examples" / "ym2413-trace"
    for rung, script in (("rung 2", "rung2.sh"), ("rung 3", "rung3.sh")):
        out = copy / f"out-{rung[-1]}"
        out.mkdir(exist_ok=True)
        result = subprocess.run(
            [str(ROOT / "scripts" / script)],
            env=dict(env, BIN=str(binary), OURS=str(out), REUSE="1", JOBS="2"),
            capture_output=True, text=True, check=False,
        )  # fmt: skip
        if result.returncode:
            caught.add(rung)
    return True, caught


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--only", default="")
    args = parser.parse_args()
    only = {int(n) for n in args.only.split(",") if n}
    built, caught = judge(make_copy(0))
    if not built or caught:
        print(f"the unbroken copy fails: {sorted(caught)}")
        return 1
    print("unbroken copy: rungs 1, 2 and 3 pass\n")
    chosen = [(i, f) for i, f in enumerate(FAULTS, start=1) if not only or i in only]
    copies = [make_copy(i, *fault[1:]) for i, fault in chosen]
    with ThreadPoolExecutor(args.jobs) as pool:
        results = list(pool.map(judge, copies))
    rungs = [*RUNG1.values(), "rung 2", "rung 3"]
    counts = dict.fromkeys(rungs, 0)
    unnoticed = []
    for (index, (what, *_)), (built, found) in zip(chosen, results, strict=True):
        for rung in found:
            counts[rung] += 1
        seen = ", ".join(rung for rung in rungs if rung in found) or "NOBODY"
        if not built:
            seen = "does not build (" + seen + ")"
        if not found:
            unnoticed.append(what)
        print(f"{index:2d}. {what}: {seen}")
    print()
    any_rung1 = sum(1 for _, (_, found) in zip(chosen, results, strict=True) if found & set(RUNG1.values()))
    print(f"rung 1 (any test file): notices {any_rung1} of {len(chosen)} faults")
    for rung in rungs:
        print(f"{rung}: notices {counts[rung]} of {len(chosen)} faults")
    print(f"unnoticed by every rung: {len(unnoticed)} {unnoticed if unnoticed else ''}")
    return 1 if unnoticed else 0


if __name__ == "__main__":
    sys.exit(main())
