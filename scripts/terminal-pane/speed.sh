#!/bin/bash
# Runs speed.py under the gate lock, so only one Xvfb measurement runs at a time on this machine.
# Usage: scripts/terminal-pane/speed.sh BINARY [--runs 3] [--scale 1] [--keys 200] [--out FILE.json]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
# -o: the clanky-gpui processes speed.py starts must not inherit the lock.
exec flock -o "${CLANKY_GATE_LOCK:-/srv/devops/build/clanky/.gate.lock}" python3 "${here}/speed.py" "$@"
