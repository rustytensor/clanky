# Clanky

A GPU-rendered terminal for Linux, written in Rust, built for people who work alongside coding agents.

> Status: Linux M2 automated gate passed, including M1 regression checks.
> See [M2 results](docs/m2-results.md). Manual Wayland, IME and image-tool checks remain pending.

## What it is

- **Fast:** drawn on the GPU with [wgpu](https://wgpu.rs); falls back to software rendering when no GPU is available.
- **Tabs and splits:** selectable window tabs, each with independently focused terminal panes.
- **Images:** a bounded Kitty direct-transfer subset for PNG, RGB and RGBA, clipped to each pane.
- **Agent-ready:** a local socket API. An agent can open a tab, run a command, wait for it to finish, and read back
  that command's exit code and output: no screen scraping, no tmux paste tricks. You decide per tab whether an agent
  may drive it.

## Built on

- [alacritty_terminal](https://crates.io/crates/alacritty_terminal) for the terminal core
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

## Why "Clanky"

A clanker is a robot. This is where they work.

## Licence

MIT. See [LICENSE](LICENSE).
