# Agent socket API and MCP

Use UTF-8 JSON lines over Clanky's private Unix socket, normally
`$XDG_RUNTIME_DIR/clanky/server.sock` (existing private `/run/user/<uid>` fallback).
`clanky --server --socket PATH` selects another private socket. Send one JSON
object plus newline per request. Versions 1–4 share the `id`, `ok`, `result`/`error`
envelope. Requests are at most 1 MiB. Agent IDs are unique nonempty strings of at
most 256 bytes; authenticated owner IDs increase as canonical decimal u64 values.

GUI terminals deny agents by default. Ctrl+Shift+A grants/revokes one terminal;
Ctrl+Shift+Escape revokes all and cancels pending agent waits, including in modals.
Agent-created tabs also default to denied: request `allow_agent:true` explicitly.
After global revocation, agents cannot evade the latch by opening another tab.
An owner resume clears that creation latch but does not regrant existing tabs.
Revocation does not kill processes or sandbox filesystem/network access.

Owner authentication uses v3 auth, a private capability and proven ancestry
outside Clanky's terminals. A process inside a terminal remains an agent even
with the capability; unknown ancestry fails closed. Never log/share the capability.
Same-user processes outside Clanky are not sandboxed. See [session security](design-m4.md).

## Masking and its limits

Agents receive fixed-width masking on v1 get_output/list_blocks; v2 block views,
copy and task payloads/labels (including send/task_get/task_update/tasks); v4
screen text/history and output_appended event replay. MCP uses the same agent
paths. Audit arguments are masked even for the owner's audit reader, with auth
arguments omitted. Agent-facing transport error messages are sanitized too.
The person's block views, selection/copy, task payloads, screen/history and
scrollback search keep the strict masking policy. Search matches masked text,
never raw cells. The authenticated owner's live display is different (approved
by the lead): unconfirmed prefixes are shown as typed. Complete built-in headers
start masking immediately, even below the agent recognizer's minimum body length;
complete known literals stay masked. This display-only exception never reaches
agent screen reads, output, event replay, copy, or search. OSC 52 writes and replies mask decoded text before
encoding. Owner v1 get_output remains a raw archive interface; owner event output
refers back to raw retained archives, or reports
`data_unavailable` when those bytes are no longer available.

The shared clanky-mask recognizer covers these conservative shapes, not issuer
validation. A token character below is ASCII alphanumeric, `_` or `-`:

| Shape | Recognition |
| --- | --- |
| `sk-` | At least 6 token characters; covers OpenAI/OpenRouter-style prefixes |
| `ghp_`, `github_pat_`, `hf_`, `rpa_` | At least 12 token characters |
| Slack `xox` + ASCII letter + `-` | At least 12 token characters |
| AWS `AKIA` | Exactly 16 following uppercase ASCII letters/digits; not arbitrary AWS secret keys |
| `Authorization: Bearer ` | Case-insensitive prefix, exact spaces, followed by at least one alphanumeric or `-._~+/=` byte |
| Private-key blocks | Standard PRIVATE KEY, RSA, EC, DSA, OPENSSH and ENCRYPTED PRIVATE KEY delimiters; suppress through matching end or EOF |
| Environment literals | Exact values of at least 12 Unicode characters whose server-startup names contain KEY, TOKEN, SECRET, PASSWORD or PASSWD, case-insensitively |

The server also registers its owner capability as a literal. There is no runtime
mask-pattern config key. Policy limits are 128 effective literals, each at most
65,536 bytes; invalid names/policy, non-UTF-8 eligible environment entries or
star-containing literals fail closed rather than expose plaintext.

Each recognized raw byte becomes one ASCII `*`. Matching continues across output
chunks; undecided suffixes are withheld until resolved or command completion.
Original raw byte offsets/retention coordinates are preserved: a page can return
no bytes while a suffix remains withheld. Use next_offset, not character counts.
Structured reads mask logical context before slicing; screen cursors retain
original cell coordinates, not indices into redacted UTF-8 strings. A partial
oldest retained logical line may be hidden conservatively. Overlapping matches
and unseparated token runs can hide more text than expected.

**Not caught automatically:** unknown formats, short/lookalike prefixes, secrets
created after server startup that match no built-in, changed environment values,
or values a program encodes/transforms before printing. Base64 *transport*
encoding is not a bypass: raw bytes are masked before encoding. This is not a
universal secret scanner or a sandbox; do not grant access on that assumption.
See [masking design](design-m5.md) and [recognizer rules](design-m6-masking.md).

## Request/response examples

Each pair uses an independent connection and a fresh, unrevoked server with a
valid masking policy. IDs/PIDs are illustrative. `t0` is a nonexistent terminal;
those examples deliberately show real not_found replies. V3 examples except
hello/auth deliberately omit owner authentication; the denial illustrates their
owner-only boundary, not a successful action. Supply real IDs and authenticate an
eligible owner for use. V4 owner-only examples likewise show agent denials.

### Version 1: terminals and commands

Request: `{"v":1,"id":"1","op":"hello"}` Response: `{"id":"1","ok":true,"result":{"version":1,"capabilities":["tabs","commands","blocks"]}}`
Request: `{"v":1,"id":"1","op":"open_tab","allow_agent":true}` Response: `{"id":"1","ok":true,"result":{"tab_id":"t1","allow_agent":true}}`
Request: `{"v":1,"id":"1","op":"list_tabs"}` Response: `{"id":"1","ok":true,"result":{"tabs":[]}}`
Request: `{"v":1,"id":"1","op":"run","tab_id":"t0","argv":["printf","hello"],"cwd":"/"}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":1,"id":"1","op":"wait","tab_id":"t0","command_id":"c1","timeout_ms":1000}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":1,"id":"1","op":"get_exit_code","tab_id":"t0","command_id":"c1"}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":1,"id":"1","op":"get_output","tab_id":"t0","command_id":"c1","offset":0,"limit":64,"encoding":"utf8"}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":1,"id":"1","op":"list_blocks","tab_id":"t0","limit":64}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":1,"id":"1","op":"close_tab","tab_id":"t0"}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`

run takes explicit nonempty argv and an existing absolute cwd, without implicit
shell interpolation. wait allows 0–30,000 ms and reports drained completion.
get_output uses raw byte offset, limit 1–65,536 and utf8/base64 encoding; retained
output is bounded to 8 MiB per command, with output_evicted on stale offsets.
list_blocks accepts optional command_id/after and up to 64 OSC 133 blocks.
Block `command` is captured between OSC 133 B and C from the same masked logical
screen context as the screen API, including wrapped rows. It is null when capture
or masking is unavailable, or the command exceeds 64 KiB. This applies even to owner block reads; the ordinary
terminal grid remains raw internally, but snapshots shown to the person are masked.
Masking has the coverage limits described above.

Blocks also carry nullable absolute-line anchors: `prompt_line` (A, or B without A),
`command_line` (C), and `end_line` (D). Lines count from the oldest retained row plus
the grid's monotonic eviction count. Evicted anchors become null. Resize, reset,
alternate-screen changes and non-preserving region scrolling invalidate old anchors;
new A/B markers establish new ones. Terminals without OSC 133 have no cards.
Owner v3 changed `snapshot` replies include `block_anchors: {viewport_line, blocks}`:
up to 64 visible-command-row block summaries, without output, captured atomically
with the grid. Summaries carry block_id, command, the three anchors, complete,
exit_code and duration_ms. Unchanged replies reuse that generation's metadata.
Cards subtract viewport_line from command_line/end_line; they never match text.
Closing a terminal terminates its processes; closing a client does not.

### Version 2: block actions and tasks

Request: `{"v":2,"id":"1","op":"hello"}` Response: `{"id":"1","ok":true,"result":{"version":2,"capabilities":["block_actions","tasks","palette"]}}`
Request: `{"v":2,"id":"1","op":"actions"}` Response: `{"id":"1","ok":true,"result":{"actions":["select","copy","collapse","rerun","send","task_update","view","tasks","task_get"]}}`
Request: `{"v":2,"id":"1","op":"view","tab_id":"t0"}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":2,"id":"1","op":"select","tab_id":"t0","block_id":"b1","start":0,"end":3}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":2,"id":"1","op":"copy","tab_id":"t0","block_id":"b1"}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":2,"id":"1","op":"collapse","tab_id":"t0","block_id":"b1","collapsed":true}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":2,"id":"1","op":"rerun","tab_id":"t0","block_id":"b1","argv":["echo","hello"],"cwd":"/"}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":2,"id":"1","op":"send","tab_id":"t0","block_id":"b1","label":"Review"}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":2,"id":"1","op":"task_update","tab_id":"t0","task_id":"task1","state":"done"}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":2,"id":"1","op":"tasks","tab_id":"t0"}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":2,"id":"1","op":"task_get","tab_id":"t0","task_id":"task1"}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`

view returns blocks/selection/task metadata, paged with after/limit. select uses
ordered UTF-8 byte boundaries, at most 64 KiB. copy requires that selection and
returns base64 bytes; collapse changes only presentation. rerun creates a new
terminal from explicit argv/cwd. send queues selected bytes, not an agent process.
Tasks have queued/running/done/failed/cancelled claimed states, separate from real
command status; task_get includes payload, tasks lists metadata. Per-terminal
limits include 128 tasks and 64 KiB per selected payload. See [block actions](design-m3.md).

### Version 3: owner-only operations

hello is public; auth attempts owner authentication. **Every other v3 operation
below is owner-only**, including focus, metadata, search and clipboard operations.

Request: `{"v":3,"id":"1","op":"hello"}` Response: `{"id":"1","ok":true,"result":{"version":3,"server_pid":1234,"capabilities":["sessions","owner"],"build":{"version":"1.5.0","executable":"64768:1234567"},"row_delta":true}}`
Request: `{"v":3,"id":"1","op":"auth","token":"invalid"}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication denied"}}`
Request: `{"v":3,"id":"1","op":"look_state"}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`

In the v3 hello result, `build.executable` is the device and inode of the server's
running executable (null if unreadable); a window running a different file warns
that its server is another build. `row_delta: true` says the server accepts
known_rows/known_cols on snapshot.

Socket `hello` accepts optional `client_name` and `model` strings (1–128 UTF-8
bytes, not whitespace-only, no control characters). Existing hello responses are
unchanged. These untrusted display labels are masked before storage and appear only
in the authenticated owner's look_state agent entries; they grant no authority.
A later hello replaces the labels; omitted labels clear them. Owners never appear
in the connected-agent list. The UI falls back to `agent <pid>` without a name.

Request: `{"v":3,"id":"1","op":"window_changes","after":0,"timeout_ms":1000}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`

Authenticated `window_changes` waits up to 0–1000 ms and returns
a result such as `{"revision":42,"changed":true}` (`changed:false` on timeout).
Pass the previous revision as `after`.
Notices contain no output bytes. Raw PTY updates notify only after the grid changes,
without waiting for the agent masking stream to release a prefix. Windows use a
separate authenticated connection so waiting cannot block input RPCs. Old servers
without this operation fall back to one-second refresh; update/restart the server
for immediate unsolicited-output updates.

For an authenticated owner, look_state returns uptime_ms, masking_ready, connected
agents, command drivers, the last 20 metadata-only events, human `recent` activity, and activity.
`recent` is a separate owner-only history of up to 20 command-run, command-finished,
agent-connected and pane-closed records; output/readiness notifications cannot evict
it. Its sequence numbers are not v4 event cursors. Command text is masked. Windows
add their own local file-open events and display pane names rather than protocol IDs. Each
look event includes `age_ms`, its monotonic age at the time of the query; the UI
shows relative seconds/minutes/hours rather than inventing wall-clock times. **activity
is exactly 30 unsigned integer command counts, oldest to newest**, for the chart
“Agent commands per minute · last 30 min”. Each bucket covers one 60-second
interval of monotonic server uptime; the final bucket is the current, partial
minute. Missing intervals (including before startup) are zero. Counts saturate at
u64::MAX and reset on server restart. A fixed-size rotating ring expires old
minutes, including minutes with no requests.

Count one successful agent v1 run or v2 rerun dispatch with a generated command
ID, regardless of eventual exit status or a later close/replacement. Owner-issued
commands, failed requests, output events, audit recency and UI polling do not
increment counts. Rerun driver metadata uses its new result tab_id/command_id;
the audit keeps the original source request unchanged. This metadata stores no
raw arguments or output. Existing look_state fields are unchanged.

Request: `{"v":3,"id":"1","op":"workspace_get"}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"shell_defaults"}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"workspace_set","revision":0,"groups":[],"active":0}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"snapshot","tab_id":"t0"}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"history","tab_id":"t0","offset":0,"limit":64}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"scroll","tab_id":"t0","delta":1}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"input","tab_id":"t0","data":"aGVsbG8="}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"resize","tab_id":"t0","rows":24,"cols":80}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"reset","tab_id":"t0","clear":false}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"finished_shells"}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"permission_get","tab_id":"t0"}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"permission_set","tab_id":"t0","allow_agent":true}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"agent_tool_set","tab_id":"t0","name":"Claude Code"}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"terminal_cwd","tab_id":"t0"}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"terminal_info","tab_id":"t0"}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"block_events","tab_id":"t0","after":null,"limit":64}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"focus_set","tab_id":null}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"window_search","tab_id":"t0","query":"hello","after":null,"backwards":false}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"clipboard_events","tab_id":"t0","write":true,"read":false}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"clipboard_reply","tab_id":"t0","request_id":1,"data":"d29ybGQ="}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`

| Operation | Successful use / result |
| --- | --- |
| workspace_get / workspace_set | Layout, focus, active group and revision; stale update revisions conflict |
| shell_defaults | Server argv, startup cwd and home (home may be null) |
| window_changes | Owner-only change revision; after/timeout_ms (0–1000); no terminal payload |
| snapshot | Owner-display-masked cells/cursor/images; unconfirmed prefixes remain visible; known_generation/known_images avoid retransfers; known_rows (one `clanky_core::row_hash` per held row) with known_cols, when both match the terminal's size, gets `delta: true` with empty `snapshot.cells` and `changed_rows` as `[row, cells]` pairs; block_anchors accompanies changed snapshots |
| history | Raw retained history with offset/limit (at most 64), total and next_offset |
| scroll | Signed delta; returns display_offset |
| input | Base64 bytes, at most 64 KiB per request; returns {} |
| resize | rows/cols in 1–1000, product at most 250,000; returns {} |
| finished_shells | `tab_ids` of terminals whose process, started by the owner, has exited and been read to the end; the window closes their panes. A process an agent started is never listed, so its result stays readable |
| reset | The window's Reset terminal: modes off, text kept; `clear: true` is Reset and clear (full reset, screen and history emptied); returns {} |
| permission_get / permission_set | Read/set allow_agent on one terminal |
| agent_tool_set | Name (1–256 bytes, no controls) of the agent tool the terminal was opened for; shown by the window as the pane title; returns {} |
| terminal_cwd | Absolute cwd or null; source osc7, proc or null; no guessed directory |
| focus_set | tab_id string or null; publish/clear this owner's focus claim, cleared on disconnect |
| terminal_info | Process cwd/exe and rows/cols; no process or unavailable data is an error |
| block_events | Completion metadata, including duration/focus at completion; after/limit paging, no terminal text |
| window_search | Literal query up to 256 bytes, optional after {row,col}/backwards; total, selected, snapshot and highlights; joins soft wraps |
| clipboard_events | Drain at most one bounded event according to write/read booleans; write data is base64 |
| clipboard_reply | Single-use request_id and base64 data, up to 64 KiB; bound to original command; returns {} |

OSC 52 writes are capped at 1 MiB; reads require explicit window opt-in. The
headless server does not invoke desktop clipboard helpers. Config and first-owner
handling are described in [configuration](config.md). block_events cursors are
block IDs; evicted cursors return not_found, not the v4 events gap format.

`terminal_info` also returns nullable `program`: the validated foreground process
invocation name (argv[0] basename), so invoking sh through a bash symlink displays
sh. Other arguments are never read into this field. It also returns nullable
`agent_tool`, the name set by `agent_tool_set` ([agent pane](agent-pane.md)). The existing resolved `exe`
path remains unchanged for callers that need it.

#### Optional cell colour provenance

Existing snapshot fields, including each cell's `ch`, `fg` and `bg`, remain unchanged.
Cells may additionally contain `color_source`; it is omitted when unavailable:

```json
{"ch":"A","fg":[229,229,229],"bg":[0,0,0],"color_source":{"fg":"truecolor","bg":"default","inverse":false}}
```

Each source is `"default"`, `{"indexed":n}` (integer 0–255), or `"truecolor"`.
The source pair describes the original SGR choices; `inverse` records whether
foreground/background were swapped. The existing RGB fields already include that
swap and retain their legacy values. New clients use provenance only at draw time
for theme palettes; truecolour and indexed colours 16–255 keep their RGB values.
Clients without provenance support can keep reading the original RGB fields.
RGB-only cells are never classified by matching their colour values.

### Version 4: agent control, events and owner safety controls

input_state, wait_input, screen, send_key and paste require terminal permission.
events is permission-filtered. **audit, revoke_all, resume_agents and post_ai are owner-only**
(after v3 authentication on the same connection). In Post-AI mode, agents are refused,
including at hello; owners authenticate before hello and keep access.

Request: `{"v":4,"id":"1","op":"hello"}` Response: `{"id":"1","ok":true,"result":{"version":4,"capabilities":["agent_control","events","masked_reads","audit","revoke"]}}`
Request: `{"v":4,"id":"1","op":"input_state","tab_id":"t0"}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":4,"id":"1","op":"wait_input","tab_id":"t0","timeout_ms":1000}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":4,"id":"1","op":"screen","tab_id":"t0","history_offset":0,"history_limit":64}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":4,"id":"1","op":"send_key","tab_id":"t0","key":"Enter"}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":4,"id":"1","op":"paste","tab_id":"t0","text":"hello"}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":4,"id":"1","op":"events","after":0,"limit":128,"timeout_ms":0}` Response: `{"id":"1","ok":true,"result":{"events":[],"next_cursor":0,"oldest_cursor":1,"gap":false}}`
Request: `{"v":4,"id":"1","op":"audit","tab_id":"t0","after":0,"limit":128}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"owner operation"}}`
Request: `{"v":4,"id":"1","op":"revoke_all"}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"owner operation"}}`
Request: `{"v":4,"id":"1","op":"resume_agents"}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"owner operation"}}`
Request: `{"v":4,"id":"1","op":"post_ai","enabled":true}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"owner operation"}}`

| Operation | Successful use / result |
| --- | --- |
| input_state | state running/waiting/exited, waiting boolean, evidence pty_read/prompt/none, command_id and bracketed_paste |
| wait_input | Same state plus timed_out; timeout_ms 0–30,000; revocation cancels the wait |
| screen | lines, cursor, rows, cols and history {total,offset,next_offset,lines}; optional history_offset and history_limit 1–64 |
| send_key | key: Enter, Ctrl-C, Up, Down, Left, Right, Escape, Tab, Backspace, Home, End, Delete, PageUp or PageDown; returns sent byte count |
| paste | UTF-8 text up to 64 KiB; bracketed only when terminal mode enables it; returns sent byte count |
| events | Required after cursor; optional tab_id, limit 1–128, timeout_ms 0–30,000; returns events, next_cursor, oldest_cursor, gap |
| audit | Required tab_id, optional after/limit 1–128; records, oldest_cursor, next_cursor, evicted |
| revoke_all | Revoke current agent grants, cancel waits and latch agent tab creation; returns revoked count and epoch |
| resume_agents | Clear creation latch only; returns resumed:true; refused while Post-AI mode is on |
| post_ai | Required boolean enabled; optional boolean force (default false); returns effective post_ai boolean |

An authenticated owner setting `enabled:true` receives `{"post_ai":true}`. Enabling
Post-AI revokes agent grants, cancels waits and disconnects agent sockets; shells and
owner connections stay alive. `force:true` forces the mode on until the server restarts,
even with `enabled:false`. Later `force:false` or config changes cannot clear that force.
Turning ordinary Post-AI off does not restore permissions or clear the revoke latch:
call `resume_agents`, then explicitly grant the needed tab permissions. See [Post-AI mode](post-ai.md).

Waiting is observed after a quiet period (200 ms), using a blocked foreground
PTY read or OSC prompt evidence; silence/sleep alone is not proof of readiness.
The background observer checks on command/input/output activity and one quiet-period
deadline, then parks. input_state and bounded wait_input calls refresh the evidence.
An events request probes at entry and timeout; a silent readiness transition without
PTY activity is detected on the next request/deadline, not by periodic idle scans.
The event ring retains 1,024 entries with ordered cursors and kinds such as
output_appended, command_finished, waiting_for_input and terminal_closed.
Output events carry base64 data and offset; replay rechecks permissions. A gap
means lost/invalid cursor history, never silently complete replay.

Audit is memory-only: 512 records per terminal, bounded history of at most 64
terminal rings plus an _connection ring. Records identify connection/peer ancestry,
Unix-millisecond timestamp, operation, masked arguments and result code. Arguments
are masked before the 8 KiB record limit; auth/capability fields are omitted.
Revoke stops API access, not the underlying command. Owners may explicitly regrant
individual terminals. These APIs are not authentication for a separate OS user.

## Agent-only MCP

Start `clanky mcp [--socket PATH]` as an MCP stdio server. It uses newline-delimited
JSON-RPC, protocol 2024-11-05, initialize/ping/tools/list/tools/call and initialization
notifications. Frames are bounded to 1 MiB. Initialize before listing/calling tools.
Each tool opens a fresh agent connection; it never loads an owner capability.
No owner, v2 block-action, resource or sampling tools are advertised.

The tools are open_tab, run, wait, get_output, list_blocks, list_tabs, close_tab,
input_state, wait_input, screen, send_key, paste and events. Arguments match their
socket counterparts (without v/id/op). Screen/input/event tools use v4; the
others use v1. tools/list exposes exact required fields, limits and key enums.

The following pairs assume MCP initialization has completed and use the same
fresh-server/t0 assumptions as above. Tool results wrap JSON as text content;
socket failures set isError:true and retain a stable error code without echoing
request-bearing diagnostics.

Request: `{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"open_tab","arguments":{"allow_agent":true}}}` Response: `{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"{\"allow_agent\":true,\"tab_id\":\"t1\"}"}],"isError":false}}`
Request: `{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"run","arguments":{"tab_id":"t0","argv":["printf","hello"],"cwd":"/"}}}` Response: `{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"{\"error\":{\"code\":\"not_found\",\"message\":\"Agent operation failed\"}}"}],"isError":true}}`
Request: `{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"wait","arguments":{"tab_id":"t0","command_id":"c1","timeout_ms":1000}}}` Response: `{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"{\"error\":{\"code\":\"not_found\",\"message\":\"Agent operation failed\"}}"}],"isError":true}}`
Request: `{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"get_output","arguments":{"tab_id":"t0","command_id":"c1","offset":0,"limit":64,"encoding":"utf8"}}}` Response: `{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"{\"error\":{\"code\":\"not_found\",\"message\":\"Agent operation failed\"}}"}],"isError":true}}`
Request: `{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_blocks","arguments":{"tab_id":"t0","limit":64}}}` Response: `{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"{\"error\":{\"code\":\"not_found\",\"message\":\"Agent operation failed\"}}"}],"isError":true}}`
Request: `{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_tabs","arguments":{}}}` Response: `{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"{\"tabs\":[]}"}],"isError":false}}`
Request: `{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"close_tab","arguments":{"tab_id":"t0"}}}` Response: `{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"{\"error\":{\"code\":\"not_found\",\"message\":\"Agent operation failed\"}}"}],"isError":true}}`
Request: `{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"input_state","arguments":{"tab_id":"t0"}}}` Response: `{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"{\"error\":{\"code\":\"not_found\",\"message\":\"Agent operation failed\"}}"}],"isError":true}}`
Request: `{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"wait_input","arguments":{"tab_id":"t0","timeout_ms":1000}}}` Response: `{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"{\"error\":{\"code\":\"not_found\",\"message\":\"Agent operation failed\"}}"}],"isError":true}}`
Request: `{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"screen","arguments":{"tab_id":"t0","history_offset":0,"history_limit":64}}}` Response: `{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"{\"error\":{\"code\":\"not_found\",\"message\":\"Agent operation failed\"}}"}],"isError":true}}`
Request: `{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"send_key","arguments":{"tab_id":"t0","key":"Enter"}}}` Response: `{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"{\"error\":{\"code\":\"not_found\",\"message\":\"Agent operation failed\"}}"}],"isError":true}}`
Request: `{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"paste","arguments":{"tab_id":"t0","text":"hello"}}}` Response: `{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"{\"error\":{\"code\":\"not_found\",\"message\":\"Agent operation failed\"}}"}],"isError":true}}`
Request: `{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"events","arguments":{"after":0,"limit":128,"timeout_ms":0}}}` Response: `{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"{\"events\":[],\"gap\":false,\"next_cursor\":0,\"oldest_cursor\":1}"}],"isError":false}}`

For wire details and safety decisions, see [M5/M6 design](design-m5.md).
