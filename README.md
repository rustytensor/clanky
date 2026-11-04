# Clanky

Clanky is a GPU-rendered Linux terminal for working alongside coding agents.
It has tabs, splits, images, command blocks, editor panes, a spell-checked
compose box and a text-mode editor. A background server keeps running sessions
alive when a window closes. Saved layouts can start fresh shells after a restart;
they do not restore old processes, output or unsaved editor/compose text.

**Safety:** agent raw-output, block/copy/task, screen/history and replayed-output
reads are secret-masked, including through MCP. Recognized bytes become fixed-width
`*` characters, preserving raw byte offsets; undecided streaming suffixes are held
back. The person's own terminal view and owner output reads are never masked. Detection
covers common OpenAI/OpenRouter, GitHub, AWS access-ID, Slack, Hugging Face and
RunPod token shapes, Bearer headers, private-key blocks and eligible secret values
captured from the server's startup environment. It is not universal secret discovery:
unknown formats, transformed values and secrets introduced later may be missed.
See [exact coverage and limits](docs/agent-api.md#masking-and-its-limits).
Ctrl+Shift+Escape revokes all agent access and cancels pending agent waits, even
inside a panel. This is access revocation, not a kill switch for running processes.

## Install

On Fedora, install an obtained RPM with `sudo dnf install ./clanky-<version>.rpm`.
See [Linux packages and SRPMs](docs/release-linux.md). To build from source,
install Rust and Cargo, then:

```sh
cargo build --workspace --bins -j 8
"${CARGO_TARGET_DIR:-target}/debug/clanky"
```

The window needs a graphical display. `clanky edit FILE[:LINE[:COL]] [--review FILE]`
works in a terminal without a display server.

## First steps

Start `clanky` and type in its shell. Ctrl+Shift+T opens a tab; Ctrl+Shift+D/E
splits right/below, using the focused terminal's directory when known.
Ctrl+Shift+O opens an editor, Ctrl+Shift+Enter opens compose, and Ctrl+Shift+F
searches retained terminal text, including wrapped lines. Ctrl+click opens an
HTTP/HTTPS URL; a plain URL click does not open it.

Grant or deny agent access to a terminal with Ctrl+Shift+A. Agents use the
versioned socket API or the agent-only `clanky mcp` adapter. Close the window
and reopen it to reattach; `clanky attach HOST` attaches over SSH.

Read [getting started](docs/getting-started.md), [configuration](docs/config.md),
[keyboard shortcuts](docs/shortcuts.md), and the [agent API](docs/agent-api.md).
Clanky uses its Alacritty terminal/parser forks, wgpu, cosmic-text and winit.

## Why "Clanky"

A clanker is a robot. This is where they work.

## Licence

Clanky is licensed under either [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE),
at your option. Dependencies retain their own licences, including MPL-2.0 for
spellbook. The bundled JetBrains Mono and Inter fonts are under the SIL Open Font
License 1.1. See [NOTICE](NOTICE) for terminal, parser and font attribution.
