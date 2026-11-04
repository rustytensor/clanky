# Clanky

A GPU-rendered terminal for Linux, written in Rust, built for people who work alongside coding agents.

> Status: Linux M3 automated gate passed, including M2 and M1 regression checks.
> See [M3 results](docs/m3-results.md). Interactive UI and clipboard acceptance remain manual.

## What it is

- **Fast:** drawn on the GPU with [wgpu](https://wgpu.rs); falls back to software rendering when no GPU is available.
- **Tabs and splits:** selectable window tabs, each with independently focused terminal panes.
- **Images:** a bounded Kitty direct-transfer subset for PNG, RGB and RGBA, clipped to each pane.
- **Agent-ready:** a local socket API. An agent can open a tab, run a command, wait for it to finish, and read back
  that command's exit code and output: no screen scraping, no tmux paste tricks. You decide per tab whether an agent
  may drive it.

## Built on

- a fork of Alacritty's terminal core ([alacritty_terminal](https://crates.io/crates/alacritty_terminal)), in `crates/clanky-vt`
- [wgpu](https://crates.io/crates/wgpu) for rendering
- [cosmic-text](https://crates.io/crates/cosmic-text) for fonts and shaping
- [winit](https://crates.io/crates/winit) for windows and input

## Build and run

```sh
cargo build --workspace --bins -j 8
"${CARGO_TARGET_DIR:-target}/debug/clanky"
```

Linux only. The GUI needs a display and `XDG_RUNTIME_DIR` for its private agent
socket. Use `clanky --headless --socket /path/to/private-directory/agent.sock`
for socket-only operation. The directory must belong to the current user and
have no group/other permissions. The app never removes an existing socket.

Ctrl+Shift+T opens a tab. Ctrl+Shift+D splits right; Ctrl+Shift+E splits below.
Click a pane to focus it; Ctrl+Shift+W closes that pane. Click the tab strip or
use Ctrl+Tab to switch groups. Ctrl+Shift+A toggles agent permission for the
focused terminal; new GUI terminals deny access by default. Socket terminal IDs
and command routing remain unchanged; see [the socket design](docs/design-m1.md).

The default font is 12 pt, scaled for the display. Ctrl+=/- adjusts it by one
point; Ctrl+0 resets it. Load JSON with `--config PATH`, otherwise from
`$XDG_CONFIG_HOME/clanky/config.json` or `$HOME/.config/clanky/config.json`:

```json
{"font_size_pt":12,"columns":80,"rows":24}
```

Font sizes are 6–48 pt. Unknown or invalid settings are errors; a missing default
file uses defaults. Zoom and split layout are session-only. Headless operation
keeps the socket's existing terminal-size defaults.

Kitty images support direct data, chunked transfer, stored placement and deletion.
File/shared-memory transport, cropping, compression controls, animation and
z-order are rejected. Explicit cell dimensions are recommended; natural sizing
uses 10x20 cells. See [M2 design](docs/design-m2.md) and the
[parser patch note](vendor/vte/CLANKY-PATCH.md).

After building, `scripts/gate-m2.sh` checks image pixels, splits, PTYs, config,
DPI/zoom and M1 regression. It needs no display; a software GPU adapter is allowed.

## Command blocks and agent tasks

The sidebar shows the focused terminal's OSC 133 command blocks and agent tasks.
It needs a shell that emits those markers. Click a block to select its output,
then Copy, Collapse, Rerun or Send. Collapse only hides the preview; retained
output is unchanged. Large/truncated blocks require an explicit API byte range.
Ctrl+Shift+B hides the sidebar; Ctrl+Shift+P opens the keyboard palette.

Rerun asks for JSON such as `{"argv":["sh","-c","printf again"],"cwd":"/"}`.
Edit it with typing/Backspace, then Enter to confirm a **new terminal**. No command
is guessed from block text. Escape cancels. Open prompts keep their original
IDs even if another client changes the focused terminal.

Send queues selected bytes for an agent; it does not launch one. Task claimed
states (queued/running/done/failed/cancelled) are shown separately from the source
process's actual status. Select a task and use Mark done to update its claim.
Scroll the sidebar and use its page controls for more blocks/tasks.

Desktop copy uses an existing `wl-copy` on Wayland or `xclip` on X11. Missing
helpers produce a visible error; none is installed automatically. Helpers are
owned until replacement or app shutdown. A successful launch is not proof of
clipboard ownership. The headless socket returns copy bytes without touching it.

New socket operations use `v:2`; v1 `run` and routing stay unchanged. See
[the M3 operation table and examples](docs/design-m3.md). `scripts/gate-m3.sh`
checks socket and palette actions, then M2 (which includes M1), once each.

## Editor core

`clanky-edit` provides standalone UTF-8 editing, selections, undo/redo, source
locations and per-hunk accept/reject review. Accepted edits stay in memory until
explicit save. Saves detect external changes and atomically replace the resolved
target, preserving symlinks. There is no editor panel or socket integration yet.
See [the API and gate](docs/design-m7-core.md) and [results](docs/m7-core-results.md).

## Masking core

`clanky-mask` recognizes common credential shapes and caller-supplied secret
values across output chunks. It returns named masks, bounds its trailing buffer,
and wipes internal secret storage on drop. It is not yet wired into agent reads;
the person's display is untouched. See [matching rules and API](docs/design-m6-masking.md)
and [gate results](docs/m6-masking-results.md) for coverage and limits.

## Why "Clanky"

A clanker is a robot. This is where they work.

## Licence

Clanky is licensed under either of

- MIT License ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.

## Thanks

Clanky's terminal engine (`crates/clanky-vt`) and escape-sequence parser (`crates/clanky-vte`) are forks of
[Alacritty](https://github.com/alacritty/alacritty)'s `alacritty_terminal` and `vte`. See [NOTICE](NOTICE).
