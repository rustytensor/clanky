# Clanky

A GPU-rendered terminal for Linux, written in Rust, built for people who work alongside coding agents.

> Status: just started. Nothing to install yet.

## What it is

- **Fast:** drawn on the GPU with [wgpu](https://wgpu.rs); falls back to software rendering when no GPU is available.
- **Tabs and splits.**
- **Images:** the Kitty graphics protocol, so `kitten icat` and friends work.
- **Agent-ready:** a local socket API. An agent can open a tab, run a command, wait for it to finish, and read back
  that command's exit code and output: no screen scraping, no tmux paste tricks. You decide per tab whether an agent
  may drive it.

## Built on

- [alacritty_terminal](https://crates.io/crates/alacritty_terminal) for the terminal core
- [wgpu](https://crates.io/crates/wgpu) for rendering
- [cosmic-text](https://crates.io/crates/cosmic-text) for fonts and shaping
- [winit](https://crates.io/crates/winit) for windows and input

## Why "Clanky"

A clanker is a robot. This is where they work.

## Licence

MIT. See [LICENSE](LICENSE).
