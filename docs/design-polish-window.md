# Window polish gate

Command: `scripts/gate-polish-window.sh`. Generated private fixtures only, under
CARGO_TARGET_DIR. Offline builds under the shared lock, eight jobs maximum.
Software defaults come from `scripts/gate-render-env.sh`. No new dependencies.

## Fixed expectations before implementation

1. Wrapped scrollback: feed `abcHELLOthere\r\nabcHELLOthere\r\n` through the real
   terminal parser in an 8-column, 2-row terminal. Search `LOth`: exactly two
   matches, starting at retained physical (row,col) (0,6) and (2,6). Next selects
   the first, then second; previous selects first. The selected viewport shows
   `abcHELLO` / `there   `, highlights cells (0,6),(0,7),(1,0),(1,1), and no
   padding cells. `LO\r\nth` across a hard line is not the same match. A real
   headless server exposes the same owner-only search; an agent is denied.
2. Ctrl+click the visible `https://example.invalid/a?q=one`: a fixture xdg-open
   records exactly one argument, that complete URL, once. Plain click records
   nothing and does not try opening a file. `http://example.invalid/` is allowed;
   file:, javascript:, empty hosts and control characters are refused. Test a URL
   containing literal shell metacharacters as an argument, never executed code.
   Hover returns the exact URL cell span; software-rendered underline pixels
   use RGB (80,160,255). No real browser or network request in the gate.
3. Start a real shell in fixture home, `cd` into fixture work and wait alive.
   The shared new-terminal factory, used by both tab and split actions, starts
   both new shells in work, confirmed by terminal_cwd. With an idle source tab
   whose cwd is null, it starts in the server's fixture HOME. No GUI-process cwd
   is substituted, including on SSH. The server advertises home additively in
   shell_defaults; existing cwd/argv fields retain their meaning.
4. Real OSC 52 parser: `ESC]52;c;aGVsbG8= BEL` yields one clipboard write of
   exactly `hello`; splitting the sequence across chunks gives the same event.
   A write of exactly 1,048,576 generated ASCII bytes is accepted; 1,048,577 is
   refused entirely, not truncated into an accepted prefix. Malformed base64 is
   refused. With osc52_write=false no desktop write callback runs. Read query
   `ESC]52;c;? BEL` invokes no read callback with default osc52_read=false.
   With explicit read permission and fixture clipboard `world`, the program
   receives exactly `ESC]52;c;d29ybGQ= BEL` through its PTY. Test helpers are fake;
   never inspect or change the person's actual desktop clipboard.
5. The clipboard event/reply operations are owner-only, bounded, and replies
   cannot be delivered to a replacement command in the same tab. Raw clipboard
   data never appears in diagnostics. Selected search cells use background
   (120,85,25), other visible matches (65,60,30), sampled at blank/background
   pixels or via the actual snapshot before rendering. Existing text stays exact.
6. `CLANKY_GATE_FAULT=wrapped-search scripts/gate-polish-window.sh` changes the
   observed first match count to zero before comparison: FAIL at wrapped search.
   Fault once, then clean once. Clean calls M8 once (which already includes M7
   panel/core, M6 masking, cwd and M1–M4), not each previous gate again.

## Behavior

Ctrl+Shift+F opens a terminal search prompt. Enter/Shift+Enter select next/previous;
Escape closes. Search joins only rows marked as soft-wrapped by the existing VT
engine, not arbitrary newlines. It searches retained history and screen. Search
results are a stable text view refreshed when the query or direction changes, so
new output cannot silently move the displayed match. No new VT parser or font
shaper. Blank query has no match. Text search is literal, case-sensitive UTF-8.
The query is limited to 256 bytes; oversized requests fail visibly.

HTTP/HTTPS URLs are recognized in visible terminal rows and underlined on hover.
Only Ctrl+click launches xdg-open, as one argv entry, without a shell. Plain URL
clicks are inert. File-path clicks keep their existing editor behavior. Wrapped
scrollback searching is supported; URL recognition is limited to a complete URL
visible in one physical row in this milestone. Helpers are owned and reaped;
shutdown never kills an unrelated browser. No new package installation.

New tabs and terminal splits query the displayed source terminal before creation.
If terminal_cwd is null, use the server's home directory; if unavailable, fail
rather than guess. A reported but unusable directory is an error, not a silent
fallback. Existing editor splits remain window-local and unchanged.

## OSC 52 and ownership

Use the existing terminal engine's ClipboardStore/ClipboardLoad events. Writes
are enabled by default and bounded to 1 MiB decoded bytes; `osc52_write=false`
disables them. Reads are disabled by default; `osc52_read=true` explicitly allows
bounded reads of at most 64 KiB so base64 replies fit existing input/RPC budgets.
Oversized reads fail rather than return truncated clipboard contents. No clipboard
helper runs in the headless server. Owner windows drain bounded events and apply
local config. With multiple windows, the first owner to drain an event handles it.

Owner-only v3 clipboard_events takes tab_id plus write/read policy booleans and
returns bounded events; clipboard_reply supplies a pending request ID and base64
bytes. Read formatter callbacks stay server-side and are single-use. Requests
are bound to the originating command; old requests cannot inject into a new shell.
No raw value in error strings or Debug output. Clipboard helper reads have a
bounded deadline; GUI polling uses a background bridge to avoid waiting on helpers.

The existing clipboard helper wrapper accepts up to 1 MiB and uses anonymous
memory-backed input rather than writing clipboard bytes to a temporary disk file.
The existing VT parser is bounded for OSC payloads and discards overflow through
its terminator. Any fork changes receive the required notice and FORK.md entry.
Do not implement another escape parser.

## Delivery and limits

Software gate defaults were committed separately as v0.4.4-polish.1. Commit this
design next, then implement under the approved Linux-RC autopilot. Five bounded
workers may own search, clipboard, URL/folder helpers, window integration and gate.
The lead reads each changed line, integrates, scans, commits/tags/pushes, reports
and stops at this gate. Existing owner auth and v1/v2 behavior remain unchanged.
No schema migration, new dependency, licence change or production action.

Manual acceptance remains actual desktop clipboard ownership, browser launch,
keyboard/hover/resize interaction and real SSH homes. The automated gate uses real
PTYs and the same pure controllers with controlled helper executables, not those
external desktop services. M5 is on the other team's branch and is not run here.
