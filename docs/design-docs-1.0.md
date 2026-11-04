# User documentation gate

Run `scripts/gate-docs.sh`. It must print PASS only when:

- `docs/config.md` lists exactly the JSON fields accepted by `Config::from_json`.
- `docs/shortcuts.md` lists exactly the application shortcut keys in the main
  input handler and the named keys handled by the palette.
- `docs/agent-api.md` has one valid JSON request and response for every v1, v2
  and v3 socket operation dispatched by the source, with no extra operations.
- Relative Markdown links in the user documentation resolve to files.
- `CLANKY_GATE_FAULT=docs scripts/gate-docs.sh` adds a fictitious config key in
  memory and must print FAIL on the config comparison, without changing files.

The comparisons use the Rust source as the independent list of implemented
features. Examples illustrate a stated server state; they do not promise that
IDs or process IDs are constant. The gate checks JSON syntax and operation names,
not the runtime semantics of each response. Features not integrated in v0.4.5
are explicitly marked coming in 1.0, including agent output masking.
