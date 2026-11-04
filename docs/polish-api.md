# Shared polish contract

No new dependencies. All added RPCs are owner-only v3; no capability-list change.

## Search

Worker owns `clanky-core/src/terminal/search.rs`; lead adds `mod search;` to
terminal.rs and the engine dispatcher after clipboard integration.
`impl Terminal { pub fn window_search(&mut self, query:&str,
after:Option<(usize,usize)>, backwards:bool)->Result<serde_json::Value,String> }`.
RPC `window_search {tab_id,query,after?:{row,col},backwards?:bool}` calls it.
Response fields: `total` match count, `selected` null or `{row,col}` in absolute
retained physical rows (oldest = 0), `snapshot` (TerminalSnapshot, images empty),
`highlights:[{row,col,selected:bool}]` in the returned snapshot coordinates.
Wrap only Flags::WRAPLINE rows; include history and live screen. Select first/last
without after, otherwise next/previous with wrap. Scroll selected start row into
view, then return the exact snapshot and visible highlight cells under one lock.
Empty query returns current view, zero total and no highlights; max query 256 bytes.

Worker also owns `clanky/src/scrollback_search.rs`, public module added by lead.
`Search::default()`, `open(&mut self,client:&Client,tab:&str)->Result<(),String>`,
`active_target()->Option<&str>`, `close()`,
`key(&mut self,client:&Client,key:&Key,text:Option<&str>,mods:ModifiersState,
composing:bool)->Result<(),String>`, `text(&mut self,client:&Client,text:&str)
->Result<(),String>`, `draw(&self,frame:&mut TerminalSnapshot,rect:&Rect)`.
Search pins the target; keeps a stable snapshot until query/navigation changes.
Enter next, Shift+Enter previous, Escape close, Backspace removes one scalar.
Draw applies selected background (120,85,25), others (65,60,30), and displays
query/status in global row 0 so it does not overwrite matched source rows.

## Desktop URL/folder helpers

Worker owns `clanky/src/window_tools.rs`, public module added by lead, and
`clanky-core/src/platform/desktop.rs` plus its declaration/re-export in platform.rs.
Do not edit linux.rs or engine.rs; clipboard worker owns those, lead adds home field.

- `UrlHit {pub url:String,pub row:usize,pub start_col:usize,pub end_col:usize}`.
- `url_at(snapshot:&TerminalSnapshot,row:usize,col:usize)->Option<UrlHit>`.
- `valid_url(url:&str)->bool` accepts only nonempty-host http/https, no controls or
  whitespace. Trim common surrounding prose punctuation without shell expansion.
- `url_click(hit:Option<&UrlHit>,control:bool,open:impl FnOnce(&str)->Result<(),String>)
  ->Result<bool,String>` returns true for any recognized URL (plain click consumed,
  zero launch calls), Ctrl invokes once; false permits ordinary file-path handling.
- `new_terminal(client:&Client,focused:Option<&str>,rows:usize,cols:usize)
  ->Result<String,String>` queries shell_defaults and terminal_cwd BEFORE open_tab;
  uses nonnull cwd or defaults["home"], never GUI cwd. Starts argv from defaults.
  Close the new tab if run fails. Both App new_tab/split use this function.
- Platform `home_dir()->Option<PathBuf>` uses valid absolute HOME, otherwise the
  current user's passwd home, never current_dir. Lead adds `home` to shell_defaults.
- Platform `UrlOpener::default()`, `open(&mut self,url:&str)->io::Result<()>`,
  `poll(&mut self)->io::Result<()>`; validate schemes defensively here too. Spawn
  xdg-open with one argument, no shell, reap only owned helper PIDs; Drop cleans
  still-running helpers without targeting external browser processes.

## Clipboard

Worker owns core terminal.rs/linux.rs/engine.rs edits, vte parser/FORK if needed,
and new app `clipboard.rs`. Lead adds search module/engine branch afterward.
Use engine ClipboardStore/ClipboardLoad events, no ad-hoc OSC parser. Bound each
write to 1 MiB, queue to at most 8 requests; reject oversize and malformed payloads
entirely. Read callbacks stay private, no Debug/value logs, bound to command IDs.

RPC `clipboard_events {tab_id,write:bool,read:bool}` drains at most ONE event:
`{events:[{id:u64,kind:"write",data:<base64>} | {id:u64,kind:"read"}]}`.
Disabled kinds are discarded without a helper call. For an allowed read, retain
its callback until the single-use reply; cap pending reads too.
RPC `clipboard_reply {tab_id,request_id,data:<base64>}` permits at most 64 KiB, validates
originating command still current, formats and enqueues its response once. Return
{}; expired/unknown/reused IDs fail. Only the clipboard selection is supported,
not primary-selection side effects. Plain headless operation has no clipboard IO.

Platform Clipboard::copy limit becomes 1 MiB; use memfd/anonymous memory-backed
input, not a temporary disk file. Add `clipboard_read()->io::Result<Vec<u8>>`
using wl-paste or xclip, max64KiB, bounded deadline and owned PID cleanup.

App `ClipboardBridge::new(client:Client,write:bool,read:bool)->Self`,
`set_tabs(&self,tabs:Vec<String>)`, `take_error(&self)->Option<String>`.
Own a background worker and platform Clipboard; poll bounded events round-robin,
no helper waits on the GUI thread. Drop stops/joins. Errors never include data.
For independent gate reuse expose
`handle_events(client:&Client,tab:&str,write:bool,read:bool,
copy:impl FnMut(&[u8])->Result<(),String>,
paste:impl FnMut()->Result<Vec<u8>,String>)->Result<(),String>`.
It performs the same owner event/reply calls and only invokes permitted callbacks.
Gate uses generated fake callbacks/helpers, never the actual desktop clipboard.

## GUI/config integration

Worker owns main.rs/panels.rs/config.rs only. Lead exports public modules and
integrates the core search/home dispatcher; do not edit other workers' files.
Config JSON/fields `osc52_write:bool` default true, `osc52_read:bool` default false.
App owns Search, UrlOpener, ClipboardBridge. Supply tabs to bridge after workspace
updates, drain errors into visible status. No raw clipboard data in status.

Ctrl+Shift+F opens search only over a focused terminal; its input/IME are consumed
before ordinary PTY input. Search and compose/file prompts must not compete.
While searching, draw its returned view over the captured pane, hide its images,
route Enter/Shift+Enter/Escape, and do not change target on workspace focus changes.

Hover/click resolve against the actual terminal snapshot and pane-relative cell.
URL hover produces a renderer Underline with global coordinates, RGB(80,160,255).
Do not underline/open links through compose, palette, file prompts or search overlays.
Plain recognized URL click returns without file opening; Ctrl invokes UrlOpener.
Existing non-URL file clicks retain behavior. Call opener.poll during normal frame
maintenance. Replace App::spawn_terminal body with window_tools::new_terminal,
passing the displayed focused terminal ID (none when no terminal is focused).
Existing split layout and remote-server argv stay unchanged.
