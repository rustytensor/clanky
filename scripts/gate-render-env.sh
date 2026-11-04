#!/bin/sh
# Shared software defaults; explicitly supplied renderer settings remain intact.
export VK_DRIVER_FILES=${VK_DRIVER_FILES:-/usr/share/vulkan/icd.d/lvp_icd.x86_64.json}
export LIBGL_ALWAYS_SOFTWARE=${LIBGL_ALWAYS_SOFTWARE:-1}
if [ ! -f "$VK_DRIVER_FILES" ]; then
    echo 'FAIL: Vulkan driver file is missing' >&2
    exit 1
fi
