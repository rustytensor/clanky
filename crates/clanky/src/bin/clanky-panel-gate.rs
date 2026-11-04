//! Fixed real-file M7 panel gate; shares the GUI controller and renderer.
use clanky::editor::EditorPane;
use clanky_api::Client;
use clanky_core::{layout::Rect, snapshot_from_chunks, TerminalSnapshot};
use serde_json::{json, Value};
use std::{env, fs, path::{Path, PathBuf}, process::{Child, Command, Stdio}, thread,
    time::{Duration, Instant}, os::unix::fs::DirBuilderExt};
use winit::keyboard::{Key, NamedKey, ModifiersState};

const INITIAL: &str = "alpha\r\nβeta\nlast\n";
fn require(ok: bool, message: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(message.into()) }
}
fn io<T>(result: std::io::Result<T>) -> Result<T, String> { result.map_err(|e| e.to_string()) }
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Result<Self, String> {
        let target = PathBuf::from(env::var_os("CARGO_TARGET_DIR").ok_or("missing CARGO_TARGET_DIR")?);
        require(target.is_absolute(), "CARGO_TARGET_DIR must be absolute")?;
        for sequence in 0..1000 {
            let path = target.join(format!("panel-{}-{sequence}", std::process::id()));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
        Err("no private fixture directory available".into())
    }
}
impl Drop for Scratch { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) { let _ = clanky_core::platform::stop_server(&mut self.0); }
}
fn call(c: &Client, v: u64, op: &str, args: Value) -> Result<Value, String> {
    c.exchange(v, op, &args).map_err(|e| format!("{op}: {}", e.code))
}
fn action(p: &mut EditorPane, name: &str, args: Value) -> Result<Value, String> {
    p.command(name, &args).map_err(|e| format!("{name}: {e}"))
}
fn select(p: &mut EditorPane, anchor: usize, cursor: usize) -> Result<(), String> {
    action(p, "select", json!({"anchor":anchor,"cursor":cursor}))?; Ok(())
}
fn key(p: &mut EditorPane, named: NamedKey, modifiers: ModifiersState) -> Result<(), String> {
    p.key(&Key::Named(named), None, modifiers, false)
}
fn letter(p: &mut EditorPane, letter: &str, modifiers: ModifiersState) -> Result<(), String> {
    p.key(&Key::Character(letter.into()), Some(letter), modifiers, false)
}
fn blank(cols: usize, rows: usize) -> Result<TerminalSnapshot, String> {
    snapshot_from_chunks(rows, cols, &[]).map_err(|e| e.to_string())
}
fn text_row(frame: &TerminalSnapshot, row: usize) -> String {
    frame.cells[row*frame.cols..(row+1)*frame.cols].iter().map(|c| c.ch).collect()
}
fn rgb(sample: [u8;4], expected: [u8;3], label: &str) -> Result<(), String> {
    require((0..3).all(|i| sample[i].abs_diff(expected[i]) <= 8), label)
}
fn quote(path: &Path) -> String { format!("'{}'", path.to_string_lossy().replace('\'', "'\\''")) }

fn run() -> Result<(), String> {
    let root = Scratch::new()?;
    let work = root.0.join("work");
    io(fs::create_dir_all(work.join("src")))?;
    let path = work.join("src/demo.txt");
    io(fs::write(&path, INITIAL.as_bytes()))?;
    let socket = root.0.join("s.sock");
    let app = io(env::current_exe())?.with_file_name("clanky");
    let mut server = Server(io(Command::new(app).args(["--server", "--socket"])
        .arg(&socket).current_dir(&root.0).stdin(Stdio::null()).stdout(Stdio::null())
        .stderr(Stdio::null()).spawn())?);
    let deadline = Instant::now() + Duration::from_secs(10);
    let client = loop {
        if let Ok(client) = Client::connect_owner(&socket) { break client; }
        require(io(server.0.try_wait())?.is_none(), "fixture server exited")?;
        require(Instant::now() < deadline, "fixture server startup deadline")?;
        thread::sleep(Duration::from_millis(20));
    };
    let opened = call(&client, 1, "open_tab", json!({"rows":24,"cols":100,"allow_agent":false}))?;
    let tab = opened["tab_id"].as_str().ok_or("missing fixture tab")?;
    // A real shell changes cwd, prints the clicked token, then waits alive in read.
    let script = format!("cd {} && printf 'src/demo.txt:2:2\\n' && read ignored", quote(&work));
    call(&client, 1, "run", json!({"tab_id":tab,"argv":["sh","-c",script],"cwd":root.0}))?;
    let deadline = Instant::now() + Duration::from_secs(10);
    let (snapshot, row, col) = loop {
        let snapshot = client.snapshot(tab).map_err(|e| e.code)?;
        let found = (0..snapshot.rows).find_map(|row| {
            text_row(&snapshot, row).find("src/demo.txt:2:2").map(|col| (row,col))
        });
        if let Some((row,col)) = found { break (snapshot,row,col+4); }
        require(Instant::now() < deadline, "shell path output deadline")?;
        thread::sleep(Duration::from_millis(20));
    };
    let cwd = call(&client, 3, "terminal_cwd", json!({"tab_id":tab}))?;
    let resolved_cwd = PathBuf::from(cwd["cwd"].as_str().ok_or("terminal cwd unavailable")?);
    require(resolved_cwd == work, "real shell cwd must be fixture work")?;
    match EditorPane::from_click(&snapshot, row, col, None) {
        Err(error) => require(error == "unknown directory", "missing cwd exact error")?,
        Ok(_) => return Err("missing cwd opened a relative file".into()),
    }
    let mut pane = EditorPane::from_click(&snapshot, row, col, Some(&resolved_cwd))?;
    require(pane.path() == path && pane.text() == INITIAL, "clicked exact initial file")?;
    require(pane.selection().cursor == 9, "2:2 must be byte9 after beta")?;
    let absolute = format!("{}:2:2", path.display());
    let absolute_snapshot = snapshot_from_chunks(3, 240, &[absolute.as_bytes()]).map_err(|e| e.to_string())?;
    require(EditorPane::from_click(&absolute_snapshot, 0, 1, None)?.selection().cursor == 9,
        "absolute click without cwd")?;
    let invalid = snapshot_from_chunks(3, 80, &[b"ordinary text"]).map_err(|e| e.to_string())?;
    require(EditorPane::from_click(&invalid, 0, 2, Some(&work)).is_err(), "invalid token must not open")?;

    letter(&mut pane, "X", ModifiersState::empty())?;
    require(pane.text() == "alpha\r\nβXeta\nlast\n", "keyboard X exact insertion")?;
    letter(&mut pane, "z", ModifiersState::CONTROL)?;
    require(pane.text() == INITIAL && pane.selection().cursor == 9, "undo bytes and cursor")?;
    letter(&mut pane, "z", ModifiersState::CONTROL | ModifiersState::SHIFT)?;
    require(pane.text() == "alpha\r\nβXeta\nlast\n", "keyboard redo")?;
    letter(&mut pane, "z", ModifiersState::CONTROL)?;
    select(&mut pane, 0, 0)?;
    letter(&mut pane, "f", ModifiersState::CONTROL)?;
    require(pane.find_active(), "Ctrl+F opens local find")?;
    pane.insert_text("last")?;
    key(&mut pane, NamedKey::Enter, ModifiersState::empty())?;
    require(pane.selection().anchor == 13 && pane.selection().cursor == 17, "find last selects13..17")?;
    key(&mut pane, NamedKey::Escape, ModifiersState::empty())?;
    // Fixed controller geometry: title row0; text starts row1,col0. Mouse selects beta.
    let rect = Rect {col:0,row:0,cols:80,rows:24};
    let mut frame = blank(80,24)?;
    pane.draw(&mut frame, &rect, true);
    pane.click(&rect, 0, 2, false)?;
    pane.drag(&rect, 1, 2)?;
    require(pane.selection().anchor == 7 && pane.selection().cursor == 9, "mouse beta scalar boundaries")?;
    letter(&mut pane, "B", ModifiersState::empty())?;
    require(pane.text() == "alpha\r\nBeta\nlast\n", "mouse replacement exact bytes")?;
    action(&mut pane, "undo", json!({}))?;
    select(&mut pane, 9, 9)?;
    key(&mut pane, NamedKey::Backspace, ModifiersState::empty())?;
    require(pane.text() == "alpha\r\neta\nlast\n", "backspace removes whole beta")?;
    action(&mut pane, "undo", json!({}))?;
    select(&mut pane, 9, 9)?;
    key(&mut pane, NamedKey::ArrowLeft, ModifiersState::empty())?;
    require(pane.selection().cursor == 7, "left crosses whole beta")?;
    key(&mut pane, NamedKey::ArrowRight, ModifiersState::SHIFT)?;
    require(pane.selection().anchor == 7 && pane.selection().cursor == 9, "shift right selects whole beta")?;
    select(&mut pane, 9, 9)?;
    key(&mut pane, NamedKey::Enter, ModifiersState::empty())?;
    require(pane.text() == "alpha\r\nβ\r\neta\nlast\n", "Enter preserves first CRLF convention")?;
    action(&mut pane, "undo", json!({}))?;
    require(pane.text() == INITIAL, "input workflow restored initial bytes")?;

    let first = action(&mut pane, "propose", json!({"start":7,"end":12,"text":"BETA"}))?["id"].clone();
    let second = action(&mut pane, "propose", json!({"start":13,"end":17,"text":"LAST"}))?["id"].clone();
    require(pane.text() == INITIAL && io(fs::read(&path))? == INITIAL.as_bytes(), "pending hunks change neither memory nor disk")?;
    // First proposal is selected explicitly, independent of proposal insertion focus.
    action(&mut pane, "review_previous", json!({}))?;
    let info = pane.draw(&mut frame, &rect, true);
    let old = info.old_row.ok_or("missing old preview row")?;
    let new = info.new_row.ok_or("missing new preview row")?;
    require(text_row(&frame, old).contains("βeta") && text_row(&frame, new).contains("BETA"), "real old/new review text")?;
    // Fixed before rendering: col79 is blank beyond the four-character previews.
    require(frame.cells[old*80+79].ch == ' ' && frame.cells[new*80+79].ch == ' ', "review sample cells must be blank")?;
    let pixels = clanky_render::offscreen_cell_samples(&frame, [(old,79),(new,79)])?;
    rgb(pixels[0], [80,24,24], "old review background RGB")?;
    rgb(pixels[1], [24,80,40], "new review background RGB")?;
    let selection_path = root.0.join("selection.txt");
    io(fs::write(&selection_path, b" \n"))?;
    let mut selection = EditorPane::open(&selection_path, 1, 1)?;
    select(&mut selection, 0, 1)?;
    let mut selected_frame = blank(80,24)?;
    selection.draw(&mut selected_frame, &rect, true);
    // Blank selected first source cell row1,col0; ordinary blank row1,col79.
    require(selected_frame.cells[80].ch == ' ', "selection sample is a real blank")?;
    let pixels = clanky_render::offscreen_cell_samples(&selected_frame, [(1,0),(1,79)])?;
    rgb(pixels[0], [40,70,120], "selected blank RGB")?;
    rgb(pixels[1], [24,24,28], "ordinary blank RGB")?;
    action(&mut pane, "accept", json!({"id":first}))?;
    action(&mut pane, "reject", json!({"id":second}))?;
    letter(&mut pane, "s", ModifiersState::CONTROL)?;
    let mut observed = io(fs::read(&path))?;
    // Fault changes only the observed result, never the independently fixed expectation.
    if env::var("CLANKY_GATE_FAULT").as_deref() == Ok("editor-save") { observed = b"WRONG".to_vec(); }
    require(observed == b"alpha\r\nBETA\nlast\n", "item1 saved bytes expected alpha\\r\\nBETA\\nlast\\n")?;

    let stale_path = root.0.join("stale.txt");
    io(fs::write(&stale_path, b"a\nb\nc\n"))?;
    let mut stale = EditorPane::open(&stale_path, 1, 1)?;
    action(&mut stale, "propose", json!({"start":0,"end":1,"text":"A"}))?;
    io(fs::write(&stale_path, b"EXTERNAL\n"))?;
    require(action(&mut stale, "refresh", json!({})).is_err(), "refresh reports external change")?;
    let mut stale_frame = blank(80,24)?;
    stale.draw(&mut stale_frame, &rect, true);
    require((0..24).any(|row| text_row(&stale_frame,row).contains("Stale")), "stale hunk visibly labelled")?;
    let _ = action(&mut stale, "accept_all", json!({}));
    require(stale.text() == "a\nb\nc\n", "stale accept-all cannot mutate memory")?;
    require(action(&mut stale, "save", json!({})).is_err(), "stale save must fail")?;
    require(io(fs::read(&stale_path))? == b"EXTERNAL\n", "stale save preserves external bytes")?;
    for accept in [true, false] {
        let path = root.0.join(if accept { "accept.txt" } else { "reject.txt" });
        io(fs::write(&path, b"a\nb\nc\n"))?;
        let mut p = EditorPane::open(&path, 1, 1)?;
        for (start,text) in [(0,"A"),(2,"B"),(4,"C")] {
            action(&mut p, "propose", json!({"start":start,"end":start+1,"text":text}))?;
        }
        action(&mut p, if accept {"accept_all"} else {"reject_all"}, json!({}))?;
        // Opposite bulk action must skip already accepted/rejected hunks.
        action(&mut p, if accept {"reject_all"} else {"accept_all"}, json!({}))?;
        action(&mut p, "save", json!({}))?;
        require(io(fs::read(&path))? == if accept { b"A\nB\nC\n" } else { b"a\nb\nc\n" }, "all-hunk exact disk bytes")?;
    }
    let big_path = root.0.join("large.txt");
    io(fs::write(&big_path, "let x = 1;\n".repeat(1_525_202)))?;
    let mut big = EditorPane::open(&big_path, 1, 1)?;
    // Fixed viewport: zero-based line 1,525,190 leaves twelve nonempty lines
    // plus the valid trailing empty EOF line visible in eighteen text rows.
    action(&mut big, "scroll", json!({"lines":1_525_190}))?;
    let rect = Rect {col:0,row:0,cols:40,rows:20};
    let mut frame = blank(40,20)?;
    let info = big.draw(&mut frame, &rect, true);
    require(info.source_lines > 0 && info.source_lines <= info.text_rows && info.text_rows <= 20, "large EOF draw bounded by visible text rows")?;
    require(text_row(&frame,12).trim_end_matches(' ') == "let x = 1;", "last nonempty EOF source line at fixed frame row12")?;
    require(text_row(&frame,13).chars().all(|c| c == ' '), "valid trailing empty EOF source line at row13")?;
    call(&client, 1, "close_tab", json!({"tab_id":tab}))?;
    drop(client);
    println!("PASS: M7 panel exact bytes, input, pixels, stale review and bounded EOF draw");
    Ok(())
}
fn main() {
    if let Err(error) = run() { eprintln!("FAIL: {error}"); std::process::exit(1); }
}
