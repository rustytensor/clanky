# Getting started

Start `clanky` on Linux with a graphical display. Ctrl+Shift+T opens a tab;
Ctrl+Shift+D/E splits right/below. Click a pane to focus it. New terminals use
the focused terminal's folder, falling back to the server's home only when unknown.
See [shortcuts](shortcuts.md) and [configuration](config.md).

## The calm window

Sessions and files are in the left sidebar. Click New tab or Split in the top bar;
drag the gap between panes to resize them. The right column shows agent activity and access.
Click an access row, not just its switch, to toggle permission. Ctrl+Shift+B hides both side areas.
Open [Settings](settings.md) from the sidebar or with Ctrl+,.

[Clanky the robot](robot.md) greets you once for 7 seconds, then for 1.2 seconds on later windows.
Any key or click skips it and still acts normally. [Themes](themes.md) describe the built-in
palettes and user files; [Post-AI mode](post-ai.md) describes the plain-terminal option.

## Agents and safety

GUI terminals deny agents by default. Ctrl+Shift+A grants/revokes the focused
terminal; Ctrl+Shift+Escape revokes all agents and cancels pending agent waits,
even in a prompt. It does not kill running commands. Agent output reads are
masked, as are your terminal view, block copies, scrollback search and OSC 52 text.
Owner raw-output archives remain available through the socket.
[Read the limits](agent-api.md#masking-and-its-limits)
before allowing access to sensitive sessions. `clanky mcp [--socket PATH]` is an
agent-only MCP server; the [socket API](agent-api.md) also supports direct clients.

## Blocks, images and links

Use a shell that emits OSC 133 markers to get command cards inside the panes.
Hover a card, or focus it with Ctrl+Shift+F6, to show Copy, Rerun and Ask agent. Copy gives the
masked text, including stars. Rerun opens a new terminal; Ask agent queues selected
text as a task, not a new agent process. The palette also offers block selection
and collapse. Large/truncated blocks need an explicit API selection.
Ctrl+Shift+P or the search field opens the palette.

Kitty direct-transfer PNG/RGB/RGBA images are clipped to panes. File/shared-memory
transport, cropping, animation and z-order are not supported. See [image details](design-m2.md).
Ctrl+Shift+F searches scrollback, including soft wraps; Enter/Shift+Enter navigates.
Hover underlines a complete visible-row HTTP/HTTPS URL; only Ctrl+click opens it.
OSC 52 clipboard writes are enabled by default; reads require an explicit config opt-in.

## Edit and review

Click `path:line[:col]`, choose Open file, or press Ctrl+Shift+O. Relative paths
use that terminal's directory; an unknown directory is an error, not a guess.
Remote terminal file clicks are refused. Editor panes support selection, find,
undo/redo and Ctrl+S save; file changes on disk are detected before overwriting.
Load changes imports proposal JSON with an exact `base` and `edits` containing
`start`, `end`, `text`. Accept/reject each hunk or all pending hunks; accepting
changes memory only until save. Stale hunks cannot apply. See [editor API](editor-panel-api.md).

For a terminal without a display server, run:

```sh
clanky edit src/main.rs:20:3
clanky edit src/main.rs --review proposal.json
```

The text-mode editor uses the same file/edit/review core. Its review JSON format
is `{"hunks":[{"start":0,"end":3,"replacement":"new"}]}`: UTF-8 byte ranges in
the opened file. This differs from the GUI import format. Ctrl+S saves; Ctrl+Q
quits with confirmation for unsaved changes. F1 lists its controls.

## Compose prompts

Focus a terminal and press Ctrl+Shift+Enter or choose Compose. Enter adds a line;
Ctrl+Enter sends one bracketed paste to the captured terminal, with no extra Enter
unless configured. Drafts are limited to **256 KiB UTF-8**; the window sends large
pastes in ordered 64 KiB pieces. A failed send keeps the draft but may have delivered
a prefix: inspect the target before retrying. Escape and successful send retain
the draft for that terminal. Closing the window can lose these memory-only drafts.

Misspellings are underlined. F7 requests suggestions, arrows select, Enter replaces;
Ctrl+Alt+A adds a personal word. Paths, flags, code identifiers, URLs and backtick
spans are excluded. Dictionaries come from the system's Hunspell directory.

## Sessions, restoration and notifications

Closing a window leaves the server and shells running; reopen to reattach.
Explicitly closing the last terminal ends its server. Running sessions and history
do not survive server loss or reboot. With `restore_layout` enabled, a fresh local
server can create new shells in saved folders and restore splits/focus/sizes;
it never reruns saved commands or restores old output. An existing workspace wins.
`clanky attach HOST` uses key-based SSH and requires Clanky on the remote host.

`notify_after_seconds` defaults to 10; 0 disables notices. Completed OSC 133 blocks
at or above that duration notify only when their terminal was unfocused at the
completion marker. Reattaching does not replay old notices. Notifications contain
exit status only, not command text or output. See [Linux packaging](release-linux.md).
