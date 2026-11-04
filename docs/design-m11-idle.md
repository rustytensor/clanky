# M11 idle and latency fix

All targets passed on the final release measurement. Each CPU sample spans 10 s,
using process utime+stime from `/proc/<pid>/stat`, normalized to one CPU core.
Latency is key-event timestamp to completed present call, not physical photons.

| Private-display release measurement | Before | Window-only WIP | Complete fix |
|---|---:|---:|---:|
| Window idle CPU | 39.40% | 0.20% | **0.30%** |
| Server idle CPU | 15.70% | 12.80% | **0.50%** |
| Idle frames / 10 s | 624 | 0 | **0** |
| Mean key-to-present, 10 keys | 10.916 ms | 8.115 ms | **4.988 ms** |
| Maximum key-to-present | 19.514 ms | 10.153 ms | **6.312 ms** |

Same private Xvfb display setup, config and generated input; LP_NUM_THREADS=8.
The separate, untouched live-window baseline was 97.40% window / 14.60% server.
No after measurement or injected input used Marc's existing window. An initial
public-display latency run was discarded (14 events for 10 injected keys).
Short measurement records and timestamp-only traces are retained locally under
`/srv/devops/build/clanky-idle-measurements`; private fixture directories were removed.

The window now waits for dirty work, uses finite animation deadlines and a separate
owner change-notification connection, and caches snapshots, measured text and static
chrome. Rails are polled at most once per second; clock-only changes do not repaint.
The server readiness monitor parks after an activity check and its 200 ms quiet
check. PTY workers block on master readiness, a control wakeup socket and child pidfd;
older kernels use a one-second child-status deadline when pidfd is unavailable.

Explicit input_state/wait_input requests refresh readiness. Event requests probe at
entry and their timeout; silent readiness changes without PTY activity are detected
at the next request/deadline, not by an endless global process scan.

Checks: real PTY read/input/exit fixture passed (`readygot:hello`, exit 0). Release
build passed. Shared-scheduler headless idle gate: **0 frames/2 s** after startup;
planted continuous-redraw fault: **47 frames**, caught. The full regression suite
was not rerun. No sleep inhibitor, new dependency or existing-process restart.
