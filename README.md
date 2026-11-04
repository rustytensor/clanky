# Clanky

Clanky is a GPU-rendered Linux terminal built for working alongside coding
agents. It has tabs, split panes, images, and a permissioned local socket that
lets agents run commands and read output. A background server keeps sessions
alive when a window closes; they do not survive a server crash or reboot.

**Safety:** agent output is not secret-masked in this version. Do not give an
agent access to a terminal whose output contains secrets it should not see.

## Install

On Fedora, install a Clanky RPM you have obtained with
`sudo dnf install ./clanky-<version>.rpm`. Packaging exists, but a published Fedora repository is
not yet available. To build from source, install Rust and Cargo, then:

```sh
cargo build --workspace --bins -j 8
"${CARGO_TARGET_DIR:-target}/debug/clanky"
```

Clanky is Linux-first. A graphical display is required for its window.

## First steps

Start `clanky` and type in its shell. Ctrl+Shift+T opens a tab; Ctrl+Shift+D
splits right and Ctrl+Shift+E splits below. Click a pane to focus it. Use
Ctrl+Shift+A to allow or deny agent access to the focused terminal. Close the
window and start Clanky again to reattach to running sessions.

Read [getting started](docs/getting-started.md), [configuration](docs/config.md),
[keyboard shortcuts](docs/shortcuts.md), and the [agent API](docs/agent-api.md).
Clanky uses the forked Alacritty terminal core, wgpu, cosmic-text, and winit.

## Coming in 1.0

The editor panel with agent-change review, masking of secrets before agents
read output, the compose box, and additional M5 agent features are not yet
integrated. The standalone editor and masking cores exist, but do not provide
those window or agent protections today.

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
