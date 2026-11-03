# Working on FTL

FTL is a GPU-rendered terminal in Rust, built for people who work alongside coding agents. Read README.md first.

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

## Layout (planned)

- `crates/ftl-core`: terminal state, PTY, tabs, command blocks (OSC 133). No GPU code.
- `crates/ftl-render`: wgpu renderer, glyph atlas, image layer. Can render offscreen for tests.
- `crates/ftl-api`: the local socket API for agents (JSON lines over a Unix socket).
- `crates/ftl`: the app: window, input, config.

## Milestones

### M1: one window, one tab, an agent can drive it

Done when `scripts/gate-m1.sh` prints PASS. The gate checks, without a display:

1. **Agent socket:** start `ftl --headless`, connect to its socket, run `sh -c 'printf hello; exit 3'` in a new tab.
   Expected: exit code 3, output exactly `hello`.
2. **Command blocks:** with a shell that emits OSC 133 markers, run `echo one` then `echo two`. Expected: the API
   lists two blocks, outputs `one` and `two`.
3. **Text rendering:** render offscreen (a software adapter is fine) an 80x24 grid holding a red full-block
   character `█` (U+2588) at row 0, column 0 on a black background. Expected: the pixel at that cell's centre is
   red (255,0,0 within 8 per channel); the pixel at the centre of row 0, column 1 is black.
4. **Images:** send a 2x2 PNG (red, green / blue, white) by the Kitty graphics protocol at cell (2,2), scaled to
   2x2 cells. Expected: each quadrant's centre pixel has that colour, within 8 per channel.
5. **Planted fault:** `FTL_GATE_FAULT=exitcode scripts/gate-m1.sh` must print FAIL.

By hand, by the owner: open a real window, run `vttest` (menu 1 and 2) and `kitten icat` on a picture.

### Later milestones (gates written when each starts)

- **M2: tabs, splits, config, Wayland polish.**
- **M3: the graphical layer.** What sets FTL apart: command blocks you can click (copy, rerun, send to an agent,
  collapse), a side panel listing agent tasks and their state, a command palette. Built on the M1 socket API, so
  everything a person can click, an agent can call.
