//! M12 observation harness. Incomplete checks remain absent, never fabricated.
use clanky::{calm, look::{BlockCard, LookState, PaneState}, themes};
use clanky_core::{layout::{Axis, Layout as Tree}, snapshot_from_chunks};
use clanky_render::{Layer, offscreen_scene};
use serde_json::{Value, json};
use std::{env, fs, path::Path};

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
        "layout":{"inset":calm::inset(&shown).array(),"a":pane(&shown,"A")?,"b":pane(&shown,"B")?},
        "hidden":{"inset":calm::inset(&hidden).array(),"a":pane(&hidden,"A")?},
        "drag":{"a":pane(&moved.layout,"A")?,"b":pane(&moved.layout,"B")?},
        "post_ai":{"a":pane(&plain,"A")?},
        "pixels":{"dark":dark,"light":light,"bedazzled":bedazzled},"ui_text_px":sizes
    }));
    Ok(())
}
fn main() { if let Err(error)=run() { eprintln!("FAIL: M12 observations: {error}"); std::process::exit(1); } }
