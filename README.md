# Clanky

A GPU-rendered terminal for Linux, written in Rust, built for people who work alongside coding agents.

> Status: Linux M4 automated gate passed, including M1–M3 regression checks.
> See [M4 results](docs/m4-results.md). Interactive window and live SSH acceptance remain manual.

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

Linux only. The GUI needs a display. It connects to a private session server,
starting one on demand at `$XDG_RUNTIME_DIR/clanky/server.sock` (or an existing
private `/run/user/<uid>` runtime directory). `--socket PATH` selects another
private socket. The directory must belong to the current user and have no
group/other permissions. Existing sockets are never blindly removed.

Closing or crashing a window leaves sessions running. Launch Clanky again to
reattach to their tabs, split layout, history, blocks, tasks and permissions.
Ctrl+Shift+W explicitly closes a pane; closing the last session ends the server.
An exited command still has a session until its tab is closed. State survives
client loss, **not server loss or reboot**.

`clanky --server --socket PATH` runs the server in the foreground; `--headless`
is also supported. New shells use the server's shell and directory defaults.
`clanky attach HOST` uses key-based SSH and the remote `clanky --stdio` bridge;
Clanky opens no TCP listener. The remote program must be installed and SSH login
must already work. `clanky --stdio --socket PATH` exposes the same local pipe path.

**Owner authority requires both outside-terminal ancestry and a private capability.**
Connections from inside the server's terminals remain agents even with the real
token; this also applies to stdio. Same-UID processes outside Clanky are not
sandboxed. An agent with SSH access can attach as owner of that host's server.
See [the M4 transport and security design](docs/design-m4.md).

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
file uses defaults. Zoom is window-local; split layout and terminal scrollback
survive reattachment for the server's lifetime. Wheel over a pane to scroll its
history. Headless operation keeps the socket's existing terminal-size defaults.

Kitty images support direct data, chunked transfer, stored placement and deletion.
File/shared-memory transport, cropping, compression controls, animation and
z-order are rejected. Explicit cell dimensions are recommended; natural sizing
uses 10x20 cells. See [M2 design](docs/design-m2.md) and the
[parser patch note](crates/clanky-vte/CLANKY-PATCH.md).

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
`scripts/gate-m4.sh` adds real client-loss/reattachment and permission checks,
then invokes M3 once. It uses a local stdio pipe, not a live SSH connection.

## Editor panes

Click a terminal `path:line[:col]`, use Ctrl+Shift+O, or choose **Open file** in
the palette. Relative paths use that terminal's current directory; an unknown
directory is an error, never a guess. Editor panes split alongside the terminal
workspace and use its font/zoom. They are window-local, not persistent sessions.
Remote terminal file clicks are refused rather than opening a local lookalike.

Type, select with mouse/Shift arrows, Ctrl+Z/Ctrl+Shift+Z undo/redo, Ctrl+S save,
and Ctrl+F find (Enter next, Escape close). Ctrl+Shift+D/E opens another editor
split when an editor has focus. Dirty pane/window closure refuses until save or
undo restores saved contents. Wheel scrolls text or the review preview under it.

**Load changes** imports a chosen proposal JSON into the focused file. Review
cards show old/new text with accept/reject and all-hunk controls; stale hunks
cannot apply. Ctrl+Alt+A/R acts on the selected hunk; add Shift for all pending.
Changes stay in memory until explicit save. The existing `clanky-edit` core
preserves UTF-8/line endings and symlinks and refuses external-file conflicts.
See [panel design](docs/design-m7-panel.md), [controller/proposal API](docs/editor-panel-api.md),
[gate results](docs/m7-panel-results.md), and [editor core](docs/design-m7-core.md).

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
