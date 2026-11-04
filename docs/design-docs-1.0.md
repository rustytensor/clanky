# User documentation gate

Run `scripts/gate-docs.sh`. The Rust source is the independent inventory; the
check does not carry a second hand-maintained list of operations or config keys.
PASS requires:

- `docs/config.md` lists exactly the JSON fields in `Config::from_json`.
- `docs/shortcuts.md` covers window, palette, file prompt, editor, compose,
  scrollback search and text-editor bindings found in their source handlers.
  Context prefixes distinguish reused keys. The revoke shortcut must precede
  modal routing in the window handler.
- `docs/agent-api.md` has exactly one JSON request/response pair per v1–v4 socket
  operation, including transport-level hello/auth/focus_set. Versions and response
  IDs/status envelopes must agree with the source inventory.
- Every MCP tool declared by `mcp::tools` has one tools/call pair, valid JSON-RPC
  envelope and parseable JSON text content. Unknown/duplicate tools fail.
- Relative Markdown links in the user pages and this design resolve to files,
  and obsolete future-release feature claims are absent.

`CLANKY_GATE_FAULT=docs scripts/gate-docs.sh` adds `made_up_config_key` to the
observed documentation key set in memory. It must FAIL at **config keys**, without
changing a file. Run that fault once and the clean gate once at completion.

This is a Python-only source/document comparison: no build, server, display,
real credentials or external API. Rust extraction is a lexical check of the
current dispatch/key-handler patterns, not a general Rust parser. New dispatch
forms must update the extractor rather than silently omit a feature.

Examples state fresh-server/authentication assumptions; IDs/PIDs are illustrative.
The gate checks coverage, JSON/envelopes and links, not runtime execution or every
example's semantic response. Descriptions, masking coverage/limits, config defaults
and example meanings are read against the implementation, not inferred from PASS.
