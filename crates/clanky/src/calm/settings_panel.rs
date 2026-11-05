//! Live controller for rebuilt pages. No legacy form is rendered or opened.
use super::{settings_frame::{self,Action,Hit,Section},scene::Paint};
use crate::{config::Config,look::{Layout,PixelRect},settings::Settings,themes::Theme};
use clanky_render::{Layer,TextMetrics,TextRun};
use serde_json::json;
use winit::keyboard::{Key,NamedKey};
pub use crate::settings_panel::Effect;
pub struct SettingsPanel {
    pub store:Settings,pub open:bool,pub error:Option<String>,
    section:Section,hits:Vec<Hit>,focus:Option<usize>,menu:bool,slider:Option<PixelRect>,effective:Config,
}
impl SettingsPanel{
    pub fn new(store:Settings)->Self{let effective=store.config().clone();Self{store,open:false,error:None,section:Section::General,hits:vec![],focus:None,menu:false,slider:None,effective}}
    pub fn toggle(&mut self){self.open=!self.open;self.menu=false;self.focus=None;self.slider=None;}
    fn save(&mut self,key:&str,value:serde_json::Value)->Effect{
        match self.store.set(key,value){Ok(_)=>{self.error=None;Effect::Changed},Err(error)=>{self.error=Some(error);Effect::None}}
    }
    fn activate(&mut self,action:Action,x:f64)->Effect{match action{
        Action::Section(section)=>{self.section=section;self.menu=false;self.focus=None;Effect::None},
        Action::Close=>{self.open=false;self.menu=false;Effect::None},
        Action::OpenFile=>Effect::Open(self.store.path().to_owned()),
        Action::FontMenu=>{self.menu=!self.menu;self.focus=None;Effect::None},
        Action::FontChoice(name)=>{self.menu=false;self.save("font_family",json!(name))},
        Action::FontSize(delta)=>self.save("font_size",json!((self.effective.font_size+delta).clamp(8.0,32.0))),
        Action::Toggle(key,on)=>self.save(key,json!(on)),
        Action::Opacity=>{self.slider=self.hits.iter().find(|h|h.action==Action::Opacity).map(|h|h.rect);self.drag(x)},
    }}
    pub fn click(&mut self,x:f64,y:f64)->Effect{
        let hit=self.hits.iter().rposition(|h|h.rect.contains(x,y));
        if let Some(i)=hit{let action=self.hits[i].action.clone();if self.menu && !matches!(action,Action::FontChoice(_)|Action::FontMenu){self.menu=false;return Effect::None;}
            self.focus=Some(i);self.activate(action,x)
        }else{self.menu=false;Effect::None}
    }
    pub fn drag(&mut self,x:f64)->Effect{
        let Some(rect)=self.slider else{return Effect::None;};
        // Hit area includes the thumb's half-width at either end.
        let inset=rect.h/4.0;let t=((x as f32-rect.x-inset)/(rect.w-2.0*inset)).clamp(0.0,1.0);
        let opacity=((0.5+t*0.5)*100.0).round()/100.0;
        if (self.store.config().opacity-opacity).abs()<0.001{return Effect::None;}
        self.save("opacity",json!(opacity))
    }
    pub fn release(&mut self){self.slider=None;}
    pub fn key(&mut self,key:&Key,_text:Option<&str>,composing:bool)->Effect{
        if composing{return Effect::None;}
        match key{
            Key::Named(NamedKey::Escape)=>{if self.menu{self.menu=false;}else{self.open=false;}Effect::None},
            Key::Named(NamedKey::Tab)=>{if !self.hits.is_empty(){self.focus=Some(self.focus.map_or(0,|i|(i+1)%self.hits.len()));}Effect::None},
            Key::Named(NamedKey::Enter)|Key::Named(NamedKey::Space)=>{if let Some(hit)=self.focus.and_then(|i|self.hits.get(i)).cloned(){self.activate(hit.action,hit.rect.x as f64+hit.rect.w as f64/2.0)}else{Effect::None}},
            Key::Named(NamedKey::ArrowLeft)|Key::Named(NamedKey::ArrowRight)=>{
                if self.focus.and_then(|i|self.hits.get(i)).is_some_and(|h|h.action==Action::Opacity){let delta=if *key==Key::Named(NamedKey::ArrowRight){0.01}else{-0.01};self.save("opacity",json!((self.effective.opacity+delta).clamp(0.5,1.0)))}else{Effect::None}
            },
            _=>Effect::None,
        }
    }
    // These pages contain no text-entry controls. IME/paste cannot reach a shell.
    pub fn insert(&mut self,_text:&str){}
    pub fn preedit(&mut self,_text:&str){}
    pub fn select_all(&mut self){}
    pub fn wheel(&mut self,_lines:i32){}
    pub fn ime_position(&self)->Option<[f32;2]>{None}
    pub fn animation_deadline(&self)->Option<std::time::Instant>{None}
    pub fn draw(&mut self,layout:&Layout,theme:&Theme,config:&Config,pointer:(f64,f64),pressed:bool,measure:&mut dyn FnMut(&TextRun)->TextMetrics)->Vec<Layer>{
        if !self.open{return vec![];}self.effective=config.clone();
        let s=layout.scale as f32;let mut frame=settings_frame::draw_live(layout.window_px,s,self.section,theme,config,pointer,pressed,measure);
        let mut p=Paint::empty(theme,measure,s,layout.window_px,pointer,pressed);
        if self.menu {
            if let Some(anchor)=frame.hits.iter().find(|h|h.action==Action::FontMenu).map(|h|h.rect){
                let menu=PixelRect{x:anchor.x,y:anchor.y+anchor.h+4.0*s,w:anchor.w,h:104.0*s};p.shape(menu,8.0*s,theme.colors.card,Some(theme.colors.input));
                for (i,name) in ["Geist Mono","JetBrains Mono","DejaVu Sans Mono"].into_iter().enumerate(){let rect=PixelRect{x:menu.x+4.0*s,y:menu.y+4.0*s+i as f32*32.0*s,w:menu.w-8.0*s,h:32.0*s};
                    if name==config.font_family || rect.contains(pointer.0,pointer.1){p.shape(rect,6.0*s,theme.colors.accent,None);}
                    p.text(PixelRect{x:rect.x+6.0*s,w:rect.w-12.0*s,..rect},name,13.0*s,theme.colors.foreground,400,false);
                    frame.hits.push(Hit{rect,action:Action::FontChoice(name.into())});
                }
            }
        }
        if let Some(error)=&self.error{p.text(frame.geometry.local(229.0,505.0,522.0,32.0),error,13.0*s,theme.colors.err,400,false);}
        if let Some(rect)=self.focus.and_then(|i|frame.hits.get(i)).map(|h|h.rect){p.shape(PixelRect{x:rect.x-4.0*s,y:rect.y-4.0*s,w:rect.w+8.0*s,h:rect.h+8.0*s},8.0*s,[0;4],Some(theme.colors.ring));}
        self.hits=frame.hits;frame.scene.layers.extend(p.out.scene.layers);frame.scene.layers
    }
}
