#!/usr/bin/env bash
set -euo pipefail
trap 'rc=$?; if (( rc != 0 )); then echo "FAIL: M11 look" >&2; fi' EXIT
cd "$(dirname "$0")/.."
export CARGO_HOME=/srv/devops/cache/clanky-cargo
export CARGO_TARGET_DIR=/srv/devops/build/clanky-pkg/target
export CLANKY_BUILD_LOCK=/srv/devops/build/clanky/.build.lock
export VK_DRIVER_FILES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json
export VK_ICD_FILENAMES=$VK_DRIVER_FILES
export LIBGL_ALWAYS_SOFTWARE=1
export WGPU_BACKEND=vulkan
case ${CLANKY_GATE_FAULT:-} in ''|look|store|settings) ;; *) echo 'FAIL: unknown look fault' >&2; exit 1;; esac
mkdir -p "$CARGO_TARGET_DIR/look"
work=$(mktemp -d "$CARGO_TARGET_DIR/look/gate.XXXXXXXX")
trap 'rc=$?; rm -rf -- "$work"; if (( rc != 0 )); then echo "FAIL: M11 look" >&2; fi' EXIT
flock "$CLANKY_BUILD_LOCK" cargo build --offline --locked -p clanky --bin clanky --bin clanky-look-gate -j 8
"$CARGO_TARGET_DIR/debug/clanky-look-gate" "$work/scene.rgba" > "$work/observations.json"
if [[ ${1:-} == --dark-only ]]; then
    python3 scripts/gate-m11-look.py --dark-only < "$work/observations.json"
else
    [[ $# == 0 ]] || { echo 'FAIL: unknown argument' >&2; exit 1; }
    python3 scripts/gate-m11-look.py < "$work/observations.json"
fi
python3 - "$work" <<'PY'
import pathlib, struct, sys, zlib
work = pathlib.Path(sys.argv[1])
w, h = 1240, 760
def chunk(kind, data):
    return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
for name in ('scene', 'scene-light'):
    raw = (work / (name + '.rgba')).read_bytes()
    if len(raw) != w * h * 4:
        raise SystemExit('FAIL: invalid picture size')
    # Readback is premultiplied; PNG stores straight alpha.
    pixels = bytearray(raw)
    for i in range(0, len(pixels), 4):
        alpha = pixels[i + 3]
        if alpha:
            for c in range(3):
                pixels[i+c] = min(255, (pixels[i+c] * 255 + alpha // 2) // alpha)
    rows = b''.join(b'\0' + pixels[y*w*4:(y+1)*w*4] for y in range(h))
    (work / (name + '.png')).write_bytes(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(rows)) + chunk(b'IEND', b''))
PY
mv -- "$work/scene.png" "$CARGO_TARGET_DIR/look/scene.png"
mv -- "$work/scene-light.png" "$CARGO_TARGET_DIR/look/scene-light.png"
if [[ ${1:-} == --dark-only ]]; then
    echo 'PASS: M11 dark only (light, settings and regression not run)'
    exit 0
fi
if [[ ${CLANKY_RC_NO_NESTED:-0} != 1 ]]; then
    env -u CLANKY_GATE_FAULT CLANKY_M11_PARENT=1 scripts/gate-rc.sh
fi
echo 'PASS: M11 look'
