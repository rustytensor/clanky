//! Live controller for rebuilt pages. No legacy form is rendered or opened.
use super::{settings_frame::{self,Action,Hit,Section},scene::Paint};
use crate::{config::Config,look::{Layout,PixelRect},settings::Settings,themes::Theme};
use clanky_render::{Layer,TextMetrics,TextRun};
use serde_json::json;
use winit::keyboard::{Key,NamedKey};
use super::settings_appearance::{self,Editor,EditKind};
use crate::theme_store::ThemeStore;
use super::settings_access;
pub use super::settings_access::PaneAccess;
use std::path::PathBuf;
pub enum ThemeAction{Save{slug:String,theme:Theme},Import{path:PathBuf,slug:String},Export{id:String,path:PathBuf}}
pub enum Effect{None,Changed,Open(PathBuf),Access(String),StopAgents,ResumeAgents,Theme(ThemeAction)}
pub struct SettingsPanel {
    pub store:Settings,pub open:bool,pub error:Option<String>,pub forced_post_ai:bool,
    section:Section,hits:Vec<Hit>,focus:Option<usize>,keyboard_focus:bool,menu:bool,slider:Option<PixelRect>,effective:Config,
    editor:Option<Editor>,scroll:f32,max_scroll:f32,active_theme:Option<Theme>,system_light:bool,
}
impl SettingsPanel{
    pub fn new(store:Settings)->Self{let effective=store.config().clone();Self{store,open:false,error:None,forced_post_ai:false,section:Section::General,hits:vec![],focus:None,keyboard_focus:false,menu:false,slider:None,effective,editor:None,scroll:0.0,max_scroll:0.0,active_theme:None,system_light:false}}
    pub fn toggle(&mut self){self.open=!self.open;self.keyboard_focus=false;self.menu=false;self.focus=None;self.slider=None;self.editor=None;}
    pub fn theme_complete(&mut self){self.editor=None;self.error=None;}
    fn save(&mut self,key:&str,value:serde_json::Value)->Effect{
        match self.store.set(key,value){Ok(_)=>{self.error=None;Effect::Changed},Err(error)=>{self.error=Some(error);Effect::None}}
    }
    fn activate(&mut self,action:Action,x:f64)->Effect{match action{
        Action::Section(section)=>{self.section=section;self.menu=false;self.focus=None;self.scroll=0.0;Effect::None},
        Action::Close=>{self.open=false;self.menu=false;Effect::None},
        Action::OpenFile=>Effect::Open(self.store.path().to_owned()),
        Action::FontMenu=>{self.menu=!self.menu;self.focus=None;Effect::None},
        Action::FontChoice(name)=>{self.menu=false;self.save("font_family",json!(name))},
        Action::FontSize(delta)=>self.save("font_size",json!((self.effective.font_size+delta).clamp(8.0,32.0))),
        Action::Toggle(key,on)=>self.save(key,json!(on)),
        Action::Opacity=>{self.slider=self.hits.iter().find(|h|h.action==Action::Opacity).map(|h|h.rect);self.drag(x)},
        Action::Mode(value)=>{self.scroll=0.0;self.save("theme",json!(value))},
        Action::Theme(id)=>{let key=if self.active_theme.as_ref().is_some_and(|t|t.appearance==crate::themes::Appearance::Light){"theme_light"}else{"theme_dark"};self.save(key,json!(id))},
        Action::Color(token)=>{if let Some(color)=self.active_theme.as_ref().and_then(|t|t.colors.get(token)){self.editor=Some(Editor::new(EditKind::Color(token),vec![settings_appearance::hex(color)]));self.error=None;}Effect::None},
        Action::ResetColor(token)=>{match self.store.set_color(token,None){Ok(_)=>{self.error=None;Effect::Changed},Err(error)=>{self.error=Some(error);Effect::None}}},
        Action::SaveTheme=>{self.editor=Some(Editor::new(EditKind::Save,vec!["My theme".into(),"my-theme".into()]));self.error=None;Effect::None},
        Action::ImportTheme|Action::ExportTheme=>{let path=self.store.path().parent().unwrap_or(std::path::Path::new(".")).join("theme-export.json").to_string_lossy().into_owned();let importing=action==Action::ImportTheme;self.editor=Some(Editor::new(if importing{EditKind::Import}else{EditKind::Export},if importing{vec![path,"imported-theme".into()]}else{vec![path]}));self.error=None;Effect::None},
        Action::Field(i)=>{if let Some(editor)=&mut self.editor{editor.field(i);}Effect::None},
        Action::Cancel=>{self.editor=None;self.error=None;Effect::None},
        Action::Confirm=>self.submit(),
        Action::Access(id)=>Effect::Access(id),
        Action::StopAgents=>Effect::StopAgents,
        Action::ResumeAgents=>Effect::ResumeAgents,
        Action::PickColor(value)=>{if let Some(editor)=&mut self.editor{editor.field(0);editor.insert(&value);}Effect::None},
    }}
    fn submit(&mut self)->Effect{
        let Some(editor)=&self.editor else{return Effect::None;};
        match &editor.kind{
            EditKind::Color(token)=>{match self.store.set_color(token,Some(editor.values[0].trim())){Ok(_)=>{self.editor=None;self.error=None;Effect::Changed},Err(error)=>{self.error=Some(error);Effect::None}}},
            EditKind::Save=>{let Some(mut theme)=self.active_theme.clone()else{return Effect::None;};theme.name=editor.values[0].trim().into();Effect::Theme(ThemeAction::Save{slug:editor.values[1].trim().into(),theme})},
            EditKind::Import=>Effect::Theme(ThemeAction::Import{path:editor.values[0].trim().into(),slug:editor.values[1].trim().into()}),
            EditKind::Export=>{if self.effective.colors.keys().any(|key|crate::themes::COLOR_TOKENS.contains(&key.as_str())){self.error=Some("Save as a new theme before exporting colour changes.".into());Effect::None}else{Effect::Theme(ThemeAction::Export{id:self.effective.selected_theme_id(self.system_light).into(),path:editor.values[0].trim().into()})}},
        }
    }
    pub fn click(&mut self,x:f64,y:f64)->Effect{
        self.keyboard_focus=false;
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
        if let Some(editor)=&mut self.editor{
            if *key==Key::Named(NamedKey::Escape){self.editor=None;self.error=None;return Effect::None;}
            if *key==Key::Named(NamedKey::Enter){if editor.focus==editor.values.len(){self.editor=None;return Effect::None;}return self.submit();}
            editor.key(key,_text);return Effect::None;
        }
        self.keyboard_focus=true;
        match key{
            Key::Named(NamedKey::PageDown)=>{self.wheel(-6);Effect::None},
            Key::Named(NamedKey::PageUp)=>{self.wheel(6);Effect::None},
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
    pub fn insert(&mut self,text:&str){if let Some(editor)=&mut self.editor{editor.insert(text);}}
    pub fn preedit(&mut self,_text:&str){}
    pub fn select_all(&mut self){if let Some(editor)=&mut self.editor{editor.select_all();}}
    pub fn wheel(&mut self,lines:i32){if self.editor.is_none() && matches!(self.section,Section::Appearance|Section::Agents|Section::Keys){self.scroll=(self.scroll-lines as f32*48.0).clamp(0.0,self.max_scroll);self.focus=None;}}
    pub fn ime_position(&self)->Option<[f32;2]>{None}
    pub fn animation_deadline(&self)->Option<std::time::Instant>{None}
    pub fn draw(&mut self,layout:&Layout,theme:&Theme,config:&Config,access:&[PaneAccess],masking_ready:bool,themes:Option<&ThemeStore>,system_light:bool,pointer:(f64,f64),pressed:bool,measure:&mut dyn FnMut(&TextRun)->TextMetrics)->Vec<Layer>{
        if !self.open{return vec![];}if config.post_ai && self.section==Section::Agents{self.section=Section::General;self.scroll=0.0;}self.effective=config.clone();self.active_theme=Some(theme.clone());self.system_light=system_light;
        let s=layout.scale as f32;let mut frame=settings_frame::draw_live(layout.window_px,s,self.section,theme,config,pointer,pressed,measure);
        if self.section==Section::Appearance{if let Some(store)=themes{match settings_appearance::page(layout.window_px,&frame.geometry,theme,config,store,system_light,self.scroll,pointer,pressed,measure){Ok((layers,hits,max))=>{frame.scene.layers.extend(layers);frame.hits.extend(hits);self.max_scroll=max;},Err(error)=>self.error=Some(error)}}else{self.error=Some("Theme storage is unavailable.".into());}}
        if self.section==Section::Agents{let(layers,hits,max)=settings_access::agents(layout.window_px,&frame.geometry,theme,access,masking_ready,self.scroll,pointer,pressed,measure);frame.scene.layers.extend(layers);frame.hits.extend(hits);self.max_scroll=max;}
        if self.section==Section::Keys{let(layers,max)=settings_access::keys(layout.window_px,&frame.geometry,theme,config.post_ai,self.scroll,measure);frame.scene.layers.extend(layers);self.max_scroll=max;}
        let mut p=Paint::empty(theme,measure,s,layout.window_px,pointer,pressed);
        if self.section==Section::General && self.forced_post_ai{
            frame.hits.retain(|hit|!matches!(hit.action,Action::Toggle("post_ai",_)));
            let mut faded=theme.colors.background;faded[3]=140;
            p.shape(frame.geometry.local(719.0,108.0,32.0,18.0),9.0*s,faded,None);
            p.text(frame.geometry.local(229.0,143.0,466.0,18.85),"Restart the server without --post-ai to turn this off.",13.0*s,theme.colors.muted_foreground,400,false);
        }
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
        if self.keyboard_focus{if let Some(hit)=self.focus.and_then(|i|frame.hits.get(i)){
            let mut rect=hit.rect;
            if matches!(hit.action,Action::Toggle(..)|Action::Access(_)) && rect.h>=60.0*s{
                let border=if matches!(hit.action,Action::Toggle("animations"|"post_ai",_)){0.0}else{s};
                rect=PixelRect{x:rect.x+rect.w-32.0*s,y:rect.y+(rect.h-border-18.0*s)/2.0,w:32.0*s,h:18.0*s};
            }
            p.shape(PixelRect{x:rect.x-4.0*s,y:rect.y-4.0*s,w:rect.w+8.0*s,h:rect.h+8.0*s},8.0*s,[0;4],Some(theme.colors.ring));
            if let Some(Layer::Shape(shape))=p.out.scene.layers.last_mut(){shape.border=Some((2.0*s,theme.colors.ring));}
        }}
        self.hits=frame.hits;frame.scene.layers.extend(p.out.scene.layers);
        if let Some(editor)=&self.editor{let(layers,hits)=editor.draw(layout.window_px,&frame.geometry,theme,self.error.as_deref(),pointer,pressed,measure);frame.scene.layers.extend(layers);self.hits=hits;}
        frame.scene.layers
    }
}
