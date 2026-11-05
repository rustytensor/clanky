//! M12 observation harness. Incomplete checks remain absent, never fabricated.
use base64::{engine::general_purpose::STANDARD, Engine as _};
use clanky::{calm, layout_store, look::{self, BlockCard, LookState, PaneState}, themes};
use clanky_api::Client;
use clanky_core::{Engine, layout::{Axis, Layout as Tree}, snapshot_from_chunks};
use clanky_render::{Layer, offscreen_scene};
use serde_json::{Value, json};
use std::{env, fs, path::Path, process::{Child, Command, Stdio}, thread, time::{Duration, Instant}};

struct Session(Engine);
impl Drop for Session { fn drop(&mut self) { self.0.shutdown(); } }
fn pty_size(engine: &Engine, pane: &look::PaneLayout) -> Result<Vec<usize>, String> {
    let call = |op: &str, args: Value| engine.request(op, &args, true).map_err(|e| e.message);
    let tab = call("open_tab", json!({"rows":pane.rows,"cols":pane.cols,"allow_agent":false}))?["tab_id"].as_str().ok_or("missing tab")?.to_owned();
    let command = call("run", json!({"tab_id":tab,"argv":["sh","-c","stty size"],"cwd":"/"}))?["command_id"].as_str().ok_or("missing command")?.to_owned();
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        if call("wait",json!({"tab_id":tab,"command_id":command,"timeout_ms":1000}))?["done"] == true { break; }
        if Instant::now() > end { return Err("PTY wait timed out".into()); }
    }
    let output = call("get_output",json!({"tab_id":tab,"command_id":command,"offset":0,"limit":1024,"encoding":"base64"}))?;
    let bytes = STANDARD.decode(output["data"].as_str().ok_or("missing PTY data")?).map_err(|e| e.to_string())?;
    let size = std::str::from_utf8(&bytes).map_err(|e| e.to_string())?.split_whitespace().map(|s|s.parse::<usize>().map_err(|e|e.to_string())).collect::<Result<Vec<_>,_>>()?;
    call("close_tab",json!({"tab_id":tab}))?;
    Ok(size)
}
struct StoreServer { child: Child, client: Client }
impl StoreServer {
    fn start(socket: &Path) -> Result<Self, String> {
        let app = env::current_exe().map_err(|e|e.to_string())?.with_file_name("clanky");
        let mut child = Command::new(app).arg("--server").arg("--socket").arg(socket).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|e|e.to_string())?;
        let deadline = Instant::now() + Duration::from_secs(10);
        let client = loop {
            if let Ok(client) = Client::connect_owner(socket) { break client; }
            if Instant::now() >= deadline || child.try_wait().map_err(|e|e.to_string())?.is_some() {
                let _ = child.kill(); let _ = child.wait(); return Err("store server startup failed".into());
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
fn restored_layout(dir: &Path, ratio: f64) -> Result<(look::Layout,String,String), String> {
    let path=dir.join("config/clanky/layout.json");
    let first=StoreServer::start(&dir.join("save.sock"))?;
    let call=|op:&str,args:Value|first.client.exchange(1,op,&args).map_err(|e|e.message);
    let mut ids=Vec::new();
    for _ in 0..2 {
        let id=call("open_tab",json!({"rows":13,"cols":81,"allow_agent":false}))?["tab_id"].as_str().ok_or("missing store tab")?.to_owned();
        call("run",json!({"tab_id":id,"argv":["sh","-c","sleep 600"],"cwd":"/"}))?;
        ids.push(id);
    }
    let mut tree=Tree::branch(Axis::Rows,Tree::leaf(&ids[0]),Tree::leaf(&ids[1]));
    tree.set_ratio(&[],ratio)?;
    let mut workspace=first.client.workspace_get().map_err(|e|e.message)?;
    workspace.groups=vec![clanky_core::WorkspaceGroup{layout:tree,focus:ids[1].clone()}]; workspace.active=0;
    first.client.workspace_set(&workspace).map_err(|e|e.message)?;
    layout_store::save(&first.client,&path)?; drop(first);
    let second=StoreServer::start(&dir.join("restore.sock"))?;
    if !layout_store::restore(&second.client,&path,true)? { return Err("store did not restore".into()); }
    let workspace=second.client.workspace_get().map_err(|e|e.message)?;
    let tree=&workspace.groups.first().ok_or("missing restored group")?.layout;
    let ids=tree.leaves(); if ids.len()!=2 { return Err("wrong restored pane count".into()); }
    Ok((calm::layout([1240,760],1.0,true,false,tree,[8,20]),ids[0].clone(),ids[1].clone()))
}

fn pane(layout: &clanky::look::Layout, id: &str) -> Result<Value, String> {
    let p = layout.pane(id).ok_or("missing fixture pane")?;
    Ok(json!({"rect":p.rect.array(),"origin":p.grid_origin,"cols":p.cols,"rows":p.rows}))
}
fn sample(rgba: &[u8], x: usize, y: usize) -> &[u8] { &rgba[(y*1240+x)*4..][..4] }
fn render(layout: &clanky::look::Layout, state: &LookState, name: &str, output: &Path, fault: bool)
    -> Result<(Value, Vec<f32>), String>
{
    let theme = themes::builtin(name)?;
    let mut presentation = calm::presentation(layout,state,&theme,13.0,(-1.0,-1.0),false,&mut clanky_render::measure_text);
    if fault {
        let inset = calm::inset(layout).array();
        let shape = presentation.scene.layers.iter_mut().find_map(|layer| match layer {
            Layer::Shape(shape) if shape.rect == inset => Some(shape), _ => None,
        }).ok_or("missing inset shape for planted fault")?;
        shape.radius = 0.0;
    }
    let sizes = presentation.scene.layers.iter().filter_map(|layer| match layer {
        Layer::Text(run) => Some(run.px), _ => None,
    }).collect();
    let rgba = offscreen_scene(&presentation.scene,1240,760)?;
    fs::write(output.join(format!("{name}.rgba")),&rgba).map_err(|e| e.to_string())?;
    let coords = [(128,700),(257,9),(264,16),(600,63),(939,250),(939,600),(930,390),(968,200),(128,20),(128,740)];
    let mut result = serde_json::Map::new();
    for (x,y) in coords { result.insert(format!("{x},{y}"),json!(sample(&rgba,x,y))); }
    Ok((Value::Object(result),sizes))
}
fn run() -> Result<(), String> {
    let output = env::args().nth(1).ok_or("missing output directory")?;
    let output = Path::new(&output);
    let tree = Tree::branch(Axis::Rows,Tree::leaf("A"),Tree::leaf("B"));
    let shown = calm::layout([1240,760],1.0,true,false,&tree,[8,20]);
    let hidden = calm::layout([1240,760],1.0,false,false,&tree,[8,20]);
    let plain = calm::layout([1240,760],1.0,true,true,&tree,[8,20]);
    let moved = calm::drag_divider(&shown,0,300.0)?;
    let engine = Session(Engine::new());
    let pty_a = pty_size(&engine.0,shown.pane("A").ok_or("pane A")?)?;
    let pty_b = pty_size(&engine.0,shown.pane("B").ok_or("pane B")?)?;
    let (restored,restored_a,restored_b) = restored_layout(output,moved.ratio)?;
    let mut a = PaneState::new("A",snapshot_from_chunks(13,81,&[]).map_err(|e| e.message)?);
    a.title = "~/git/clanky".into();
    let mut b = PaneState::new("B",snapshot_from_chunks(13,81,&[b"cargo build"]).map_err(|e| e.message)?);
    b.title = "cargo build".into();
    b.blocks.push(BlockCard{id:"build".into(),first_row:0,last_row:0,command:"cargo build".into(),exit_code:Some(0),duration_ms:Some(41_170),running:false});
    let state = LookState { focused:Some("B".into()),panes:vec![a,b],opacity:1.0,..Default::default() };
    let fault = env::var("CLANKY_GATE_FAULT").as_deref() == Ok("look");
    let (dark,sizes) = render(&shown,&state,"stone-dark",output,fault)?;
    let (light,_) = render(&shown,&state,"stone-light",output,false)?;
    let (bedazzled,_) = render(&shown,&state,"ai-bedazzled",output,false)?;
    println!("{}",json!({
        "layout":{"inset":calm::inset(&shown).array(),"a":pane(&shown,"A")?,"b":pane(&shown,"B")?,"pty_a":pty_a,"pty_b":pty_b},
        "hidden":{"inset":calm::inset(&hidden).array(),"a":pane(&hidden,"A")?},
        "drag":{"a":pane(&moved.layout,"A")?,"b":pane(&moved.layout,"B")?,"restored_a":pane(&restored,&restored_a)?,"restored_b":pane(&restored,&restored_b)?},
        "post_ai":{"a":pane(&plain,"A")?},
        "pixels":{"dark":dark,"light":light,"bedazzled":bedazzled},"ui_text_px":sizes
    }));
    Ok(())
}
fn main() { if let Err(error)=run() { eprintln!("FAIL: M12 observations: {error}"); std::process::exit(1); } }
