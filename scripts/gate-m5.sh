#!/bin/sh
set -eu
ulimit -c 0
exec python3 "$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)/gate-m5.py" "$@"
