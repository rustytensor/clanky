//! Appearance contents and sheet-style edit popovers; storage stays in ThemeStore.
use super::{scene::Paint,settings_frame::{Action,Geometry,Hit}};
use crate::{config::Config,look::PixelRect,theme_store::ThemeStore,themes::{Theme,Color,COLOR_TOKENS}};
use clanky_render::{Layer,TextRun,TextMetrics};
use winit::keyboard::{Key,NamedKey};
fn r(x:f32,y:f32,w:f32,h:f32)->PixelRect{PixelRect{x,y,w,h}}
pub(super) fn hex(c:Color)->String{if c[3]==255{format!("#{:02x}{:02x}{:02x}",c[0],c[1],c[2])}else{format!("#{:02x}{:02x}{:02x}{:02x}",c[0],c[1],c[2],c[3])}}
fn label(token:&str)->String{let text=token.replace('_'," ");let mut chars=text.chars();chars.next().map(|c|c.to_uppercase().collect::<String>()+chars.as_str()).unwrap_or_default()}
fn button(p:&mut Paint<'_>,rect:PixelRect,text:&str,t:&Theme,s:f32,pointer:(f64,f64),pressed:bool,outline:bool){
    let fill=if rect.contains(pointer.0,pointer.1){if pressed{t.colors.pressed}else{t.colors.accent}}else{t.colors.background};
    if outline || rect.contains(pointer.0,pointer.1){p.shape(rect,6.0*s,fill,outline.then_some(t.colors.input));}
    let w=p.width(text,13.0*s,500);p.text(r(rect.x+(rect.w-w)/2.0,rect.y,w,rect.h),text,13.0*s,t.colors.foreground,500,false);
}
#[allow(clippy::too_many_arguments,clippy::type_complexity)]
pub(super) fn page(window:[u32;2],g:&Geometry,t:&Theme,cfg:&Config,store:&ThemeStore,system_light:bool,scroll:f32,pointer:(f64,f64),pressed:bool,measure:&mut dyn FnMut(&TextRun)->TextMetrics,palettes:&[(String,serde_json::Value)],highlight:Option<usize>,editing:Option<super::settings_form::Editing<'_>>)->Result<(Vec<Layer>,Vec<Hit>,f32,Vec<super::settings_layout::PickItem>),String>{
    let s=g.scale;let mut p=Paint::empty(t,measure,s,window,pointer,pressed);let mut hits=vec![];
    let rect=|x,y,w,h|g.local(x,y-scroll,w,h);
    p.out.scene.layers.push(Layer::Clip(Some(g.body.array())));
    let gs=Geometry{panel:PixelRect{y:g.panel.y-scroll*s,..g.panel},..g.clone()};
    let layout_end=super::settings_layout::layout_group(&mut p,&gs,t,cfg,&mut hits);
    let (end,items)=super::settings_layout::theme_group(&mut p,&gs,t,cfg,store,palettes,system_light,highlight,&mut hits,layout_end+20.0)?;
    let end=end+6.0;
    p.shape(rect(229.0,end-1.0,522.0,1.0),0.0,t.colors.border,None);
    let end=text_group(&mut p,&gs,t,cfg,editing,&mut hits,end+20.0);
    p.text(rect(229.0,end+20.0,522.0,16.0),&format!("Window colours ({})",t.name),12.0*s,t.colors.muted_foreground,500,false);
    let selected_id=cfg.selected_theme_id(system_light);let base=store.get(selected_id).ok();let mut y=end+40.0;
    let tokens=["sidebar","background","agent"].into_iter().chain(COLOR_TOKENS.iter().copied().filter(|token|!["sidebar","background","agent"].contains(token)));
    for token in tokens{let Some(color)=t.colors.get(token)else{continue;};let row=rect(229.0,y,522.0,40.0);
        p.shape(rect(229.0,y+10.0,20.0,20.0),5.0*s,color,Some(t.colors.input));let name=if token=="agent"{"Agent accent".into()}else{label(token)};p.text(rect(259.0,y,260.0,40.0),&name,14.0*s,t.colors.foreground,400,false);
        let changed=cfg.colors.get(token).is_some_and(|value|base.as_ref().and_then(|b|b.colors.get(token)).as_ref()!=Some(value));
        let value=hex(color);let width=p.mono_width(&value,13.0*s,400);let right=if changed{679.0}else{751.0};p.text(r(g.panel.x+right*s-width,g.panel.y+(y-scroll)*s,width,40.0*s),&value,13.0*s,t.colors.muted_foreground,400,true);
        hits.push(Hit{rect:row,action:Action::Color(token)});
        if changed{let b=rect(689.0,y+6.0,62.0,28.0);button(&mut p,b,"Reset",t,s,pointer,pressed,false);hits.push(Hit{rect:b,action:Action::ResetColor(token)});}
        p.shape(rect(229.0,y+39.0,522.0,1.0),0.0,t.colors.border,None);y+=40.0;
    }
    let mut x=229.0;
    for(name,action)in[("Save as new theme",Action::SaveTheme),("Import",Action::ImportTheme),("Export",Action::ExportTheme)]{let width=p.width(name,13.0*s,500)/s+20.0;let b=rect(x,y+16.0,width,28.0);button(&mut p,b,name,t,s,pointer,pressed,true);hits.push(Hit{rect:b,action});x+=width+8.0;}
    hits.retain_mut(|hit|{let top=hit.rect.y.max(g.body.y);let bottom=(hit.rect.y+hit.rect.h).min(g.body.y+g.body.h);hit.rect.y=top;hit.rect.h=(bottom-top).max(0.0);hit.rect.h>0.0});
    p.out.scene.layers.push(Layer::Clip(None));Ok((p.out.scene.layers,hits,(y+68.0-543.0).max(0.0),items))
}
/// Text and window rows: font size, terminal and interface fonts, cell width, line height, opacity.
fn text_group(p:&mut Paint<'_>,g:&Geometry,t:&Theme,cfg:&Config,editing:Option<super::settings_form::Editing<'_>>,hits:&mut Vec<Hit>,top:f32)->f32{
    use super::settings_form::{self as form,Control,row};
    let s=g.scale;let c=&t.colors;let num=super::settings_pages::decimal;
    let mut y=form::heading(p,g,t,top,"Text");
    y=form::rows_then(p,g,t,y,&[row("Font size","Ctrl + and Ctrl − change it for one window.",Control::Field("font_size",num(cfg.font_size),80.0,true))],editing,hits,true);
    // Terminal font: a field, and a menu of the installed monospace families beside it.
    let control=super::settings_frame::row(p,g,t,y,236.0,"Terminal font","Used in every terminal pane.",false);
    let field=r(control.x,control.y,200.0*s,control.h);
    form::field(p,field,&cfg.font_family,false,editing.filter(|e|e.key=="font_family"),t,s,"");
    hits.push(Hit{rect:field,action:Action::Edit("font_family")});
    let menu=r(control.x+204.0*s,control.y,32.0*s,control.h);
    let fill=if menu.contains(p.pointer.0,p.pointer.1){if p.pressed{c.pressed}else{c.accent}}else{c.background};p.shape(menu,8.0*s,fill,Some(c.input));
    let mut arrow=c.foreground;arrow[3]=153;super::settings_frame::icon(p,"chevron",r(menu.x+9.0*s,menu.y+9.0*s,14.0*s,14.0*s),arrow,s*0.875);
    hits.push(Hit{rect:menu,action:Action::FontMenu});
    y+=form::ROW;
    y=form::rows_then(p,g,t,y,&[
        row("Interface font","Used for menus, tabs and Settings.",Control::Field("ui_font_family",cfg.ui_font_family.clone(),200.0,false)),
        row("Cell width","Wider cells space the letters out, as in Ptyxis.",Control::Field("cell_width_scale",num(cfg.cell_width_scale),80.0,true)),
        row("Line height","Taller rows space the lines out, as in Ptyxis.",Control::Field("cell_height_scale",num(cfg.cell_height_scale),80.0,true)),
    ],editing,hits,false);
    y=form::heading(p,g,t,y+20.0,"Window");
    // Opacity: a slider, and its value as a field.
    let control=super::settings_frame::row(p,g,t,y,240.0,"Opacity","How much of the desktop shows through.",true);
    let progress=((cfg.opacity-0.5)/0.5).clamp(0.0,1.0);let cy=control.y+16.0*s;let track=r(control.x+8.0*s,cy-2.0*s,140.0*s,4.0*s);
    p.shape(track,2.0*s,c.muted,None);p.shape(r(track.x,track.y,track.w*progress,track.h),2.0*s,c.foreground,None);
    p.shape(r(track.x+track.w*progress-8.0*s,cy-8.0*s,16.0*s,16.0*s),8.0*s,c.background,Some(c.input));
    hits.push(Hit{rect:r(track.x-8.0*s,control.y,track.w+16.0*s,control.h),action:Action::Opacity});
    let field=r(control.x+control.w-72.0*s,control.y,72.0*s,control.h);
    form::field(p,field,&num(cfg.opacity),true,editing.filter(|e|e.key=="opacity"),t,s,"");
    hits.push(Hit{rect:field,action:Action::Edit("opacity")});
    y+form::ROW
}
#[cfg(test)]
#[test]
fn theme_card_names_fit(){
    // Picker rows in a 250 px column leave 250 - 12 - 68 - 20 = 150 px for the name.
    for entry in crate::themes::BUILTINS.iter(){
        let theme=entry.load().unwrap();
        assert!(clanky_render::measure_text(&TextRun{text:theme.name.clone(),px:13.0,weight:400,..Default::default()}).width<=150.0,"name does not fit: {}",theme.name);
    }
    let store=ThemeStore::open(std::path::PathBuf::from("/srv/devops/build").join(format!("clanky-label-fixture-{}",std::process::id()))).unwrap();
    let theme=crate::themes::builtin("stone-dark").unwrap();let cfg=Config::window_defaults();let g=super::settings_frame::geometry([1240,760],1.0);
    let(layers,_,_,_)=page([1240,760],&g,&theme,&cfg,&store,false,0.0,(-1.0,-1.0),false,&mut clanky_render::measure_text,&[],None,None).unwrap();
    // The picker (mockup section 5) names Catppuccin flavours in full: "Catppuccin Mocha".
    for title in ["Catppuccin Frappé","Catppuccin Macchiato","Catppuccin Mocha","Catppuccin Latte"]{assert!(layers.iter().any(|layer|matches!(layer,Layer::Text(text) if text.text==title)),"{title}");}
}

#[derive(Clone)]
pub(super) enum EditKind{Color(&'static str),Save,Import,Export,TerminalColor(&'static str),Palette,
    /// In place: a text field (its key), the new agent's name and command; in the picker: one ANSI colour.
    Field(&'static str),NewAgent,Ansi(usize)}
pub(super) struct Editor{pub kind:EditKind,pub values:Vec<String>,pub focus:usize,caret:usize,all:bool}
impl Editor{
    pub fn new(kind:EditKind,values:Vec<String>)->Self{let caret=values[0].len();Self{kind,values,focus:0,caret,all:true}}
    /// Edited in place on its page (a text field, the new agent), not in a sheet.
    pub fn inline(&self)->bool{matches!(self.kind,EditKind::Field(_)|EditKind::NewAgent)}
    pub fn editing(&self)->Option<super::settings_form::Editing<'_>>{
        let key=match self.kind{EditKind::Field(key)=>key,EditKind::NewAgent=>"agents",_=>return None};
        self.values.get(self.focus).map(|text|super::settings_form::Editing{key,index:self.focus,text,caret:self.caret,all:self.all})
    }
    pub fn field(&mut self,index:usize){self.focus=index;self.caret=self.values[index].len();self.all=true;}
    pub fn select_all(&mut self){self.all=true;}
    pub fn insert(&mut self,text:&str){let Some(value)=self.values.get_mut(self.focus)else{return;};let text:String=text.chars().filter(|c|!c.is_control()).take(4096).collect();if self.all{value.clear();self.caret=0;self.all=false;}if value.len()+text.len()>4096{return;}value.insert_str(self.caret,&text);self.caret+=text.len();}
    pub fn key(&mut self,key:&Key,text:Option<&str>){
        if *key==Key::Named(NamedKey::Tab){self.focus=(self.focus+1)%(self.values.len()+2);if let Some(value)=self.values.get(self.focus){self.caret=value.len();self.all=true;}return;}
        let Some(value)=self.values.get_mut(self.focus)else{return;};
        match key{
            Key::Named(NamedKey::Backspace)|Key::Named(NamedKey::Delete)=>{if self.all{value.clear();self.caret=0;self.all=false;}else if *key==Key::Named(NamedKey::Backspace){if let Some((i,_))=value[..self.caret].char_indices().last(){value.drain(i..self.caret);self.caret=i;}}else if let Some(c)=value[self.caret..].chars().next(){value.drain(self.caret..self.caret+c.len_utf8());}},
            Key::Named(NamedKey::ArrowLeft)=>{self.caret=value[..self.caret].char_indices().last().map_or(0,|(i,_)|i);self.all=false;},
            Key::Named(NamedKey::ArrowRight)=>{self.caret+=value[self.caret..].chars().next().map_or(0,char::len_utf8);self.all=false;},
            Key::Named(NamedKey::Home)=>{self.caret=0;self.all=false;},Key::Named(NamedKey::End)=>{self.caret=value.len();self.all=false;},
            _=>if let Some(text)=text{self.insert(text);},
        }
    }
    pub fn draw(&self,window:[u32;2],g:&Geometry,t:&Theme,error:Option<&str>,pointer:(f64,f64),pressed:bool,measure:&mut dyn FnMut(&TextRun)->TextMetrics)->(Vec<Layer>,Vec<Hit>){
        let s=g.scale;let mut p=Paint::empty(t,measure,s,window,pointer,pressed);let mut hits=vec![];
        p.shape(g.panel,12.0*s,[0,0,0,100],None);let box_rect=g.local(220.0,140.0,548.0,330.0);p.shape(box_rect,12.0*s,t.colors.card,Some(t.colors.border));
        let (title,labels)=match self.kind{EditKind::Color(_)=>("Choose colour",vec!["Hex colour"]),EditKind::Save=>("Save as new theme",vec!["Name","File name"]),EditKind::Import=>("Import theme",vec!["Source file","File name"]),EditKind::Export=>("Export theme",vec!["Destination file"]),
            EditKind::TerminalColor(_)=>("Choose terminal colour",vec!["Hex colour"]),EditKind::Ansi(i)=>(if i<8{"Choose a normal ANSI colour"}else{"Choose a bright ANSI colour"},vec!["Hex colour"]),
            EditKind::Field(_)|EditKind::NewAgent=>("",vec![]),EditKind::Palette=>("Import Ptyxis palette",vec!["Palette file (its [Dark] colours are used)"])};
        p.text(g.local(240.0,156.0,500.0,32.0),title,16.0*s,t.colors.foreground,600,false);
        for(i,(name,value))in labels.iter().zip(&self.values).enumerate(){let y=202.0+i as f32*68.0;p.text(g.local(240.0,y-20.0,500.0,18.0),name,13.0*s,t.colors.muted_foreground,400,false);let input=g.local(240.0,y,508.0,32.0);p.shape(input,8.0*s,t.colors.background,Some(if self.focus==i{t.colors.ring}else{t.colors.input}));
            let caret=if self.focus==i{self.caret}else{value.len()};let mut start=0;while p.mono_width(&value[start..caret],13.0*s,400)>488.0*s{start+=value[start..].chars().next().unwrap().len_utf8();}
            p.text(r(input.x+10.0*s,input.y,input.w-20.0*s,input.h),&value[start..],13.0*s,t.colors.foreground,400,true);
            if self.focus==i{let x=input.x+10.0*s+p.mono_width(&value[start..caret],13.0*s,400);p.shape(r(x,input.y+8.0*s,s,16.0*s),0.0,t.colors.foreground,None);}
            hits.push(Hit{rect:input,action:Action::Field(i)});
        }
        if let EditKind::Color(_)|EditKind::TerminalColor(_)|EditKind::Ansi(_)=self.kind{for(i,color)in t.ansi.iter().enumerate(){let rect=g.local(240.0+i as f32*31.0,256.0,26.0,26.0);p.shape(rect,5.0*s,*color,Some(t.colors.input));hits.push(Hit{rect,action:Action::PickColor(hex(*color))});}}
        if let Some(error)=error{p.text(g.local(240.0,350.0,508.0,42.0),error,13.0*s,t.colors.err,400,false);}
        for (x,name,action)in[(568.0,"Cancel",Action::Cancel),(664.0,"Apply",Action::Confirm)]{let rect=g.local(x,416.0,84.0,32.0);button(&mut p,rect,name,t,s,pointer,pressed,true);hits.push(Hit{rect,action});}
        (p.out.scene.layers,hits)
    }
}
