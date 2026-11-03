# Clanky

A GPU-rendered terminal for Linux, written in Rust, built for people who work alongside coding agents.

> Status: Linux M1 automated gate passed. Manual window, vttest and fish acceptance remain pending.
> See [M1 results](docs/m1-results.md). Images and splits are planned for M2.

## What it is

- **Fast:** drawn on the GPU with [wgpu](https://wgpu.rs); falls back to software rendering when no GPU is available.
- **Tabs:** a selectable tab bar; splits are planned for M2.
- **Images:** Kitty graphics support is planned for M2; M1 does not handle image transfers.
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

Ctrl+Shift+T opens a tab; Ctrl+Shift+W closes it. Click a tab or use Ctrl+Tab to
switch. Ctrl+Shift+A toggles agent permission for the selected tab; the window
title shows whether access is enabled. New GUI tabs deny agent access by default.
The socket protocol is documented in [the design](docs/design-m1.md).

## Why "Clanky"

A clanker is a robot. This is where they work.

## Licence

MIT. See [LICENSE](LICENSE).
