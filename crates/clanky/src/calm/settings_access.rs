//! Readable agent permissions and the shortcuts currently available in the calm UI.
use super::{scene::Paint,settings_frame::{self,Action,Geometry,Hit}};
use crate::{look::PixelRect,themes::Theme};
use clanky_render::{Layer,TextRun,TextMetrics};
#[derive(Clone)]
pub struct PaneAccess{pub id:String,pub label:String,pub allowed:bool}
fn r(x:f32,y:f32,w:f32,h:f32)->PixelRect{PixelRect{x,y,w,h}}
fn clipped(hits:&mut Vec<Hit>,g:&Geometry){hits.retain_mut(|hit|{let top=hit.rect.y.max(g.body.y);let bottom=(hit.rect.y+hit.rect.h).min(g.body.y+g.body.h);hit.rect.y=top;hit.rect.h=(bottom-top).max(0.0);hit.rect.h>0.0});}
pub(super) fn agents(window:[u32;2],g:&Geometry,t:&Theme,panes:&[PaneAccess],masking_ready:bool,scroll:f32,pointer:(f64,f64),pressed:bool,measure:&mut dyn FnMut(&TextRun)->TextMetrics)->(Vec<Layer>,Vec<Hit>,f32){
    let mut p=Paint::empty(t,measure,g.scale,window,pointer,pressed);let s=g.scale;let mut hits=vec![];let mut y=65.0-scroll;
    p.out.scene.layers.push(Layer::Clip(Some(g.body.array())));
    let control=settings_frame::row(&mut p,g,t,y,80.0,"Secret masking","Masks secrets before agents read terminal output.",false);
    p.text(control,if masking_ready{"On"}else{"Checking"},13.0*s,t.colors.muted_foreground,500,false);y+=68.84375;
    for(name,description,label,action)in[("Stop agent access","Revoke access and cancel pending agent waits.","Stop agents",Action::StopAgents),("Resume agent access","Allow new agents; grant pane permissions separately.","Resume",Action::ResumeAgents)]{
        let w=p.width(label,13.0*s,500)/s+24.0;let control=settings_frame::row(&mut p,g,t,y,w,name,description,false);
        p.shape(control,8.0*s,if control.contains(pointer.0,pointer.1){if pressed{t.colors.pressed}else{t.colors.accent}}else{t.colors.background},Some(t.colors.input));let width=p.width(label,13.0*s,500);p.text(r(control.x+(control.w-width)/2.0,control.y,width,control.h),label,13.0*s,t.colors.foreground,500,false);hits.push(Hit{rect:control,action});y+=68.84375;
    }
    for (index,pane) in panes.iter().enumerate(){let control=settings_frame::row(&mut p,g,t,y,32.0,&pane.label,"Allow this pane to accept agent commands.",index+1==panes.len());let sw=r(control.x,control.y+7.0*s,32.0*s,18.0*s);
        p.shape(sw,9.0*s,if pane.allowed{t.colors.primary}else{t.colors.input},None);p.shape(r(sw.x+if pane.allowed{16.0*s}else{2.0*s},sw.y+2.0*s,14.0*s,14.0*s),7.0*s,if pane.allowed{t.colors.primary_foreground}else{t.colors.background},None);
        hits.push(Hit{rect:g.local(229.0,y,522.0,68.84375),action:Action::Access(pane.id.clone())});y+=68.84375;
    }
    clipped(&mut hits,g);p.out.scene.layers.push(Layer::Clip(None));(p.out.scene.layers,hits,(y+scroll+24.0-543.0).max(0.0))
}
pub(super) fn keys(window:[u32;2],g:&Geometry,t:&Theme,post_ai:bool,scroll:f32,measure:&mut dyn FnMut(&TextRun)->TextMetrics)->(Vec<Layer>,f32){
    let mut p=Paint::empty(t,measure,g.scale,window,(-1.0,-1.0),false);let s=g.scale;let mut y=65.0-scroll;
    p.out.scene.layers.push(Layer::Clip(Some(g.body.array())));
    let entries:[(&str,&str,&[&str],bool);13]=[
        ("Settings","Open or close this panel.",&["Ctrl",","],false),
        ("New tab","Start a shell in a new session.",&["Ctrl","Shift","T"],false),
        ("Split columns","Put a new pane beside the current one.",&["Ctrl","Shift","D"],false),
        ("Split rows","Put a new pane below the current one.",&["Ctrl","Shift","E"],false),
        ("Close pane","Close the current pane.",&["Ctrl","Shift","W"],false),
        ("Next session","Switch to the next session.",&["Ctrl","Tab"],false),
        ("Side columns","Show or hide the side columns.",&["Ctrl","Shift","B"],false),
        ("Larger text","Increase text size for this window.",&["Ctrl","+"],false),
        ("Smaller text","Decrease text size for this window.",&["Ctrl","−"],false),
        ("Reset zoom","Return to the saved text size.",&["Ctrl","0"],false),
        ("Pane agent access","Toggle agent access for the focused pane.",&["Ctrl","Shift","A"],true),
        ("Stop agents","Revoke agent access and cancel pending waits.",&["Ctrl","Shift","Esc"],true),
        ("Close Settings","Cancel an edit first, or close this panel.",&["Esc"],false),
    ];
    for(label,description,keys,agent)in entries{if post_ai && agent{continue;}let width=keys.iter().map(|key|p.mono_width(key,12.0*s,400)/s+10.0).sum::<f32>()+4.0*(keys.len()-1) as f32;
        let control=settings_frame::row(&mut p,g,t,y,width,label,description,false);p.kbd(control.x+control.w,control.y+6.0*s,keys);y+=68.84375;
    }
    p.out.scene.layers.push(Layer::Clip(None));(p.out.scene.layers,(y+scroll+24.0-543.0).max(0.0))
}
