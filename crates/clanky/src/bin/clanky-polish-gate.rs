//! Fixed docs/design-polish-window.md expectations on real server/parser/PTY paths.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use clanky::{clipboard::handle_events, scrollback_search::Search, window_tools};
use clanky_api::Client;
use clanky_core::{TerminalSnapshot, layout::Rect, platform, snapshot_from_chunks};
use serde_json::{Value, json};
use std::{env, fs, os::unix::fs::{DirBuilderExt, PermissionsExt}, path::{Path, PathBuf},
    process::{Child, Command, Stdio}, thread, time::{Duration, Instant}};
use winit::keyboard::{Key, ModifiersState, NamedKey};

const URL: &str = "https://example.invalid/a?q=one";
const SHELL_URL: &str = "https://example.invalid/$(touch${IFS}NEVER);x=1&y=2";
const REPLY: &[u8] = b"\x1b]52;c;d29ybGQ=\x07";
// Generated fixtures only. This is an output producer/raw input recorder, not a VT parser.
const EMITTER: &str = r#"import base64, os, select, sys, time
from pathlib import Path
payload, output, split = Path(sys.argv[1]), Path(sys.argv[2]), int(sys.argv[3])
def write(data):
    while data:
        n = os.write(1, data)
        data = data[n:]
data = payload.read_bytes()
# Large output is generated inside the actual PTY command, never in RPC argv.
if data in (b'LIMIT', b'OVERFLOW'):
    size = 1048576 if data == b'LIMIT' else 1048577
    data = b'\x1b]52;c;' + base64.b64encode(b'A' * size) + b'\x07'
if split:
    write(data[:split])
    time.sleep(0.15)
    write(data[split:])
else:
    write(data)
write(b'\r\nPOLISH_READY\r\n')
received = bytearray()
deadline = time.monotonic() + 12
while time.monotonic() < deadline:
    readable, _, _ = select.select([0], [], [], 0.3 if received else 0.1)
    if readable:
        part = os.read(0, 65536)
        if not part:
            break
        if part == b'Q':
            sys.exit(0)
        received.extend(part)
    elif received:
        break
temporary = output.with_suffix('.tmp')
temporary.write_bytes(received)
temporary.replace(output)
# Keep the original command alive until the gate explicitly closes the tab.
while os.read(0, 65536):
    pass
"#;
fn require(ok: bool, label: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(label.into()) }
}
fn io<T>(result: std::io::Result<T>) -> Result<T, String> { result.map_err(|e| e.to_string()) }
fn call(c: &Client, v: u64, op: &str, args: Value) -> Result<Value, String> {
    c.exchange(v, op, &args).map_err(|e| format!("{op}: {}", e.code))
}
fn quote(path: &Path) -> String { format!("'{}'", path.to_string_lossy().replace('\'', "'\\''")) }
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Result<Self, String> {
        let target = PathBuf::from(env::var_os("CARGO_TARGET_DIR").ok_or("missing CARGO_TARGET_DIR")?);
        require(target.is_absolute(), "CARGO_TARGET_DIR must be absolute")?;
        for n in 0..1000 {
            let path = target.join(format!("polish-{}-{n}", std::process::id()));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
        Err("private fixture directory unavailable".into())
    }
}
impl Drop for Scratch { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
struct Server(Child);
impl Drop for Server { fn drop(&mut self) { let _ = platform::stop_server(&mut self.0); } }
struct Fixture { owner: Client, agent: Client, _server: Server }
impl Fixture {
    fn start(root: &Path) -> Result<Self, String> {
        let socket = root.join("s.sock");
        io(fs::create_dir(root.join("home")))?;
        io(fs::create_dir(root.join("work")))?;
        io(fs::write(root.join("emitter.py"), EMITTER))?;
        let mut server = Server(io(Command::new(io(env::current_exe())?.with_file_name("clanky"))
            .args(["--server", "--socket"]).arg(&socket).current_dir(root)
            .env("HOME", root.join("home")).env("SHELL", "/bin/sh")
            .env_remove("ENV").env_remove("BASH_ENV").env_remove("ZDOTDIR")
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn())?);
        let deadline = Instant::now() + Duration::from_secs(10);
        let owner = loop {
            if let Ok(owner) = Client::connect_owner(&socket) { break owner; }
            require(io(server.0.try_wait())?.is_none(), "fixture server exited")?;
            require(Instant::now() < deadline, "fixture server startup deadline")?;
            thread::sleep(Duration::from_millis(20));
        };
        let agent = io(Client::connect_agent(&socket))?;
        // Keep one idle tab until shutdown so inter-case gaps cannot trigger idle exit.
        call(&owner, 1, "open_tab", json!({"allow_agent":false}))?;
        Ok(Self { owner, agent, _server: server })
    }
    fn tab(&self, rows: usize, cols: usize) -> Result<String, String> {
        call(&self.owner, 1, "open_tab", json!({"rows":rows,"cols":cols,"allow_agent":true}))?
            ["tab_id"].as_str().map(str::to_owned).ok_or("missing tab id".into())
    }
    fn run(&self, tab: &str, cwd: &Path, argv: Value) -> Result<Value, String> {
        call(&self.owner, 1, "run", json!({"tab_id":tab,"cwd":cwd,"argv":argv}))
    }
    fn close(&self, tab: &str) -> Result<(), String> {
        call(&self.owner, 1, "close_tab", json!({"tab_id":tab})).map(|_| ())
    }
    fn wait(&self, tab: &str, command: &Value) -> Result<(), String> {
        let reply = call(&self.owner, 1, "wait", json!({"tab_id":tab,
            "command_id":command["command_id"],"timeout_ms":5000}))?;
        require(reply["done"] == true && reply["exit_code"] == 0, "fixture command exits zero")
    }
    fn ready(&self, tab: &str, marker: &str) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let snap = self.owner.snapshot(tab).map_err(|e| e.code)?;
            let text: String = snap.cells.iter().map(|c| c.ch).collect();
            if text.contains(marker) { return Ok(()); }
            require(Instant::now() < deadline, "fixture output readiness deadline")?;
            thread::sleep(Duration::from_millis(20));
        }
    }
    fn emit(&self, root: &Path, name: &str, payload: &[u8], split: usize)
        -> Result<(String, Value, PathBuf), String> {
        let input = root.join(format!("{name}.osc"));
        let output = root.join(format!("{name}.bytes"));
        io(fs::write(&input, payload))?;
        let tab = self.tab(8, 80)?;
        let script = format!("stty raw -echo && exec python3 {} {} {} {split}",
            quote(&root.join("emitter.py")), quote(&input), quote(&output));
        let command = self.run(&tab, root, json!(["/bin/sh","-c",script]))?;
        self.ready(&tab, "POLISH_READY")?;
        Ok((tab, command, output))
    }
}
fn read_file(path: &Path) -> Result<Vec<u8>, String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match fs::read(path) {
            Ok(data) => return Ok(data),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.to_string()),
        }
        require(Instant::now() < deadline, "fixture file deadline")?;
        thread::sleep(Duration::from_millis(20));
    }
}
fn search(f: &Fixture, root: &Path) -> Result<(), String> {
    let tab = f.tab(2, 8)?;
    let command = f.run(&tab, root, json!(["/bin/sh","-c","printf 'abcHELLOthere\\r\\nabcHELLOthere\\r\\n'"]))?;
    f.wait(&tab, &command)?;
    let lookup = |after: Value, backwards: bool| call(&f.owner, 3, "window_search",
        json!({"tab_id":tab,"query":"LOth","after":after,"backwards":backwards}));
    let first = lookup(Value::Null, false)?;
    let mut count = first["total"].as_u64();
    // Only observed count is corrupted. The pre-calculated expected value remains two.
    if env::var("CLANKY_GATE_FAULT").as_deref() == Ok("wrapped-search") { count = Some(0); }
    require(count == Some(2), "wrapped search: expected exactly two matches")?;
    require(first["selected"] == json!({"row":0,"col":6}), "wrapped search first (0,6)")?;
    let second = lookup(first["selected"].clone(), false)?;
    require(second["total"] == 2 && second["selected"] == json!({"row":2,"col":6}), "wrapped search next (2,6)")?;
    let previous = lookup(second["selected"].clone(), true)?;
    require(previous["selected"] == json!({"row":0,"col":6}), "wrapped search previous (0,6)")?;
    let snapshot: TerminalSnapshot = serde_json::from_value(previous["snapshot"].clone()).map_err(|_| "search snapshot")?;
    require(snapshot.rows == 2 && snapshot.cols == 8 && snapshot.images.is_empty(), "search viewport dimensions")?;
    require(snapshot.cells.iter().map(|c| c.ch).collect::<String>() == "abcHELLOthere   ", "search exact viewport text")?;
    let highlights = previous["highlights"].as_array().ok_or("search highlights")?;
    let mut observed = highlights.iter().map(|h| (h["row"].as_u64(),h["col"].as_u64(),h["selected"].as_bool())).collect::<Vec<_>>();
    observed.sort();
    require(observed == vec![(Some(0),Some(6),Some(true)),(Some(0),Some(7),Some(true)),
        (Some(1),Some(0),Some(true)),(Some(1),Some(1),Some(true))], "search exact highlights, no padding")?;
    denied(&f.agent, "window_search", json!({"tab_id":tab,"query":"LOth"}))?;
    require(call(&f.owner, 3, "window_search", json!({"tab_id":tab,"query":""}))?["total"] == 0, "blank query has zero matches")?;
    require(f.owner.exchange(3, "window_search", &json!({"tab_id":tab,"query":"x".repeat(257)})).is_err(), "oversize query rejected")?;

    // Exercise the actual window search controller and its selected backgrounds.
    let mut controller = Search::default();
    controller.open(&f.owner, &tab)?;
    controller.text(&f.owner, "LOth")?;
    require(controller.active_target() == Some(tab.as_str()), "search target pinned")?;
    let mut frame = snapshot_from_chunks(3, 8, &[]).map_err(|e| e.code)?;
    controller.draw(&mut frame, &Rect { row:1,col:0,rows:2,cols:8 });
    require(frame.cells[8..].iter().map(|c| c.ch).collect::<String>() == "abcHELLOthere   ", "controller retains source text")?;
    for (r,c) in [(0,6),(0,7),(1,0),(1,1)] {
        require(frame.cells[(r+1)*8+c].bg == [120,85,25], "selected search background RGB120,85,25")?;
    }
    controller.key(&f.owner, &Key::Named(NamedKey::Enter), None, ModifiersState::empty(), false)?;
    controller.key(&f.owner, &Key::Named(NamedKey::Enter), None, ModifiersState::SHIFT, false)?;
    controller.close();
    f.close(&tab)?;
    let hard = f.tab(2, 8)?;
    let command = f.run(&hard, root, json!(["/bin/sh","-c","printf 'LO\\r\\nth'"]))?;
    f.wait(&hard, &command)?;
    require(call(&f.owner, 3, "window_search", json!({"tab_id":hard,"query":"LOth"}))?["total"] == 0, "hard line must not join")?;
    f.close(&hard)?;
    // A larger real viewport exposes selected and unselected matches together.
    let both = f.tab(6, 8)?;
    let command = f.run(&both, root, json!(["/bin/sh","-c","printf 'abcHELLOthere\\r\\nabcHELLOthere\\r\\n'"]))?;
    f.wait(&both, &command)?;
    controller.open(&f.owner, &both)?;
    controller.text(&f.owner, "LOth")?;
    let mut frame = snapshot_from_chunks(7,8,&[]).map_err(|e| e.code)?;
    controller.draw(&mut frame, &Rect { row:1,col:0,rows:6,cols:8 });
    for (r,c) in [(2,6),(2,7),(3,0),(3,1)] {
        require(frame.cells[(r+1)*8+c].bg == [65,60,30], "other search background RGB65,60,30")?;
    }
    controller.close();
    f.close(&both)
}
fn denied(agent: &Client, op: &str, args: Value) -> Result<(), String> {
    let error = agent.exchange(3, op, &args).err().ok_or("owner operation allowed agent")?;
    require(error.code == "permission_denied", "owner operation agent permission_denied")
}
fn urls(root: &Path) -> Result<(), String> {
    let bin = root.join("bin");
    io(fs::create_dir(&bin))?;
    let log = root.join("open.argv");
    let helper = bin.join("xdg-open");
    // NUL-separated argv records distinguish one literal argument from shell splitting.
    io(fs::write(&helper, "#!/bin/sh\nprintf '%s\\000' \"$#\" \"$@\" >> \"$POLISH_OPEN_LOG\"\n"))?;
    io(fs::set_permissions(&helper, fs::Permissions::from_mode(0o700)))?;
    // A child process gets the fake PATH; never mutate environment in a threaded process.
    let status = io(Command::new(io(env::current_exe())?)
        .arg("--url-fixture").arg(root).env("PATH", &bin).env("POLISH_OPEN_LOG", &log)
        .stdin(Stdio::null()).status())?;
    require(status.success(), "URL fixture child")
}
fn url_checks(root: &Path) -> Result<(), String> {
    let log = root.join("open.argv");
    let snapshot = snapshot_from_chunks(2,80,&[URL.as_bytes()]).map_err(|e| e.code)?;
    let hit = window_tools::url_at(&snapshot,0,10).ok_or("URL hover hit missing")?;
    require(hit.url == URL && hit.row == 0 && hit.start_col == 0 && hit.end_col == URL.len(), "URL exact hover span")?;
    let mut calls = 0;
    require(window_tools::url_click(Some(&hit), false, |_| { calls += 1; Ok(()) })? && calls == 0,
        "plain URL click consumed with no opener/file action")?;
    require(!log.exists(), "plain click no browser helper")?;
    require(window_tools::valid_url("http://example.invalid/"), "http allowed")?;
    for bad in ["file:///tmp/x","javascript:alert(1)","https:///empty","https://","https://example.invalid/\n","https://example.invalid/\u{7}"] {
        require(!window_tools::valid_url(bad), "forbidden URL refused")?;
        let mut opener = platform::UrlOpener::default();
        require(opener.open(bad).is_err(), "platform rejects forbidden URL before launch")?;
    }
    require(!log.exists(), "forbidden URLs launch nothing")?;
    let mut opener = platform::UrlOpener::default();
    require(window_tools::url_click(Some(&hit),true,|url| io(opener.open(url)))?, "Ctrl URL click consumed")?;
    let deadline = Instant::now() + Duration::from_secs(5);
    let expected = format!("1\0{URL}\0").into_bytes();
    loop {
        io(opener.poll())?;
        if fs::read(&log).ok().as_deref() == Some(expected.as_slice()) { break; }
        require(Instant::now() < deadline, "URL exact single argv deadline")?;
        thread::sleep(Duration::from_millis(20));
    }
    let meta = snapshot_from_chunks(2,80,&[SHELL_URL.as_bytes()]).map_err(|e| e.code)?;
    let hit = window_tools::url_at(&meta,0,10).ok_or("literal metachar URL hit")?;
    require(hit.url == SHELL_URL, "metacharacters remain literal URL")?;
    window_tools::url_click(Some(&hit),true,|url| io(opener.open(url)))?;
    let expected = format!("1\0{URL}\01\0{SHELL_URL}\0").into_bytes();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        io(opener.poll())?;
        if fs::read(&log).ok().as_deref() == Some(expected.as_slice()) { break; }
        require(Instant::now() < deadline, "literal metachar argv deadline")?;
        thread::sleep(Duration::from_millis(20));
    }
    // The real underline renderer samples at the bottom pixel of the hovered row.
    let underline = clanky_render::Underline { row:0,start_col:0,end_col:URL.len(),color:[80,160,255] };
    let y = clanky_render::CELL_HEIGHT as usize - 1;
    let samples = clanky_render::offscreen_underlined_samples(&snapshot, &[underline],
        [(8,y),(URL.len()*clanky_render::CELL_WIDTH as usize-8,y),(URL.len()*clanky_render::CELL_WIDTH as usize+8,y)])?;
    require(samples[0][..3] == [80,160,255] && samples[1][..3] == [80,160,255], "hover underline RGB80,160,255 bottom pixels")?;
    require(samples[2][..3] != [80,160,255], "hover span excludes adjacent padding")?;
    Ok(())
}
fn folders(f: &Fixture, root: &Path) -> Result<(), String> {
    let source = f.tab(8,80)?;
    let command = f.run(&source,&root.join("home"),json!(["/bin/sh","-c",
        "stty -echo; cd ../work || exit 2; printf 'FOLDER_READY\\n'; while IFS= read -r line; do :; done"]))?;
    f.ready(&source,"FOLDER_READY")?;
    let cwd = |tab: &str| call(&f.owner,3,"terminal_cwd",json!({"tab_id":tab}));
    require(cwd(&source)?["cwd"] == json!(root.join("work")), "source really cd work")?;
    require(call(&f.owner,3,"shell_defaults",json!({}))?["home"] == json!(root.join("home")), "server advertises fixture HOME")?;
    for _ in 0..2 {
        let tab = window_tools::new_terminal(&f.owner,Some(&source),8,80)?;
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if cwd(&tab)?["cwd"] == json!(root.join("work")) { break; }
            require(Instant::now() < deadline,"new terminal work cwd deadline")?;
            thread::sleep(Duration::from_millis(20));
        }
        f.close(&tab)?;
    }
    require(call(&f.owner,1,"get_exit_code",json!({"tab_id":source,"command_id":command["command_id"]}))?["done"] == false,
        "source shell stays alive")?;
    let idle = f.tab(8,80)?;
    require(cwd(&idle)?["cwd"].is_null(), "idle source null cwd")?;
    let tab = window_tools::new_terminal(&f.owner,Some(&idle),8,80)?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if cwd(&tab)?["cwd"] == json!(root.join("home")) { break; }
        require(Instant::now() < deadline,"null source HOME fallback deadline")?;
        thread::sleep(Duration::from_millis(20));
    }
    f.close(&tab)?; f.close(&idle)?; f.close(&source)
}
fn osc(data: &[u8]) -> Vec<u8> { format!("\x1b]52;c;{}\x07",STANDARD.encode(data)).into_bytes() }
fn clipboard(f: &Fixture, root: &Path) -> Result<(), String> {
    let hello = osc(b"hello");
    require(hello == b"\x1b]52;c;aGVsbG8=\x07", "fixed hello sequence")?;
    let cases = [
        ("hello",hello.clone(),0,Some(b"hello".to_vec()),true),
        ("split",hello.clone(),10,Some(b"hello".to_vec()),true),
        ("limit",b"LIMIT".to_vec(),0,Some(vec![b'A';1048576]),true),
        ("overflow",b"OVERFLOW".to_vec(),0,None,true),
        ("invalid",b"\x1b]52;c;!!!!\x07".to_vec(),0,None,true),
        ("disabled",hello,0,None,false),
    ];
    for (name,payload,split,expected,write) in cases {
        let (tab,_,_) = f.emit(root,name,&payload,split)?;
        let mut copies = Vec::new();
        let mut reads = 0;
        handle_events(&f.owner,&tab,write,false,
            |bytes| { copies.push(bytes.to_vec()); Ok(()) },
            || { reads += 1; Ok(b"world".to_vec()) })?;
        match expected {
            Some(bytes) => require(copies.len() == 1 && copies[0] == bytes,"OSC52 exactly one complete write of fixed bytes")?,
            None => require(copies.is_empty(),"OSC52 rejected/disabled write invokes zero callbacks")?,
        }
        require(reads == 0,"write never invokes read callback")?;
        let mut extra = 0;
        handle_events(&f.owner,&tab,true,true,|_| { extra += 1; Ok(()) },|| Err("unexpected read callback".into()))?;
        require(extra == 0,"OSC52 event drained once, no overflow accepted prefix")?;
        f.close(&tab)?;
    }
    let (tab,_,_) = f.emit(root,"read-disabled",b"\x1b]52;c;?\x07",0)?;
    let mut reads = 0;
    handle_events(&f.owner,&tab,true,false,|_| Err("unexpected write".into()),|| { reads += 1; Ok(b"world".to_vec()) })?;
    require(reads == 0,"default read false invokes zero callbacks")?;
    denied(&f.agent,"clipboard_events",json!({"tab_id":tab,"write":true,"read":true}))?;
    denied(&f.agent,"clipboard_reply",json!({"tab_id":tab,"request_id":1,"data":"d29ybGQ="}))?;
    f.close(&tab)?;
    let (tab,_,output) = f.emit(root,"read-world",b"\x1b]52;c;?\x07",0)?;
    let mut reads = 0;
    handle_events(&f.owner,&tab,true,true,|_| Err("unexpected write".into()),|| { reads += 1; Ok(b"world".to_vec()) })?;
    require(reads == 1 && read_file(&output)? == REPLY,"explicit read world exact OSC52 reply through raw PTY")?;
    f.close(&tab)?;

    // Bounded queues and one-event drain, without assuming an undocumented eviction order.
    let burst = osc(b"hello").repeat(12);
    let (tab,_,_) = f.emit(root,"burst",&burst,0)?;
    let mut count = 0;
    loop {
        let reply = call(&f.owner,3,"clipboard_events",json!({"tab_id":tab,"write":true,"read":true}))?;
        let events = reply["events"].as_array().ok_or("clipboard events array")?;
        require(events.len() <= 1,"clipboard RPC drains at most one event")?;
        if events.is_empty() { break; }
        require(events[0]["kind"] == "write" && events[0]["data"] == "aGVsbG8=","bounded queue retains exact generated writes")?;
        count += 1;
        require(count <= 8,"clipboard event queue bounded to eight")?;
    }
    f.close(&tab)?;
    let (tab,_,output) = f.emit(root,"single-use",b"\x1b]52;c;?\x07",0)?;
    let event = call(&f.owner,3,"clipboard_events",json!({"tab_id":tab,"write":true,"read":true}))?;
    require(event["events"].as_array().map(Vec::len) == Some(1) && event["events"][0]["kind"] == "read","pending read event")?;
    let args = json!({"tab_id":tab,"request_id":event["events"][0]["id"],"data":"d29ybGQ="});
    call(&f.owner,3,"clipboard_reply",args.clone())?;
    require(f.owner.exchange(3,"clipboard_reply",&args).is_err(),"read reply single use")?;
    require(read_file(&output)? == REPLY,"single-use reply exactly once on PTY")?;
    f.close(&tab)?;
    let (tab,command,_) = f.emit(root,"expired",b"\x1b]52;c;?\x07",0)?;
    let event = call(&f.owner,3,"clipboard_events",json!({"tab_id":tab,"write":true,"read":true}))?;
    require(event["events"][0]["kind"] == "read","old command pending read")?;
    f.owner.input(&tab,b"Q").map_err(|e| e.code)?;
    f.wait(&tab,&command)?;
    f.run(&tab,root,json!(["/bin/sh","-c","printf 'REPLACEMENT_READY\\n'; while IFS= read -r line; do :; done"]))?;
    f.ready(&tab,"REPLACEMENT_READY")?;
    require(f.owner.exchange(3,"clipboard_reply",&json!({"tab_id":tab,"request_id":event["events"][0]["id"],"data":"d29ybGQ="})).is_err(),
        "old command reply cannot target replacement command")?;
    f.close(&tab)
}
fn run() -> Result<(), String> {
    let root = Scratch::new()?;
    let fixture = Fixture::start(&root.0)?;
    search(&fixture,&root.0)?;
    urls(&root.0)?;
    folders(&fixture,&root.0)?;
    clipboard(&fixture,&root.0)?;
    // Drop clients/server before deleting the private fixture tree, also on every error.
    drop(fixture);
    println!("PASS: window polish real wrapped search, URL argv/underline, shell cwd and OSC52 PTY bytes");
    Ok(())
}
fn main() {
    let args: Vec<_> = env::args_os().skip(1).collect();
    let result = if args.len() == 2 && args[0] == "--url-fixture" {
        url_checks(Path::new(&args[1]))
    } else if args.is_empty() { run() } else { Err("invalid gate arguments".into()) };
    if let Err(error) = result { eprintln!("FAIL: {error}"); std::process::exit(1); }
}
