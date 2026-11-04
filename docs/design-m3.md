# M3: command-block actions, tasks and a palette

## Gate first

Command: `scripts/gate-m3.sh`. The expectations here precede implementation.
Missing binaries, timeouts and unsupported adapters fail; no skipped checks or
downloads. Python makes the inputs and compares observations from real code.

1. Start the real headless socket server. Open one allowed terminal using v1.
   Run a generated shell script in its PTY: OSC 133 C, `printf 'one\n'`, OSC 133
   D; then OSC 133 C, `printf 'two\n'`, OSC 133 D. Both D markers carry exit 0.
   The two completed block outputs must be exactly UTF-8 `one` and `two`.
   Their distinct block IDs belong to the returned command ID. The raw command
   transcript must be exactly `ESC]133;C BEL one CR LF ESC]133;D;0 BEL` followed
   by the same sequence for `two`, with no spaces between these named pieces.
2. Select bytes [0,3) of the first block and copy. Expected base64 `b25l`, length
   3, text `one`. Collapse that block: its view flag becomes true, the second
   remains false, both retained block outputs and the entire raw transcript
   stay byte-for-byte equal to the above constants. Expand restores false.
   Selection is a UTF-8 byte range, never a screen coordinate or a guessed command.
3. Rerun that block with explicitly supplied argv `["sh","-c","printf again"]`
   and the fixture directory as cwd. It creates a NEW terminal and a new command
   ID, not input in the source shell. Wait using v1: exact output `again`, exit 0.
   The original two blocks and transcript remain intact. A rerun lacking argv
   returns `invalid_request` and creates no terminal.
4. Send the selected `one` bytes to an agent task labelled `Review first`.
   Expected task payload base64 `b25l`, state `queued`, and the source IDs.
   Update its claimed state to `running`, then `done`; the source process remains
   actually exited with code 0 throughout. Task state is not process status.
   A task list returns that task without its payload; task_get returns the payload.
5. Run the same fixture in a second Engine through the exact palette dispatcher
   used by the GUI, via `clanky-actions-gate`. Compare its observations against
   these SAME fixed constants: selection/copy, collapse/expand, rerun output and
   task transitions. Normalize generated IDs only by following the returned IDs;
   require source/rerun command IDs to differ in each path. The fixture is real
   PTY output in both paths, not fabricated blocks. An explicitly denied socket
   terminal refuses a v2 view with `permission_denied`.
6. Clean mode invokes unchanged `scripts/gate-m2.sh` ONCE, which already includes
   M1. No separate duplicate M1 run.
7. Planted fault: `CLANKY_GATE_FAULT=copy scripts/gate-m3.sh` replaces only the
   observed socket copy payload with base64 `WG5l` (`Xne`). Require FAIL at the
   expected `b25l` comparison, not a crash or an earlier error. Run this once,
   then clean mode once and require final PASS.

Fixture files are private, temporary and removed. Socket startup has a 5-second
limit; each command wait 10 seconds, helper 30 seconds. Cleanup closes the socket,
terminates/reaps the saved server PID and shuts down helper Engines on errors.
Tests never touch the desktop clipboard or a real agent service. Manual acceptance
covers sidebar clicks, palette input, clipboard ownership and interactive shells.

## Additive protocol and shared actions

The v1 envelope, hello reply, operation fields and `run` behavior do not change.
New operations use `v:2`, with the same id/ok/result/error envelope. v2 hello
reports version 2 and capabilities `block_actions`, `tasks`, `palette`. A v2 run
is not introduced. Unknown versions/operations/fields are errors.

Internally `Engine::request("v2.<op>", args, trusted)` handles these additions.
`Engine::palette_action(action, args, trusted)` dispatches the same operation,
with action names equal to the op names below. The GUI uses this entry point;
the gate uses it too. The transport accepts the namespace only under v2, so v1
requests cannot bypass version checks. Metadata is per terminal, shared between
GUI and socket, and discarded when that terminal closes.

Every operation except hello and the static actions list requires `tab_id` and
checks the same terminal agent permission as v1. GUI calls are trusted local
user actions. Block identity is `(tab_id, block_id)` and is resolved against
currently retained OSC blocks. Evicted/missing IDs fail rather than retargeting.
No callback holds a terminal or action-state lock while starting/waiting for a PTY.

| v2 op / palette action | Additional fields | Result |
| --- | --- | --- |
| actions | none | `actions`: select, copy, collapse, rerun, send, task_update, view, tasks, task_get |
| view | optional after, limit (1–64; default 64) | blocks with existing fields plus collapsed; selection or null; has_more, next_after; tasks metadata |
| select | block_id, start, end | selection `{block_id,start,end}` |
| copy | block_id | `{encoding:"base64",data,bytes}` for matching selected range |
| collapse | block_id, collapsed (bool) | `{block_id,collapsed}` |
| rerun | block_id, argv (nonempty string array), cwd (absolute path) | `{tab_id,command_id,source_tab_id,block_id}` |
| send | block_id, label (nonempty, at most 256 UTF-8 bytes) | newly queued task, including payload |
| task_update | task_id, state | updated task metadata |
| tasks | none | `{tasks:[metadata...]}` |
| task_get | task_id | task with payload |

Example: `{"v":2,"id":"pick","op":"select","tab_id":"t1","block_id":"b1","start":0,"end":3}`.
Copy and send require a current selection for that exact block. Selection bounds
must be ordered UTF-8 boundaries within retained output and at most 64 KiB. Copy
returns the block text bytes, NOT raw PTY escape sequences: existing OSC block
normalization removes terminal line endings. Raw output remains in v1 get_output.
Copying does not clear selection. Collapse is view-only and does not alter bytes.
When blocks are evicted, obsolete selection/collapse metadata is pruned; task
payloads are snapshots and survive block eviction until the terminal closes.

Rerun never guesses argv from OSC text and never writes into the existing shell.
It creates a normal terminal inheriting source permission, then uses the unchanged
v1 run path. Failure closes that new terminal. An explicit GUI JSON prompt asks
for argv and absolute cwd; Enter confirms execution in a new terminal. A busy
interactive source shell therefore does not prevent a rerun.

Tasks have `task_id`, `label`, `state`, `tab_id`, `block_id`, `command_id`, and
`process` (actual source command status, or null if no longer retained). Detailed
tasks additionally have `encoding:"base64"`, `data`, `bytes`. Claimed states are
queued/running/done/failed/cancelled. Send queues data only: it starts no agent,
network request or command. A local agent reads tasks and updates their state.
Keep at most 128 tasks per terminal, each payload at most 64 KiB; refuse overflow.
Views use the existing 256 KiB block-output budget and 64-item pagination. Never
silently copy the truncated display preview instead of the selected source bytes.

## Window controls

A bounded right sidebar shows the focused terminal's block list and tasks, with
scrolling for overflow. A toggle restores all width to the terminal. Layout and
PTY resizing reserve its columns; images cannot cover it. Clicking a block selects
its complete output when it fits 64 KiB. Buttons provide Copy, Collapse/Expand,
Rerun and Send. Tasks show claimed state separately from process exit; clicking a
task can mark it done through task_update. Long outputs have clipped previews,
not giant terminal snapshots. Existing terminal input and split shortcuts remain.

Ctrl+Shift+P toggles a keyboard command palette. Up/Down choose actions, Enter
executes against the explicitly selected block/task, Escape cancels. Rerun uses
an editable JSON prompt; a labelled Send prompt may default to `Review block`.
Palette text must not leak into the PTY, including IME commits. Ctrl+Shift+B
shows/hides the sidebar. The same shared dispatcher backs buttons and palette.
A missing selection is a visible error, not an action on whichever output is last.
An open modal pins its displayed target IDs until it closes; a socket close cannot
retarget it. Whole-block selection rejects truncated previews rather than copying
an incomplete block as though it were complete.

Copy also sends the returned bytes to the desktop clipboard through a Linux
platform wrapper, using an available `wl-copy` (Wayland) or `xclip` (X11).
No new Rust dependency or package installation. Missing helper is a visible
clipboard error; the core copy payload remains available. Track the foreground
clipboard helper and terminate/reap it on replacement or app shutdown. Bound
input to 64 KiB and avoid blocking GUI writes to a pipe. Headless copy has no
clipboard side effect. Clipboard display integration is a manual check.

## Delivery

Up to five bounded workers: core actions; GUI sidebar/palette; clipboard platform
adapter; gate harness; spare only if needed. The lead owns transport integration,
manifests and docs, reads all changed lines, and performs serialized -j8 builds.
Workers may check only their own crate at -j2 under the shared build lock. No new
third-party library. No full suites or benchmarks beyond the milestone gate.

Commit/tag/push the design first, then proceed. Patch tags precede the gate;
v0.3.0 records its passing result. Stop on a first real failed check, disallowed
licence, a v1-breaking design question, or about 610k context tokens. Compile
errors during integration remain ordinary development. M4 does not start here.
