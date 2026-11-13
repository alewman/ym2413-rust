#!/usr/bin/env bash
# Rung 2: register-stream lockstep. Seeded random and scripted write/run scripts for both
# variants (scripts/lockstep_gen.py) run on both cores; every slot word, every output pair,
# every take_samples() value, the whole state at checkpoints and the computed tables must be
# identical, byte for byte.
# Usage: scripts/rung2.sh [count per variant, default 48] [seed, default 1] [steps, default 3000]
# BIN: another build of examples/ym2413-trace to judge (scripts/seeded_faults.py uses it);
# OURS: where its traces go; REUSE=1 keeps the reference traces already in external/lockstep.
set -euo pipefail
source "$(dirname "$0")/env.sh"
COUNT=${1:-48}; SEED=${2:-1}; STEPS=${3:-3000}
DIR="$ROOT/external/lockstep/seed$SEED-n$COUNT-s$STEPS"
mkdir -p "$DIR/scripts" "$DIR/ref"
if [ -z "${BIN:-}" ]; then
    (cd "$ROOT" && nice cargo build --release -q --example ym2413-trace)
    BIN="$ROOT/target/release/examples/ym2413-trace"
fi
if [ "${REUSE:-0}" != 1 ] || [ ! -f "$DIR/ref.done" ]; then
    python3 "$ROOT/scripts/lockstep_gen.py" "$DIR/scripts" --count "$COUNT" --seed "$SEED" --steps "$STEPS" >/dev/null
    ls "$DIR"/scripts/*.txt | nice xargs -P "$JOBS" -n 4 env PYTHONPATH="$REF/src" PYPY_GC_NURSERY=1M "$PYTHON" "$ROOT/scripts/lockstep_ref.py" --out-dir "$DIR/ref"
    touch "$DIR/ref.done"
fi
OURS=${OURS:-"$DIR/ours"}
mkdir -p "$OURS"
ls "$DIR"/scripts/*.txt | nice xargs -P "$JOBS" -n 8 "$BIN" --out-dir "$OURS"
status=0; same=0; total=0
for ref in "$DIR"/ref/*.trace; do
    total=$((total + 1))
    name=$(basename "$ref")
    if cmp -s "$ref" "$OURS/$name"; then
        same=$((same + 1))
    else
        status=1
        first=$(diff <(cut -c1-160 "$ref") <(cut -c1-160 "$OURS/$name") | head -4 | tr '\n' ' ' || true)
        echo "DIFFERENT ${name%.trace}: $first"
    fi
done
words=$(cat "$DIR"/ref/*.trace | grep -c '^s ' || true)
pairs=$(cat "$DIR"/ref/*.trace | awk '$1=="take"{n+=$2} END{print n+0}')
states=$(cat "$DIR"/ref/*.trace | grep -c '^state ' || true)
writes=$(cat "$DIR"/scripts/*.txt | grep -c '^w ' || true)
echo "rung 2: $same of $total scripts identical ($writes writes, $pairs output pairs, $words traced samples of 18 slot words, $states state checkpoints)"
[ $status = 0 ] && echo "rung 2: PASS" || echo "rung 2: FAIL"
exit $status
