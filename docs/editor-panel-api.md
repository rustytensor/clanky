# Shared editor controller contract

The app library exports `pub mod editor`. This controller owns no Client or window.
All methods below return `Result<_, String>` unless specified otherwise.

- `EditorPane::open(path: &Path, line: usize, column: usize) -> EditorPane`
- `EditorPane::from_click(snapshot: &TerminalSnapshot, row: usize, col: usize,
  cwd: Option<&Path>) -> EditorPane`: rows/columns relative to that terminal.
- `pub fn location_at(snapshot: &TerminalSnapshot, row: usize, col: usize)
  -> Option<clanky_edit::Location>` extracts the whitespace-delimited clicked
  path:line[:column] token (trailing comma, semicolon and closing brackets allowed).
- `pub fn resolve_location(location: &Location, cwd: Option<&Path>) -> PathBuf`:
  relative with no cwd returns exactly `unknown directory`; absolute ignores cwd.
- `command(&mut self, action: &str, args: &serde_json::Value) -> Value`
- `key(&mut self, key: &winit::keyboard::Key, text: Option<&str>,
  modifiers: winit::keyboard::ModifiersState, composing: bool) -> ()`
- `insert_text(&mut self, text: &str) -> ()`: handles a find prompt if active,
  otherwise performs an ordinary insertion. Used for IME commit too.
- `click(&mut self, rect: &Rect, col: usize, row: usize, extend: bool) -> ()`;
  `drag(&mut self, rect: &Rect, col: usize, row: usize) -> ()` extends only text
  selections; dragging never presses a review button.
- `draw(&mut self, frame: &mut TerminalSnapshot, rect: &Rect, focused: bool)
  -> DrawInfo` is infallible and paints only that rectangle. DrawInfo has public
  `source_lines: usize`, `text_rows: usize`, `old_row: Option<usize>`,
  `new_row: Option<usize>` (absolute frame row indices for offscreen sampling).
- Borrowed getters `text() -> &str`, `path() -> &Path`; `dirty() -> bool`,
  `selection() -> clanky_edit::Selection`, `find_active() -> bool`.

Command actions (unknown actions/invalid fields fail):

| action | arguments | effect/result |
| --- | --- | --- |
| insert | text | replace selection; {} |
| select | anchor, cursor | checked byte offsets; {} |
| move | kind, extend optional false | left/right/up/down/home/end/pageup/pagedown; {} |
| backspace, delete, undo, redo, save | none | {}; save retains history |
| find | query | select next exact match; {} or visible no-match error |
| find_next | none | repeat current query with wrap; {} |
| propose | start, end, text | {id} |
| accept, reject | id | {} |
| accept_all, reject_all | none | {}; pending only, stale cannot apply |
| review_next, review_previous | none | select review card; {} |
| scroll | lines signed, columns optional signed | document scroll; {} |
| review_scroll | lines signed | selected old/new preview scroll; {} |
| refresh | none | refresh stale state without loading external bytes; {} |
| load_proposals | path | validate/import JSON base+edits from chosen local file; {} |

No direct public mutable Document access: old text previews must be captured when
proposing. The renderer uses old/new backgrounds (80,24,24)/(24,80,40), selection
(40,70,120), ordinary text (24,24,28). Review buttons may be clipped in narrow panes;
keyboard Ctrl+Alt+A/R accepts/rejects selected, with Shift applies to all pending.
Headers and status expose clipping and errors. All coordinates/strings validate
before mutation. Existing clanky-edit is the only text/history/review engine.

GUI integration owns its local Layout and map of EditorPane objects. The existing
terminal workspace is one local-layout leaf; its real terminal panes are computed
inside that leaf's rectangle. Other leaves are editor IDs, never sent to server.
Open file and Load changes are local palette actions, not server RPCs. The GUI
calls terminal_cwd through existing Client::exchange(3, "terminal_cwd", ...).

The renderer adds `offscreen_cell_samples<const N: usize>(snapshot:
&TerminalSnapshot, coordinates: [(usize, usize); N]) -> Result<[[u8;4];N], String>`.
Coordinates are (row,col); use the existing offscreen pipeline and software adapter.
