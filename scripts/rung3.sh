#!/usr/bin/env bash
# Rung 3: every stream of ym2413-python's validation/streams.py (scripts/export_streams.py),
# through both cores and both variants: the output pairs and every slot word (as per-take
# hashes) and the final state must be identical. To find where one differs, change its
# script's second line to "mode full" and run both sides on it (lockstep_ref.py, the example).
# BIN / OURS / REUSE: as for rung 2.
set -euo pipefail
source "$(dirname "$0")/env.sh"
DIR="$ROOT/external/streams"
mkdir -p "$DIR/scripts" "$DIR/ref"
if [ -z "${BIN:-}" ]; then
    (cd "$ROOT" && nice cargo build --release -q --example ym2413-trace)
    BIN="$ROOT/target/release/examples/ym2413-trace"
fi
if [ "${REUSE:-0}" != 1 ] || [ ! -f "$DIR/ref.done" ]; then
    "$PYTHON" "$ROOT/scripts/export_streams.py" "$REF" "$DIR/scripts" >/dev/null
    ls -S "$DIR"/scripts/*.txt | nice xargs -P "$JOBS" -n 1 env PYTHONPATH="$REF/src" PYPY_GC_NURSERY=1M "$PYTHON" "$ROOT/scripts/lockstep_ref.py" --out-dir "$DIR/ref"
    touch "$DIR/ref.done"
fi
OURS=${OURS:-"$DIR/ours"}
mkdir -p "$OURS"
ls "$DIR"/scripts/*.txt | nice xargs -P "$JOBS" -n 4 "$BIN" --out-dir "$OURS"
status=0; same=0; total=0
for ref in "$DIR"/ref/*.trace; do
    total=$((total + 1))
    name=$(basename "$ref")
    if cmp -s "$ref" "$OURS/$name"; then
        same=$((same + 1))
    else
        status=1
        echo "DIFFERENT ${name%.trace}: $(diff "$ref" "$OURS/$name" | head -3 | cut -c1-160 | tr '\n' ' ' || true)"
    fi
done
samples=$(cat "$DIR"/ref/*.trace | awk '$1=="take"{n+=$2} END{print n+0}')
words=$(cat "$DIR"/ref/*.trace | awk '$1=="take"{n+=$6} END{print n+0}')
echo "rung 3: $same of $total stream runs identical ($((total / 2)) streams x 2 variants; $samples output pairs, $words slot words)"
[ $status = 0 ] && echo "rung 3: PASS" || echo "rung 3: FAIL"
exit $status
