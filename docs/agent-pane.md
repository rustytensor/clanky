# Start an agent in a pane

One action opens a new pane running an agent's own command-line tool, with agent
access already on for that pane only. Clanky makes no AI calls of its own and stores
no keys or logins: it starts the tool you installed, the same as typing its name.

## In the window

- The **Agent** button next to Split, **Start an agent…** in the command palette, or
  **Ctrl+Shift+G** opens the picker. The palette also lists **Start agent NAME** for
  each installed tool.
- The picker lists the tools from the config. A tool whose command is not on `PATH`
  is shown greyed as "not installed" and cannot be started.
- **Enter** starts the highlighted tool. **Open in** is a split to the right of the
  focused pane (the default) or a new tab: Tab, Left or Right switches it, or click it.
  The pane opens in the focused pane's folder.
- The new pane runs your normal shell, and the tool's command is typed into it. When
  the tool exits you are back at a prompt in the same pane.
- Agent access is on for the new pane alone (`agent_pane_access`, default
  `read+run`). While the tool runs, the pane is drawn as an agent pane (amber edge and
  chip) and its title starts with the tool's name. Ctrl+Shift+Escape still revokes
  every agent at once.
- Recent credits commands an agent ran to the agent by name, not to the pane.
- The **New git worktree** row is shown as "Coming next".
- [Post-AI mode](post-ai.md) removes the button, the palette entries and the shortcut.

## From the command line

```sh
clanky agent claude              # split right of the focused pane, in the current folder
clanky agent codex ~/git/weather # in a given folder
clanky agent pi --tab            # in a new tab
clanky agent --list              # "name<TAB>command<TAB>found|not installed", in config order
```

`NAME` matches a tool's name (case ignored) or its command. `clanky agent` asks the
running Clanky (`--socket PATH`, default the usual socket) to open the pane; with no
Clanky on the default socket it starts one first. It exits 0 when the pane opened,
1 when the tool is not installed (no pane opens) or another error, and 2 for bad
arguments or in Post-AI mode. It needs owner rights, so run it from a terminal
outside Clanky's panes; inside Clanky use Ctrl+Shift+G.

## Config

```json
"agents": [
  {"name": "Claude Code", "command": "claude"},
  {"name": "Codex",       "command": "codex"},
  {"name": "Pi",          "command": "pi"}
],
"agent_pane_access": "read+run"
```

Those are the defaults. `command` is typed into the shell as written, so it may carry
arguments. `agent_pane_access` is `read+run` or `off`.
