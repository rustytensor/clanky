# Post-AI mode

Post-AI mode blocks agents while keeping your terminals, shells, owner API and secret
masking available. Server enforcement, command-line startup and MCP refusal are connected.

## Turn it on

Set `{"post_ai":true}` in your [config file](config.md), or start with:

```sh
clanky --post-ai
```

The server applies config changes live. An authenticated owner can also use the
[v4 post_ai operation](agent-api.md#version-4-agent-control-events-and-owner-safety-controls).

- New agent connections and requests are refused, including requests that skip hello.
- Existing agents are disconnected and pending waits are cancelled.
- Agent hello reports `agents are off (Post-AI mode)`.
- `clanky mcp` exits with status 2 and that message, even if its own config says false
  but the target server has the mode enabled.
- Existing shells and owner connections stay alive.

**`--post-ai` forces the target server on until it restarts**, including when attaching to
an existing server. Config edits and owner RPC cannot turn off that forced mode.

Without a force, set `post_ai` to false to turn it off. Revoked permissions are not restored:
the owner must explicitly resume agents and grant the required tab permissions again.

## Window controls

Settings → General now switches the mode live. The window hides Agents, the right
column, agent badges and access switches; terminal panes use the freed width.
At the 1240×760 reference size, pane A is 952 pixels wide with 116 columns.
Settings → Agents provides explicit Resume and per-pane grants after leaving the mode.

Tabs, splits, Settings, themes and masking remain available. Unconverted palette,
search, compose and editor entries stay hidden in this review build in either mode.

See [verification results](m12-postai-results.md) and [agent access](getting-started.md#agents-and-safety).
