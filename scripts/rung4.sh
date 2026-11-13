#!/usr/bin/env bash
# Rung 4: the 30 seeded faults of ym2413-python's validation/faults.py, broken the same way in
# this crate (scripts/seeded_faults.py): each must fail rung 1, 2 or 3. Writes rung 2's and
# rung 3's reference traces first if they are not there.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
REUSE=1 "$ROOT/scripts/rung2.sh" >/dev/null
REUSE=1 "$ROOT/scripts/rung3.sh" >/dev/null
python3 "$ROOT/scripts/seeded_faults.py" --jobs "${JOBS:-4}" "$@" && echo "rung 4: PASS" || { echo "rung 4: FAIL"; exit 1; }
