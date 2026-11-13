#!/usr/bin/env python3
"""Write register-stream lockstep scripts (rung 2): seeded random and scripted write/run
sequences for both variants, which both cores run (scripts/lockstep_ref.py,
examples/ym2413-trace.rs).

    python3 scripts/lockstep_gen.py OUT_DIR [--count 48] [--seed 1] [--steps 3000]

``--count`` scripts per variant (YM2413, DS1001), seeded so a run is reproducible. Imports
nothing from either core.

A script is text, one call per line (numbers decimal, ports and bytes hex):

    variant YM2413 | mode full     the constructor's variant; full traces (first two lines)
    w PORT VAL                     write(PORT, VAL); ports 0 to 3 (the chip reads bit 0)
    run N                          run(N) input clocks, any N (odd ones, inside a slot)
    reset                          reset()
    trace 1 / trace 0              trace = [] / None (a list set mid-sample fills from slot 0)
    take                           take_outputs(), and the slot words traced since the last take
    samples                        take_samples()
    state                          every attribute
    tables                         the computed tables

The random part writes to every address $00-$FF (the registers, the aliases $19-$1F,
$29-$2F, $39-$3F, the unmapped $08-$0D and $40 up), data without a new address, odd and
unaligned run() lengths, long runs that let envelopes and the LFOs move, resets, and trace
switched on and off mid-sample. The scripted parts are notes properly set up on the user tone
and the ROM voices (so most channels sound), rhythm mode with its keys and volumes, the test
register's bits, key-on races and the SUS bit.
"""

from __future__ import annotations

import argparse
import random
from pathlib import Path


class Script:
    def __init__(self, rng: random.Random, variant: str) -> None:
        self.rng = rng
        self.lines = [f"variant {variant}", "mode full"]

    def emit(self, line: str) -> None:
        self.lines.append(line)

    def reg(self, register: int, value: int) -> None:
        rng = self.rng
        self.emit(f"w {rng.choice((0, 0, 0, 2)):X} {register & 0xFF:02X}")
        if rng.random() < 0.7:
            self.run(rng.choice((12, 12, 1, 3, 5, 7)))
        self.emit(f"w {rng.choice((1, 1, 1, 3)):X} {value & 0xFF:02X}")
        if rng.random() < 0.8:
            self.run(rng.choice((84, 84, 1, 2, 13, 71, 73)))

    def run(self, clocks: int) -> None:
        self.emit(f"run {clocks}")

    def samples(self, count: int) -> None:
        self.run(72 * count + self.rng.choice((0, 0, 1, 2, 3, 37, 71)))

    # ----- the scripted parts --------------------------------------------------------------

    def user_tone(self) -> None:
        rng = self.rng
        tone = [
            rng.randrange(256),
            rng.randrange(256),
            rng.randrange(256),
            rng.randrange(256),
            rng.choice((0xF0, 0xF2, 0xC4, 0x84, rng.randrange(256))),
            rng.choice((0xF0, 0xF4, 0xA2, 0x83, rng.randrange(256))),
            rng.randrange(256),
            rng.randrange(256),
        ]
        for register, value in enumerate(tone):
            self.reg(register, value)

    def note(self, channel: int, key: int = 1) -> None:
        rng = self.rng
        f_number = rng.randrange(512)
        block = rng.randrange(8)
        sus = int(rng.random() < 0.3)
        self.reg(0x10 + channel, f_number & 0xFF)
        self.reg(0x20 + channel, sus << 5 | key << 4 | block << 1 | f_number >> 8)

    def music(self) -> None:
        rng = self.rng
        if rng.random() < 0.5:
            self.user_tone()
        for channel in rng.sample(range(9), rng.randrange(1, 10)):
            self.reg(0x30 + channel, rng.randrange(16) << 4 | rng.choice((0, 0, 2, 5, 15)))
            self.note(channel)
        self.samples(rng.choice((200, 800, 3000)))
        for channel in rng.sample(range(9), rng.randrange(1, 10)):
            self.note(channel, key=0)
        self.samples(rng.choice((100, 600, 2000)))

    def rhythm(self) -> None:
        rng = self.rng
        for channel in (6, 7, 8):
            self.note(channel, key=int(rng.random() < 0.2))
        self.reg(0x36, rng.randrange(256))
        self.reg(0x37, rng.randrange(256))
        self.reg(0x38, rng.randrange(256))
        for _ in range(rng.randrange(2, 8)):
            self.reg(0x0E, 0x20 | rng.randrange(32))
            self.samples(rng.choice((30, 300, 1200)))
            self.reg(0x0E, rng.choice((0x20, 0x00, 0x3F, rng.randrange(256))))
            self.samples(rng.choice((5, 100, 400)))

    def test_register(self) -> None:
        rng = self.rng
        for _ in range(rng.randrange(1, 6)):
            self.reg(0x0F, rng.choice((0x01, 0x02, 0x04, 0x08, 0x0F, 0xF0, rng.randrange(256))))
            self.samples(rng.choice((3, 50, 700)))
        self.reg(0x0F, 0x00)

    def key_races(self) -> None:
        rng = self.rng
        for _ in range(rng.randrange(10, 60)):
            channel = rng.randrange(9)
            self.emit(f"w 0 {0x20 + channel:02X}")
            self.emit(f"w 1 {rng.choice((0x10, 0x00, 0x30, 0x18)) | rng.randrange(16):02X}")
            self.run(rng.randrange(1, 90))

    # ----- the random part -----------------------------------------------------------------

    def random_step(self) -> None:
        rng = self.rng
        roll = rng.random()
        if roll < 0.35:
            register = rng.choice(
                (rng.randrange(0x40), rng.randrange(0x40), rng.randrange(0x40), rng.randrange(256))
            )
            self.reg(register, rng.randrange(256))
        elif roll < 0.42:
            self.emit(f"w {rng.choice((1, 3)):X} {rng.randrange(256):02X}")  # data, same address
        elif roll < 0.47:
            self.emit(f"w {rng.choice((0, 2)):X} {rng.randrange(256):02X}")  # address only
        elif roll < 0.70:
            self.run(rng.randrange(0, 300))
        elif roll < 0.76:
            self.samples(rng.randrange(50, 4000))
        elif roll < 0.86:
            self.emit("take")
        elif roll < 0.89:
            self.emit("state")
        elif roll < 0.90:
            self.emit("samples")
        elif roll < 0.92:
            self.emit(f"trace {int(rng.random() < 0.8)}")
        elif roll < 0.925:
            self.emit("reset")
        elif roll < 0.95:
            self.music()
        elif roll < 0.97:
            self.rhythm()
        elif roll < 0.985:
            self.test_register()
        else:
            self.key_races()


def script(rng: random.Random, variant: str, steps: int) -> list[str]:
    s = Script(rng, variant)
    s.emit("tables")
    s.emit("state")
    s.emit("trace 1")
    s.music()  # something sounding from the start
    while len(s.lines) < steps:
        s.random_step()
    s.emit("take")
    s.emit("state")
    return s.lines


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("out_dir", type=Path)
    parser.add_argument("--count", type=int, default=48)
    parser.add_argument("--seed", type=int, default=1)
    parser.add_argument("--steps", type=int, default=3000)
    args = parser.parse_args()
    args.out_dir.mkdir(parents=True, exist_ok=True)
    for variant in ("YM2413", "DS1001"):
        for n in range(args.count):
            rng = random.Random(f"{args.seed}-{variant}-{n}")
            lines = script(rng, variant, args.steps)
            (args.out_dir / f"{variant}-{n:03d}.txt").write_text("\n".join(lines) + "\n")
    print(f"{2 * args.count} scripts in {args.out_dir}")


if __name__ == "__main__":
    main()
