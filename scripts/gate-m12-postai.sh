#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_HOME="${CARGO_HOME:-/srv/devops/cache/clanky-cargo}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/target}"
mkdir -p "$CARGO_TARGET_DIR"
flock /srv/devops/build/clanky/.build.lock cargo build --offline -j 8 -p clanky --bin clanky --bin clanky-postai-gate
if "$CARGO_TARGET_DIR/debug/clanky-postai-gate"; then
    echo 'PASS: M12 Post-AI runtime'
else
    echo 'FAIL: M12 Post-AI runtime' >&2
    exit 1
fi
