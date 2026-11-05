# Interrupted latency investigation

Based on `c49060b` (`origin/m11-idle`), not the current M12 tree.
No valid before/after numbers, performance fix or child-shell fix were produced.

## Measurement changes

- `frame_trace.rs` adds opt-in phase durations and numeric cursor rectangles.
- `studio.rs` measures preparation, chrome construction, renderer call and total
  frame work. Durations overlap; do not sum them.
- Renderer `lib.rs` logs its adapter when `CLANKY_FRAME_TRACE` is set.
- Renderer `profile.rs` is an **unwired, uncompiled draft**, written after the build.
  It is not declared as a module and contributed no measurement results.

The first three changes built in release mode with 8 build jobs. Binary:
`/srv/devops/build/clanky-latency/clanky-m11`.

## Observations and limits

The app verified RX 7900 XT, RADV/Vulkan, discrete GPU, PCI `0000:03:00.0`,
Mesa 26.2.5. Window: 2576×1078, scale 1, X11/Xwayland, two stacked fish panes.
The generated HOME/config was isolated; the Radeon ICD excluded lavapipe.

XSendEvent produced zero traced keys. A focus-guarded XTest probe recorded three
key events but timed out detecting glyph changes. Neither attempt completed the
benchmark; setup frames are not workload statistics. All probe processes stopped.
The maintainer replaced pixel detection with frame-trace key-to-present timing. That
replacement was not implemented before handover. Do not rely on the pixel probe.

Next: measure typing and `seq 1 200000` on both releases; separate layout, shaping,
scene build and GPU submit. No measured bottleneck has been identified.
M12 `foreground_program` reads the foreground group leader's argv[0], not child
shells sharing its group. This is a code observation, not a reproduced/fixed bug.
`origin/m12-stone-light-contrast` was identified but not measured here.

Artifacts: `/srv/devops/build/clanky-latency/` (failed probe, traces, logs, binary),
and `/srv/devops/build/clanky-real-gpu-summary.txt` (device listing).
Paused palette work remains in stash `81a4cc3431b2114a970e4957d887d6d93ef686d8`;
tracked backup: `/srv/devops/build/clanky-palette-paused.patch`.
