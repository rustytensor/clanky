# M5 and remaining M6 results

Gate-tested code: `755aca8` (`v0.4.6`). Release `v0.5.0` changes only version
metadata and documentation. Fixed expectations: [design](design-m5.md).

| Check | Result |
| --- | --- |
| Planted fault | Exit 1 at the fixed star-masking comparison, as intended |
| Clean gate | Exit 0, `PASS: M5 agent control and safety` |
| Input readiness | Real PTY read detected; sleep rejected; Enter completed `got:hello` |
| Screen and input | Fixed visible lines/cursor/history and all 19 named-key/paste bytes matched |
| Masking | Generated 24-byte literal and 23-byte token became exact stars; owner raw bytes stayed exact |
| Replay | Ordered output/finish/wait/close events; identical replay; 1025 acknowledged writes forced an explicit gap |
| MCP and audit | Agent-only permissions, matching results, real peer/terminal ancestry and masked arguments |
| Revocation | Pending input/event/v1 waits denied; all tabs denied; new agent tabs blocked; resume did not regrant old tabs |
| Earlier gates | M4 (including M1–M3), labelled masking, editor core and Linux packaging all passed once |

```sh
CLANKY_GATE_FAULT=mask scripts/gate-m5.sh
scripts/gate-m5.sh
```

Build/capture used the shared lock at `-j8`; 312 package licence choices passed.
No new third-party crate. Three superseded internal text helpers produced unused
warnings; the existing fork resolver warning also remains. No benchmark was run.

Limits: readiness requires observable PTY reads or prompt evidence. Masking is
pattern-based and rejects star-containing literals. Native hotkey/IME/window
behavior and live SSH were not certified by this headless gate. No later
milestone was started.
