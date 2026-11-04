//! Mouse/keyboard settings form. All writes go through Settings, never a second store.
use crate::{config::{Config, ThemePreference}, look::{Layout, PixelRect, Theme, COLOR_TOKENS}, settings::Settings};
use clanky_render::{Fill, Layer, Shape, Shadow, TextRun};
use serde_json::{json, Value};
use std::path::PathBuf;
use winit::keyboard::{Key, NamedKey};

#[derive(Clone)]
enum Action { Page(usize), Close, Set(String,Value), Edit(String,bool), Color(String), Reset(String),
    Slider(String,f32,f32), Import, Export, Open, Apply, Cancel, ConfirmExport, Access(String) }
#[derive(Default)]
pub enum Effect { #[default] None, Changed, Open(PathBuf), Access(String) }
struct Editing { key: String, text: String, number: bool }
pub struct SettingsPanel {
    pub store: Settings,
    pub open: bool,
    page: usize,
    scroll: usize,
    editing: Option<Editing>,
    color: Option<String>,
    confirm_export: Option<PathBuf>,
    pub error: Option<String>,
    hits: Vec<(PixelRect,Action)>,
    dragging: Option<(PixelRect,Action)>,
    preedit: String,
    theme: Theme,
}
impl SettingsPanel {
    pub fn new(store: Settings) -> Self {
        Self { store,open:false,page:0,scroll:0,editing:None,color:None,confirm_export:None,
            error:None,hits:vec![],dragging:None,preedit:String::new(),theme:Theme::dark() }
    }
    pub fn toggle(&mut self) { self.open = !self.open; self.editing=None; self.color=None; self.confirm_export=None; self.dragging=None; self.preedit.clear(); }
    pub fn wheel(&mut self, delta: i32) {
        self.scroll = (self.scroll as i32-delta).clamp(0,24) as usize;
    }
    pub fn release(&mut self) { self.dragging=None; }
    pub fn preedit(&mut self, text: &str) { self.preedit=text.chars().take(128).collect(); }
    pub fn insert(&mut self, text: &str) {
        if let Some(edit) = &mut self.editing {
            if edit.text.len()+text.len()<=4096 { edit.text.extend(text.chars().filter(|c| !c.is_control())); }
        }
        self.preedit.clear();
    }
    pub fn key(&mut self, key: &Key, text: Option<&str>, composing: bool) -> Effect {
        if *key==Key::Named(NamedKey::Escape) {
            if self.editing.take().is_none() && self.color.take().is_none() && self.confirm_export.take().is_none() { self.open=false; }
            return Effect::None;
        }
        if composing { return Effect::None; }
        if *key==Key::Named(NamedKey::Enter) && self.editing.is_some() { return self.activate(Action::Apply); }
        if *key==Key::Named(NamedKey::Backspace) {
            if let Some(edit)=&mut self.editing { edit.text.pop(); }
        } else if self.editing.is_some() {
            if let Some(text)=text { self.insert(text); }
        } else if *key==Key::Named(NamedKey::ArrowDown) { self.wheel(-1); }
        else if *key==Key::Named(NamedKey::ArrowUp) { self.wheel(1); }
        Effect::None
    }
    pub fn click(&mut self, x:f64,y:f64) -> Effect {
        let Some((rect,action))=self.hits.iter().rev().find(|(r,_)|r.contains(x,y)).cloned() else { return Effect::None; };
        if matches!(action,Action::Slider(..)) {
            self.dragging=Some((rect,action.clone()));
            return self.slide(rect,action,x);
        }
        self.activate(action)
    }
    pub fn drag(&mut self,x:f64) -> Effect {
        match self.dragging.clone() { Some((rect,action))=>self.slide(rect,action,x),None=>Effect::None }
    }
    fn slide(&mut self,rect:PixelRect,action:Action,x:f64)->Effect {
        let Action::Slider(key,min,max)=action else { return Effect::None; };
        let fraction=((x as f32-rect.x)/rect.w.max(1.0)).clamp(0.0,1.0);
        let value=if key=="opacity" { ((min+(max-min)*fraction)*100.0).round()/100.0 }
            else { (min+(max-min)*fraction).round() };
        if let Some(channel)=key.strip_prefix("rgb").and_then(|s|s.parse::<usize>().ok()) {
            let Some(token)=self.color.clone() else { return Effect::None; };
            let mut color=token_color(&self.theme,&token);
            color[channel]=value as u8;
            return self.save_color(&token,&format!("#{:02x}{:02x}{:02x}",color[0],color[1],color[2]));
        }
        self.activate(Action::Set(key,json!(value)))
    }
    fn save_color(&mut self,token:&str,color:&str)->Effect {
        match self.store.set_color(token,Some(color)) {
            Ok(_)=>{self.error=None;Effect::Changed}, Err(error)=>{self.error=Some(error);Effect::None}
        }
    }
    fn activate(&mut self,action:Action)->Effect {
        let result: Result<Effect,String>=(|| { match action {
            Action::Page(page)=>{self.page=page;self.scroll=0;self.color=None;self.editing=None;},
            Action::Close=>self.toggle(),
            Action::Set(key,value)=>{self.store.set(&key,value)?;return Ok(Effect::Changed);},
            Action::Edit(key,number)=>{let text=key.strip_prefix("color:").map(|token| {let c=token_color(&self.theme,token);format!("#{:02x}{:02x}{:02x}",c[0],c[1],c[2])}).or_else(||value(self.store.config(),&key).map(|v| if let Some(s)=v.as_str(){s.into()}else{v.to_string()})).unwrap_or_default();self.editing=Some(Editing{key,text,number});},
            Action::Color(token)=>{self.color=Some(token);self.scroll=0;},
            Action::Reset(token)=>{self.store.set_color(&token,None)?;return Ok(Effect::Changed);},
            Action::Import=>self.editing=Some(Editing{key:"@import".into(),text:String::new(),number:false}),
            Action::Export=>self.editing=Some(Editing{key:"@export".into(),text:String::new(),number:false}),
            Action::Open=>return Ok(Effect::Open(self.store.path().to_owned())),
            Action::Access(id)=>return Ok(Effect::Access(id)),
            Action::Cancel=>{self.editing=None;self.color=None;self.confirm_export=None;},
            Action::ConfirmExport=>{if let Some(path)=self.confirm_export.take(){self.store.export(&path)?;}},
            Action::Apply=>{
                let edit=self.editing.as_ref().ok_or("No setting selected")?;
                let key=edit.key.clone();let text=edit.text.clone();
                if key=="@import" {self.store.import(std::path::Path::new(&text))?;}
                else if key=="@export" {
                    if text.is_empty(){return Err("Choose an export path".into());}
                    let path=PathBuf::from(&text);
                    if path.exists(){self.confirm_export=Some(path);}else{self.store.export(&path)?;}
                    self.editing=None;return Ok(Effect::None);
                } else if let Some(token)=key.strip_prefix("color:") {self.store.set_color(token,Some(&text))?;}
                else {let v=if edit.number {serde_json::from_str(&text).map_err(|_|"Enter a JSON number")?}else{json!(text)};self.store.set(&key,v)?;}
                self.editing=None;return Ok(Effect::Changed);
            },
            Action::Slider(..)=>(),
        };Ok(Effect::None) })();
        match result {Ok(effect)=>{self.error=None;effect},Err(error)=>{self.error=Some(error);Effect::None}}
    }
    pub fn draw(&mut self,layout:&Layout,theme:&Theme,access:&[(String,bool)]) -> Vec<Layer> {
        self.hits.clear();
        self.theme=theme.clone();
        if !self.open{return vec![];}
        let scale=(layout.metrics.panel_padding/12.0) as f32;
        let pad=layout.metrics.panel_padding as f32;let row=layout.metrics.rail_row_height as f32;let font=layout.metrics.ui_font_px as f32;
        let width=(layout.metrics.panel_max_width as f32).min((layout.window_px[0] as f32-2.0*pad).max(1.0));
        let height=(layout.metrics.panel_max_height as f32).min((layout.window_px[1] as f32-2.0*pad).max(1.0));
        let panel=PixelRect{x:(layout.window_px[0] as f32-width)/2.0,y:(layout.window_px[1] as f32-height)/2.0,w:width,h:height};
        let mut out=vec![];
        out.push(Layer::Shape(Shape{rect:panel.array(),radius:layout.metrics.panel_radius as f32,fill:Fill::Solid(theme.panel),border:Some((1.0,theme.line)),shadow:Some(Shadow{offset:[0.0,4.0*scale],blur:20.0*scale,spread:0.0,color:theme.shadow})}));
        let x=panel.x+pad;let w=(width-2.0*pad).max(1.0);let mut y=panel.y+pad;
        label(&mut out,[x,y,w-row,row],"Settings",font,theme.ink);
        self.button(&mut out,PixelRect{x:x+w-row,y,w:row,h:row},"×",Action::Close,theme,font);y+=row;
        for (index,title) in ["Appearance","Terminal","Agents / access","Keys"].iter().enumerate(){
            self.button(&mut out,PixelRect{x:x+index as f32*w/4.0,y,w:w/4.0-2.0*scale,h:row},title,Action::Page(index),theme,font);
        }
        y+=row+pad;
        if width<400.0*scale || height<360.0*scale {
            label(&mut out,[x,y,w,row],"Enlarge the window to edit Settings",font,theme.dim);
            return out;
        }
        let bottom=panel.y+height-3.0*row-pad;
        if self.confirm_export.is_some(){
            label(&mut out,[x,y,w,row],"Replace the existing export file?",font,theme.ink);y+=row;
            self.button(&mut out,PixelRect{x,y,w:w/2.0,h:row},"Replace",Action::ConfirmExport,theme,font);
            self.button(&mut out,PixelRect{x:x+w/2.0,y,w:w/2.0,h:row},"Cancel",Action::Cancel,theme,font);
        }else if let Some(edit)=&self.editing {
            label(&mut out,[x,y,w,row],&format!("{} · Enter applies, Escape cancels",edit.key),font,theme.dim);y+=row;
            label(&mut out,[x,y,w,row],&format!("{}{}▏",edit.text,self.preedit),font,theme.ink);y+=row;
            self.button(&mut out,PixelRect{x,y,w:w/2.0,h:row},"Apply",Action::Apply,theme,font);
            self.button(&mut out,PixelRect{x:x+w/2.0,y,w:w/2.0,h:row},"Cancel",Action::Cancel,theme,font);
        }else if let Some(token)=self.color.clone(){
            let color=token_color(theme,&token);
            label(&mut out,[x,y,w,row],&format!("{token}: #{:02x}{:02x}{:02x}",color[0],color[1],color[2]),font,theme.ink);y+=row;
            for channel in 0..3 {self.slider(&mut out,PixelRect{x,y,w,h:row},["Red","Green","Blue"][channel],&format!("rgb{channel}"),color[channel] as f32,0.0,255.0,theme,font);y+=row;}
            self.button(&mut out,PixelRect{x,y,w:w/3.0,h:row},"Hex…",Action::Edit(format!("color:{token}"),false),theme,font);
            self.button(&mut out,PixelRect{x:x+w/3.0,y,w:w/3.0,h:row},"Reset",Action::Reset(token),theme,font);
            self.button(&mut out,PixelRect{x:x+2.0*w/3.0,y,w:w/3.0,h:row},"Back",Action::Cancel,theme,font);
        }else{
            let config=self.store.config().clone();
            let keys:Vec<&str>=match self.page {
                0=>["theme","opacity","font_family","ui_font_family","font_size","rails","animations"].into_iter().chain(COLOR_TOKENS).collect(),
                1=>vec!["columns","rows","compose_dictionary","compose_dictionary_dir","compose_send_enter","notify_after_seconds","restore_layout"],
                2=>vec!["osc52_write","osc52_read"],_=>vec![],
            };
            let count=keys.len()+if self.page==2{access.len()}else{0};
            let capacity=((bottom-y)/row).floor().max(1.0) as usize;
            self.scroll=self.scroll.min(count.saturating_sub(capacity));
            for key in keys.into_iter().skip(self.scroll).take(capacity){
                let r=PixelRect{x,y,w,h:row};
                if COLOR_TOKENS.contains(&key){
                    let c=token_color(theme,key);let swatch=PixelRect{x,y:y+4.0*scale,w:24.0*scale,h:row-8.0*scale};
                    out.push(Layer::Shape(Shape{rect:swatch.array(),radius:4.0*scale,fill:Fill::Solid(c),border:Some((1.0,theme.line)),shadow:None}));
                    self.button(&mut out,PixelRect{x:x+32.0*scale,w:w-120.0*scale,..r},&format!("{key}  #{:02x}{:02x}{:02x}",c[0],c[1],c[2]),Action::Color(key.into()),theme,font);
                    self.button(&mut out,PixelRect{x:x+w-80.0*scale,w:80.0*scale,..r},"Reset",Action::Reset(key.into()),theme,font);
                }else if key=="opacity"||key=="font_size"{
                    let v=value(&config,key).and_then(|v|v.as_f64()).unwrap_or(16.0) as f32;
                    let (min,max)=if key=="opacity"{(0.5,1.0)}else{(8.0,32.0)};
                    self.slider(&mut out,r,key,key,v,min,max,theme,font);
                }else if key=="theme"{
                    let name=match config.theme{ThemePreference::Dark=>"dark",ThemePreference::Light=>"light",ThemePreference::System=>"system"};
                    let next=match name{"dark"=>"light","light"=>"system",_=>"dark"};
                    self.button(&mut out,r,&format!("Theme: {name}  → {next}"),Action::Set(key.into(),json!(next)),theme,font);
                }else if let Some(v)=value(&config,key){
                    let text=if let Some(s)=v.as_str(){s.to_owned()}else{v.to_string()};
                    let action=if let Some(b)=v.as_bool(){Action::Set(key.into(),json!(!b))}else{Action::Edit(key.into(),v.is_number())};
                    self.button(&mut out,r,&format!("{key}: {text}"),action,theme,font);
                }
                y+=row;
            }
            if self.page==2{for (id,allowed) in access.iter().skip(self.scroll.saturating_sub(2)){
                if y+row>bottom{break;}
                self.button(&mut out,PixelRect{x,y,w,h:row},&format!("{id}: agents {}",if *allowed{"on"}else{"off"}),Action::Access(id.clone()),theme,font);y+=row;
            }}
            if self.page==3{for text in ["Ctrl+, Settings · Escape close","Ctrl+Shift+P Palette · Ctrl+Shift+B Rails","Ctrl+Shift+T New tab · D/E Split right/below","Ctrl+Tab Next tab · Ctrl+Shift+W Close pane","Ctrl+= / - / 0 Zoom / reset","Ctrl+Shift+O Open file · Ctrl+S Save","Ctrl+Z / Ctrl+Shift+Z Undo / redo","Ctrl+Shift+Enter Compose · Ctrl+Enter Send","Ctrl+Shift+A Toggle agent access","Ctrl+Shift+Escape Revoke all agent access"]{if y+row>bottom{break;}label(&mut out,[x,y,w,row],text,font,theme.ink);y+=row;}}
        }
        let y=panel.y+height-3.0*row;
        if let Some(error)=&self.error{label(&mut out,[x,y,w,row],error,font,theme.err_ink);}else{label(&mut out,[x,y,w,row],"Changes save immediately · scroll for more",font,theme.dim);}
        let y=y+row;
        for (i,(title,action)) in [("Import…",Action::Import),("Export…",Action::Export),("Open settings file",Action::Open)].into_iter().enumerate(){self.button(&mut out,PixelRect{x:x+i as f32*w/3.0,y,w:w/3.0-2.0*scale,h:row},title,action,theme,font);}
        out
    }
    fn button(&mut self,out:&mut Vec<Layer>,r:PixelRect,title:&str,action:Action,t:&Theme,font:f32){
        out.push(Layer::Shape(Shape{rect:r.array(),radius:4.0,fill:Fill::Solid(t.card),border:Some((1.0,t.line)),shadow:None}));
        label(out,[r.x+6.0,r.y+4.0,(r.w-12.0).max(0.0),r.h],title,font,t.ink);self.hits.push((r,action));
    }
    fn slider(&mut self,out:&mut Vec<Layer>,r:PixelRect,title:&str,key:&str,v:f32,min:f32,max:f32,t:&Theme,font:f32){
        label(out,[r.x,r.y,r.w*0.42,r.h],&format!("{title}: {v:.2}"),font,t.ink);
        let track=PixelRect{x:r.x+r.w*0.44,y:r.y+8.0,w:r.w*0.54,h:16.0};
        out.push(Layer::Shape(Shape{rect:track.array(),radius:8.0,fill:Fill::Solid(t.line),border:None,shadow:None}));
        let x=track.x+((v-min)/(max-min)).clamp(0.0,1.0)*track.w;
        out.push(Layer::Shape(Shape{rect:[x-4.0,track.y,8.0,track.h],radius:4.0,fill:Fill::Solid(t.accent),border:None,shadow:None}));
        self.hits.push((track,Action::Slider(key.into(),min,max)));
    }
}
fn label(out:&mut Vec<Layer>,r:[f32;4],text:&str,px:f32,color:[u8;4]){
    let count=(r[2]/px.max(1.0)).max(0.0) as usize;
    let mut used=0;let text:String=text.chars().map(|c|if c.is_control(){' '}else{c}).take_while(|c|{used+=if c.is_ascii(){1}else{2};used<=count}).collect();
    out.push(Layer::Text(TextRun{origin:[r[0],r[1]],text,px,color,weight:400,mono:false,opacity:1.0,letter_spacing:0.0}));
}
fn value(c:&Config,key:&str)->Option<Value>{Some(match key{
    "opacity"=>json!(c.opacity),"font_size"=>json!(c.font_size),"font_family"=>json!(c.font_family),"ui_font_family"=>json!(c.ui_font_family),
    "rails"=>json!(c.rails),"animations"=>json!(c.animations),"columns"=>json!(c.columns),"rows"=>json!(c.rows),
    "compose_dictionary"=>json!(c.compose_dictionary),"compose_dictionary_dir"=>json!(c.compose_dictionary_dir),"compose_send_enter"=>json!(c.compose_send_enter),
    "notify_after_seconds"=>json!(c.notify_after_seconds),"restore_layout"=>json!(c.restore_layout),"osc52_write"=>json!(c.osc52_write),"osc52_read"=>json!(c.osc52_read),
    _=>return None,
})}
fn token_color(t:&Theme,key:&str)->[u8;4]{match key{
    "window"=>t.window,"rail"=>t.rail,"panel"=>t.panel,"line"=>t.line,"ink"=>t.ink,"dim"=>t.dim,"accent"=>t.accent,
    "violet"=>t.violet,"teal"=>t.teal,"ok"=>t.ok,"err"=>t.err,"focus"=>t.focus,"ok_ink"=>t.ok_ink,"err_ink"=>t.err_ink,_=>t.ink,
}}
