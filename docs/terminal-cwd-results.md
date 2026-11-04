# Terminal cwd result

`v0.4.4-panel.2`: `scripts/gate-terminal-cwd.sh` passed once, exit 0.
A real shell changed directory and the owner query returned that absolute path
with source `proc`. A local, percent-encoded OSC 7 report then took precedence
with source `osc7`. Foreign/malformed reports left the valid report intact.
Idle terminals returned null; agent and v1/v2 access were denied.

This is the narrow approved owner-only v3 addition. No workspace schema,
capability list or existing operation changed. The panel milestone gate remains
separate and will exercise the query as part of opening a real clicked file.
