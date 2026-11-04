//! Fixed M8 expectations: actual compose controller, PTY bytes and font pipeline.
use clanky::compose::{Compose, ComposeEvent, Spellchecker};
use clanky_api::Client;
use clanky_core::{layout::Rect, snapshot_from_chunks};
use serde_json::{json, Value};
use std::{env, fs, os::unix::fs::DirBuilderExt, path::{Path, PathBuf},
    process::{Child, Command, Stdio}, thread, time::{Duration, Instant}};
use winit::keyboard::{Key, ModifiersState, NamedKey};

const PROSE: &str = "Please review this sentnce and teh changes.\nRun --quik src/mispeld.rs foo_barr std::vec https://exampl.invalid `wrng wrod`.\n";
const DRAFT: &str = "first line\nsecond β\n";
// Independently specified bytes: 21 text bytes + two six-byte markers.
const PASTE: &[u8] = b"\x1b[200~first line\nsecond \xce\xb2\n\x1b[201~";
const PASTE_ENTER: &[u8] = b"\x1b[200~first line\nsecond \xce\xb2\n\x1b[201~\r";
const RECEIVER: &str = r#"import os, select, sys, time
from pathlib import Path
output, ready = map(Path, sys.argv[1:])
def publish(path, data):
    temporary = path.with_name(path.name + '.tmp')
    temporary.write_bytes(data)
    temporary.replace(path)
publish(ready, b'ready')
data = bytearray()
deadline = time.monotonic() + 10
while time.monotonic() < deadline:
    closed = b'\x1b[201~' in data
    readable, _, _ = select.select([0], [], [], 0.3 if closed else min(0.3, max(0, deadline-time.monotonic())))
    if readable:
        chunk = os.read(0, 65536)
        if not chunk:
            break
        data.extend(chunk)
    elif closed:
        break
publish(output, data)
"#;
fn require(ok: bool, why: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(why.into()) }
}
fn io<T>(v: std::io::Result<T>) -> Result<T, String> { v.map_err(|e| e.to_string()) }
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Result<Self, String> {
        let target = PathBuf::from(env::var_os("CARGO_TARGET_DIR").ok_or("missing CARGO_TARGET_DIR")?);
        require(target.is_absolute(), "CARGO_TARGET_DIR must be absolute")?;
        for n in 0..1000 {
            let path = target.join(format!("compose-{}-{n}", std::process::id()));
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
fn call(c: &Client, op: &str, args: Value) -> Result<Value, String> {
    c.exchange(1, op, &args).map_err(|e| format!("{op}: {}", e.code))
}
fn key(c: &mut Compose, named: NamedKey, mods: ModifiersState) -> Result<ComposeEvent, String> {
    c.key(&Key::Named(named), None, mods, false)
}
fn letter(c: &mut Compose, text: &str, mods: ModifiersState) -> Result<(), String> {
    c.key(&Key::Character(text.into()), Some(text), mods, false)?; Ok(())
}
fn text(c: &Compose, target: &str) -> Result<String, String> {
    Ok(c.draft(target).ok_or("missing draft")?.text().to_string())
}
fn wait_file(path: &Path) -> Result<Vec<u8>, String> {
    let until = Instant::now() + Duration::from_secs(12);
    loop {
        match fs::read(path) {
            Ok(bytes) => return Ok(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.to_string()),
        }
        require(Instant::now() < until, "PTY fixture file deadline")?;
        thread::sleep(Duration::from_millis(20));
    }
}
fn quote(path: &Path) -> String { format!("'{}'", path.to_string_lossy().replace('\'', "'\\''")) }
fn receiver(client: &Client, root: &Path, name: &str) -> Result<(String, PathBuf), String> {
    let output = root.join(format!("{name}.bytes"));
    let ready = root.join(format!("{name}.ready"));
    let tab = call(client, "open_tab", json!({"rows":24,"cols":100,"allow_agent":false}))?
        ["tab_id"].as_str().ok_or("missing tab")?.to_owned();
    let script = format!("stty raw -echo && exec python3 {} {} {}",
        quote(&root.join("receiver.py")), quote(&output), quote(&ready));
    call(client, "run", json!({"tab_id":tab,"argv":["sh","-c",script],"cwd":root}))?;
    require(wait_file(&ready)? == b"ready", "raw receiver readiness")?;
    Ok((tab, output))
}
fn run() -> Result<(), String> {
    let root = Scratch::new()?;
    // Only the system dictionary and explicit private fixture personal paths are read.
    let dictionary = Path::new("/usr/share/hunspell");
    let personal = root.0.join("personal.txt");
    let mut checker = Spellchecker::load(dictionary, "en_US", Some(personal.clone()))?;
    let flagged = checker.flagged(PROSE);
    require(flagged.len() == 2 && flagged[0].word == "sentnce" && flagged[0].range == (19..26)
        && flagged[1].word == "teh" && flagged[1].range == (31..34), "exact prose flags/ranges; code tokens excluded")?;
    require(checker.suggest("sentnce").iter().any(|s| s == "sentence"), "sentnce suggests sentence")?;
    require(checker.suggest("teh").iter().any(|s| s == "the"), "teh suggests the")?;
    checker.add_word("sentnce")?;
    require(io(fs::read(&personal))? == b"sentnce\n", "personal exact sentnce LF")?;
    let fresh = Spellchecker::load(dictionary, "en_US", Some(personal))?;
    let flagged = fresh.flagged(PROSE);
    require(flagged.len() == 1 && flagged[0].word == "teh" && flagged[0].range == (31..34), "fresh checker only teh")?;

    io(fs::write(root.0.join("receiver.py"), RECEIVER))?;
    let socket = root.0.join("s.sock");
    let mut server = Server(io(Command::new(io(env::current_exe())?.with_file_name("clanky"))
        .args(["--server", "--socket"]).arg(&socket).current_dir(&root.0)
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn())?);
    let until = Instant::now() + Duration::from_secs(10);
    let client = loop {
        if let Ok(c) = Client::connect_owner(&socket) { break c; }
        require(io(server.0.try_wait())?.is_none(), "fixture server exited")?;
        require(Instant::now() < until, "fixture server startup deadline")?;
        thread::sleep(Duration::from_millis(20));
    };
    let (a, observed_a) = receiver(&client, &root.0, "a")?;
    let (b, observed_b) = receiver(&client, &root.0, "b")?;
    let mut compose = Compose::new(dictionary, "en_US", Some(root.0.join("unused.txt")), false);
    require(compose.spell_error().is_none(), "system dictionary loads")?;
    compose.open(&a)?;
    compose.insert_text(DRAFT)?;
    key(&mut compose, NamedKey::ArrowLeft, ModifiersState::SHIFT)?;
    let selection = compose.draft(&a).ok_or("A draft")?.selection();
    let (anchor, cursor) = (selection.anchor, selection.cursor);
    key(&mut compose, NamedKey::Escape, ModifiersState::empty())?;
    require(compose.active_target().is_none(), "Escape closes compose")?;
    compose.open(&a)?;
    let selection = compose.draft(&a).ok_or("A draft")?.selection();
    require(text(&compose,&a)? == DRAFT && selection.anchor == anchor && selection.cursor == cursor,
        "reopen preserves bytes and selection")?;
    compose.open(&b)?;
    require(text(&compose,&b)?.is_empty(), "B independent empty draft")?;
    compose.open(&a)?;
    key(&mut compose, NamedKey::ArrowRight, ModifiersState::empty())?;
    letter(&mut compose, "X", ModifiersState::empty())?;
    require(text(&compose,&a)? == "first line\nsecond β\nX", "X insertion exact")?;
    letter(&mut compose, "z", ModifiersState::CONTROL)?;
    require(text(&compose,&a)? == DRAFT, "undo X exact")?;
    letter(&mut compose, "z", ModifiersState::CONTROL | ModifiersState::SHIFT)?;
    require(text(&compose,&a)? == "first line\nsecond β\nX", "redo X exact")?;
    letter(&mut compose, "z", ModifiersState::CONTROL)?;
    require(text(&compose,&b)?.is_empty(), "undo/redo independent of B")?;
    let mut failed_calls = 0;
    require(compose.send_with(|target,_| { failed_calls += 1; require(target == a, "failed target A")?; Err("fixture refused".into()) }).is_err(), "send failure surfaced")?;
    require(failed_calls == 1 && compose.active_target() == Some(a.as_str()) && text(&compose,&a)? == DRAFT,
        "failed send retains box and draft")?;
    require(key(&mut compose, NamedKey::Enter, ModifiersState::CONTROL)? == ComposeEvent::Send,
        "Ctrl+Enter requests Send")?;
    let mut calls = 0;
    compose.send_with(|target,bytes| {
        calls += 1;
        require(target == a, "send pinned A while B exists")?;
        client.input(target, bytes).map_err(|e| e.code)
    })?;
    require(calls == 1 && compose.active_target().is_none() && text(&compose,&a)? == DRAFT,
        "one input; success closes and retains draft")?;
    let mut observed = wait_file(&observed_a)?;
    // Change observed PTY data only; fixed expected bytes above never change.
    if env::var("CLANKY_GATE_FAULT").as_deref() == Ok("compose-send") {
        if observed.starts_with(b"\x1b[200~") { observed.drain(..6); }
    }
    require(PASTE.len() == 33 && observed == PASTE, "PTY default exact 33 bytes, no trailing Enter")?;
    let mut with_enter = Compose::new(dictionary, "en_US", Some(root.0.join("enter.txt")), true);
    with_enter.open(&b)?;
    with_enter.insert_text(DRAFT)?;
    require(key(&mut with_enter, NamedKey::Enter, ModifiersState::CONTROL)? == ComposeEvent::Send, "enter option requests Send")?;
    let mut calls = 0;
    with_enter.send_with(|target,bytes| {
        calls += 1;
        require(target == b, "optional Enter target B")?;
        client.input(target,bytes).map_err(|e| e.code)
    })?;
    require(calls == 1 && PASTE_ENTER.len() == 34 && wait_file(&observed_b)? == PASTE_ENTER,
        "PTY exact 34 bytes with one CR")?;

    let missing = root.0.join("missing-dictionary");
    let mut degraded = Compose::new(&missing, "en_US", Some(root.0.join("missing-words.txt")), false);
    require(degraded.spell_error().is_some(), "missing dictionary visible error")?;
    degraded.open(&a)?;
    degraded.insert_text(DRAFT)?;
    let mut calls = 0;
    degraded.send_with(|_,bytes| { calls += 1; require(bytes == PASTE, "dictionary failure still sends exact bytes") })?;
    require(calls == 1, "dictionary failure sends once")?;
    for (index, bad) in ["bad\u{1b}", "bad\0", "bad\u{7}"].into_iter().enumerate() {
        degraded.open(&format!("invalid-{index}"))?;
        if degraded.insert_text(bad).is_ok() {
            let mut calls = 0;
            require(degraded.send_with(|_,_| { calls += 1; Ok(()) }).is_err() && calls == 0,
                "control rejected before callback")?;
        }
    }
    let mut empty = Compose::new(dictionary, "en_US", Some(root.0.join("empty.txt")), false);
    empty.open(&a)?;
    let mut calls = 0;
    require(empty.send_with(|_,_| { calls += 1; Ok(()) }).is_err() && calls == 0, "empty refuses callback")?;

    let mut render = Compose::new(dictionary, "en_US", Some(root.0.join("render.txt")), false);
    render.open(&a)?;
    render.insert_text(PROSE)?;
    let mut frame = snapshot_from_chunks(24,100,&[]).map_err(|e| e.to_string())?;
    let rect = Rect { row:0,col:0,rows:24,cols:100 };
    let underlines = render.draw(&mut frame, &rect);
    let row = (0..frame.rows).find(|&row| frame.cells[row*100..(row+1)*100].iter()
        .map(|c| c.ch).collect::<String>().starts_with("Please review this sentnce"))
        .ok_or("actual compose prose not drawn at column zero")?;
    require(frame.cells[row*100+18].ch == ' ' && frame.cells[row*100+19].ch == 's', "fixed blank18 / word19 geometry")?;
    // Caret is on the final empty line, not on the underlined prose.
    let x = |col| col * clanky_render::CELL_WIDTH as usize + 8;
    let y = (row+1) * clanky_render::CELL_HEIGHT as usize;
    let pixels = clanky_render::offscreen_underlined_samples(&frame, &underlines,
        [(x(19),y-1),(x(19),y-2),(x(18),y-1)])?;
    require(pixels[0][..3] == [255,96,96] && pixels[1][..3] == [255,96,96], "word bottom two pixels RGB255,96,96")?;
    require(pixels[2][..3] == [24,24,28], "adjacent blank RGB24,24,28")?;
    call(&client, "close_tab", json!({"tab_id":a}))?;
    call(&client, "close_tab", json!({"tab_id":b}))?;
    drop(client);
    println!("PASS: M8 exact spelling, personal words, drafts, PTY bytes and underline pixels");
    Ok(())
}
fn main() {
    if let Err(e) = run() { eprintln!("FAIL: {e}"); std::process::exit(1); }
}
