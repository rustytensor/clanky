//! Live controller for rebuilt pages. No legacy form is rendered or opened.
use super::{settings_frame::{self,Action,Hit,Section},scene::Paint};
use crate::{config::Config,look::{Layout,PixelRect},settings::Settings,themes::Theme};
use clanky_render::{Layer,TextMetrics,TextRun};
use serde_json::json;
use winit::keyboard::{Key,NamedKey};
use super::settings_appearance::{self,Editor,EditKind};
use crate::theme_store::ThemeStore;
use super::settings_access;
use super::settings_layout::{PickItem,PickKind};
pub use super::settings_access::PaneAccess;
use std::path::PathBuf;
pub enum ThemeAction{Save{slug:String,theme:Theme},Import{path:PathBuf,slug:String},Export{id:String,path:PathBuf}}
pub enum Effect{None,Changed,Open(PathBuf),Access(String),StopAgents,ResumeAgents,Theme(ThemeAction),
    /// The highlighted theme changed: redraw with its preview (nothing is saved).
    Preview}
pub struct SettingsPanel {
    pub store:Settings,pub open:bool,pub error:Option<String>,pub forced_post_ai:bool,
    section:Section,hits:Vec<Hit>,focus:Option<usize>,keyboard_focus:bool,menu:bool,slider:Option<PixelRect>,effective:Config,
    editor:Option<Editor>,scroll:f32,max_scroll:f32,active_theme:Option<Theme>,system_light:bool,
    /// Installed monospace families for the font menu (the window fills it from its fonts), and its scroll.
    pub mono_fonts:Vec<String>,menu_scroll:usize,
    /// Theme picker: the rows last drawn, the keyboard highlight, its unsaved preview, and Ptyxis palettes.
    picks:Vec<PickItem>,highlight:Option<usize>,preview:(Option<Theme>,Option<crate::config::TerminalColors>),palettes:Option<Vec<(String,serde_json::Value)>>,
}
/// Layout keys and the value Clanky leaves out of the file (the default).
const LAYOUT_DEFAULTS:[(&str,&str);5]=[("tab_style","horizontal"),("tools_panel","on"),("command_input","editor"),("block_style","quiet"),("status_line","show")];
/// Font menu rows shown at once; the wheel scrolls the rest.
const MENU_ROWS:usize=10;
/// terminal_colors as the settings file stores it (only the colours that are set).
fn terminal_colors_json(t:&crate::config::TerminalColors)->serde_json::Value{
    let hex=|c:crate::themes::Color|settings_appearance::hex([c[0],c[1],c[2],255]);
    let mut value=json!({});
    for(key,color)in[("background",t.background),("foreground",t.foreground),("cursor",t.cursor)]{if let Some(color)=color{value[key]=json!(hex(color));}}
    if let Some(ansi)=t.ansi{value["ansi"]=json!(ansi.iter().map(|c|hex(*c)).collect::<Vec<_>>());}
    value
}
impl SettingsPanel{
    /// The bundled terminal fonts first, then installed monospace families.
    fn fonts(&self)->Vec<String>{
        let mut out:Vec<String>=["Geist Mono","JetBrains Mono","DejaVu Sans Mono"].map(String::from).into();
        for name in &self.mono_fonts{if !out.iter().any(|f|f.eq_ignore_ascii_case(name)){out.push(name.clone());}}
        out
    }
    pub fn new(store:Settings)->Self{let effective=store.config().clone();Self{store,open:false,error:None,forced_post_ai:false,section:Section::General,hits:vec![],focus:None,keyboard_focus:false,menu:false,slider:None,effective,editor:None,scroll:0.0,max_scroll:0.0,active_theme:None,system_light:false,mono_fonts:Vec::new(),menu_scroll:0,picks:Vec::new(),highlight:None,preview:(None,None),palettes:None}}
    pub fn toggle(&mut self){self.open=!self.open;self.keyboard_focus=false;self.menu=false;self.focus=None;self.slider=None;self.editor=None;self.end_preview();self.palettes=None;}
    fn end_preview(&mut self){self.highlight=None;self.preview=(None,None);}
    /// The theme or terminal colours shown behind Settings while a picker row is highlighted.
    pub fn preview_theme(&self)->Option<&Theme>{self.preview.0.as_ref()}
    pub fn preview_terminal(&self)->Option<&crate::config::TerminalColors>{self.preview.1.as_ref()}
    /// A Layout choice: a default value is left out of the file; tools_panel also drops the old rails key.
    fn save_layout(&mut self,key:&str,value:serde_json::Value)->Effect{
        let default=match key{"tools"=>json!(crate::config::TOOLS.iter().map(|t|t.1).collect::<Vec<_>>()),_=>LAYOUT_DEFAULTS.iter().find(|d|d.0==key).map(|d|json!(d.1)).unwrap_or(serde_json::Value::Null)};
        let mut changes=vec![(key,(value!=default).then_some(value))];
        if key=="tools_panel"{changes.push(("rails",None));}
        match self.store.set_many(&changes){Ok(_)=>{self.error=None;Effect::Changed},Err(error)=>{self.error=Some(error);Effect::None}}
    }
    /// Save a picker row: a theme goes in its own appearance's slot (and sets the mode when not
    /// following the system); a Ptyxis palette becomes the terminal colours.
    fn pick(&mut self,index:usize)->Effect{
        self.end_preview();
        let Some(item)=self.picks.get(index).cloned() else{return Effect::None;};
        let result=match item.kind{
            PickKind::Theme{id,appearance,..}=>{
                let slot=if appearance==crate::themes::Appearance::Light{"theme_light"}else{"theme_dark"};
                if self.effective.theme==crate::config::ThemePreference::System{self.store.set(slot,json!(id)).map(|_|())}
                else{self.store.set_many(&[("theme",Some(json!(if appearance==crate::themes::Appearance::Light{"light"}else{"dark"}))),(slot,Some(json!(id)))]).map(|_|())}
            }
            PickKind::Palette{value,..}=>self.store.set("terminal_colors",value).map(|_|()),
        };
        match result{Ok(())=>{self.error=None;Effect::Changed},Err(error)=>{self.error=Some(error);Effect::None}}
    }
    /// Move the picker highlight one row (the first move starts from the saved theme) and preview it.
    fn step(&mut self,down:bool)->Effect{
        if self.picks.is_empty(){return Effect::None;}
        let saved=self.effective.selected_theme_id(self.system_light).to_owned();
        let start=self.highlight.or_else(||self.picks.iter().position(|item|matches!(&item.kind,PickKind::Theme{id,..} if *id==saved)));
        let last=self.picks.len()-1;
        let next=match (start,down){(Some(i),true)=>(i+1).min(last),(Some(i),false)=>i.saturating_sub(1),(None,true)=>0,(None,false)=>last};
        self.highlight=Some(next);
        self.preview=match &self.picks[next].kind{PickKind::Theme{theme,..}=>(Some((**theme).clone()),None),PickKind::Palette{colors,..}=>(None,Some(colors.clone()))};
        // Keep the highlighted row inside the page body, and show the whole list at once when it
        // fits (mockup section 5: every row and the hint under them): first the list with its
        // column heads, Ptyxis palettes and hint, else the built-in themes with their heads.
        let y=self.picks[next].y;let (top,bottom)=(57.0+8.0,543.0-8.0);
        let heads=if self.effective.theme==crate::config::ThemePreference::System{22.0}else{0.0};
        let first=self.picks.iter().map(|item|item.y).fold(f32::MAX,f32::min)-heads;
        let all=self.picks.iter().map(|item|item.y+38.0).fold(first,f32::max)+50.0;
        let themes=self.picks.iter().filter(|item|matches!(item.kind,PickKind::Theme{..})).map(|item|item.y+38.0).fold(first,f32::max);
        let span=[(first,all),(first,themes),(y,y+38.0)].into_iter().find(|(a,b)|b-a<=bottom-top).unwrap_or((y,y+38.0));
        if span.0-self.scroll<top{self.scroll=span.0-top;}else if span.1-self.scroll>bottom{self.scroll=span.1-bottom;}
        self.scroll=self.scroll.max(0.0).min(self.max_scroll.max(y+38.0-bottom));
        Effect::Preview
    }
    pub fn theme_complete(&mut self){self.editor=None;self.error=None;}
    fn save(&mut self,key:&str,value:serde_json::Value)->Effect{
        match self.store.set(key,value){Ok(_)=>{self.error=None;Effect::Changed},Err(error)=>{self.error=Some(error);Effect::None}}
    }
    fn activate(&mut self,action:Action,x:f64)->Effect{match action{
        Action::Section(section)=>{self.section=section;self.menu=false;self.focus=None;self.scroll=0.0;self.end_preview();self.palettes=None;Effect::None},
        Action::Choice(key,value)=>self.save_layout(key,json!(value)),
        Action::ToolChip(tool)=>{
            let mut tools=self.effective.tools.clone();
            if let Some(i)=tools.iter().position(|t|*t==tool){tools.remove(i);}else{tools.push(tool);}
            let list:Vec<_>=crate::config::TOOLS.iter().filter(|t|tools.contains(&t.0)).map(|t|t.1).collect();
            self.save_layout("tools",json!(list))
        },
        Action::Follow(on)=>{self.end_preview();self.save("theme",json!(if on{"system"}else if self.system_light{"light"}else{"dark"}))},
        Action::Pick(index)=>self.pick(index),
        Action::Close=>{self.open=false;self.menu=false;Effect::None},
        Action::OpenFile=>Effect::Open(self.store.path().to_owned()),
        Action::FontMenu=>{self.menu=!self.menu;self.menu_scroll=self.fonts().iter().position(|f|*f==self.effective.font_family).unwrap_or(0).saturating_sub(4);self.focus=None;Effect::None},
        Action::CellHeight(delta)=>self.save("cell_height_scale",json!((((self.effective.cell_height_scale+delta)*20.0).round()/20.0).clamp(1.0,2.0))),
        Action::Blink(mode)=>self.save("cursor_blink",json!(mode)),
        Action::TerminalColor(key)=>{let t=&self.effective.terminal_colors;let c=self.active_theme.as_ref().map(|t|t.colors.clone());
            let color=match key{"background"=>t.background.or(c.as_ref().map(|c|c.background)),"foreground"=>t.foreground.or(c.as_ref().map(|c|c.foreground)),_=>t.cursor.or(t.foreground).or(c.as_ref().map(|c|c.foreground))};
            if let Some(color)=color{self.editor=Some(Editor::new(EditKind::TerminalColor(key),vec![settings_appearance::hex([color[0],color[1],color[2],255])]));self.error=None;}Effect::None},
        Action::ThemeTerminalColors=>{match self.store.reset("terminal_colors"){Ok(_)=>{self.error=None;Effect::Changed},Err(error)=>{self.error=Some(error);Effect::None}}},
        Action::ImportPalette=>{let dir=std::env::var_os("HOME").map(|home|PathBuf::from(home).join(".local/share/org.gnome.Ptyxis/palettes/")).unwrap_or_default();
            self.editor=Some(Editor::new(EditKind::Palette,vec![dir.to_string_lossy().into_owned()]));self.error=None;Effect::None},
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
            EditKind::TerminalColor(key)=>{
                let color=match crate::themes::parse_color(editor.values[0].trim()){Ok(c)=>c,Err(_)=>{self.error=Some("Enter a colour as #rrggbb.".into());return Effect::None;}};
                let mut value=terminal_colors_json(&self.effective.terminal_colors);
                value[*key]=json!(settings_appearance::hex([color[0],color[1],color[2],255]));
                match self.store.set("terminal_colors",value){Ok(_)=>{self.editor=None;self.error=None;Effect::Changed},Err(error)=>{self.error=Some(error);Effect::None}}
            }
            EditKind::Palette=>{
                let path=editor.values[0].trim();
                let imported=std::fs::read_to_string(path).map_err(|e|format!("{path}: {e}")).and_then(|text|crate::config::import_ptyxis_palette(&text));
                match imported.and_then(|value|self.store.set("terminal_colors",value).map(|_|())){Ok(())=>{self.editor=None;self.error=None;Effect::Changed},Err(error)=>{self.error=Some(error);Effect::None}}
            }
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
            Key::Named(NamedKey::Escape)=>{if self.menu{self.menu=false;Effect::None}else if self.highlight.is_some(){self.end_preview();Effect::Preview}else{self.open=false;Effect::None}},
            Key::Named(NamedKey::ArrowDown)|Key::Named(NamedKey::ArrowUp) if self.section==Section::Appearance && !self.menu=>self.step(*key==Key::Named(NamedKey::ArrowDown)),
            Key::Named(NamedKey::Enter) if self.highlight.is_some()=>{let index=self.highlight.unwrap_or(0);self.pick(index)},
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
    pub fn wheel(&mut self,lines:i32){
        if self.menu{let n=self.fonts().len();self.menu_scroll=(self.menu_scroll as i64-lines as i64).clamp(0,n.saturating_sub(MENU_ROWS) as i64) as usize;return;}
        if self.editor.is_none() && matches!(self.section,Section::Appearance|Section::Terminal|Section::Agents|Section::Keys){self.scroll=(self.scroll-lines as f32*48.0).clamp(0.0,self.max_scroll);self.focus=None;}}
    pub fn ime_position(&self)->Option<[f32;2]>{None}
    pub fn animation_deadline(&self)->Option<std::time::Instant>{None}
    pub fn draw(&mut self,layout:&Layout,theme:&Theme,config:&Config,access:&[PaneAccess],masking_ready:bool,themes:Option<&ThemeStore>,system_light:bool,pointer:(f64,f64),pressed:bool,measure:&mut dyn FnMut(&TextRun)->TextMetrics)->Vec<Layer>{
        if !self.open{return vec![];}if config.post_ai && self.section==Section::Agents{self.section=Section::General;self.scroll=0.0;}self.effective=config.clone();self.active_theme=Some(theme.clone());self.system_light=system_light;
        let s=layout.scale as f32;let mut frame=settings_frame::draw_scrolled(layout.window_px,s,self.section,theme,config,pointer,pressed,measure,if self.section==Section::Terminal{self.scroll}else{0.0});
        if self.section==Section::Terminal{self.max_scroll=frame.max_scroll;self.scroll=self.scroll.min(self.max_scroll);}
        if self.section==Section::Appearance{if let Some(store)=themes{if self.palettes.is_none(){self.palettes=Some(super::settings_layout::scan_palettes());}
            let palettes=self.palettes.clone().unwrap_or_default();
            match settings_appearance::page(layout.window_px,&frame.geometry,theme,config,store,system_light,self.scroll,pointer,pressed,measure,&palettes,self.highlight){Ok((layers,hits,max,picks))=>{frame.scene.layers.extend(layers);frame.hits.extend(hits);self.max_scroll=max;self.scroll=self.scroll.min(max);self.picks=picks;},Err(error)=>self.error=Some(error)}}else{self.error=Some("Theme storage is unavailable.".into());}}
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
                let fonts=self.fonts();let shown=fonts.len().min(MENU_ROWS);let first=self.menu_scroll.min(fonts.len().saturating_sub(shown));
                let menu=PixelRect{x:anchor.x,y:anchor.y+anchor.h+4.0*s,w:anchor.w,h:(8.0+32.0*shown as f32)*s};p.shape(menu,8.0*s,theme.colors.card,Some(theme.colors.input));
                for (i,name) in fonts.iter().skip(first).take(shown).enumerate(){let name=name.as_str();let rect=PixelRect{x:menu.x+4.0*s,y:menu.y+4.0*s+i as f32*32.0*s,w:menu.w-8.0*s,h:32.0*s};
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
