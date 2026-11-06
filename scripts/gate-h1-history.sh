#!/usr/bin/env bash
# H1 saved history gate. `gate-h1-history.sh` must print PASS;
# `gate-h1-history.sh --faults` runs each planted fault and must show every one caught.
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_HOME="${CARGO_HOME:-/srv/devops/cache/clanky-cargo}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/target}"
mkdir -p "$CARGO_TARGET_DIR"
flock /srv/devops/build/clanky/.build.lock cargo build --offline -j 8 -p clanky --bin clanky
fail=0
# Linux links the system SQLite; nothing bundled.
if ldd "$CARGO_TARGET_DIR/debug/clanky" | grep -q 'libsqlite3\.so\.0 => /'; then
    echo 'PASS: links system libsqlite3'
else
    echo 'FAIL: links system libsqlite3'; fail=1
fi
if cargo tree --offline -p clanky -e features -i libsqlite3-sys 2>/dev/null | grep -q 'feature "bundled'; then
    echo 'FAIL: SQLite bundled on Linux'; fail=1
else
    echo 'PASS: SQLite not bundled on Linux'
fi
if [ "${1:-}" = --faults ]; then
    for f in secret restart actor delete; do
        if python3 scripts/gate-h1-history.py --fault "$f" >/dev/null; then
            echo "FAIL: planted fault '$f' was not caught"; fail=1
        else
            echo "PASS: planted fault '$f' caught"
        fi
    done
elif ! python3 scripts/gate-h1-history.py; then
    fail=1
fi
if [ "$fail" = 0 ]; then echo 'PASS: H1 saved history'; else echo 'FAIL: H1 saved history' >&2; exit 1; fi
