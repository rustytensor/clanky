# M1 dependency inventory

Cargo selected the current stable direct releases on 2026-11-03; the eight
remaining direct crates are pinned with `=` in the member manifests. The direct
png dependency was removed when images moved to M2; base64 remains for API output. Cargo.lock is the exact
resolved inventory, including platform-specific dependencies for later targets.
No crate has been compiled in this step.

Downloaded crate archives: **58,875,968 bytes (56.15 MiB)** across
306 archives. Of these, the final lockfile uses 58,202,658 bytes.
This measures compressed crate payloads in the dedicated fetch cache, not HTTP
headers, registry index traffic or extracted disk usage. It is not a measurement
of total network transfer. Sequential dependency resolution may cache archives
not retained in the final lockfile.

Inventory: 301 packages, including four MIT workspace packages.
Licences come from fetched package manifests via Cargo metadata. For OR choices,
the final column selects an allowed licence; a disallowed alternative does not
force its use. AND obligations are retained. Legacy slash declarations are
alternatives. The licence check rejected a planted GPL-only declaration before
accepting this inventory. This is a manifest-licence check, not a legal opinion
or an audit of every source file's notices.

The separately bundled DejaVu Sans Mono font is not a crate. Its complete licence
is in `assets/fonts/LICENSE`: the explicitly approved Bitstream Vera/public-domain
font exception, with accompanying notices. The font was copied, not downloaded.

| Crate | Locked version | Declared licence | Selected licence |
| --- | --- | --- | --- |
| ab_glyph | 0.2.32 | Apache-2.0 | Apache-2.0 |
| ab_glyph_rasterizer | 0.1.10 | Apache-2.0 | Apache-2.0 |
| ahash | 0.8.12 | MIT OR Apache-2.0 | MIT |
| aho-corasick | 1.1.5 | Unlicense OR MIT | Unlicense |
| alacritty_terminal | 0.26.0 | Apache-2.0 | Apache-2.0 |
| allocator-api2 | 0.2.21 | MIT OR Apache-2.0 | MIT |
| android-activity | 0.6.1 | MIT OR Apache-2.0 | MIT |
| android-properties | 0.2.2 | MIT | MIT |
| android_system_properties | 0.1.6 | MIT OR Apache-2.0 | MIT |
| arrayref | 0.3.9 | BSD-2-Clause | BSD-2-Clause |
| arrayvec | 0.7.8 | MIT OR Apache-2.0 | MIT |
| as-raw-xcb-connection | 1.0.1 | MIT OR Apache-2.0 | MIT |
| ash | 0.38.0+1.3.281 | MIT OR Apache-2.0 | MIT |
| atomic-waker | 1.1.2 | Apache-2.0 OR MIT | Apache-2.0 |
| autocfg | 1.5.1 | Apache-2.0 OR MIT | Apache-2.0 |
| base64 | 0.22.1 | MIT OR Apache-2.0 | MIT |
| base64 | 0.23.1 | MIT OR Apache-2.0 | MIT |
| bit-set | 0.10.0 | Apache-2.0 OR MIT | Apache-2.0 |
| bit-vec | 0.9.1 | Apache-2.0 OR MIT | Apache-2.0 |
| bitflags | 1.3.2 | MIT/Apache-2.0 | MIT |
| bitflags | 2.13.2 | MIT OR Apache-2.0 | MIT |
| block2 | 0.5.1 | MIT | MIT |
| block2 | 0.6.2 | MIT | MIT |
| bumpalo | 3.20.3 | MIT OR Apache-2.0 | MIT |
| bytemuck | 1.25.2 | Zlib OR Apache-2.0 OR MIT | Zlib |
| bytemuck_derive | 1.12.1 | Zlib OR Apache-2.0 OR MIT | Zlib |
| bytes | 1.12.1 | MIT | MIT |
| calloop | 0.13.0 | MIT | MIT |
| calloop-wayland-source | 0.3.0 | MIT | MIT |
| cc | 1.11.0 | MIT OR Apache-2.0 | MIT |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 | MIT |
| cfg_aliases | 0.2.2 | MIT | MIT |
| clanky | 0.0.9 | MIT | MIT |
| clanky-api | 0.0.9 | MIT | MIT |
| clanky-core | 0.0.9 | MIT | MIT |
| clanky-render | 0.0.9 | MIT | MIT |
| codespan-reporting | 0.13.1 | Apache-2.0 | Apache-2.0 |
| combine | 4.6.9 | MIT | MIT |
| concurrent-queue | 2.5.0 | Apache-2.0 OR MIT | Apache-2.0 |
| core-foundation | 0.9.4 | MIT OR Apache-2.0 | MIT |
| core-foundation-sys | 0.8.7 | MIT OR Apache-2.0 | MIT |
| core-graphics | 0.23.2 | MIT OR Apache-2.0 | MIT |
| core-graphics-types | 0.1.3 | MIT OR Apache-2.0 | MIT |
| core_maths | 0.1.1 | MIT | MIT |
| cosmic-text | 0.19.0 | MIT OR Apache-2.0 | MIT |
| crossbeam-utils | 0.8.23 | MIT OR Apache-2.0 | MIT |
| crunchy | 0.2.4 | MIT | MIT |
| cursor-icon | 1.2.0 | MIT OR Apache-2.0 OR Zlib | MIT |
| dispatch | 0.2.0 | MIT | MIT |
| dispatch2 | 0.3.1 | Zlib OR Apache-2.0 OR MIT | Zlib |
| dlib | 0.5.3 | MIT | MIT |
| document-features | 0.2.12 | MIT OR Apache-2.0 | MIT |
| downcast-rs | 1.2.1 | MIT/Apache-2.0 | MIT |
| dpi | 0.1.2 | Apache-2.0 AND MIT | Apache-2.0 AND MIT |
| equivalent | 1.0.3 | Apache-2.0 OR MIT | Apache-2.0 |
| errno | 0.3.14 | MIT OR Apache-2.0 | MIT |
| fastrand | 2.5.0 | Apache-2.0 OR MIT | Apache-2.0 |
| find-msvc-tools | 0.1.15 | MIT OR Apache-2.0 | MIT |
| foldhash | 0.2.0 | Zlib | Zlib |
| font-types | 0.11.3 | MIT OR Apache-2.0 | MIT |
| font-types | 0.12.7 | MIT OR Apache-2.0 | MIT |
| fontconfig-parser | 0.5.8 | MIT | MIT |
| fontdb | 0.23.0 | MIT | MIT |
| foreign-types | 0.5.0 | MIT/Apache-2.0 | MIT |
| foreign-types-macros | 0.2.4 | MIT/Apache-2.0 | MIT |
| foreign-types-shared | 0.3.1 | MIT/Apache-2.0 | MIT |
| futures-core | 0.3.34 | MIT OR Apache-2.0 | MIT |
| futures-io | 0.3.34 | MIT OR Apache-2.0 | MIT |
| futures-task | 0.3.34 | MIT OR Apache-2.0 | MIT |
| futures-util | 0.3.34 | MIT OR Apache-2.0 | MIT |
| gethostname | 1.1.0 | Apache-2.0 | Apache-2.0 |
| getrandom | 0.3.4 | MIT OR Apache-2.0 | MIT |
| getrandom | 0.4.3 | MIT OR Apache-2.0 | MIT |
| gl_generator | 0.14.0 | Apache-2.0 | Apache-2.0 |
| glow | 0.17.0 | MIT OR Apache-2.0 OR Zlib | MIT |
| glutin_wgl_sys | 0.6.1 | Apache-2.0 | Apache-2.0 |
| gpu-allocator | 0.28.0 | MIT OR Apache-2.0 | MIT |
| half | 2.7.1 | MIT OR Apache-2.0 | MIT |
| harfrust | 0.5.2 | MIT | MIT |
| hashbrown | 0.16.1 | MIT OR Apache-2.0 | MIT |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 | MIT |
| hermit-abi | 0.5.3 | MIT OR Apache-2.0 | MIT |
| home | 0.5.12 | MIT OR Apache-2.0 | MIT |
| indexmap | 2.14.2 | Apache-2.0 OR MIT | Apache-2.0 |
| itoa | 1.0.18 | MIT OR Apache-2.0 | MIT |
| jni | 0.22.4 | MIT OR Apache-2.0 | MIT |
| jni-macros | 0.22.4 | MIT OR Apache-2.0 | MIT |
| jni-sys | 0.3.1 | MIT OR Apache-2.0 | MIT |
| jni-sys | 0.4.1 | MIT OR Apache-2.0 | MIT |
| jni-sys-macros | 0.4.1 | MIT OR Apache-2.0 | MIT |
| jobserver | 0.1.35 | MIT OR Apache-2.0 | MIT |
| js-sys | 0.3.107 | MIT OR Apache-2.0 | MIT |
| khronos-egl | 6.0.0 | MIT/Apache-2.0 | MIT |
| khronos_api | 3.1.0 | Apache-2.0 | Apache-2.0 |
| libc | 0.2.191 | MIT OR Apache-2.0 | MIT |
| libloading | 0.8.9 | ISC | ISC |
| libm | 0.2.16 | MIT | MIT |
| libredox | 0.1.25 | MIT | MIT |
| linebender_resource_handle | 0.1.1 | Apache-2.0 OR MIT | Apache-2.0 |
| linux-raw-sys | 0.4.15 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | Apache-2.0 |
| linux-raw-sys | 0.12.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | Apache-2.0 |
| litrs | 1.0.1 | MIT OR Apache-2.0 | MIT |
| lock_api | 0.4.14 | MIT OR Apache-2.0 | MIT |
| log | 0.4.35 | MIT OR Apache-2.0 | MIT |
| memchr | 2.8.3 | Unlicense OR MIT | Unlicense |
| memmap2 | 0.9.12 | MIT OR Apache-2.0 | MIT |
| miow | 0.6.1 | MIT OR Apache-2.0 | MIT |
| naga | 30.0.1 | MIT OR Apache-2.0 | MIT |
| naga-types | 30.0.1 | MIT OR Apache-2.0 | MIT |
| ndk | 0.9.0 | MIT OR Apache-2.0 | MIT |
| ndk-context | 0.1.1 | MIT OR Apache-2.0 | MIT |
| ndk-sys | 0.6.0+11769913 | MIT OR Apache-2.0 | MIT |
| num-traits | 0.2.19 | MIT OR Apache-2.0 | MIT |
| num_enum | 0.7.6 | BSD-3-Clause OR MIT OR Apache-2.0 | BSD-3-Clause |
| num_enum_derive | 0.7.6 | BSD-3-Clause OR MIT OR Apache-2.0 | BSD-3-Clause |
| objc-sys | 0.3.5 | MIT | MIT |
| objc2 | 0.5.3 | MIT | MIT |
| objc2 | 0.6.5 | MIT | MIT |
| objc2-app-kit | 0.2.2 | MIT | MIT |
| objc2-cloud-kit | 0.2.2 | MIT | MIT |
| objc2-contacts | 0.2.2 | MIT | MIT |
| objc2-core-data | 0.2.2 | MIT | MIT |
| objc2-core-foundation | 0.3.2 | Zlib OR Apache-2.0 OR MIT | Zlib |
| objc2-core-graphics | 0.3.2 | Zlib OR Apache-2.0 OR MIT | Zlib |
| objc2-core-image | 0.2.2 | MIT | MIT |
| objc2-core-location | 0.2.2 | MIT | MIT |
| objc2-encode | 4.1.0 | MIT | MIT |
| objc2-foundation | 0.2.2 | MIT | MIT |
| objc2-foundation | 0.3.2 | MIT | MIT |
| objc2-io-surface | 0.3.2 | Zlib OR Apache-2.0 OR MIT | Zlib |
| objc2-link-presentation | 0.2.2 | MIT | MIT |
| objc2-metal | 0.2.2 | MIT | MIT |
| objc2-metal | 0.3.2 | Zlib OR Apache-2.0 OR MIT | Zlib |
| objc2-quartz-core | 0.2.2 | MIT | MIT |
| objc2-quartz-core | 0.3.2 | Zlib OR Apache-2.0 OR MIT | Zlib |
| objc2-symbols | 0.2.2 | MIT | MIT |
| objc2-ui-kit | 0.2.2 | MIT | MIT |
| objc2-uniform-type-identifiers | 0.2.2 | MIT | MIT |
| objc2-user-notifications | 0.2.2 | MIT | MIT |
| once_cell | 1.21.4 | MIT OR Apache-2.0 | MIT |
| orbclient | 0.3.55 | MIT | MIT |
| ordered-float | 5.5.1 | MIT | MIT |
| owned_ttf_parser | 0.25.1 | Apache-2.0 | Apache-2.0 |
| parking_lot | 0.12.5 | MIT OR Apache-2.0 | MIT |
| parking_lot_core | 0.9.12 | MIT OR Apache-2.0 | MIT |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 | MIT |
| pin-project | 1.1.13 | Apache-2.0 OR MIT | Apache-2.0 |
| pin-project-internal | 1.1.13 | Apache-2.0 OR MIT | Apache-2.0 |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT | Apache-2.0 |
| piper | 0.2.5 | MIT OR Apache-2.0 | MIT |
| pkg-config | 0.3.34 | MIT OR Apache-2.0 | MIT |
| plain | 0.2.3 | MIT/Apache-2.0 | MIT |
| polling | 3.11.0 | Apache-2.0 OR MIT | Apache-2.0 |
| portable-atomic | 1.15.0 | Apache-2.0 OR MIT | Apache-2.0 |
| portable-atomic-util | 0.2.8 | Apache-2.0 OR MIT | Apache-2.0 |
| presser | 0.3.1 | MIT OR Apache-2.0 | MIT |
| proc-macro-crate | 3.5.0 | MIT OR Apache-2.0 | MIT |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 | MIT |
| profiling | 1.0.18 | MIT OR Apache-2.0 | MIT |
| quick-xml | 0.41.0 | MIT | MIT |
| quote | 1.0.47 | MIT OR Apache-2.0 | MIT |
| r-efi | 5.3.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later | MIT |
| r-efi | 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later | MIT |
| range-alloc | 0.1.5 | MIT OR Apache-2.0 | MIT |
| rangemap | 1.8.0 | MIT/Apache-2.0 | MIT |
| raw-window-handle | 0.6.2 | MIT OR Apache-2.0 OR Zlib | MIT |
| raw-window-metal | 1.1.0 | MIT OR Apache-2.0 | MIT |
| read-fonts | 0.37.0 | MIT OR Apache-2.0 | MIT |
| read-fonts | 0.41.0 | MIT OR Apache-2.0 | MIT |
| redox_syscall | 0.4.1 | MIT | MIT |
| redox_syscall | 0.5.18 | MIT | MIT |
| redox_syscall | 0.9.5 | MIT | MIT |
| regex-automata | 0.4.18 | MIT OR Apache-2.0 | MIT |
| regex-syntax | 0.8.11 | MIT OR Apache-2.0 | MIT |
| renderdoc-sys | 1.1.0 | MIT OR Apache-2.0 | MIT |
| roxmltree | 0.20.0 | MIT OR Apache-2.0 | MIT |
| rustc-hash | 1.1.0 | Apache-2.0/MIT | Apache-2.0 |
| rustc-hash | 2.1.3 | Apache-2.0 OR MIT | Apache-2.0 |
| rustc_version | 0.4.1 | MIT OR Apache-2.0 | MIT |
| rustix | 0.38.44 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | Apache-2.0 |
| rustix | 1.1.6 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | Apache-2.0 |
| rustix-openpty | 0.2.0 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | Apache-2.0 |
| rustversion | 1.0.23 | MIT OR Apache-2.0 | MIT |
| same-file | 1.0.6 | Unlicense/MIT | Unlicense |
| scoped-tls | 1.0.1 | MIT/Apache-2.0 | MIT |
| scopeguard | 1.2.0 | MIT OR Apache-2.0 | MIT |
| sctk-adwaita | 0.10.1 | MIT | MIT |
| self_cell | 1.3.0 | Apache-2.0 OR GPL-2.0-only | Apache-2.0 |
| semver | 1.0.28 | MIT OR Apache-2.0 | MIT |
| serde | 1.0.229 | MIT OR Apache-2.0 | MIT |
| serde_core | 1.0.229 | MIT OR Apache-2.0 | MIT |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 | MIT |
| serde_json | 1.0.154 | MIT OR Apache-2.0 | MIT |
| shlex | 2.0.1 | MIT OR Apache-2.0 | MIT |
| signal-hook | 0.4.6 | MIT OR Apache-2.0 | MIT |
| signal-hook-registry | 1.4.8 | MIT OR Apache-2.0 | MIT |
| simd_cesu8 | 1.2.0 | Apache-2.0 OR MIT | Apache-2.0 |
| simdutf8 | 0.1.5 | MIT OR Apache-2.0 | MIT |
| skrifa | 0.40.0 | MIT OR Apache-2.0 | MIT |
| skrifa | 0.44.0 | MIT OR Apache-2.0 | MIT |
| slab | 0.4.12 | MIT | MIT |
| slotmap | 1.1.1 | Zlib | Zlib |
| smallvec | 1.16.4 | MIT OR Apache-2.0 | MIT |
| smithay-client-toolkit | 0.19.2 | MIT | MIT |
| smol_str | 0.2.2 | MIT OR Apache-2.0 | MIT |
| smol_str | 0.3.7 | MIT OR Apache-2.0 | MIT |
| spirv | 0.4.0+sdk-1.4.341.0 | Apache-2.0 | Apache-2.0 |
| static_assertions | 1.1.0 | MIT OR Apache-2.0 | MIT |
| strict-num | 0.1.1 | MIT | MIT |
| swash | 0.2.10 | Apache-2.0 OR MIT | Apache-2.0 |
| syn | 2.0.119 | MIT OR Apache-2.0 | MIT |
| syn | 3.0.9 | MIT OR Apache-2.0 | MIT |
| sys-locale | 0.3.2 | MIT OR Apache-2.0 | MIT |
| termcolor | 1.4.1 | Unlicense OR MIT | Unlicense |
| thiserror | 1.0.69 | MIT OR Apache-2.0 | MIT |
| thiserror | 2.0.21 | MIT OR Apache-2.0 | MIT |
| thiserror-impl | 1.0.69 | MIT OR Apache-2.0 | MIT |
| thiserror-impl | 2.0.21 | MIT OR Apache-2.0 | MIT |
| tiny-skia | 0.11.4 | BSD-3-Clause | BSD-3-Clause |
| tiny-skia-path | 0.11.4 | BSD-3-Clause | BSD-3-Clause |
| tinyvec | 1.13.3 | Zlib OR Apache-2.0 OR MIT | Zlib |
| tokio | 1.54.1 | MIT | MIT |
| toml_datetime | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 | MIT |
| toml_edit | 0.25.17+spec-1.1.0 | MIT OR Apache-2.0 | MIT |
| toml_parser | 1.1.5+spec-1.1.0 | MIT OR Apache-2.0 | MIT |
| tracing | 0.1.44 | MIT | MIT |
| tracing-core | 0.1.36 | MIT | MIT |
| ttf-parser | 0.25.1 | MIT OR Apache-2.0 | MIT |
| unicode-bidi | 0.3.18 | MIT OR Apache-2.0 | MIT |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 | MIT AND Unicode-3.0 |
| unicode-linebreak | 0.1.5 | Apache-2.0 | Apache-2.0 |
| unicode-script | 0.5.9 | MIT OR Apache-2.0 | MIT |
| unicode-segmentation | 1.13.3 | MIT OR Apache-2.0 | MIT |
| unicode-width | 0.2.3 | MIT OR Apache-2.0 | MIT |
| version_check | 0.9.5 | MIT/Apache-2.0 | MIT |
| vte | 0.15.0 | Apache-2.0 OR MIT | Apache-2.0 |
| walkdir | 2.5.0 | Unlicense/MIT | Unlicense |
| wasip2 | 1.0.4+wasi-0.2.12 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | Apache-2.0 |
| wasm-bindgen | 0.2.130 | MIT OR Apache-2.0 | MIT |
| wasm-bindgen-futures | 0.4.80 | MIT OR Apache-2.0 | MIT |
| wasm-bindgen-macro | 0.2.130 | MIT OR Apache-2.0 | MIT |
| wasm-bindgen-macro-support | 0.2.130 | MIT OR Apache-2.0 | MIT |
| wasm-bindgen-shared | 0.2.130 | MIT OR Apache-2.0 | MIT |
| wayland-backend | 0.3.17 | MIT | MIT |
| wayland-client | 0.31.15 | MIT | MIT |
| wayland-csd-frame | 0.3.0 | MIT | MIT |
| wayland-cursor | 0.31.14 | MIT | MIT |
| wayland-protocols | 0.32.13 | MIT | MIT |
| wayland-protocols-plasma | 0.3.12 | MIT | MIT |
| wayland-protocols-wlr | 0.3.12 | MIT | MIT |
| wayland-scanner | 0.31.11 | MIT | MIT |
| wayland-sys | 0.31.11 | MIT | MIT |
| web-sys | 0.3.107 | MIT OR Apache-2.0 | MIT |
| web-time | 1.1.0 | MIT OR Apache-2.0 | MIT |
| wgpu | 30.0.1 | MIT OR Apache-2.0 | MIT |
| wgpu-core | 30.0.1 | MIT OR Apache-2.0 | MIT |
| wgpu-core-deps-apple | 30.0.1 | MIT OR Apache-2.0 | MIT |
| wgpu-core-deps-emscripten | 30.0.1 | MIT OR Apache-2.0 | MIT |
| wgpu-core-deps-windows-linux-android | 30.0.1 | MIT OR Apache-2.0 | MIT |
| wgpu-hal | 30.0.1 | MIT OR Apache-2.0 | MIT |
| wgpu-naga-bridge | 30.0.1 | MIT OR Apache-2.0 | MIT |
| wgpu-types | 30.0.1 | MIT OR Apache-2.0 | MIT |
| winapi-util | 0.1.11 | Unlicense OR MIT | Unlicense |
| windows | 0.62.2 | MIT OR Apache-2.0 | MIT |
| windows-collections | 0.3.2 | MIT OR Apache-2.0 | MIT |
| windows-core | 0.62.2 | MIT OR Apache-2.0 | MIT |
| windows-future | 0.3.2 | MIT OR Apache-2.0 | MIT |
| windows-implement | 0.60.2 | MIT OR Apache-2.0 | MIT |
| windows-interface | 0.59.3 | MIT OR Apache-2.0 | MIT |
| windows-link | 0.2.1 | MIT OR Apache-2.0 | MIT |
| windows-numerics | 0.3.1 | MIT OR Apache-2.0 | MIT |
| windows-result | 0.4.1 | MIT OR Apache-2.0 | MIT |
| windows-strings | 0.5.1 | MIT OR Apache-2.0 | MIT |
| windows-sys | 0.52.0 | MIT OR Apache-2.0 | MIT |
| windows-sys | 0.59.0 | MIT OR Apache-2.0 | MIT |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 | MIT |
| windows-targets | 0.52.6 | MIT OR Apache-2.0 | MIT |
| windows-threading | 0.2.1 | MIT OR Apache-2.0 | MIT |
| windows_aarch64_gnullvm | 0.52.6 | MIT OR Apache-2.0 | MIT |
| windows_aarch64_msvc | 0.52.6 | MIT OR Apache-2.0 | MIT |
| windows_i686_gnu | 0.52.6 | MIT OR Apache-2.0 | MIT |
| windows_i686_gnullvm | 0.52.6 | MIT OR Apache-2.0 | MIT |
| windows_i686_msvc | 0.52.6 | MIT OR Apache-2.0 | MIT |
| windows_x86_64_gnu | 0.52.6 | MIT OR Apache-2.0 | MIT |
| windows_x86_64_gnullvm | 0.52.6 | MIT OR Apache-2.0 | MIT |
| windows_x86_64_msvc | 0.52.6 | MIT OR Apache-2.0 | MIT |
| winit | 0.30.13 | Apache-2.0 | Apache-2.0 |
| winnow | 1.0.4 | MIT | MIT |
| wit-bindgen | 0.57.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | Apache-2.0 |
| x11-dl | 2.21.0 | MIT | MIT |
| x11rb | 0.13.2 | MIT OR Apache-2.0 | MIT |
| x11rb-protocol | 0.13.2 | MIT OR Apache-2.0 | MIT |
| xcursor | 0.3.11 | MIT | MIT |
| xkbcommon-dl | 0.4.2 | MIT | MIT |
| xkeysym | 0.2.1 | MIT OR Apache-2.0 OR Zlib | MIT |
| xml-rs | 0.8.29 | MIT | MIT |
| yazi | 0.2.2 | Apache-2.0 OR MIT | Apache-2.0 |
| zeno | 0.3.3 | Apache-2.0 OR MIT | Apache-2.0 |
| zerocopy | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT | BSD-2-Clause |
| zerocopy-derive | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT | BSD-2-Clause |
| zmij | 1.0.23 | MIT | MIT |
