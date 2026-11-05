# Post-AI server and runtime wiring

Gate: `scripts/gate-m12-postai.sh` (headless; no GPU or full regression suite).
Run first with `CLANKY_POSTAI_FAULT=1`: the gate turns the policy back off before
an agent hello, which must fail the expected-refusal check.

Expected before running:
- With config `post_ai: true` or `--post-ai`, agent hello returns permission_denied
  and exactly `agents are off (Post-AI mode)`. Skipping hello cannot bypass it.
- Owners authenticate before hello and can still list/open/close tabs.
- Enabling through owner RPC cancels a pending 30-second event wait and closes
  an idle agent socket within 2 seconds; existing shells/owner connections stay alive.
- Changing config on disk switches a headless server on within 2 seconds.
- MCP exits 2, stdout empty, stderr exactly the reason and one newline, even when
  its own config says false but the target server says true.
- A server forced by CLI cannot be switched off by a config reload or owner RPC.
- A custom theme changed to #010203 resolves to (1,2,3,255); an unchanged timer
  poll returns false, so it does not request another frame.

Implementation: v4 owner-only `post_ai` takes boolean `enabled` and optional
boolean `force`; replies with effective `post_ai`. A CLI force lasts until the
server restarts, including when a window attaches to an existing server. Turning
ordinary Post-AI off does not restore revoked tab permissions or clear the safety
latch: the owner can explicitly resume agents and grant tab permissions.

Theme data is held in `App.theme_runtime`; the existing 500 ms settings timer
polls it and requests a frame only on change/error. Its `active()` is the last-good
M12 theme. The editor team still maps it into its scene and Settings picker.
No layout or Settings UI file is changed here.

Result: planted policy-off fault failed at the agent-hello refusal; normal gate passed
on its first run. No GUI/GPU test or full regression suite was run. The gate exercised
three real server processes sequentially and cleaned them up. Existing unused-method
and workspace-resolver warnings did not fail the build.
After the gate, startup was tightened to apply policy before layout restore and use
the settings snapshot's initial mode. Final `cargo check -p clanky --bin clanky`
passed; the already-passing gate was not repeated.
