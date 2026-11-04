//! Real app layout and scene observations for the M11 gate.
use base64::{engine::general_purpose::STANDARD, Engine as _};
use clanky::{config::Config, look::{self, BlockCard, LookState, PaneState}};
use clanky_core::{Engine, layout::{Axis, Layout as Tree}, snapshot_from_chunks};
use clanky_render::{Layer, offscreen_scene};
use serde_json::{json, Value};
use std::{env, fs, path::Path, time::{Duration, Instant}};

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
    let saved = serde_json::to_vec(&dragged.workspace_layout).map_err(|e| e.to_string())?;
    let restored: Tree = serde_json::from_slice(&saved).map_err(|e| e.to_string())?;
    let restored = look::layout([1240,760],1.0,true,&restored,[10,20]);
    let engine = Session(Engine::new());
    let pty_a = pty_size(&engine.0, shown.pane("A").ok_or("pane A")?)?;
    let pty_b = pty_size(&engine.0, shown.pane("B").ok_or("pane B")?)?;
    let snap_a = snapshot_from_chunks(14,71,&[]).map_err(|e| e.message)?;
    let snap_b = snapshot_from_chunks(14,71,&[b"cargo build"]).map_err(|e| e.message)?;
    let a = PaneState::new("A",snap_a);
    let mut b = PaneState::new("B",snap_b);
    b.blocks.push(BlockCard { id:"fixture".into(),first_row:0,last_row:0,command:"cargo build".into(),exit_code:Some(0),duration_ms:Some(0),running:false });
    let mut state = LookState { focused:Some("B".into()),panes:vec![a,b], ..Default::default() };
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
    let mut light_pixels = serde_json::Map::new();
    for (x,y) in [(590,44),(950,380),(221,51),(226,56),(100,700),(1100,700),(230,451),(936,451),(215,577)] {
        light_pixels.insert(format!("{x},{y}"),json!(sample(&light,x,y)));
    }
    state.panes[0].allow_agent = true; state.panes[0].agent_running = true;
    let light_agent = offscreen_scene(&look::scene(&shown,&state),1240,760)?;
    let ease = look::ease_out_cubic;
    println!("{}",json!({
        "layout":{"content":shown.content.array(),"a":pane(&shown,"A"),"b":pane(&shown,"B"),"pty_a":pty_a,"pty_b":pty_b},
        "rails_hidden":{"a":pane(&hidden,"A"),"b":pane(&hidden,"B")},
        "drag":{"a":pane(&dragged.layout,"A"),"b":pane(&dragged.layout,"B"),"restored_a":pane(&restored,"A"),"restored_b":pane(&restored,"B")},
        "pixels":pixels,"agent_glow":sample(&agent,215,222),
        "light_pixels":light_pixels,"light_agent_glow":sample(&light_agent,215,222),
        "motion":{"cursor":[ease(100.,200.,0.,80.,true),ease(100.,200.,40.,80.,true),ease(100.,200.,80.,80.,true),ease(100.,200.,120.,80.,true)],"panel_60ms":ease(0.,1.,60.,120.,true),"cursor_disabled_0ms":ease(100.,200.,0.,80.,false),"panel_disabled_0ms":ease(0.,1.,0.,120.,false)},
        "config":{"opacity_0_3_error":Config::from_json("{\"opacity\":0.3}").is_err(),"rails_yes_error":Config::from_json("{\"rails\":\"yes\"}").is_err(),"opacity_one_pixel":sample(&opaque,590,44)}
    }));
    Ok(())
}
