# Shared compose implementation contract

All Result errors below are String. No server or serialized snapshot changes.

## Memory editing (clanky-edit and clanky::editor)

- `Document::from_text(text: &str) -> Document`: memory-only text, same history,
  selections and hunks. `refresh()` is Ok(true); `save()` returns a specific NoPath
  error. Existing `Document::open` stays file-backed with unchanged safety.
- `EditorPane::from_text(text: &str) -> EditorPane`: same keyboard, mouse, scroll,
  find and rendering, no temporary file. Display path `[compose]` is only a label.
- `pub struct ByteCell { pub byte: usize, pub row: usize, pub col: usize }`
  and `EditorPane::visible_byte_cells(&self, rect: &Rect) -> Vec<ByteCell>` enumerate
  visible source cells only, with absolute frame coordinates and UTF-8 byte starts.
  Tabs may yield four cells sharing a byte offset. This uses the same cached
  viewport/line indexing as draw, excluding title, status and review rows.

## clanky::compose

Re-export `Spellchecker` and `Misspelling` from its spelling module.
`Misspelling { pub range: Range<usize>, pub word: String }`.

- `Spellchecker::load(dir: &Path, language: &str, personal: Option<PathBuf>)
  -> Result<Spellchecker,String>`; use
  `spellbook::Dictionary<std::collections::hash_map::RandomState>` and
  `Dictionary::new_with_hasher(..., RandomState::new())` (default features disabled).
- `flagged(&self, text: &str) -> Vec<Misspelling>`; `suggest(&self, word: &str)
  -> Vec<String>`; `add_word(&mut self, word: &str) -> Result<(),String>`.
- `Compose::new(dir: &Path, language: &str, personal: Option<PathBuf>,
  send_enter: bool) -> Compose`: dictionary failure is retained as visible status,
  never a failure to create/edit/send drafts. Load once, not per frame.
- `open(&mut self, terminal: &str) -> Result<(),String>`; `close(&mut self)`;
  `active_target(&self) -> Option<&str>`; `has_drafts(&self) -> bool` reports any
  nonempty retained draft; `draft(&self, terminal: &str)
  -> Option<&EditorPane>`; `spell_error(&self) -> Option<&str>`.
- `insert_text(&mut self, text: &str) -> Result<(),String>`;
  `key(&mut self, key: &Key, text: Option<&str>, modifiers: ModifiersState,
  composing: bool) -> Result<ComposeEvent,String>`, where
  `#[derive(Clone,Copy,Debug,PartialEq,Eq)] enum ComposeEvent { None, Send }`.
  Ctrl+Enter without Shift requests Send (unless composing); Escape closes popups
  or a nested find prompt first, otherwise closes compose retaining draft.
- `send_with(&mut self, send: impl FnOnce(&str,&[u8])->Result<(),String>)
  -> Result<(),String>` validates, assembles exactly one bracketed payload and
  invokes the callback once with the captured target. No callback on invalid/empty
  draft. Success closes, failure stays open; neither clears the draft. Optional CR
  follows the closing marker. Reject ESC/NUL/other controls except TAB/CR/LF.
- `misspellings(&self) -> &[Misspelling]` cached for active draft;
  `suggestions(&mut self) -> Result<Vec<String>,String>` opens the suggestion list
  at/after the caret (wrap); `add_current_word(&mut self) -> Result<(),String>`.
- `click(&mut self, rect:&Rect,col:usize,row:usize,extend:bool)->Result<(),String>`;
  `drag` same without extend; `scroll(&mut self,lines:i64,columns:i64)->Result<(),String>`;
  `preedit(&mut self,text:&str)` for a bounded IME display.
- `draw(&mut self,frame:&mut TerminalSnapshot,rect:&Rect)->Vec<Underline>` paints
  the reused editor plus compose controls/status/suggestions and returns clipped
  underline spans. It does not send or alter file-backed editors. No dictionaries
  loaded or full spelling scans during draw. Text changes invalidate spelling.

Keep drafts memory-only. Check the 256 KiB limit before changes; undo/redo do not
bypass it. Suggestions are selected with Up/Down and Enter, F7 requests them,
Ctrl+Alt+A adds the selected misspelling. Correct via normal editor commands so
undo works. Ctrl+S never saves a draft file.

## Config and GUI

Config gains `pub compose_dictionary: String` (en_US),
`pub compose_dictionary_dir: PathBuf` (/usr/share/hunspell),
`pub compose_send_enter: bool` (false), `pub compose_words: Option<PathBuf>`.
The first three are configurable JSON fields; compose_words is resolved by load
from the config directory and is not a JSON field. from_json/default have no
personal path. Missing personal path disables Add with a visible error.

App owns Compose. Open only with a focused terminal (not a file editor), via
Ctrl+Shift+Enter or local palette Compose. Capture terminal ID and pane rectangle;
while open, keyboard/IME/mouse route only to Compose. Ctrl+Enter calls send_with
once, whose closure calls Client::input exactly once. Ignore repeat send keys.
A closed target is an error, not a fallback to current focus. Escape resets IME
focus cleanly. Remove/clip images that would cover the overlay. Preserve existing
local editors and their layout; no workspace/RPC additions.

## Renderer

`#[derive(Clone,Copy,Debug,PartialEq,Eq)] pub struct Underline { pub row:usize,
pub start_col:usize, pub end_col:usize, pub color:[u8;3] }` (half-open cell columns).
`Renderer::draw_with_underlines(&mut self,snapshot:&TerminalSnapshot,
underlines:&[Underline])->Result<(),String>`. Existing draw delegates with [].
Render underlines as bottom-two-pixel solid quads after glyphs, before the caret.
`offscreen_underlined_samples<const N:usize>(snapshot:&TerminalSnapshot,
underlines:&[Underline], coordinates:[(usize,usize);N])->Result<[[u8;4];N],String>`
uses pixel (x,y) sample coordinates and the existing offscreen pipeline.
Other offscreen wrappers still pass no underline list. No new text renderer.
