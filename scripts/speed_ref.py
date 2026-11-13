#!/usr/bin/env python3
"""examples/speed.rs on ym2413-python: nine channels sounding, chip time over wall time.

    PYTHONPATH=REFERENCE/src pypy3 scripts/speed_ref.py [seconds, default 20]
"""

import sys
import time

from ym2413_python import OPLL, YM2413

CLOCK_HZ = 3_579_545
seconds = int(sys.argv[1]) if len(sys.argv) > 1 else 20
chip = OPLL(YM2413)
for channel in range(9):
    f_number = 0x120 + 23 * channel
    for register, value in (
        (0x30 + channel, (channel + 1) << 4),
        (0x10 + channel, f_number & 0xFF),
        (0x20 + channel, 0x10 | 4 << 1 | f_number >> 8),
    ):
        chip.write(0, register)
        chip.write(1, value)
        chip.run(84)
chip.take_outputs()
start = time.perf_counter()
heard = 0
for _ in range(seconds):
    chip.run(CLOCK_HZ)
    heard += sum(1 for v in chip.take_samples() if v)
wall = time.perf_counter() - start
print(f"{seconds} s of chip time in {wall:.3f} s: {seconds / wall:.1f} x real time ({heard} non-zero samples)")
