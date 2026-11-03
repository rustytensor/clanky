# Working on Clanky

Clanky is a GPU-rendered terminal in Rust, built for people who work alongside coding agents. Read README.md first.

## Rules

- Build on the libraries in README.md. Do not write a VT parser, a font shaper or a window system of our own.
- Every milestone ends at a gate: one command that says PASS or FAIL on inputs whose expected results were
  worked out before the run. A gate is shown to fail once (a planted fault) before it counts.
- Tests compare real output with an expected value worked out in advance. No tests for wiring, no test matrices.
- Run the one check that exercises your change, once. The full gate runs at the end of a milestone.
- Builds: at most 8 jobs (`cargo build -j 8`). Keep `target/` where `CARGO_TARGET_DIR` points.
- Commits: plain English, what changed and why. No AI tool names in commits or code.
- New dependencies: name each one and why in the commit message. MIT, Apache-2.0, BSD, Zlib or ISC only.
- Never weaken an expected value to make a test pass.

## Versions and tags

Every commit is tagged `vMAJOR.MINOR.PATCH`, annotated (`git tag -a vX.Y.Z -m vX.Y.Z`), and pushed with its tag.

- PATCH (last number): bug fixes and small changes, documents included.
- MINOR (middle): a new feature. Resets PATCH.
- MAJOR (first): a major release (1.0.0, 2.0.0); after 1.0.0, also any change that breaks the socket API or
  config. Resets MINOR and PATCH.
- Before 1.0.0 the API may still change between minor versions.
- `Cargo.toml` versions match the tag.

## Platforms

Linux first; Windows and macOS later. Keep platform code (PTY, process groups, socket permissions) in one
module per platform behind a small interface, so the rest of the code never calls a platform API directly.

## Layout (planned)

- `crates/clanky-core`: terminal state, PTY, tabs, command blocks (OSC 133). No GPU code.
- `crates/clanky-render`: wgpu renderer, glyph atlas, image layer. Can render offscreen for tests.
- `crates/clanky-api`: the local socket API for agents (JSON lines over a Unix socket).
- `crates/clanky`: the app: window, input, config.

## Milestones

### M1: one window with tabs, an agent can drive it

Done when `scripts/gate-m1.sh` prints PASS. The gate checks, without a display:

1. **Agent socket:** start `clanky --headless`, connect to its socket, run `sh -c 'printf hello; exit 3'` in a new tab.
   Expected: exit code 3, output exactly `hello`.
2. **Command blocks:** with a shell that emits OSC 133 markers, run `echo one` then `echo two`. Expected: the API
   lists two blocks, outputs `one` and `two`.
3. **Text rendering:** render offscreen (a software adapter is fine) an 80x24 grid holding a red full-block
   character `█` (U+2588) at row 0, column 0 on a black background. Expected: the pixel at that cell's centre is
   red (255,0,0 within 8 per channel); the pixel at the centre of row 0, column 1 is black.
4. **Images:** send a 2x2 PNG (red, green / blue, white) by the Kitty graphics protocol at cell (2,2), scaled to
   2x2 cells. Expected: each quadrant's centre pixel has that colour, within 8 per channel.
5. **Tabs:** open two tabs through the API and run `printf A` in the first and `printf B` in the second.
   Expected: the first command's output is exactly `A`, the second's exactly `B`, and the tab list holds both.
6. **Planted fault:** `CLANKY_GATE_FAULT=exitcode scripts/gate-m1.sh` must print FAIL.

By hand, by the owner: open a real window, run `vttest` (menu 1 and 2) and `kitten icat` on a picture.

### Later milestones (gates written when each starts)

- **M2: splits (several terminals side by side in one window), config, Wayland polish.**
- **M3: the graphical layer.** What sets Clanky apart: command blocks you can click (copy, rerun, send to an agent,
  collapse), a side panel listing agent tasks and their state, a command palette. Built on the M1 socket API, so
  everything a person can click, an agent can call.
