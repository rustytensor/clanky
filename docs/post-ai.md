# Post-AI mode

**Not enforced on this branch yet.** The config loader accepts `post_ai`, but the window,
server refusal and MCP exit behaviour are not connected. The main command line does not yet
accept `--post-ai`. Do not rely on this setting to block agents; use the existing access
controls and Ctrl+Shift+Escape instead.

## M12 behaviour

Post-AI mode is a plain terminal with tabs and splits, without agent features.
The intended controls are **Settings → General → Post-AI mode**, `{"post_ai":true}` in
the config file, or `clanky --post-ai`.

- Keep tabs, splits, command cards, search, Settings, the editor and themes.
- Keep secret masking: it is a safety feature, not an agent feature.
- Hide Agents, the activity column, agent badges, access switches, Take over, Ask agent,
  agent compose and pending-change review.
- Refuse new agents and disconnect existing agents when the mode is enabled.
- Refuse agent `hello` with `agents are off (Post-AI mode)`.
- Make `clanky mcp` exit with status 2 and the same message; keep the owner API available.

The command-line switch is intended to stay on for that process even if the config file
changes. Without that switch, `post_ai: false` turns the mode off.

See [agent access and masking](getting-started.md#agents-and-safety) for the controls that work now.
