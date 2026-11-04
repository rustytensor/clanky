# M11 idle and input latency fix

Gate: the shared window frame scheduler draws one startup frame, then the headless
look gate observes **0 frames during 2 seconds without input**. The planted `idle`
fault forces continuous redraw and must fail. The gate renders through the real
offscreen scene renderer whenever the scheduler requests a frame.

Before changing scheduling, measured over 10 seconds from `/proc/<pid>/stat`:
- Marc's existing release window 369296: 97.40% of one core; server 369297: 14.60%.
- Isolated release test window on a private Xvfb display: 39.40%; server: 15.70%;
  624 frames in 10 seconds. Ten generated key events: mean key-to-present 10.916 ms,
  maximum 19.514 ms. The trace logs timestamps only, never typed text.

The first direct-desktop latency run was discarded: 14 events appeared for 10
injected keys. All subsequent before/after probes use a private display and socket,
with the same config, generated input and renderer environment (LP_NUM_THREADS=8).
The live-window baseline and isolated-display baseline are reported separately.

Plan: event-driven dirty frames; finite animation deadlines; owner-only generation
notices over a separate authenticated connection (no masked-output buffering delay,
no blocking of input RPC); rail polling at most once per second; cached snapshots,
measured text and static chrome. File/settings checks may wake the event loop but
must not redraw unchanged content. No sleep inhibitor or new dependency.

After targets: window and server each below 1% CPU over 10 idle seconds, and every
measured key-to-present interval below 16 ms. This is application event-to-present
call timing, not a physical photon measurement. Marc's existing processes are not
restarted or sent synthetic input. Only private test processes are stopped.
