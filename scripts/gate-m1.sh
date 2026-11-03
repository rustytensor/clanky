#!/bin/sh
set -eu
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/srv/devops/build/clanky}"
export CARGO_HOME="${CARGO_HOME:-/srv/devops/cache/clanky-cargo}"
exec python3 "$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)/gate-m1.py" "$@"
