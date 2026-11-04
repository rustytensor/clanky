# Clanky

A GPU-rendered terminal for Linux, written in Rust, built for people who work alongside coding agents.

> Status: M5 and remaining M6 automated gate passed, including earlier milestone, masking,
> editor-core and Linux-packaging gates. See [results](docs/m5-results.md). UI shortcuts remain manual checks.

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

## Agent control and safety

Protocol v4 adds `input_state`, `wait_input`, `screen`, `send_key`, `paste` and
cursor-based `events`. Existing v1–v3 operations remain available. Input readiness
uses an observed blocked foreground PTY read after 200 ms of quiet, or OSC prompt
markers; unproven readiness is not guessed from silence. Screen replies contain
visible text, the original cursor coordinates and a paged history range.
Named keys follow terminal modes; paste uses bracketed-paste markers when enabled.

`clanky mcp --socket PATH` exposes agent tools over MCP stdio. It never loads an
owner capability. Each tool connects on demand as an agent; failed requests are
not silently replayed. It cannot expose the owner audit or revocation operations.

Agent raw output, blocks, screen text, tasks and event payloads pass through
`clanky-mask`. Recognized bytes become stars without changing raw byte offsets;
undecided stream prefixes are withheld. Owner content stays unmasked.
**Masking is pattern-based, not a sandbox or universal secret detector.**
It covers eligible environment literals, the server capability and documented
built-ins. Star-containing configured literals fail closed to avoid collisions.
Screen masking is applied to raw-grid logical text before paging, not by reflowing
replacement characters through a second screen.

**Ctrl+Shift+Escape revokes agent permissions and cancels pending waits.**
The same owner action is `v:4, op:"revoke_all"`. New agent-created tabs are then
blocked. Owner `resume_agents` permits their creation again but does not regrant
existing tabs; per-tab owner grants remain explicit. This revokes API access,
not the operating-system processes themselves.

Owner `audit` reads bounded, masked operation records with peer PID, terminal
ancestry, timestamps and result codes. There are 512 records per terminal, with
explicit eviction metadata; `_connection` holds unscoped requests. Events retain
1024 records and report replay gaps rather than silently dropping history.
See [the v4 design and gate](docs/design-m5.md). Existing running servers must be
restarted explicitly to load new code; upgrading a binary does not replace them.

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
