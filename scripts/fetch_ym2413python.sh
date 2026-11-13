#!/usr/bin/env bash
# The reference, ym2413-python, at the pin, into external/ym2413-python. It imports nothing, so
# no venv: the scripts run it with PYTHONPATH (PyPy when found).
# Usage: scripts/fetch_ym2413python.sh [source repo, default ../ym2413-python]
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
PIN=f88dccf174780d7c958a45129ce9e337450f387b
SRC=${1:-"$ROOT/../ym2413-python"}
DEST="$ROOT/external/ym2413-python"
mkdir -p "$ROOT/external"
[ -d "$DEST/.git" ] || git clone -q "$SRC" "$DEST"
git -C "$DEST" checkout -q "$PIN"
echo "ym2413-python at $(git -C "$DEST" rev-parse --short HEAD) in $DEST"
