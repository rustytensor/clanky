//! Observe the live Settings controller's layout and scenes without a window.
use clanky::{calm::{self,settings_frame::{self,Action,Section},settings_panel::SettingsPanel}, config::Config,
             settings::Settings,theme_store::ThemeStore,themes};
use clanky_core::layout::{Axis,Layout as Tree};
use clanky_render::{Layer,Scene,offscreen_scene};
use serde_json::json;
use std::{env,path::Path};

fn run()->Result<(),String>{
    let directory=env::args().nth(1).ok_or("missing fixture directory")?;
    let directory=Path::new(&directory);
    let tree=Tree::branch(Axis::Rows,Tree::leaf("A"),Tree::leaf("B"));
    let layout=calm::layout([1240,760],1.0,true,false,&tree,[8,20]);
    let config=Config::window_defaults();
    let geometry=settings_frame::geometry([1240,760],1.0);
    let store=ThemeStore::open(directory.join("themes"))?;
    let mut pixels=serde_json::Map::new();let mut labels=Vec::new();let mut row_heights=Vec::new();let mut row_gaps=Vec::new();let mut radii=Vec::new();
    for id in ["stone-dark","stone-light"]{
        let theme=themes::builtin(id)?;
        let mut panel=SettingsPanel::new(Settings::open(directory.join(format!("{id}.json")))?);
        panel.toggle();
        for (index,section) in Section::ALL.into_iter().enumerate(){
            // Draw before click so the controller uses its real navigation hits.
            let _=panel.draw(&layout,&theme,&config,&[],true,Some(&store),false,(-1.0,-1.0),false,&mut clanky_render::measure_text);
            let _=panel.click(270.0,154.0+34.0*index as f64);
            let layers=panel.draw(&layout,&theme,&config,&[],true,Some(&store),false,(-1.0,-1.0),false,&mut clanky_render::measure_text);
            let mut row_labels=Vec::new();
            for layer in &layers {if let Layer::Text(text)=layer {
                labels.push(text.text.clone());
                if (text.origin[0]-(geometry.panel.x+229.0)).abs()<0.1 && text.px==14.0 && text.weight==500 && text.origin[1]>=geometry.body.y && text.origin[1]<geometry.body.y+geometry.body.h { row_labels.push(text.origin[1]); }
            }}
            row_labels.sort_by(f32::total_cmp);
            row_gaps.extend(row_labels.windows(2).map(|pair|pair[1]-pair[0]));
            let frame=settings_frame::draw_live([1240,760],1.0,section,&theme,&config,(-1.0,-1.0),false,&mut clanky_render::measure_text);
            for hit in frame.hits {if matches!(hit.action,Action::Toggle(..)) {row_heights.push(hit.rect.h);}}
            if index==0 {
                let radius=layers.iter().find_map(|layer|match layer{Layer::Shape(shape) if shape.rect==geometry.panel.array() && shape.border.is_some()=>Some(shape.radius),_=>None}).ok_or("missing panel shape")?;
                radii.push(radius);
                let scene=Scene{clear:[0,0,0,255],layers};let rgba=offscreen_scene(&scene,1240,760)?;
                let pixel=|x:usize,y:usize|rgba[(y*1240+x)*4..][..4].to_vec();
                pixels.insert(id.into(),json!({"nav":pixel(234,180),"header":pixel(436,90)}));
            }
        }
    }
    println!("{}",json!({"panel":geometry.panel.array(),"nav":geometry.nav.array(),"header":geometry.header.array(),"footer":geometry.footer.array(),"pixels":pixels,"row_heights":row_heights,"row_gaps":row_gaps,"panel_radii":radii,"labels":labels}));
    Ok(())
}
fn main(){if let Err(error)=run(){eprintln!("FAIL: Settings observations: {error}");std::process::exit(1);}}
