#!/usr/bin/env bash
# Rung 1: every test in ym2413-python's tests/ (contract, manual, measured, community,
# provisional), transcribed to tests/*.rs, the strict expected failure kept as one.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)  # needs nothing but cargo: not the reference
STABLE="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin"
[ -x "$STABLE/cargo" ] && PATH="$STABLE:$PATH"
cd "$ROOT"
out=$(nice cargo test --release -q --tests -- --nocapture 2>&1) || { echo "$out"; echo "rung 1: FAIL"; exit 1; }
echo "$out" | grep -E "^test result" || true
skipped=$(echo "$out" | grep -c "^SKIPPED" || true)
passed=$(echo "$out" | grep -oE '[0-9]+ passed' | awk '{n+=$1} END{print n}')
echo "rung 1: PASS ($passed test functions, $skipped skipped for an absent sibling file)"
