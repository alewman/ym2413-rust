#!/usr/bin/env python3
"""Write rung 3's scripts: every stream of ym2413-python's validation/streams.py, for both
variants, in the lockstep script format (scripts/lockstep_gen.py), replayed as the reference's
own harness replays them (run(4 x cycles to the event), write, and run to the stream's end).

    python3 scripts/export_streams.py REFERENCE OUT_DIR

Digest mode: each take prints the count and an FNV-1a hash of the output pairs and of every
slot word traced since the last take; a take every 4,096 samples, and the state at the end.
"""

from __future__ import annotations

import sys
from pathlib import Path

TAKE = 72 * 4096  # input clocks between takes


def main() -> None:
    reference, out_dir = Path(sys.argv[1]), Path(sys.argv[2])
    sys.path.insert(0, str(reference / "validation"))
    from streams import STREAMS  # noqa: E402  (the reference's test inputs, unchanged)

    out_dir.mkdir(parents=True, exist_ok=True)
    for name, (events, length) in STREAMS.items():
        lines = ["trace 1"]
        since = 0

        def run(clocks: int) -> None:
            nonlocal since
            while clocks:
                piece = min(clocks, TAKE - since)
                lines.append(f"run {piece}")
                clocks -= piece
                since += piece
                if since == TAKE:
                    lines.append("take")
                    since = 0

        now = 0
        for cycle, port, value in events:
            run(4 * (cycle - now))
            now = cycle
            lines.append(f"w {port:X} {value:02X}")
        run(4 * (length - now))
        lines += ["take", "state"]
        for variant in ("YM2413", "DS1001"):
            text = "\n".join([f"variant {variant}", "mode digest", *lines]) + "\n"
            (out_dir / f"{name}-{variant}.txt").write_text(text)
    print(f"{2 * len(STREAMS)} scripts in {out_dir}")


if __name__ == "__main__":
    main()
