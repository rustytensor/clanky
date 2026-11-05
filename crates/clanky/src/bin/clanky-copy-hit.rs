//! Observe the calm scene's real Copy hit target for the fixed two-pane fixture.
use clanky::{calm,look::{BlockCard,HitAction,LookState,PaneState},themes};
use clanky_core::{layout::{Axis,Layout as Tree},snapshot_from_chunks};
use serde_json::json;

fn run()->Result<(),String>{
    let duration=std::env::args().nth(1).ok_or("missing duration")?.parse::<u64>().map_err(|e|e.to_string())?;
    let tree=Tree::branch(Axis::Rows,Tree::leaf("A"),Tree::leaf("B"));
    let layout=calm::layout([1240,760],1.0,true,false,&tree,[8,20]);
    let mut b=PaneState::new("B",snapshot_from_chunks(13,81,&[b"echo fixture"]).map_err(|e|e.message)?);
    b.blocks.push(BlockCard{id:"block".into(),first_row:0,last_row:1,command:"echo fixture".into(),exit_code:Some(0),duration_ms:Some(duration),running:false});
    let mut state=LookState{focused:Some("B".into()),panes:vec![b],opacity:1.0,animations:false,..Default::default()};
    state.block_actions.insert(("B".into(),"block".into()),1.0);
    let theme=themes::builtin("stone-dark")?;
    let presentation=calm::presentation(&layout,&state,&theme,13.0,(500.0,470.0),false,&mut clanky_render::measure_text);
    let hit=presentation.hits.iter().find(|hit|matches!(&hit.action,HitAction::Block{tab_id,block_id,action} if tab_id=="B" && block_id=="block" && action=="copy")).ok_or("hovered Copy hit is missing")?;
    println!("{}",json!({"rect":hit.rect.array(),"hover":[500,470]}));Ok(())
}
fn main(){if let Err(error)=run(){eprintln!("FAIL: M12 Copy hit: {error}");std::process::exit(1);}}
