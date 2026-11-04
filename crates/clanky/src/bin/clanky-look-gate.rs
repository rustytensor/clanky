//! Real app layout and scene observations for the M11 gate.
use base64::{engine::general_purpose::STANDARD, Engine as _};
use clanky::{config::Config, layout_store, look::{self, AgentItem, BlockCard, FileItem, LookState, PaneState, SessionItem, TimelineItem}, settings::Settings};
use clanky_api::Client;
use clanky_core::{Engine, layout::{Axis, Layout as Tree}, snapshot_from_chunks};
use clanky_render::{Layer, offscreen_scene};
use serde_json::{json, Value};
use std::{env, fs, path::Path, process::{Child, Command, Stdio}, thread, time::{Duration, Instant}};

struct Session(Engine);
impl Drop for Session { fn drop(&mut self) { self.0.shutdown(); } }

fn pty_size(engine: &Engine, pane: &look::PaneLayout) -> Result<Vec<usize>, String> {
    let call = |op: &str, args: Value| engine.request(op, &args, true).map_err(|e| e.message);
    let tab = call("open_tab", json!({"rows":pane.rows,"cols":pane.cols,"allow_agent":false}))?["tab_id"]
        .as_str().ok_or("missing tab")?.to_owned();
    let command = call("run", json!({"tab_id":tab,"argv":["sh","-c","stty size"],"cwd":"/"}))?["command_id"]
        .as_str().ok_or("missing command")?.to_owned();
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        if call("wait", json!({"tab_id":tab,"command_id":command,"timeout_ms":1000}))?["done"] == true { break; }
        if Instant::now() > end { return Err("PTY wait timed out".into()); }
    }
    let output = call("get_output", json!({"tab_id":tab,"command_id":command,"offset":0,"limit":1024,"encoding":"base64"}))?;
    let bytes = STANDARD.decode(output["data"].as_str().ok_or("missing PTY data")?).map_err(|e| e.to_string())?;
    let size = std::str::from_utf8(&bytes).map_err(|e| e.to_string())?.split_whitespace()
        .map(|s| s.parse::<usize>().map_err(|e| e.to_string())).collect::<Result<Vec<_>,_>>()?;
    call("close_tab", json!({"tab_id":tab}))?;
    Ok(size)
}
struct StoreServer { child: Child, client: Client }
impl StoreServer {
    fn start(socket: &Path) -> Result<Self, String> {
        let app = env::current_exe().map_err(|e| e.to_string())?.with_file_name("clanky");
        let mut child = Command::new(app).arg("--server").arg("--socket").arg(socket)
            .stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|e| e.to_string())?;
        let deadline = Instant::now() + Duration::from_secs(10);
        let client = loop {
            if let Ok(client) = Client::connect_owner(socket) { break client; }
            if Instant::now() >= deadline || child.try_wait().map_err(|e| e.to_string())?.is_some() {
                let _ = child.kill(); let _ = child.wait();
                return Err("store server startup failed".into());
            }
            thread::sleep(Duration::from_millis(20));
        };
        Ok(Self { child, client })
    }
}
impl Drop for StoreServer {
    fn drop(&mut self) {
        let _ = Command::new("kill").arg("-TERM").arg(self.child.id().to_string()).status();
        let _ = self.child.wait();
    }
}
fn restored_layout(dir: &Path, ratio: f64) -> Result<(look::Layout, String, String), String> {
    let path = dir.join("config").join("clanky").join("layout.json");
    let first = StoreServer::start(&dir.join("save.sock"))?;
    let call = |op: &str, args: Value| first.client.exchange(1, op, &args).map_err(|e| e.message);
    let mut ids = Vec::new();
    for _ in 0..2 {
        let tab = call("open_tab", json!({"rows":14,"cols":71,"allow_agent":false}))?["tab_id"].as_str().ok_or("store tab")?.to_owned();
        call("run",json!({"tab_id":tab,"argv":["sh","-c","sleep 600"],"cwd":"/"}))?;
        ids.push(tab);
    }
    let mut tree = Tree::branch(Axis::Rows,Tree::leaf(&ids[0]),Tree::leaf(&ids[1]));
    tree.set_ratio(&[],ratio)?;
    let mut workspace = first.client.workspace_get().map_err(|e| e.message)?;
    workspace.groups = vec![clanky_core::WorkspaceGroup { layout: tree, focus: ids[1].clone() }];
    workspace.active = 0;
    first.client.workspace_set(&workspace).map_err(|e| e.message)?;
    layout_store::save(&first.client,&path)?;
    drop(first);
    let second = StoreServer::start(&dir.join("restore.sock"))?;
    if !layout_store::restore(&second.client,&path,true)? { return Err("store did not restore".into()); }
    let workspace = second.client.workspace_get().map_err(|e| e.message)?;
    let tree = &workspace.groups.first().ok_or("missing restored group")?.layout;
    let ids = tree.leaves();
    if ids.len() != 2 { return Err("wrong restored pane count".into()); }
    let layout = look::layout([1240,760],1.0,true,tree,[10,20]);
    Ok((layout,ids[0].clone(),ids[1].clone()))
}
fn pane(layout: &look::Layout, id: &str) -> Value {
    let p = layout.pane(id).expect("fixture pane");
    json!({"rect":p.rect.array(),"origin":p.grid_origin,"cols":p.cols,"rows":p.rows})
}
fn sample(bytes: &[u8], x: usize, y: usize) -> &[u8] { &bytes[(y*1240+x)*4..][..4] }
fn main() {
    if let Err(error) = run() { eprintln!("FAIL: M11 observation: {error}"); std::process::exit(1); }
}
fn run() -> Result<(), String> {
    let output = env::args().nth(1).ok_or("missing raw image path")?;
    let tree = Tree::branch(Axis::Rows, Tree::leaf("A"), Tree::leaf("B"));
    let shown = look::layout([1240,760],1.0,true,&tree,[10,20]);
    let hidden = look::layout([1240,760],1.0,false,&tree,[10,20]);
    let dragged = look::drag_divider(&shown,0,300.0)?;
    let work = Path::new(&output).parent().ok_or("missing fixture directory")?;
    let ratio = if env::var("CLANKY_GATE_FAULT").as_deref() == Ok("store") { 0.5 } else { dragged.ratio };
    let (restored, restored_a, restored_b) = restored_layout(work, ratio)?;
    let engine = Session(Engine::new());
    let pty_a = pty_size(&engine.0, shown.pane("A").ok_or("pane A")?)?;
    let pty_b = pty_size(&engine.0, shown.pane("B").ok_or("pane B")?)?;
    let snap_a = snapshot_from_chunks(14,71,&["~/projects/clanky ❯ ".as_bytes()]).map_err(|e| e.message)?;
    let snap_b = snapshot_from_chunks(14,71,&[b"cargo build"]).map_err(|e| e.message)?;
    let mut a = PaneState::new("A",snap_a);
    a.title = "~/projects/clanky".into(); a.driver = "fish".into();
    let mut b = PaneState::new("B",snap_b);
    b.title = "cargo build".into(); b.driver = "fish".into();
    b.blocks.push(BlockCard { id:"fixture".into(),first_row:0,last_row:0,command:"cargo build".into(),exit_code:Some(0),duration_ms:Some(41_200),running:false });
    let mut state = LookState {
        focused:Some("B".into()), panes:vec![a,b], uptime_seconds:21_600, masking_ready:true,
        sessions:vec![
            SessionItem { id:"A".into(),title:"clanky".into(),active:true },
            SessionItem { id:"session-2".into(),title:"api".into(),active:false },
            SessionItem { id:"session-3".into(),title:"docs".into(),active:false },
        ],
        agents:vec![
            AgentItem { connection_id:1,peer_pid:1842,tab_id:Some("A".into()),running:false },
            AgentItem { connection_id:2,peer_pid:1856,tab_id:Some("B".into()),running:true },
        ],
        files:vec![
            FileItem { id:"main.rs".into(),title:"main.rs".into(),pending:2,active:true },
            FileItem { id:"config.rs".into(),title:"config.rs".into(),pending:0,active:false },
        ],
        timeline:vec![
            TimelineItem { label:"Session opened".into(),detail:"clanky".into() },
            TimelineItem { label:"Agent connected".into(),detail:"idle".into() },
            TimelineItem { label:"File changed".into(),detail:"main.rs".into() },
            TimelineItem { label:"Build finished".into(),detail:"41.2 s".into() },
        ],
        activity:vec![0.,1.,0.,2.,1.,3.,2.,1.,4.,2.,3.,1.],
        ..Default::default()
    };
    let mut scene = look::scene(&shown,&state);
    if env::var("CLANKY_GATE_FAULT").as_deref() == Ok("look") {
        for layer in &mut scene.layers {
            if let Layer::Shape(shape) = layer { if shape.rect == [220.0,50.0,740.0,345.0] { shape.radius = 0.0; } }
        }
    }
    let rgba = offscreen_scene(&scene,1240,760)?;
    fs::write(Path::new(&output),&rgba).map_err(|e| e.to_string())?;
    let mut pixels = serde_json::Map::new();
    for (x,y) in [(590,44),(950,380),(221,51),(226,56),(100,700),(1100,700),(230,451),(936,451),(215,577)] {
        pixels.insert(format!("{x},{y}"),json!(sample(&rgba,x,y)));
    }
    state.panes[0].allow_agent = true;
    state.panes[0].agent_running = true;
    let agent = offscreen_scene(&look::scene(&shown,&state),1240,760)?;
    state.opacity = Config::from_json("{\"opacity\":1}")?.opacity;
    state.panes[0].allow_agent = false; state.panes[0].agent_running = false;
    let opaque = offscreen_scene(&look::scene(&shown,&state),1240,760)?;
    state.opacity = 0.94;
    state.theme = Config::from_json("{\"theme\":\"light\"}")?.resolved_theme(false);
    let light = offscreen_scene(&look::scene(&shown,&state),1240,760)?;
    fs::write(Path::new(&output).with_file_name("scene-light.rgba"), &light).map_err(|e| e.to_string())?;
    let mut light_pixels = serde_json::Map::new();
    for (x,y) in [(590,44),(950,380),(221,51),(226,56),(100,700),(1100,700),(230,451),(936,451),(215,577)] {
        light_pixels.insert(format!("{x},{y}"),json!(sample(&light,x,y)));
    }
    state.panes[0].allow_agent = true; state.panes[0].agent_running = true;
    let light_agent = offscreen_scene(&look::scene(&shown,&state),1240,760)?;
    let config_path = work.join("settings.json");
    fs::write(&config_path, b"{\"rails\":false,\"opacity\":0.94,\"theme\":\"light\"}\n").map_err(|e| e.to_string())?;
    let mut settings = Settings::open(&config_path)?;
    settings.set("opacity",json!(0.8))?;
    let loaded = Config::load(Some(&config_path))?;
    let before = fs::read(&config_path).map_err(|e| e.to_string())?;
    let text = std::str::from_utf8(&before).map_err(|e| e.to_string())?;
    let key_order = text.find("\"rails\"") < text.find("\"opacity\"") && text.find("\"opacity\"") < text.find("\"theme\"");
    state.opacity = loaded.opacity;
    state.theme = loaded.resolved_theme(false);
    state.panes[0].allow_agent = false; state.panes[0].agent_running = false;
    let opacity_image = offscreen_scene(&look::scene(&shown,&state),1240,760)?;
    let invalid_error = settings.set("opacity",json!(0.3)).err().ok_or("invalid settings accepted")?;
    let loader_error = Config::from_json("{\"opacity\":0.3}").err().ok_or("invalid loader config accepted")?;
    let unchanged = fs::read(&config_path).map_err(|e| e.to_string())? == before;
    settings.set_color("panel",Some("#202020"))?;
    state.theme = settings.config().resolved_theme(false);
    let override_image = offscreen_scene(&look::scene(&shown,&state),1240,760)?;
    let export = work.join("settings-export.json");
    settings.export(&export)?;
    settings.reset("rails")?;
    settings.import(&export)?;
    let identical = fs::read(&config_path).map_err(|e| e.to_string())? == fs::read(&export).map_err(|e| e.to_string())?;
    let mut opacity_pixel = sample(&opacity_image,590,44).to_vec();
    if env::var("CLANKY_GATE_FAULT").as_deref() == Ok("settings") { opacity_pixel[3] = 255; }
    let ease = look::ease_out_cubic;
    println!("{}",json!({
        "layout":{"content":shown.content.array(),"a":pane(&shown,"A"),"b":pane(&shown,"B"),"pty_a":pty_a,"pty_b":pty_b},
        "rails_hidden":{"a":pane(&hidden,"A"),"b":pane(&hidden,"B")},
        "drag":{"a":pane(&dragged.layout,"A"),"b":pane(&dragged.layout,"B"),"restored_a":pane(&restored,&restored_a),"restored_b":pane(&restored,&restored_b)},
        "pixels":pixels,"agent_glow":sample(&agent,215,222),
        "light_pixels":light_pixels,"light_agent_glow":sample(&light_agent,215,222),
        "settings":{"opacity_readback":loaded.opacity,"opacity_pixel":opacity_pixel,"invalid_error":invalid_error,"loader_error":loader_error,"invalid_unchanged":unchanged,"panel_override_pixel":sample(&override_image,950,380),"export_import_identical":identical,"key_order_preserved":key_order},
        "motion":{"cursor":[ease(100.,200.,0.,80.,true),ease(100.,200.,40.,80.,true),ease(100.,200.,80.,80.,true),ease(100.,200.,120.,80.,true)],"panel_60ms":ease(0.,1.,60.,120.,true),"cursor_disabled_0ms":ease(100.,200.,0.,80.,false),"panel_disabled_0ms":ease(0.,1.,0.,120.,false)},
        "config":{"opacity_0_3_error":Config::from_json("{\"opacity\":0.3}").is_err(),"rails_yes_error":Config::from_json("{\"rails\":\"yes\"}").is_err(),"opacity_one_pixel":sample(&opaque,590,44)}
    }));
    Ok(())
}
