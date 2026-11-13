# Sourced by the rung scripts: the reference, its interpreter and the toolchain.
ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
REF=${YM2413_PYTHON:-"$ROOT/external/ym2413-python"}
[ -f "$REF/src/ym2413_python/opll.py" ] || { echo "no reference at $REF: run scripts/fetch_ym2413python.sh" >&2; exit 2; }
if [ -z "${PYTHON:-}" ]; then
    for candidate in /data/emu/ym2413-python/.venv-pypy/bin/python "$(command -v pypy3 || true)" "$HOME/.local/opt/pypy/bin/pypy3"; do
        [ -n "$candidate" ] && [ -x "$candidate" ] && { PYTHON=$candidate; break; }
    done
    [ -n "${PYTHON:-}" ] || PYTHON=python3
fi
# A cargo whose clippy matches its rustc, when the default one's does not.
STABLE="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin"
[ -x "$STABLE/cargo" ] && PATH="$STABLE:$PATH"
JOBS=${JOBS:-8}
