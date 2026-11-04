#!/bin/sh
set -eu
. "$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)/gate-render-env.sh"
exec python3 "$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)/gate-m2.py" "$@"
