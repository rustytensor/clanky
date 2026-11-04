# Working on Clanky

Clanky is a GPU-rendered terminal in Rust, built for people who work alongside coding agents. Read README.md first.

## Rules

- Build on the libraries in README.md. The terminal engine and parser are our forks of Alacritty's
  (`crates/clanky-vt`, `crates/clanky-vte`): change them freely, list every change in their FORK.md, and put the
  notice line at the top of each changed file. Do not write another VT parser, font shaper or window system.
- Every milestone ends at a gate: one command that says PASS or FAIL on inputs whose expected results were
  worked out before the run. A gate is shown to fail once (a planted fault) before it counts.
- Tests compare real output with an expected value worked out in advance. No tests for wiring, no test matrices.
- Run the one check that exercises your change, once. The full gate runs at the end of a milestone.
- Builds: at most 8 jobs (`cargo build -j 8`). Keep `target/` where `CARGO_TARGET_DIR` points.
- Commits: plain English, what changed and why. No AI tool names in commits or code.
- New dependencies: name each one and why in the commit message. MIT, Apache-2.0, BSD, Zlib, ISC, Unicode-3.0, MPL-2.0 (spellbook only), or Unlicense (Unlicense only where MIT is also offered).
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
4. **Tabs:** open two tabs through the API and run `printf A` in the first and `printf B` in the second.
   Expected: the first command's output is exactly `A`, the second's exactly `B`, and the tab list holds both.
5. **Planted fault:** `CLANKY_GATE_FAULT=exitcode scripts/gate-m1.sh` must print FAIL.

By hand, by the owner: open a real window and run `vttest` (menu 1 and 2), and two commands in fish (two blocks).

### M2: splits, images, config and Wayland polish

Automated gate passed: `scripts/gate-m2.sh`, after its planted image fault failed.
The gate and fixed expectations are in `docs/design-m2.md`; results and remaining
manual checks are in `docs/m2-results.md`. The vendored vte APC patch replaces no
VT parser, and the socket's terminal IDs and command routing stay unchanged.

### Later milestones (gates written when each starts)

- **M2: splits, images (Kitty graphics protocol), config, font size and zoom, Wayland polish.** Done: v0.2.0.
- **M3: the graphical layer.** Command blocks you can click (copy, rerun, send to an agent, collapse), a side panel
  listing agent tasks and their state, a command palette. Everything a person can click, an agent can call.
  Automated gate passed: v0.3.0, `scripts/gate-m3.sh` (including M2/M1).
  See `docs/design-m3.md` and `docs/m3-results.md` for expectations and manual checks.
- **M4: persistent sessions.** A background server owns the shells; windows and agents are clients. Close the window
  and reattach later, locally or over SSH (`clanky attach host`).
  Automated gate passed: v0.4.0, `scripts/gate-m4.sh` (including M1–M3).
  Owner authority requires outside-terminal peer ancestry and the capability.
  See `docs/design-m4.md` and `docs/m4-results.md`; live SSH/window checks remain manual.
- **M5: the rest of the agent API.** Know when a program waits for input, read the screen as text, send keys,
  events instead of polling, an MCP server.
- **M6: safety.** Secrets masked before an agent reads output, a per-tab log of what each agent typed, one key that
  stops every agent.
- **M7: GUI editor.** Open files fast (click `path:line` anywhere), edit beside the agent, see its changes live and
  accept or reject each one.
- **M8: compose box.** A multi-line editor with spell-check for long prompts, pasted into the tab.
- **M9: TUI editor.** The same editing core in a text-mode editor for machines without a display.
- **M10: Linux 1.0.** Release packaging for Linux distributions: offline builds (`cargo vendor`),
  a `.desktop` file, AppStream metadata, an icon, a man page, the system's monospace font by default (DejaVu stays the
  gates' test font). Fedora COPR first, then the Arch AUR, then a Fedora package review.
- **After 1.0: Windows and macOS (1.x).**

Smaller items (scrollback search, clickable links and paths, notifications for long commands, new tab in the same
folder, OSC 52 clipboard, drop-down terminal, session restore, Kitty keyboard protocol, accessibility) are folded
into the milestones above as they fit.
