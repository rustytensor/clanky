# M4 results

Gate-tested code: `2c2fdc7` (`v0.3.4`). Release `v0.4.0` changes only metadata
and documentation after that run. Expectations: [M4 design](design-m4.md).

| Check | Result |
| --- | --- |
| Planted fault | Exit 1 at reattached A output: expected `one`, injected `lost` |
| Clean gate | Exit 0; M4 and M1–M3 passed, each once |
| Client SIGKILL | Same server PID and both original terminal IDs survived |
| Detached command | B finished before reattachment; exact output/block and exit 7 retained |
| Retained state | Exact split/focus, A history and scrolled screen, blocks, selection, collapse, task and permissions |
| Inside-terminal probe | Allowed B presented the real token through stdio: owner auth and denied-A read both returned `permission_denied` |
| Outside owner | Fresh client reattached through the real local stdio bridge and read both sessions |
| Last close | Server PID disappeared within the five-second limit |

```sh
CLANKY_GATE_FAULT=reattach scripts/gate-m4.sh
scripts/gate-m4.sh
```

The offline `-j 8` build and binary capture used the shared lock. No new crate;
310 package licence choices were allowed.
Manual checks remain: actual window crash/reattach, concurrent windows, clipboard,
and live remote SSH. The gate used a local pipe; no SSH service was started.
Same-UID outside processes are not sandboxed; SSH access can confer host-owner
authority. Server loss/reboot persistence is not claimed. M5 has not started.
