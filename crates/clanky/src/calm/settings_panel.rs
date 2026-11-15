//! Live controller for rebuilt pages. No legacy form is rendered or opened.
use super::{settings_frame::{self,Action,Hit,Section},scene::Paint};
use crate::{config::Config,look::{Layout,PixelRect},settings::Settings,themes::Theme};
use clanky_render::{Layer,TextMetrics,TextRun};
use serde_json::json;
use winit::keyboard::{Key,NamedKey};
use super::settings_appearance::{self,Editor,EditKind};
use crate::theme_store::ThemeStore;
use super::settings_access;
use super::settings_schemes::{self,Picker,Row};
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
    /// The page's visible band (top and bottom, panel px) as last drawn: below the problem banner, above the footer.
    view:(f32,f32),
    /// Terminal › Colour scheme: its search text, focus, highlight and the rows last drawn.
    schemes:Picker,
}
/// Layout keys and the value Clanky leaves out of the file (the default).
const LAYOUT_DEFAULTS:[(&str,&str);5]=[("tab_style","horizontal"),("tools_panel","on"),("command_input","editor"),("block_style","quiet"),("status_line","show")];
/// Font menu rows shown at once; the wheel scrolls the rest.
const MENU_ROWS:usize=10;
impl SettingsPanel{
    /// The bundled terminal fonts first, then installed monospace families.
    fn fonts(&self)->Vec<String>{
        let mut out:Vec<String>=["Geist Mono","JetBrains Mono","DejaVu Sans Mono"].map(String::from).into();
        for name in &self.mono_fonts{if !out.iter().any(|f|f.eq_ignore_ascii_case(name)){out.push(name.clone());}}
        out
    }
    pub fn new(store:Settings)->Self{let effective=store.config().clone();Self{store,open:false,error:None,forced_post_ai:false,section:Section::General,hits:vec![],focus:None,keyboard_focus:false,menu:false,slider:None,effective,editor:None,scroll:0.0,max_scroll:0.0,active_theme:None,system_light:false,mono_fonts:Vec::new(),menu_scroll:0,picks:Vec::new(),highlight:None,preview:(None,None),palettes:None,view:(57.0,543.0),schemes:Picker::default()}}
    /// The click targets of the last draw, in window pixels (read by the Settings geometry check).
    pub fn hits(&self)->&[Hit]{&self.hits}
    /// Settings opens on General, at the top.
    pub fn toggle(&mut self){self.open=!self.open;if self.open{self.section=Section::General;self.scroll=0.0;}self.keyboard_focus=false;self.menu=false;self.focus=None;self.slider=None;self.editor=None;self.end_preview();self.palettes=None;self.schemes=Picker::default();}
    fn end_preview(&mut self){self.highlight=None;self.schemes.highlight=None;self.preview=(None,None);}
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
        let y=self.picks[next].y;let (top,bottom)=(self.view.0+8.0,self.view.1-8.0);
        let heads=22.0;
        let first=self.picks.iter().map(|item|item.y).fold(f32::MAX,f32::min)-heads;
        let all=self.picks.iter().map(|item|item.y+38.0).fold(first,f32::max)+50.0;
        let themes=self.picks.iter().filter(|item|matches!(item.kind,PickKind::Theme{..})).map(|item|item.y+38.0).fold(first,f32::max);
        let span=[(first,all),(first,themes),(y,y+38.0)].into_iter().find(|(a,b)|b-a<=bottom-top).unwrap_or((y,y+38.0));
        if span.0-self.scroll<top{self.scroll=span.0-top;}else if span.1-self.scroll>bottom{self.scroll=span.1-bottom;}
        self.scroll=self.scroll.max(0.0).min(self.max_scroll.max(y+38.0-bottom));
        Effect::Preview
    }
    /// Keep a Colour scheme row: Follow removes terminal_colors, a scheme writes all 20 of its colours.
    fn keep_scheme(&mut self,index:usize)->Effect{
        self.end_preview();
        let Some((row,_))=self.schemes.rows.get(index).copied() else{return Effect::None;};
        let result=match row{Row::Follow=>self.store.reset("terminal_colors").map(|_|()),
            _=>{let colors=self.schemes.colors(row);self.store.set("terminal_colors",crate::terminal_schemes::to_value(&colors)).map(|_|())}};
        self.schemes.added=None;
        match result{Ok(())=>{self.error=None;Effect::Changed},Err(error)=>{self.error=Some(error);Effect::None}}
    }
    /// Move the Colour scheme highlight one row (the first move starts from the saved row) and preview it.
    fn step_scheme(&mut self,down:bool)->Effect{
        if self.schemes.rows.is_empty(){return Effect::None;}
        let last=self.schemes.rows.len()-1;
        let start=self.schemes.highlight.or_else(||self.schemes.saved(&self.effective));
        let next=match (start,down){(Some(i),true)=>(i+1).min(last),(Some(i),false)=>i.saturating_sub(1),(None,true)=>0,(None,false)=>last};
        self.schemes.highlight=Some(next);
        let (row,y)=self.schemes.rows[next];
        self.preview=(None,Some(self.schemes.colors(row)));
        // Keep the highlighted row inside the page body.
        let (top,bottom)=(self.view.0+8.0,self.view.1-8.0);
        if y-self.scroll<top{self.scroll=y-top;}else if y+38.0-self.scroll>bottom{self.scroll=y+38.0-bottom;}
        self.scroll=self.scroll.clamp(0.0,self.max_scroll.max(0.0));
        Effect::Preview
    }
    /// A change of the search text: the highlight and its preview end.
    fn search_changed(&mut self)->Effect{self.end_preview();Effect::Preview}
    pub fn theme_complete(&mut self){self.editor=None;self.error=None;}
    fn save(&mut self,key:&str,value:serde_json::Value)->Effect{
        match self.store.set(key,value){Ok(_)=>{self.error=None;Effect::Changed},Err(error)=>{self.error=Some(error);Effect::None}}
    }
    fn activate(&mut self,action:Action,x:f64)->Effect{match action{
        Action::Section(section)=>{self.section=section;self.menu=false;self.focus=None;self.scroll=0.0;self.end_preview();self.palettes=None;self.schemes=Picker::default();Effect::Preview},
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
            let color=match key{"background"=>t.background.or(c.as_ref().map(|c|c.background)),"foreground"=>t.foreground.or(c.as_ref().map(|c|c.foreground)),
                "selection"=>t.selection.or_else(||Some(crate::terminal_schemes::default_selection(t.background.or(c.as_ref().map(|c|c.background))?,t.foreground.or(c.as_ref().map(|c|c.foreground))?))),
                _=>t.cursor.or(t.foreground).or(c.as_ref().map(|c|c.foreground))};
            if let Some(color)=color{self.editor=Some(Editor::new(EditKind::TerminalColor(key),vec![settings_appearance::hex([color[0],color[1],color[2],255])]));self.error=None;}Effect::None},
        Action::SchemeSearch=>{self.schemes.focused=true;Effect::Preview},
        Action::SchemeRow(index)=>self.keep_scheme(index),
        Action::SchemeImport=>{let dir=std::env::var_os("HOME").map(|home|format!("{}/",PathBuf::from(home).display())).unwrap_or_default();
            self.editor=Some(Editor::new(EditKind::Palette,vec![dir]).caret_at_end());self.error=None;Effect::None},
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
        Action::Edit(key)=>{self.menu=false;self.editor=Some(Editor::new(EditKind::Field(key),vec![field_text(&self.effective,key)]));self.error=None;Effect::None},
        Action::AnsiColor(index)=>{let ansi=self.effective.terminal_colors.ansi.or(self.active_theme.as_ref().map(|t|t.ansi));
            if let Some(ansi)=ansi{self.editor=Some(Editor::new(EditKind::Ansi(index),vec![settings_appearance::hex([ansi[index][0],ansi[index][1],ansi[index][2],255])]));self.error=None;}Effect::None},
        Action::AddAgent=>{self.editor=Some(Editor::new(EditKind::NewAgent,vec![String::new(),String::new()]));self.error=None;Effect::None},
        Action::RemoveAgent(index)=>{let mut list=agents_json(&self.effective);if index<list.len(){list.remove(index);}self.save("agents",serde_json::Value::Array(list))},
    }}
    fn submit(&mut self)->Effect{
        let Some(editor)=&self.editor else{return Effect::None;};
        match &editor.kind{
            EditKind::Color(token)=>{match self.store.set_color(token,Some(editor.values[0].trim())){Ok(_)=>{self.editor=None;self.error=None;Effect::Changed},Err(error)=>{self.error=Some(error);Effect::None}}},
            EditKind::TerminalColor(key)=>{
                let color=match crate::themes::parse_color(editor.values[0].trim()){Ok(c)=>c,Err(_)=>{self.error=Some("Enter a colour as #rrggbb.".into());return Effect::None;}};
                let mut value=crate::terminal_schemes::to_value(&self.effective.terminal_colors);
                value[*key]=json!(settings_appearance::hex([color[0],color[1],color[2],255]));
                match self.store.set("terminal_colors",value){Ok(_)=>{self.editor=None;self.error=None;Effect::Changed},Err(error)=>{self.error=Some(error);Effect::None}}
            }
            EditKind::Field(key)=>{
                let key=*key;let text=editor.values[0].trim().to_owned();
                // A problem is shown in the row itself, in its own words, with the field marked.
                let value=match parse_field(key,&text){Ok(value)=>value,Err(error)=>{if let Some(editor)=&mut self.editor{editor.problem=Some(error);}return Effect::None;}};
                // A font size in pixels replaces the old point key.
                let result=if key=="font_size"{self.store.set_many(&[("font_size",Some(value)),("font_size_pt",None)]).map(|_|())}else{self.store.set(key,value).map(|_|())};
                match result{Ok(())=>{self.editor=None;self.error=None;Effect::Changed},Err(error)=>{if let Some(editor)=&mut self.editor{editor.problem=Some(friendly(key,&error));}Effect::None}}
            }
            EditKind::NewAgent=>{
                let (name,command)=(editor.values[0].trim().to_owned(),editor.values[1].trim().to_owned());
                if name.is_empty() || command.is_empty(){self.error=Some("Give the agent a name and a command.".into());return Effect::None;}
                let mut list=agents_json(&self.effective);list.push(json!({"name":name,"command":command}));
                match self.store.set("agents",serde_json::Value::Array(list)){Ok(_)=>{self.editor=None;self.error=None;Effect::Changed},Err(error)=>{self.error=Some(error);Effect::None}}
            }
            EditKind::Ansi(index)=>{
                let index=*index;
                let color=match crate::themes::parse_color(editor.values[0].trim()){Ok(c)=>c,Err(_)=>{self.error=Some("Enter a colour as #rrggbb.".into());return Effect::None;}};
                let mut value=crate::terminal_schemes::to_value(&self.effective.terminal_colors);
                if value.get("ansi").is_none(){let ansi=self.active_theme.as_ref().map(|t|t.ansi).unwrap_or([[0,0,0,255];16]);value["ansi"]=json!(ansi.iter().map(|c|settings_appearance::hex([c[0],c[1],c[2],255])).collect::<Vec<_>>());}
                value["ansi"][index]=json!(settings_appearance::hex([color[0],color[1],color[2],255]));
                match self.store.set("terminal_colors",value){Ok(_)=>{self.editor=None;self.error=None;Effect::Changed},Err(error)=>{self.error=Some(error);Effect::None}}
            }
            EditKind::Palette=>{
                let path=editor.values[0].trim();
                let path=match path.strip_prefix("~/"){Some(rest)=>std::env::var_os("HOME").map(|home|PathBuf::from(home).join(rest)).unwrap_or_else(||PathBuf::from(path)),None=>PathBuf::from(path)};
                let imported=crate::terminal_schemes::import_file(&path);
                match imported.and_then(|value|self.store.set("terminal_colors",value.clone()).map(|_|value)){
                    Ok(value)=>{
                        // Kept next to the config file, so the list gets its own row (named after the file) and
                        // it can be picked again later. The search is cleared so that row shows.
                        let added=crate::terminal_schemes::keep_imported(self.store.path(),&path,&value);
                        self.schemes.rescan();self.schemes.query.clear();self.schemes.highlight=None;
                        self.editor=None;
                        match added{Ok(name)=>{self.schemes.added=Some(name);self.error=None;},Err(error)=>{self.schemes.added=None;self.error=Some(format!("The colours are in use, but the list could not keep them: {error}"));}}
                        Effect::Changed},
                    Err(error)=>{self.error=Some(error);Effect::None}}
            }
            EditKind::Save=>{let Some(mut theme)=self.active_theme.clone()else{return Effect::None;};theme.name=editor.values[0].trim().into();Effect::Theme(ThemeAction::Save{slug:editor.values[1].trim().into(),theme})},
            EditKind::Import=>Effect::Theme(ThemeAction::Import{path:editor.values[0].trim().into(),slug:editor.values[1].trim().into()}),
            EditKind::Export=>{if self.effective.colors.keys().any(|key|crate::themes::COLOR_TOKENS.contains(&key.as_str())){self.error=Some("Save as a new theme before exporting colour changes.".into());Effect::None}else{Effect::Theme(ThemeAction::Export{id:self.effective.selected_theme_id(self.system_light).into(),path:editor.values[0].trim().into()})}},
        }
    }
    pub fn click(&mut self,x:f64,y:f64)->Effect{
        self.keyboard_focus=false;self.schemes.focused=false;
        let hit=self.hits.iter().rposition(|h|h.rect.contains(x,y));
        // A click outside the field being edited keeps what was typed (as a press of Enter would).
        if self.editor.as_ref().is_some_and(|e|e.inline()){
            let own=hit.is_some_and(|i|match (&self.hits[i].action,self.editor.as_ref().map(|e|&e.kind)){
                (Action::Edit(a),Some(EditKind::Field(b)))=>a==b,(Action::Field(_),Some(EditKind::NewAgent))=>true,_=>false});
            if !own{
                let adding=matches!(self.editor.as_ref().map(|e|&e.kind),Some(EditKind::NewAgent));
                let effect=if adding{self.editor=None;Effect::None}else{self.submit()};
                if self.editor.is_some(){return effect;}
                if !matches!(effect,Effect::None){return effect;}
            }else if let Some(i)=hit{if let Action::Field(index)=self.hits[i].action{if let Some(editor)=&mut self.editor{editor.field(index);}}return Effect::None;}
            else{return Effect::None;}
        }
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
            if editor.inline(){
                if *key==Key::Named(NamedKey::Enter){return self.submit();}
                if *key==Key::Named(NamedKey::Tab){
                    if matches!(editor.kind,EditKind::NewAgent){let next=(editor.focus+1)%2;editor.field(next);return Effect::None;}
                    return self.submit();
                }
                editor.key(key,_text);return Effect::None;
            }
            if *key==Key::Named(NamedKey::Enter){if editor.focus==editor.values.len(){self.editor=None;return Effect::None;}return self.submit();}
            editor.key(key,_text);return Effect::None;
        }
        self.keyboard_focus=true;
        if self.section==Section::Terminal && !self.menu{
            match key{
                Key::Named(NamedKey::ArrowDown)|Key::Named(NamedKey::ArrowUp)=>return self.step_scheme(*key==Key::Named(NamedKey::ArrowDown)),
                Key::Named(NamedKey::Enter) if self.schemes.highlight.is_some()=>{let index=self.schemes.highlight.unwrap_or(0);return self.keep_scheme(index);},
                Key::Named(NamedKey::Backspace) if self.schemes.focused=>{if self.schemes.query.pop().is_some(){return self.search_changed();}return Effect::None;},
                Key::Named(NamedKey::Delete)|Key::Named(NamedKey::Home)|Key::Named(NamedKey::End)|Key::Named(NamedKey::ArrowLeft)|Key::Named(NamedKey::ArrowRight) if self.schemes.focused=>return Effect::None,
                Key::Named(NamedKey::Space)|Key::Character(_) if self.schemes.focused=>{
                    let typed=_text.map(str::to_owned).unwrap_or_else(||if let Key::Character(ch)=key{ch.to_string()}else{" ".into()});
                    let typed:String=typed.chars().filter(|c|!c.is_control()).collect();
                    if typed.is_empty()||self.schemes.query.chars().count()>=60{return Effect::None;}
                    self.schemes.query.push_str(&typed);return self.search_changed();
                },
                _=>{}
            }
        }
        match key{
            Key::Named(NamedKey::PageDown)=>{self.wheel(-6);Effect::None},
            Key::Named(NamedKey::PageUp)=>{self.wheel(6);Effect::None},
            Key::Named(NamedKey::Escape)=>{if self.menu{self.menu=false;Effect::None}else if self.highlight.is_some()||self.schemes.highlight.is_some(){self.end_preview();Effect::Preview}else{self.open=false;Effect::None}},
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
    pub fn insert(&mut self,text:&str){
        if let Some(editor)=&mut self.editor{editor.insert(text);}
        else if self.section==Section::Terminal && self.schemes.focused{let text:String=text.chars().filter(|c|!c.is_control()).take(60).collect();self.schemes.query.push_str(&text);self.end_preview();}
    }
    pub fn preedit(&mut self,_text:&str){}
    pub fn select_all(&mut self){if let Some(editor)=&mut self.editor{editor.select_all();}}
    pub fn wheel(&mut self,lines:i32){
        if self.menu{let n=self.fonts().len();self.menu_scroll=(self.menu_scroll as i64-lines as i64).clamp(0,n.saturating_sub(MENU_ROWS) as i64) as usize;return;}
        if self.editor.as_ref().is_none_or(|e|e.inline()){self.scroll=(self.scroll-lines as f32*48.0).clamp(0.0,self.max_scroll);self.focus=None;}}
    pub fn ime_position(&self)->Option<[f32;2]>{None}
    pub fn animation_deadline(&self)->Option<std::time::Instant>{None}
    pub fn draw(&mut self,layout:&Layout,theme:&Theme,config:&Config,access:&[PaneAccess],masking_ready:bool,themes:Option<&ThemeStore>,system_light:bool,pointer:(f64,f64),pressed:bool,measure:&mut dyn FnMut(&TextRun)->TextMetrics)->Vec<Layer>{
        if !self.open{return vec![];}self.effective=config.clone();self.active_theme=Some(theme.clone());self.system_light=system_light;
        let top=layout.top_bar.y+layout.top_bar.h;
        let mut frame=settings_frame::draw_fitted(layout.window_px,layout.scale as f32,top,self.section,theme,config,pointer,pressed,measure);
        let g=frame.geometry.clone();let s=g.scale;let bottom=g.h-57.0;
        // A problem (the file's, with its line, or the last change's) sits above the page, which moves down for it.
        let problem=self.error.clone().or_else(||self.store.error().map(str::to_owned));
        let banner_lines=problem.as_deref().map(|text|banner_lines(text,s,measure)).unwrap_or_default();
        let banner=if banner_lines.is_empty(){0.0}else{16.0+18.0*banner_lines.len() as f32+12.0};
        let scroll=self.scroll-banner;
        // The banner has its own band: the page is clipped below it, so nothing scrolls under it.
        let view=PixelRect{y:g.body.y+banner*s,h:(g.body.h-banner*s).max(0.0),..g.body};
        self.view=(57.0+banner,bottom);
        let gv=settings_frame::Geometry{body:view,..g.clone()};
        let inline=self.editor.as_ref().filter(|e|e.inline());
        let editing=inline.and_then(|e|e.editing());
        let adding=inline.filter(|e|matches!(e.kind,EditKind::NewAgent)).map(|e|e.values.clone());
        let first=frame.hits.len();
        let mut end=0.0;
        {
            let gs=settings_frame::Geometry{panel:PixelRect{y:g.panel.y-scroll*s,..g.panel},..gv.clone()};
            let mut p=Paint::empty(theme,measure,s,layout.window_px,pointer,pressed);
            p.out.scene.layers.push(Layer::Clip(Some(view.array())));
            match self.section{
                Section::General=>end=super::settings_pages::general(&mut p,&gs,theme,config,self.forced_post_ai,editing,&mut frame.hits),
                Section::Terminal=>{
                    let rows=super::settings_pages::terminal(&mut p,&gs,theme,config,editing,&mut frame.hits);
                    // The Colour scheme group follows the colour rows; this controller keeps its search.
                    self.schemes.config=Some(self.store.path().to_path_buf());
                    end=settings_schemes::group(&mut p,&gs,theme,config,&mut self.schemes,rows+4.0,&mut frame.hits);
                }
                Section::Editor=>end=super::settings_pages::editor(&mut p,&gs,theme,config,editing,&mut frame.hits),
                Section::History=>end=super::settings_pages::history(&mut p,&gs,theme,config,editing,&mut frame.hits),
                Section::Agents=>end=super::settings_pages::agents(&mut p,&gs,theme,config,editing,adding.as_deref(),&mut frame.hits),
                Section::Appearance|Section::Keys=>{}
            }
            p.out.scene.layers.push(Layer::Clip(None));
            frame.scene.layers.extend(p.out.scene.layers);
            super::settings_form::clip_hits(&mut frame.hits,first,view);
        }
        let mut max=(end+24.0-bottom+banner).max(0.0);
        if self.section==Section::Appearance{if let Some(store)=themes{if self.palettes.is_none(){self.palettes=Some(super::settings_layout::scan_palettes());}
            let palettes=self.palettes.clone().unwrap_or_default();
            match settings_appearance::page(layout.window_px,&gv,theme,config,store,system_light,scroll,pointer,pressed,measure,&palettes,self.highlight,editing){Ok((layers,hits,page_max,picks))=>{frame.scene.layers.extend(layers);frame.hits.extend(hits);max=page_max+banner;self.picks=picks;},Err(error)=>self.error=Some(error)}}else{self.error=Some("Theme storage is unavailable.".into());}}
        if self.section==Section::Agents && !config.post_ai{let(layers,hits,page_max)=settings_access::agents(layout.window_px,&gv,theme,access,masking_ready,scroll-(end-65.0),pointer,pressed,measure);frame.scene.layers.extend(layers);frame.hits.extend(hits);max=page_max+(end-65.0)+banner;}
        if self.section==Section::Keys{let(layers,page_max)=settings_access::keys(layout.window_px,&gv,theme,config.post_ai,scroll,measure);frame.scene.layers.extend(layers);max=page_max+banner;}
        self.max_scroll=max;self.scroll=self.scroll.min(self.max_scroll);
        let mut p=Paint::empty(theme,measure,s,layout.window_px,pointer,pressed);
        if self.max_scroll>0.0{
            // A thin scroll thumb at the page's right edge: how long the page is, and where you are.
            // Quiet (11% of the way to the text colour), so it never reads as a control.
            let (top,bottom)=self.view;let track=(bottom-top-8.0).max(1.0);
            let thumb=(track*track/(track+self.max_scroll)).max(28.0).min(track);
            let y=top+4.0+(track-thumb)*(self.scroll/self.max_scroll).clamp(0.0,1.0);
            let ink=super::scene::blend(theme.colors.background,theme.colors.foreground,0.11);
            p.shape(g.local(770.0,y,6.0,thumb),3.0*s,[ink[0],ink[1],ink[2],255],None);
        }
        if !banner_lines.is_empty(){
            let rect=g.local(229.0,65.0,522.0,banner-16.0);
            let mut tint=theme.colors.err;tint[3]=28;p.shape(rect,8.0*s,tint,Some(super::scene::alpha(theme.colors.err,0.45)));
            for (i,line) in banner_lines.iter().enumerate(){p.text(g.local(241.0,71.0+18.0*i as f32,498.0,18.0),line,13.0*s,theme.colors.err,500,false);}
        }
        if self.menu {
            let field=frame.hits.iter().find(|h|h.action==Action::Edit("font_family")).map(|h|h.rect);
            if let Some(button)=frame.hits.iter().find(|h|h.action==Action::FontMenu).map(|h|h.rect){
                let anchor=field.map_or(button,|f|PixelRect{x:f.x,y:button.y,w:button.x+button.w-f.x,h:button.h});
                let fonts=self.fonts();let shown=fonts.len().min(MENU_ROWS);let first=self.menu_scroll.min(fonts.len().saturating_sub(shown));
                let menu=PixelRect{x:anchor.x,y:anchor.y+anchor.h+4.0*s,w:anchor.w,h:(8.0+32.0*shown as f32)*s};p.shape(menu,8.0*s,theme.colors.card,Some(theme.colors.input));
                for (i,name) in fonts.iter().skip(first).take(shown).enumerate(){let name=name.as_str();let rect=PixelRect{x:menu.x+4.0*s,y:menu.y+4.0*s+i as f32*32.0*s,w:menu.w-8.0*s,h:32.0*s};
                    if name==config.font_family || rect.contains(pointer.0,pointer.1){p.shape(rect,6.0*s,theme.colors.accent,None);}
                    p.text(PixelRect{x:rect.x+6.0*s,w:rect.w-12.0*s,..rect},name,14.0*s,theme.colors.foreground,400,false);
                    frame.hits.push(Hit{rect,action:Action::FontChoice(name.into())});
                }
            }
        }
        // The Colour scheme search field and rows draw their own focus (one ring on the field while it
        // takes the keys, a filled row while Up/Down preview it), so they never look selected twice.
        if self.keyboard_focus{if let Some(hit)=self.focus.and_then(|i|frame.hits.get(i)).filter(|h|!matches!(h.action,Action::SchemeSearch|Action::SchemeRow(_))){
            let mut rect=hit.rect;
            if matches!(hit.action,Action::Toggle(..)|Action::Access(_)) && rect.h>=60.0*s{
                rect=PixelRect{x:rect.x+rect.w-32.0*s,y:rect.y+(rect.h-s-18.0*s)/2.0,w:32.0*s,h:18.0*s};
            }
            p.shape(PixelRect{x:rect.x-4.0*s,y:rect.y-4.0*s,w:rect.w+8.0*s,h:rect.h+8.0*s},8.0*s,[0;4],Some(theme.colors.ring));
            if let Some(Layer::Shape(shape))=p.out.scene.layers.last_mut(){shape.border=Some((2.0*s,theme.colors.ring));}
        }}
        self.hits=frame.hits;frame.scene.layers.extend(p.out.scene.layers);
        if let Some(editor)=self.editor.as_ref().filter(|e|!e.inline()){let(layers,hits)=editor.draw(layout.window_px,&frame.geometry,theme,self.error.as_deref(),pointer,pressed,measure);frame.scene.layers.extend(layers);self.hits=hits;}
        frame.scene.layers
    }
}

/// The problem text wrapped into lines that fit the banner (13 px, 498 px wide), at most three.
fn banner_lines(text:&str,s:f32,measure:&mut dyn FnMut(&TextRun)->TextMetrics)->Vec<String>{
    let width=|t:&str,measure:&mut dyn FnMut(&TextRun)->TextMetrics|measure(&TextRun{text:t.into(),px:13.0*s,weight:500,..TextRun::default()}).width;
    let mut lines:Vec<String>=Vec::new();let mut line=String::new();
    for word in text.split_whitespace(){
        let candidate=if line.is_empty(){word.to_owned()}else{format!("{line} {word}")};
        if !line.is_empty() && width(&candidate,measure)>498.0*s{lines.push(std::mem::replace(&mut line,word.to_owned()));}else{line=candidate;}
    }
    if !line.is_empty(){lines.push(line);}
    lines.truncate(3);lines
}
/// The label of the row that edits a field, for its messages.
fn field_label(key:&str)->&str{
    match key{"notify_after_seconds"=>"Notify after","columns"=>"Columns","rows"=>"Rows","cursor_blink_time_ms"=>"Blink cycle",
        "cursor_blink_timeout_s"=>"Stop blinking after","history_days"=>"Keep for","history_output_kib"=>"Output per command",
        "font_size"=>"Font size","cell_width_scale"=>"Cell width","cell_height_scale"=>"Line height","opacity"=>"Opacity",
        "font_family"=>"Terminal font","ui_font_family"=>"Interface font","compose_dictionary"=>"Dictionary",
        "compose_dictionary_dir"=>"Dictionary folder",_=>key}
}
/// A value check's message in the row's words: "Font size must be between 8 and 32."
fn friendly(key:&str,error:&str)->String{
    let Some(rest)=error.strip_prefix(key).filter(|rest|rest.starts_with(' ')) else{return error.to_owned();};
    let rest=match rest.trim(){
        r if r.starts_with("must be a nonempty font family")=>"must name a font family (at most 256 bytes)".to_owned(),
        "must be absolute"=>"must be a full path, starting with /".to_owned(),
        r if r.starts_with("must be a safe locale name")=>"must be a language code such as en_US".to_owned(),
        r=>r.replace("finite and ","").replace("an integer between","a whole number between").replace("an integer from","a whole number from")
            .replace("a nonnegative integer","a whole number").replace("a positive integer","a whole number above 0").replace("a number from","from"),
    };
    format!("{} {}.",field_label(key),rest.trim_end_matches('.'))
}
/// A field's text: the stored value as config.xml writes it.
fn field_text(c:&Config,key:&str)->String{
    let d=super::settings_pages::decimal;
    match key{
        "notify_after_seconds"=>c.notify_after_seconds.to_string(),"columns"=>c.columns.to_string(),"rows"=>c.rows.to_string(),
        "cursor_blink_time_ms"=>c.cursor_blink_time_ms.to_string(),"cursor_blink_timeout_s"=>c.cursor_blink_timeout_s.to_string(),
        "history_days"=>c.history_days.to_string(),"history_output_kib"=>c.history_output_kib.to_string(),
        "font_size"=>d(c.font_size),"cell_width_scale"=>d(c.cell_width_scale),"cell_height_scale"=>d(c.cell_height_scale),"opacity"=>d(c.opacity),
        "font_family"=>c.font_family.clone(),"ui_font_family"=>c.ui_font_family.clone(),
        "compose_dictionary"=>c.compose_dictionary.clone(),"compose_dictionary_dir"=>c.compose_dictionary_dir.to_string_lossy().into_owned(),
        _=>String::new(),
    }
}
/// The typed text as the setting's JSON value; Config's own checks then judge its range.
fn parse_field(key:&str,text:&str)->Result<serde_json::Value,String>{
    if crate::config_xml::is_int(key){return text.parse::<u64>().map(|n|json!(n)).map_err(|_|"Enter a whole number.".into());}
    if crate::config_xml::is_number(key){return text.replace(',',".").parse::<f64>().ok().filter(|v|v.is_finite()).map(|v|json!(v)).ok_or_else(||"Enter a number.".into());}
    Ok(json!(text))
}
fn agents_json(c:&Config)->Vec<serde_json::Value>{c.agents.iter().map(|a|json!({"name":a.name,"command":a.command})).collect()}
