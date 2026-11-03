# M1 design: one window, one tab, an agent can drive it

Status: design approved with the revisions below. Dependency downloads and
implementation require separate approval. This document adds neither.
M1 covers Linux, a single visible terminal, and independently addressable API tabs.
Tab bars, splits, config files and the graphical command-block UI come later.

## Crate boundaries

| Crate | Owns | Depends on |
| --- | --- | --- |
| `clanky-core` | PTY lifecycle, terminal state, tabs, process results, OSC 133 blocks, decoded terminal events | No window or GPU code |
| `clanky-render` | wgpu device, cosmic-text shaping, glyph atlas, Kitty image decoding/storage, shared window/offscreen drawing | `clanky-core` snapshot types |
| `clanky-api` | Unix listener, JSON-lines validation, permission checks, request dispatch | `clanky-core` |
| `clanky` | CLI, headless service, winit window, keyboard/resize, renderer integration, shutdown | All three libraries |

Core exposes typed operations corresponding to the API below, plus immutable
`TerminalSnapshot` values (cells, colors, cursor, dimensions, image placements).
The API does not touch renderer state. The app routes bounded Kitty events from
core to render and returns protocol replies to the PTY. Headless mode creates no
window or graphics device; the separate offscreen gate harness creates a device.

One core owner serializes tab mutations. PTY readers, child reapers and socket
workers communicate through bounded queues; blocking reads and waits never hold
the terminal-state lock. A completed command means both child status is known
and its PTY output has been drained. Linux PTY `EIO` after slave closure counts as
EOF. A descendant holding the slave open keeps the command draining; `wait`
can time out without returning incomplete output as a completed result.

Use alacritty_terminal's `tty` module for PTY allocation, child spawning,
controlling-terminal/session setup, PTY read/write handles, resizing and child
exit notifications wherever its public API provides them. Adapt those handles
to the core's reader/reaper threads; do not duplicate its spawn or PTY machinery.
Use libc only for missing Unix operations: process-group signaling/cleanup
(and group setup only if not supplied by `tty`) and socket peer credentials
(`SO_PEERCRED`). Verify the exact `tty` API after download approval; if it cannot
support a required lifecycle operation, report the gap before changing this split.

Each `run` starts a fresh process group on a fresh PTY, not pasted shell input.
Only one command may run per tab. Interactive shells are ordinary long-lived
runs. Closing a tab or the app signals its process group, closes the PTY and
reaps children; escalate to SIGKILL after a bounded grace period. The app owns
cleanup, including on gate failures. No detached service remains after the gate.

## Terminal and command output

Use alacritty_terminal for terminal state and its upstream parser interfaces for
VT decoding. Observe OSC 133 and Kitty APC through the upstream parser's handler
hooks while forwarding normal terminal handling. Do not implement a second VT
parser. Before coding, verify the selected upstream version exposes those hooks;
a missing hook is a design blocker, not permission to add a private parser.

Raw command output is the exact PTY byte stream, with stdout/stderr merged. It
includes escape sequences and terminal line endings; it is not screen scraping.
Text API output decodes UTF-8 with replacement for invalid sequences; base64
output preserves all bytes. Retain at most 8 MiB per command and report retained
range and truncation explicitly. Commands remain queryable until tab closure.

OSC 133 handling follows A (prompt), B (input), C (output begins), D;status
(output ends). Assign a block at C; seal it at D. A/B/prompt/input text never
enters block output. Nested C seals the preceding block as incomplete; EOF does
likewise. D without C is ignored. A complete block's exit code comes from D,
not the shell process exit. `command` is null in M1: typed command reconstruction
is not promised by OSC 133 alone.

Block `output` is plain text from printable/control events between C and D:
strip escape controls, normalize CRLF to LF, preserve internal newlines, and
remove exactly one final LF. Thus `echo one` yields `one`, while raw command
output still contains its real line ending. This is a transcript, not a promise
to reconstruct arbitrary full-screen applications. Cap retained block text at
1 MiB and retained blocks at 1,024 per tab; expose truncation/eviction.

## Socket protocol v1

Start with `clanky --headless --socket <path>`; GUI mode can use the same option.
Default: `$XDG_RUNTIME_DIR/clanky/<instance>.sock`, in an owner-only directory.
Require a private directory, bind with mode 0600, never unlink an existing socket
blindly, and check peer UID on Linux. No TCP listener. Remove only the socket
this process created. Socket access permits command execution as the app user.

UTF-8 JSON, one object per line, at most 1 MiB per line. Requests carry `v:1`, a
connection-unique string `id`, and `op`. Responses echo `id` and contain either
`ok:true,result` or `ok:false,error`. Unknown fields/operations or malformed
values are errors. Process requests in order per connection; other connections
remain usable during a wait. No subscriptions or unsolicited messages in M1.
A disconnected client does not cancel its command. Retry `run` only after
checking state: request IDs do not provide durable execution deduplication.

IDs below are examples, opaque strings allocated by the server. All dimensions
and offsets are nonnegative integers; rows/columns must be positive and at most
1,000 each, with at most 250,000 cells. argv must be nonempty, NUL-free and bounded
by the request limit. `cwd` must be an existing absolute directory. Commands
inherit the app environment, with terminal variables set for the PTY; v1 has no
environment override or shell interpolation. Use explicit `sh -c` when wanted.

### Every request and response

**`hello`** — protocol version and server readiness (also used by the gate).

```json
{"v":1,"id":"1","op":"hello"}
{"id":"1","ok":true,"result":{"version":1,"capabilities":["tabs","commands","blocks"]}}
```

**`open_tab`** — create an empty tab, default 80x24 and `allow_agent:false`.
An API-created tab may explicitly opt in. GUI-created tabs remain denied until
the person grants access locally; the API cannot grant access to existing tabs.

```json
{"v":1,"id":"2","op":"open_tab","rows":24,"cols":80,"allow_agent":true}
{"id":"2","ok":true,"result":{"tab_id":"t1","allow_agent":true}}
```

**`run`** — spawn once, return immediately after successful spawn. Reject a busy
or unauthorized tab. A spawn failure creates no command result.

```json
{"v":1,"id":"3","op":"run","tab_id":"t1","argv":["sh","-c","printf hello; exit 3"],"cwd":"/tmp"}
{"id":"3","ok":true,"result":{"command_id":"c1","state":"running"}}
```

**`wait`** — wait for fully drained completion; `timeout_ms` is required, 0–30000.
Timeout is a normal `done:false` response, not cancellation. Signal termination
returns `exit_code:null` and its numeric signal, never a fabricated exit code.

```json
{"v":1,"id":"4","op":"wait","tab_id":"t1","command_id":"c1","timeout_ms":5000}
{"id":"4","ok":true,"result":{"done":true,"state":"exited","exit_code":3,"signal":null}}
```

A timed-out response is `{"id":"4","ok":true,"result":{"done":false,"state":"running","exit_code":null,"signal":null}}`;
`state` may also be `draining` when the child has exited but output remains open.

**`get_exit_code`** — immediate status query, including after reconnecting.

```json
{"v":1,"id":"5","op":"get_exit_code","tab_id":"t1","command_id":"c1"}
{"id":"5","ok":true,"result":{"done":true,"state":"exited","exit_code":3,"signal":null}}
```

The status fields have exactly the same meanings as `wait`.

**`get_output`** — byte-offset pagination of the retained raw PTY stream.
`limit` is 1–65536 bytes; `encoding` is `utf8` or `base64`. Offsets always count
original bytes, even when UTF-8 replacement changes returned string length.
Clients requiring lossless streaming should use base64. An evicted offset
returns `output_evicted` with the earliest available offset, never silent skipping.
`eof` means the command is complete and `next_offset` reaches the final byte.

```json
{"v":1,"id":"6","op":"get_output","tab_id":"t1","command_id":"c1","offset":0,"limit":65536,"encoding":"utf8"}
{"id":"6","ok":true,"result":{"encoding":"utf8","data":"hello","next_offset":5,"retained_start":0,"total_bytes":5,"truncated":false,"eof":true}}
```

For `encoding:"base64"`, this example returns `data:"aGVsbG8="` and the same
byte offsets. Reads while running return currently available bytes with `eof:false`.

**`list_blocks`** — per-tab, optionally restricted to a command, in creation order.
`after` is an exclusive block ID or null; `limit` is 1–64. Page by the returned
`next_after`; `has_more` describes the current snapshot. `evicted` counts blocks
removed from this tab. Incomplete blocks have `complete:false,exit_code:null`.

```json
{"v":1,"id":"7","op":"list_blocks","tab_id":"t2","command_id":"c2","after":null,"limit":64}
{"id":"7","ok":true,"result":{"blocks":[{"block_id":"b1","command_id":"c2","command":null,"output":"one","complete":true,"exit_code":0,"truncated":false},{"block_id":"b2","command_id":"c2","command":null,"output":"two","complete":true,"exit_code":0,"truncated":false}],"next_after":"b2","has_more":false,"evicted":0}}
```

Apply a 256 KiB aggregate text budget per page. A single larger block returns
its text prefix with `truncated:true`; pagination advances by block, not text.

**`close_tab`** — terminate any active process group and release retained state.
Respond only after cleanup; closing an unknown tab returns `not_found`.

```json
{"v":1,"id":"8","op":"close_tab","tab_id":"t1"}
{"id":"8","ok":true,"result":{"closed":true}}
```

All operations referencing an existing tab require its local agent permission,
including reads and close. There is no API operation to bypass that restriction.
Errors use codes `invalid_request`, `unsupported_version`, `unknown_operation`,
`permission_denied`, `not_found`, `busy`, `spawn_failed`, `output_evicted`,
`resource_limit`, or `internal`; human messages are not stable API values.

```json
{"id":"3","ok":false,"error":{"code":"permission_denied","message":"Agent access is disabled for this tab"}}
```

An unparseable request uses `id:null`; an oversized line gets `invalid_request`
and the connection closes. Cap concurrent connections at 32 and tabs at 64.

## Rendering and Kitty images

Window and offscreen rendering use the same snapshot-to-draw path. cosmic-text
shapes glyphs into a cached atlas; wgpu draws backgrounds, glyphs, images and
cursor. Offscreen uses an RGBA8Unorm target, opaque black clear, no sRGB conversion,
nearest image sampling, cell-sized image scaling, and no image antialiasing. Request a
normal adapter first, then a fallback/software adapter. No adapter is FAIL, not
SKIP. Readback respects wgpu row alignment and removes padding before sampling.

The text fixture uses fixed 16x32-pixel cells, an 80x24 grid, cursor hidden and a
bundled DejaVu Sans Mono. After the separate dependency/download go, copy
`/usr/share/fonts/dejavu-sans-mono-fonts/DejaVuSansMono.ttf` to
`assets/fonts/DejaVuSansMono.ttf` and
`/usr/share/licenses/dejavu-sans-mono-fonts/LICENSE` to `assets/fonts/LICENSE`.
Preserve the complete licence file. This is the approved font-specific exception
to the crate licence list: Bitstream Vera terms, public-domain DejaVu changes,
and the accompanying notices, not OFL. Do not copy either file in this step.
Load the bundled font rather than depending on runtime host fonts; verify its
U+2588 glyph during implementation. Use the real glyph path, not a special red
rectangle for the gate.

M1 Kitty scope: direct base64 transmission of RGB, RGBA or PNG, chunk assembly,
transmit-and-display, explicit image/placement IDs, cell-sized placement and
deletion. Honor reply suppression and send protocol success/error replies.
Reject unsupported filesystem/shared-memory transport without opening paths.
Limit encoded payload to 16 MiB, dimensions to 4096 per axis and decoded image
storage to 64 MiB across the app; reject before oversized allocation. Kitty
parsing/decoding must handle arbitrary PTY chunk boundaries. The gate uses a
single PNG transfer; manual `kitten icat` checks establish actual compatibility,
not a claim that every Kitty extension works.

## Gate: inputs and predeclared results

There are five checks, numbered as in AGENTS.md: four functional checks and
one planted-fault check. Command: `scripts/gate-m1.sh`. A clean run exits 0 with
final `PASS` only if checks 1–4 succeed; otherwise it exits nonzero with final
`FAIL` and the failing assertion. Check 5 is a separate fault-injected run and
must exit nonzero with `FAIL`. Use
Python standard-library orchestration and a Rust offscreen harness. No downloads
inside the gate. Run against the built app; missing binaries are failures.
Use a private temporary directory, bounded readiness/wait deadlines (5 seconds
for socket readiness, 10 seconds per PTY case, 30 seconds for rendering), and
trap cleanup by saved process IDs. A timeout is FAIL. The offscreen harness
accepts `--case text` and `--case image`, returns sampled RGBA values, and exits
nonzero on device/render/readback errors. Python compares these against the
constants below; the renderer never supplies its own expected values.

1. **Socket and process result.** Start headless, poll `hello` until ready, open
   an authorized tab, and run `["sh","-c","printf hello; exit 3"]`. Wait for
   drained completion. Require exit code 3, signal null, raw output exactly the
   five bytes `hello`, no truncation, and EOF. This exercises the public socket,
   actual process status and PTY, not a fake core backend.
2. **OSC 133 blocks.** Run a real `sh` with this script through `run`:

   ```sh
   printf '\033]133;A\007\033]133;B\007\033]133;C\007'
   echo one
   printf '\033]133;D;0\007\033]133;A\007\033]133;B\007\033]133;C\007'
   echo two
   printf '\033]133;D;0\007'
   ```

   Wait for completion and query `list_blocks`. Require exactly two complete,
   untruncated blocks, outputs `one` and `two`, exit codes 0, no evictions.
   This controlled marker-emitting shell avoids host shell startup files.
3. **Text pixels.** Feed `ESC[31m` followed by UTF-8 `█` through core into an
   otherwise black 80x24 terminal. Use a fixed palette whose red is (255,0,0).
   Draw through the shared renderer. Sample pixel (8,16), expected (255,0,0),
   and (24,16), expected (0,0,0); alpha must be 255. Each RGB channel may differ
   by at most 8. Coordinates are zero-based with a top-left origin.
4. **Image pixels.** Generate a 2x2 PNG with opaque red/green on the top row and
   blue/white below, using Python's struct/zlib/base64 modules. Feed cursor move
   `ESC[3;3H` then `ESC_Ga=T,f=100,t=d,i=1,c=2,r=2,q=2;<base64>ESC\` through
   the same core/parser and Kitty event path used by PTY input. The placement
   begins at zero-based cell (2,2), covers two columns and two rows. Sample
   pixels (40,80), (56,80), (40,112), (56,112): respectively (255,0,0),
   (0,255,0), (0,0,255), (255,255,255), alpha 255, RGB tolerance 8.

5. **Planted fault.** Run `CLANKY_GATE_FAULT=exitcode scripts/gate-m1.sh`. The Python
   harness changes the observed exit-code value from 3 to 0 immediately before
   the normal comparison. It must reach and fail that assertion, reporting
   expected 3 / observed 0 and printing `FAIL`; an unrelated crash does not
   demonstrate the fault was caught. The application and expected value are
   unchanged. After implementation, run this once, then run once without the
   variable and require `PASS` for checks 1–4.

The automated gate does not establish window/input correctness. Manual acceptance
also requires opening a window, running `vttest` menus 1 and 2, and displaying a
picture with `kitten icat`. Report these separately if not performed.

## Dependencies and implementation handoff

Use std threads/channels and wgpu's own polling rather than adding an async
runtime. Keep fresh PTYs, limits, error codes and the permission model as specified
above. No dependency resolution is part of this documentation step. Verify the
selected upstream parser/tty hooks and version compatibility after download
approval; missing hooks remain a design blocker. Check direct and transitive
crate licences against MIT, Apache-2.0, BSD, Zlib or ISC before acceptance; the
approved DejaVu font exception is separate from crate licensing.

After approval, three bounded tasks implement core, render, and API/gate against
these contracts. Core supplies shared snapshot/event types; render owns the
Kitty image consumer and offscreen harness; API owns transport and gate script.
The integration lead owns the app, manifests, window/headless wiring and review
of every changed line. Workers check only their own crate with two build jobs;
whole-tree builds are serialized and use at most eight jobs. No worker commits.

This step ends with the revised document committed and pushed. No build,
executable gate, worker launch, font copy, dependency addition, download or
implementation is authorized yet.

## Exact direct dependency request

These exact pins use the newest stable releases reported in the design review
from a crates.io check on 2026-11-03. They are not a resolved or tested dependency
set. Compatibility and published licence metadata remain unverified in this
no-download step. The licences below are the expected upstream declarations and
must be confirmed before use. Do not silently substitute versions if verification
finds a mismatch.

| Crate | Exact version | Expected licence | Why |
| --- | --- | --- | --- |
| `alacritty_terminal` | `=0.26.0` | Apache-2.0 | Terminal state, upstream VT handling and `tty` PTY/spawn lifecycle |
| `wgpu` | `=30.0.1` | MIT OR Apache-2.0 | Shared GPU window/offscreen renderer and readback |
| `cosmic-text` | `=0.19.0` | MIT OR Apache-2.0 | Font loading, shaping and glyph rasterization |
| `winit` | `=0.30.13` | Apache-2.0 | Linux window, keyboard input and resize events |
| `serde` | `=1.0.229` | MIT OR Apache-2.0 | Typed protocol serialization; enable `derive` |
| `serde_json` | `=1.0.154` | MIT OR Apache-2.0 | JSON-lines encoding and decoding |
| `libc` | `=0.2.191` | MIT OR Apache-2.0 | Process-group operations absent from `tty`, and Unix peer credentials |
| `png` | `=0.18.1` | MIT OR Apache-2.0 | Decode Kitty PNG payloads |
| `base64` | `=0.23.1` | MIT OR Apache-2.0 | Kitty transfers and lossless API byte output |

Use alacritty_terminal's re-exported parser interfaces rather than requesting a
separate direct `vte` dependency. If those interfaces are insufficient, stop and
report instead of adding a parser dependency or writing a replacement.

This is the complete requested **direct** crate set, not the complete download
inventory: Cargo will also resolve transitive dependencies (including derive
macros, platform support and rasterization libraries). Their exact versions and
licences cannot be represented as verified without resolution. After permission,
record the resolved closure in Cargo.lock and review it before building; any
additional direct dependency or disallowed licence needs separate approval.
No async runtime, PTY wrapper crate or downloaded font is requested.
