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

The Settings → General switch and hiding agent-only controls are still being connected.
You may still see agent controls, but server refusal is enforced. The completed plain-terminal
view keeps tabs, splits, command cards, search, Settings, editing, themes and masking; it hides
Agents, activity, agent badges/access controls, Take over, Ask agent, agent compose and change review.

See [verification results](m12-postai-results.md) and [agent access](getting-started.md#agents-and-safety).
