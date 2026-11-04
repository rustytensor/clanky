# M5 and remaining M6: observable, permissioned agent control

## Gate first

Command: `scripts/gate-m5.sh`. Expected results below precede implementation.
Use real PTYs and runtime-generated synthetic secrets, never actual credentials.
The gate must not print generated secret values, even on failure.

1. Run `sh -c 'read x; echo got:$x'` in an allowed terminal. After at least
   200 ms without output, an actual blocked PTY read must yield waiting=true,
   evidence=pty_read. Query and bounded wait must agree. Send pasted `hello`
   followed by named Enter. Require exit 0 and a completed output line
   `got:hello`. A sleeping process must not be called waiting for input.
2. Run a generated full-screen fixture on an 8-column, 3-row terminal. It clears
   the screen, writes `alpha` at (0,0), `beta` at (1,0), positions the cursor at
   (2,2), and waits on a release file. Require lines `["alpha","beta",""]` and
   cursor `[2,2]`. A separate three-row fixture prints h0 through h4 with LF;
   require oldest history `["h0","h1","h2"]`, independent of viewport scrolling.
3. A real raw-mode PTY fixture enables bracketed paste and records its input to
   a private fixture file. Named Up, Ctrl-C, Enter and pasted `xy` must produce
   exact bytes `ESC[A`, `03`, `0d`, `ESC[200~xyESC[201~`, respectively. Expectations
   are literal protocol bytes, not computed with the key encoder being tested.
4. Generate a 24-byte ASCII environment literal under CLANKY_TEST_SECRET and
   a built-in token consisting of `sk-` plus 20 random ASCII letters. A program
   prints both, splitting each across writes, inside OSC 133 block markers.
   Owner output must match the original fixture bytes. Every agent output path
   must return 24 stars for the literal and 23 stars for the token, never any
   undecided prefix or secret suffix. Exercise arbitrary small raw-output reads,
   blocks, screen text and replayed events; concatenate pages and compare exact
   expected redacted bytes. Raw byte offsets remain original-stream offsets.
5. Subscribe with a replay cursor before a command writes `event\n` and exits 3.
   Require ordered output_appended and command_finished observations, exit 3,
   monotonic cursors, and identical replay from the saved cursor. Observe a real
   waiting_for_input transition and terminal_closed. A deliberately old cursor
   after bounded-ring overflow must report an explicit gap, not silent loss.
6. Through `clanky mcp`, initialize MCP, list tools and execute the same agent
   screen/input/output operations against real terminals. Results must have the
   same bytes and permissions as the socket. Denied terminals stay denied; this
   command never loads an owner capability or upgrades its connection.
7. Owner audit reads must identify the actual agent peer PID and terminal ancestry,
   operation, masked arguments, timestamp and result code. Require no generated
   secret in the audit. Start pending input/event waits, then revoke all agents:
   both return permission_denied, every current tab is denied, and an agent cannot
   create a fresh allowed tab to evade the stop. Owner reads remain unmasked.
8. Clean mode runs M4 once, which includes M1–M3, plus the existing masker and
   Linux-packaging gates once. Do not rebuild or run unrelated full suites.
9. `CLANKY_GATE_FAULT=mask scripts/gate-m5.sh` changes only an observed masked
   fixture field to `unmasked`. The ordinary exact star comparison must FAIL
   there. Run fault once, then clean once and require final PASS.

All waits have deadlines (at most 30 seconds); fixture cleanup uses saved process
IDs. Capture binaries under the shared build lock. No clipboard, production,
remote-host or real-secret testing. Detailed fixture observations may be private;
reports contain only fixed labels, counts and pass/fail, never secret bytes.

## Settled safety decisions

**Fixed-width masking:** add an opt-in byte-preserving mode to clanky-mask; its
existing labelled-marker mode and tests remain unchanged. Recognized bytes become
ASCII `*`, one per input byte, including recognized token/private-key tails.
Undecided suffixes remain withheld until resolved or stream completion. Maintain
an incremental masked archive beside each raw command archive, before retention
trimming, so arbitrary offsets cannot restart matching halfway through a secret.
Agent cursors use raw-byte coordinates; pending unsafe suffixes are unavailable,
not plaintext. Owner archives and renderer snapshots remain untouched.

Structured block/screen/task/audit reads use the same masking engine on complete
logical text before pagination or slicing. Active incomplete text withholds its
undecided suffix. Screen coordinates remain original terminal cell coordinates;
redacted UTF-8 literals may occupy more text bytes than original characters.
Cross-line screen matching includes line separators and wrapped continuations;
never mask each requested page independently. Treat encoded output as bytes:
decode, mask, then re-encode, never scan only the base64 representation.

**Marker collisions:** star output contains no user-controlled labels and is
never reinterpreted as a secret marker. Reject a masking policy whose literal
itself consists solely of stars; fail closed rather than silently emitting that
literal. Do not claim universal secret discovery: matching covers configured
eligible environment literals and the masker's documented built-ins. No custom
secret scanner beside clanky-mask. Errors never echo policy values.

**Audit ring:** 512 records per terminal, 8 KiB maximum masked argument rendering
per record. Mask before truncation. Record sequence, Unix milliseconds, connection
ID, peer PID plus captured ancestry, operation and result code. Owner-only paging
reports evicted count and oldest cursor. Closed-terminal records remain in a
bounded server-level history, at most 64 terminal rings. No disk log or raw-secret
copy is added. Authentication tokens are omitted, not logged or merely truncated.

**Revocation:** one atomic server epoch change denies all existing tabs, wakes
all agent waits and latches new agent-created tabs off. An explicit owner resume
operation permits new agent-created tabs again but does not regrant old tabs.
Individual owner grants remain possible. Waits hold a permission/epoch lease and
recheck after wakeup and before returning data. The owner view is never masked.
Ctrl+Shift+Escape invokes the same revoke operation, including inside a modal.

## Additive v4 operations and events

Keep v1–v3 envelopes, fields and run semantics. Apply masking and cancellation
at the shared agent boundary, including legacy reads and v2 copies/tasks.
New v4 operations: input_state, wait_input, screen, send_key, paste, events,
audit (owner-only), revoke_all (owner-only), resume_agents (owner-only).
All terminal-specific agent calls check the existing per-tab permission.
The editor team owns v3 terminal_cwd `{tab_id}->{cwd,source}`; do not duplicate it.

Input state distinguishes running, waiting and exited; reports evidence rather
than guessing from silence alone. Linux inspects the foreground PTY reader's
blocked read and activity, with OSC prompt markers as additional evidence.
Unknown evidence is not waiting. Platform observations stay in the Linux module.
Named keys use terminal modes where applicable; paste uses bracket markers only
when enabled. Input is bounded and never interpreted as shell syntax.

Events use a 1024-entry server ring with monotonic sequence numbers and bounded
payloads. Subscriptions are bounded long-poll reads by cursor, not unbounded
per-client queues. Payload bytes come from the safe masked stream for agents;
permission checks happen again during replay. Report explicit gaps for evicted
cursors. Owner records may reference unmasked output separately; do not retain
an extra plaintext event copy. Terminal closure and revocation wake subscribers.

## MCP and delivery

Implement the small MCP stdio tool surface with JSON-RPC initialize, ping,
tools/list, tools/call and initialization notifications. Use protocol version
2024-11-05, bounded frames, explicit schemas and ordinary MCP text-content results.
No resources, sampling or owner capabilities are advertised. Tool calls use an
agent socket connection even when MCP was launched outside a terminal. No new
third-party dependency is planned.

Five bounded workers: masking; input state/screen/keys; events/audit/revocation;
MCP/transport/UI integration; gate. The lead integrates and reviews every changed
line. All builds/checks use the shared build lock, lead -j8 and own-crate workers
-j2. Never touch the editor checkout. Patch tags precede the fault/clean gate;
v0.5.0 records success. Stop at the usual first real failure, disallowed licence,
v1-breaking question or approximately 610k context tokens. No M7 UI work here.
