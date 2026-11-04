# M7 editor panes

## Gate first

Command: `scripts/gate-m7-panel.sh`. Build offline under the shared build lock,
maximum eight jobs. Build/cache paths are CARGO_TARGET_DIR/CARGO_HOME; the lock
defaults to `/srv/devops/build/clanky/.build.lock`. Fixtures live in a private
scratch directory under the build tree and are removed. No existing file is edited.
The gate uses the same editor controller, hit testing and cell painting as the GUI.

1. A real shell changes directory into the fixture's `work` directory and prints
   `src/demo.txt:2:2` into a real terminal. Obtain that terminal's current directory
   via owner v3 terminal_cwd, click within the printed path using the shared click
   resolver and open the editor. The generated file's exact initial UTF-8 bytes
   are `alpha\r\nβeta\nlast\n`; cursor is byte 9, after β. Inserting `X` yields
   `alpha\r\nβXeta\nlast\n`. Undo restores the initial text and cursor. Redo then
   undo again leaves the initial text. Propose 7..12 -> `BETA` and 13..17 -> `LAST`;
   accept the first, reject the second, save. Exact disk bytes:
   `alpha\r\nBETA\nlast\n`. Pending hunks changed neither disk nor text before
   acceptance. Ctrl+S goes through this same save action, never an in-place write.
2. Before accepting, draw a review card for the first hunk. It displays old `βeta`
   and new `BETA`, with old background RGB (80,24,24), new (24,80,40). Render this
   actual snapshot offscreen through clanky-render and sample a blank cell in
   each review row: within 8 per RGB channel of those constants. Selection uses
   (40,70,120); a selected blank cell must sample that colour. The ordinary text
   background is (24,24,28). No GPU-specific pixel expectation outside these
   solid backgrounds; use the existing software fallback, not desktop GPU work.
3. Find `last` from the beginning selects bytes 13..17 in the initial file. Mouse
   select β by its cell boundaries and type `B`: `alpha\r\nBeta\nlast\n`. Undo
   restores the initial text. Backspace at byte 9 deletes the whole β, not half
   its UTF-8 bytes; undo restores it. Cursor movement and selection use Unicode
   scalar boundaries. CRLF is one line ending; typing Enter preserves the file's
   first observed newline convention. Input handled by the editor is never sent
   to the PTY, including IME commits and find/open prompts.
4. External overwrite of an open file makes its pending hunks stale and save
   fails without overwriting external bytes. Accept-all skips nonpending hunks
   and never applies stale ones; reject-all rejects pending only. A separate
   three-hunk file `a\nb\nc\n` with replacements `A`, `B`, `C` ends as
   `A\nB\nC\n` after accept-all and save. Reject-all leaves original bytes.
5. Create `let x = 1;\n` repeated 1,525,202 times. Scroll near EOF and draw a
   40-column, 20-row editor pane. The reported number of source lines painted
   never exceeds the pane's visible text rows; the last source line displays
   `let x = 1;`. Cached line offsets avoid rescanning the whole file every frame.
   This is a bounded rendering check, not a latency claim or a benchmark.
6. A relative clicked path with no cwd fails with exactly `unknown directory`;
   no file is opened. An absolute clicked path needs no cwd. Clicking text with
   no valid path:line suffix does not open anything. Editor and terminal panes
   use the existing Layout split geometry; editor IDs never enter server workspace
   messages. Closing a dirty editor refuses silently discarding its contents.
7. `CLANKY_GATE_FAULT=editor-save scripts/gate-m7-panel.sh` alters the observed
   saved bytes to `WRONG` immediately before item 1 comparison. Require FAIL at
   that comparison, not a crash. Run once, then clean once. Clean invokes the
   unchanged M4 gate once (which includes M3, M2 and M1); do not invoke them twice.

Manual acceptance remains real desktop keyboard/IME, drag selection, scrolling,
palette entry, split resizing and zoom. Offscreen success is not a claim those
human interactions were manually exercised.

## Approved terminal cwd addition (separate commit)

Owner-only v3 `terminal_cwd {tab_id}` returns
`{cwd: absolute path or null, source: "osc7" | "proc" | null}`. No new version,
capability change or restructuring. Existing owner authentication gates the op;
v1/v2 or unauthenticated calls cannot use it. No new third-party dependency.

Prefer the terminal's last valid OSC 7 `file://host/path`. The host must identify
this machine (its hostname, localhost or an empty local file-URL authority).
Percent-decode the absolute path; reject bad escapes, NULs, invalid UTF-8,
credentials/ports and nonlocal authorities. Observe OSC 7 in the existing vte
observer, not a new escape parser. Invalid reports do not replace a valid one.
Otherwise read `/proc/<foreground process-group leader>/cwd`, then the spawned
shell's cwd. Keep platform calls in the Linux module; check process identity to
avoid PID reuse and return null on unavailability, never a startup/default guess.
Do not hold terminal locks during filesystem/proc IO or join while holding them.

Narrow gate `scripts/gate-terminal-cwd.sh`: real shell `cd` from fixture root to
its child `work`, then wait for a marker and query. Expected absolute `work`,
source proc. Send a valid OSC 7 for another fixture directory: expected that path,
source osc7 even while proc still reports work. A foreign-host report and malformed
percent escape leave the last valid report unchanged. A new idle terminal returns
both null; an agent connection is denied. The shell stays alive for the queries.
This gate is part of the panel milestone; run it once for its separate commit,
then do not repeat it in the final panel gate. The final gate exercises real cd
again only as the input source for the actual file-click workflow.

## Window-local editor layout and controller

Reuse clanky-edit for all file bytes, selections, edits, history and review state.
Add a shared `clanky::editor` controller used by the GUI and offscreen gate.
No additional text engine, shaper or parser. It paints only visible source lines
into TerminalSnapshot cells and uses the existing renderer, font and zoom metrics.
Maintain a line-start index after opening/editing; draw never splits/scans the
whole file. Horizontal scrolling and tab expansion are presentation only.

Keep editor splits window-local, alongside the unchanged server terminal layout.
A local Layout starts with a terminal-workspace leaf; opening a file splits that
leaf or the focused editor horizontally. Splitting an editor vertically or
horizontally opens another file prompt. Terminal layout and its persistence stay
server-owned; editor content/layout does not persist across window close. Switching
terminal tabs leaves local editors open. A terminal disappearing cannot discard an
editor's unsaved work. No editor IDs or document bytes go into workspace_set.

Ctrl+Shift+O and the palette's Open file action prompt for `path[:line[:col]]`;
plain paths default to 1:1. Terminal path clicks use their own terminal's fresh
cwd query. Ctrl+S saves; Ctrl+Z/Ctrl+Shift+Z undo/redo; Ctrl+F opens a local find
prompt, Enter finds the next exact occurrence, Escape leaves the prompt. Arrow,
Home/End, PageUp/Down, Shift selection, Backspace/Delete, Tab and Enter are editor
input while focused. Mouse click/drag selects, wheel scrolls, dividers do not edit.
Errors remain visible without closing the window or sending failed input to a PTY.
Dirty editor/window close is refused until save or undo returns to saved content.

Editors operate on local files. Terminal clicks from SSH attachments are refused
as remote files, not opened against a coincidentally matching local path. Explicit
local absolute paths may still be opened from the palette. Remote file transport
is a later feature; no shell command is used to fetch or modify remote files.

## Pending changes

A review card shows the selected hunk's ID/state, old/new coloured text, accept,
reject, accept-all, reject-all and previous/next controls. Long previews explicitly
say they are clipped; scrolling the review preview reveals the rest. Stale cards
are labelled stale and never applicable. Review metadata does not change core
undo semantics. Refresh disk before review actions and when refocusing a file.

Agents can produce a local proposal JSON file; the person chooses Load changes
from the palette. Format: `{base: <exact current text>, edits: [{start,end,text}]}`.
Validate the whole proposal against the currently open document before adding any
hunks; mismatched base or invalid UTF-8 boundaries rejects the import. This is an
explicit local import, not an unversioned socket change or automatic disk apply.
The shared controller also exposes propose/accept/reject actions for a later
versioned agent transport. No competing M5 editor or server operation is added.

## Delivery

Commit design first and continue. The cwd operation is its own next commit.
Bounded workers may own cwd, editor controller, GUI integration, and the panel
gate/renderer sampler. The lead fixes the shared contract, reads every changed
line, integrates, scans secrets, tags `v0.4.4-panel.N` and pushes both remotes.
No new dependency beyond the existing local clanky-edit crate in the app. Stop
at the complete gate or the first unexpected real failed check, not compile errors.
