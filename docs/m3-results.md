# M3 results

The automated gate passed on `8e627cf` (`v0.2.3`). Release `v0.3.0` changes only
version metadata and documentation after that run.

| Check | Result |
| --- | --- |
| Planted copy fault | Exit 1: expected `b25l` (`one`), observed `WG5l` (`Xne`) |
| Clean M3 gate | Exit 0, final PASS |
| Socket and palette paths | Both used real PTYs and the same predetermined two-command fixture |
| Select/copy | Exact UTF-8 bytes `one`, range [0,3), length 3 |
| Collapse/expand | Flags changed and restored; both original blocks and raw transcript stayed exact |
| Explicit rerun | New terminal and command IDs; exact output `again`, exit 0; missing argv created no terminal |
| Tasks | Exact selected payload, queued/running/done claims; actual source process stayed exited 0 |
| Permission | Denied terminal refused the new view operation |
| Regression | M2 passed once, including its unchanged M1 checks |

Commands, once in this order:

```sh
CLANKY_GATE_FAULT=copy scripts/gate-m3.sh
scripts/gate-m3.sh
```

The offline workspace/binary build passed at `-j 8`. No new third-party package
was resolved. The app reuses the already locked base64 crate for clipboard bytes;
existing dependency licence choices still apply. Both release commits were
secret-scanned and pushed with annotated tags.

Manual checks remain: block/button clicks, sidebar paging and resizing, palette
editing/IME, interactive-shell OSC integration, desktop clipboard ownership and
helper cleanup in an actual window. The gate does not touch the desktop clipboard
or start an agent service. No GUI screenshot claim, performance benchmark or
full upstream suite is implied. M4 has not started.
