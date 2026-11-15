//! Appearance contents and sheet-style edit popovers; storage stays in ThemeStore.
use super::{scene::Paint,settings_frame::{Action,Geometry,Hit}};
use crate::{config::Config,look::PixelRect,theme_store::ThemeStore,themes::{Theme,Color,COLOR_TOKENS}};
use clanky_render::{Layer,TextRun,TextMetrics};
use winit::keyboard::{Key,NamedKey};
fn r(x:f32,y:f32,w:f32,h:f32)->PixelRect{PixelRect{x,y,w,h}}
/// Hue (degrees), saturation and lightness (0 to 1) of a colour.
fn hsl(c:Color)->(f32,f32,f32){
    let [r,g,b]=[c[0],c[1],c[2]].map(|v|v as f32/255.0);let (max,min)=(r.max(g).max(b),r.min(g).min(b));let l=(max+min)/2.0;let d=max-min;
    if d<1e-6{return (0.0,0.0,l);}
    let s=d/(1.0-(2.0*l-1.0).abs());
    let h=if max==r{60.0*(((g-b)/d).rem_euclid(6.0))}else if max==g{60.0*((b-r)/d+2.0)}else{60.0*((r-g)/d+4.0)};
    (h,s,l)
}
fn from_hsl(h:f32,s:f32,l:f32,alpha:u8)->Color{
    let h=h.rem_euclid(360.0);let c=(1.0-(2.0*l-1.0).abs())*s;let x=c*(1.0-((h/60.0).rem_euclid(2.0)-1.0).abs());let m=l-c/2.0;
    let (r,g,b)=match (h/60.0) as u32{0=>(c,x,0.0),1=>(x,c,0.0),2=>(0.0,c,x),3=>(0.0,x,c),4=>(x,0.0,c),_=>(c,0.0,x)};
    [r,g,b].map(|v|((v+m)*255.0).round().clamp(0.0,255.0) as u8).into_iter().chain([alpha]).collect::<Vec<_>>().try_into().unwrap_or([0,0,0,alpha])
}
pub(super) fn hex(c:Color)->String{if c[3]==255{format!("#{:02x}{:02x}{:02x}",c[0],c[1],c[2])}else{format!("#{:02x}{:02x}{:02x}{:02x}",c[0],c[1],c[2],c[3])}}
/// A colour token's name in Settings: its config key in words ("muted_foreground" is "Muted foreground").
pub(super) fn label(token:&str)->String{let text=token.replace('_'," ");let mut chars=text.chars();chars.next().map(|c|c.to_uppercase().collect::<String>()+chars.as_str()).unwrap_or_default()}
fn button(p:&mut Paint<'_>,rect:PixelRect,text:&str,t:&Theme,s:f32,pointer:(f64,f64),pressed:bool,outline:bool){
    let fill=if rect.contains(pointer.0,pointer.1){if pressed{t.colors.pressed}else{t.colors.accent}}else{t.colors.background};
    if outline || rect.contains(pointer.0,pointer.1){p.shape(rect,8.0*s,fill,outline.then_some(t.colors.input));}
    let w=p.width(text,14.0*s,500);p.text(r(rect.x+(rect.w-w)/2.0,rect.y,w,rect.h),text,14.0*s,t.colors.foreground,500,false);
}
#[allow(clippy::too_many_arguments,clippy::type_complexity)]
pub(super) fn page(window:[u32;2],g:&Geometry,t:&Theme,cfg:&Config,store:&ThemeStore,system_light:bool,scroll:f32,pointer:(f64,f64),pressed:bool,measure:&mut dyn FnMut(&TextRun)->TextMetrics,palettes:&[(String,serde_json::Value)],highlight:Option<usize>,editing:Option<super::settings_form::Editing<'_>>)->Result<(Vec<Layer>,Vec<Hit>,f32,Vec<super::settings_layout::PickItem>),String>{
    let s=g.scale;let mut p=Paint::empty(t,measure,s,window,pointer,pressed);let mut hits=vec![];
    let rect=|x,y,w,h|g.local(x,y-scroll,w,h);
    p.out.scene.layers.push(Layer::Clip(Some(g.body.array())));
    let gs=Geometry{panel:PixelRect{y:g.panel.y-scroll*s,..g.panel},..g.clone()};
    // Layout, the theme picker (the setting used most, one scroll away), the text and window rows,
    // then the window colours that adjust the theme in use (the longest group, last).
    let layout_end=super::settings_layout::layout_group(&mut p,&gs,t,cfg,&mut hits);
    let (end,items)=super::settings_layout::theme_group(&mut p,&gs,t,cfg,store,palettes,system_light,highlight,&mut hits,layout_end+20.0)?;
    let end=text_group(&mut p,&gs,t,cfg,editing,&mut hits,end+8.0);
    // Window colours, under Opacity in the Window group: two columns of swatches, each with its
    // value; a changed one also has Reset.
    let mut y=end+6.0;
    if super::settings_frame::fits(&gs,y,18.0){p.text(rect(229.0,y,522.0,18.0),&format!("Window colours of {}",t.name),12.0*s,crate::calm::scene_faint(&t.colors),500,false);}
    y+=22.0;
    let selected_id=cfg.selected_theme_id(system_light);let base=store.get(selected_id).ok();
    // The colours changed most first (accents and signals), then surfaces, text and lines; any
    // token not named here follows, so the grid always shows every token of the theme.
    const ORDER:[&str;19]=["accent","agent","ok","err","background","sidebar","card","muted","pressed","sidebar_accent",
        "foreground","muted_foreground","primary","primary_foreground","primary_pressed","border","input","ring","robot_paint"];
    let tokens:Vec<&str>=ORDER.iter().copied().filter(|token|COLOR_TOKENS.contains(token)).chain(COLOR_TOKENS.iter().copied().filter(|token|!ORDER.contains(token))).collect();
    for (i,token) in tokens.iter().copied().enumerate(){let Some(color)=t.colors.get(token)else{continue;};
        let x=if i%2==0{229.0}else{501.0};let cy=y+(i/2) as f32*32.0;let right=x+250.0;
        if !super::settings_frame::fits(&gs,cy,32.0){continue;}
        p.shape(rect(x,cy+6.0,20.0,20.0),5.0*s,color,Some(t.colors.input));
        let changed=cfg.colors.get(token).is_some_and(|value|base.as_ref().and_then(|b|b.colors.get(token)).as_ref()!=Some(value));
        let reset=if changed{p.width("Reset",14.0*s,500)/s+20.0}else{0.0};
        let value=hex(color);let vw=p.mono_width(&value,12.0*s,400)/s;let vx=right-reset-if changed{6.0}else{0.0}-vw;
        p.text(rect(vx,cy,vw+1.0,32.0),&value,12.0*s,t.colors.muted_foreground,400,true);
        p.text(rect(x+30.0,cy,(vx-8.0-x-30.0).max(0.0),32.0),&label(token),14.0*s,t.colors.foreground,400,false);
        hits.push(Hit{rect:rect(x,cy,vx+vw-x,32.0),action:Action::Color(token)});
        if changed{let b=rect(right-reset,cy,reset,32.0);button(&mut p,b,"Reset",t,s,pointer,pressed,true);hits.push(Hit{rect:b,action:Action::ResetColor(token)});}
    }
    y+=tokens.len().div_ceil(2) as f32*32.0;
    let mut x=229.0;
    for(name,action)in[("Save as new theme",Action::SaveTheme),("Import",Action::ImportTheme),("Export",Action::ExportTheme)]{if !super::settings_frame::fits(&gs,y+12.0,32.0){break;}let width=p.width(name,14.0*s,500)/s+24.0;let b=rect(x,y+12.0,width,32.0);button(&mut p,b,name,t,s,pointer,pressed,true);hits.push(Hit{rect:b,action});x+=width+8.0;}
    let y=y+48.0;
    hits.retain_mut(|hit|{let top=hit.rect.y.max(g.body.y);let bottom=(hit.rect.y+hit.rect.h).min(g.body.y+g.body.h);hit.rect.y=top;hit.rect.h=(bottom-top).max(0.0);hit.rect.h>0.0});
    p.out.scene.layers.push(Layer::Clip(None));Ok((p.out.scene.layers,hits,(y+24.0-(g.h-57.0)).max(0.0),items))
}
/// Text and window rows: font size, terminal and interface fonts, cell width, line height, opacity.
fn text_group(p:&mut Paint<'_>,g:&Geometry,t:&Theme,cfg:&Config,editing:Option<super::settings_form::Editing<'_>>,hits:&mut Vec<Hit>,top:f32)->f32{
    use super::settings_form::{self as form,Control,row};
    let s=g.scale;let c=&t.colors;let num=super::settings_pages::decimal;
    let mut y=form::heading(p,g,t,top,"Text");
    y=form::rows_then(p,g,t,y,&[row("Font size","Ctrl + and Ctrl − change it for one window.",Control::Field("font_size",num(cfg.font_size),80.0,true))],editing,hits,true);
    // Terminal font: a field, and a menu of the installed monospace families beside it.
    if super::settings_frame::row_fits(g,y){
    let problem=editing.filter(|e|e.key=="font_family").and_then(|e|e.problem);
    let control=super::settings_frame::level(g,super::settings_frame::row_noted(p,g,t,y,236.0,"Terminal font","Used in every terminal pane.",problem,false));
    let field=r(control.x,control.y,200.0*s,control.h);
    form::field(p,field,&cfg.font_family,false,editing.filter(|e|e.key=="font_family"),t,s,"");
    hits.push(Hit{rect:field,action:Action::Edit("font_family")});
    let menu=r(control.x+204.0*s,control.y,32.0*s,control.h);
    let fill=if menu.contains(p.pointer.0,p.pointer.1){if p.pressed{c.pressed}else{c.accent}}else{c.background};p.shape(menu,8.0*s,fill,Some(c.input));
    let mut arrow=c.foreground;arrow[3]=153;super::settings_frame::icon(p,"chevron",r(menu.x+9.0*s,menu.y+9.0*s,14.0*s,14.0*s),arrow,s*0.875);
    hits.push(Hit{rect:menu,action:Action::FontMenu});
    }
    y+=form::ROW;
    y=form::rows_then(p,g,t,y,&[
        row("Interface font","Used for menus, tabs and Settings.",Control::Field("ui_font_family",cfg.ui_font_family.clone(),200.0,false)),
        row("Cell width","Wider cells space the letters out, as in Ptyxis.",Control::Field("cell_width_scale",num(cfg.cell_width_scale),80.0,true)),
        row("Line height","Taller rows space the lines out, as in Ptyxis.",Control::Field("cell_height_scale",num(cfg.cell_height_scale),80.0,true)),
    ],editing,hits,false);
    y=form::heading(p,g,t,y+20.0,"Window");
    // Opacity: its value as a field level with the label, and under the text a slider the
    // width of the page (as a GNOME scale row), so it can be set finely.
    if !super::settings_frame::fits(g,y,94.0){return y+form::ROW+30.0;}
    let problem=editing.filter(|e|e.key=="opacity").and_then(|e|e.problem);
    let control=super::settings_frame::level(g,super::settings_frame::row_noted(p,g,t,y,72.0,"Opacity","How much of the desktop shows through.",problem,true));
    let progress=((cfg.opacity-0.5)/0.5).clamp(0.0,1.0);
    let lane=g.local(229.0,y+60.0,522.0,32.0);let cy=lane.y+16.0*s;let track=r(lane.x+8.0*s,cy-2.0*s,lane.w-16.0*s,4.0*s);
    p.shape(track,2.0*s,c.muted,None);p.shape(r(track.x,track.y,track.w*progress,track.h),2.0*s,c.foreground,None);
    p.shape(r(track.x+track.w*progress-8.0*s,cy-8.0*s,16.0*s,16.0*s),8.0*s,c.background,Some(c.ring));
    hits.push(Hit{rect:r(track.x-8.0*s,lane.y,track.w+16.0*s,lane.h),action:Action::Opacity});
    form::field(p,control,&num(cfg.opacity),true,editing.filter(|e|e.key=="opacity"),t,s,"");
    hits.push(Hit{rect:control,action:Action::Edit("opacity")});
    y+form::ROW+30.0
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
    // Cards are drawn only while whole on the page, so read the page at several scroll positions.
    let layers:Vec<Layer>=(0..8).flat_map(|i|page([1240,760],&g,&theme,&cfg,&store,false,i as f32*200.0,(-1.0,-1.0),false,&mut clanky_render::measure_text,&[],None,None).unwrap().0).collect();
    // The picker (mockup section 5) names Catppuccin flavours in full: "Catppuccin Mocha".
    for title in ["Catppuccin Frappé","Catppuccin Macchiato","Catppuccin Mocha","Catppuccin Latte"]{assert!(layers.iter().any(|layer|matches!(layer,Layer::Text(text) if text.text==title)),"{title}");}
}

#[derive(Clone)]
pub(super) enum EditKind{Color(&'static str),Save,Import,Export,TerminalColor(&'static str),Palette,
    /// In place: a text field (its key), the new agent's name and command; in the picker: one ANSI colour.
    Field(&'static str),NewAgent,Ansi(usize)}
pub(super) struct Editor{pub kind:EditKind,pub values:Vec<String>,pub focus:usize,caret:usize,all:bool,
    /// The values it opened with (the colour picker shows the current colour beside the new one).
    pub start:Vec<String>,
    /// Why the last Enter did not save, in the row's words (fields edited in place).
    pub problem:Option<String>}
impl Editor{
    pub fn new(kind:EditKind,values:Vec<String>)->Self{let caret=values[0].len();Self{kind,start:values.clone(),values,focus:0,caret,all:true,problem:None}}
    /// Open with the caret after the text and nothing selected, so typing adds to it (a folder to finish).
    pub fn caret_at_end(mut self)->Self{self.all=false;self}
    /// Edited in place on its page (a text field, the new agent), not in a sheet.
    pub fn inline(&self)->bool{matches!(self.kind,EditKind::Field(_)|EditKind::NewAgent)}
    pub fn editing(&self)->Option<super::settings_form::Editing<'_>>{
        let key=match self.kind{EditKind::Field(key)=>key,EditKind::NewAgent=>"agents",_=>return None};
        self.values.get(self.focus).map(|text|super::settings_form::Editing{key,index:self.focus,text,caret:self.caret,all:self.all,problem:self.problem.as_deref()})
    }
    pub fn field(&mut self,index:usize){self.focus=index;self.caret=self.values[index].len();self.all=true;}
    pub fn select_all(&mut self){self.all=true;}
    pub fn insert(&mut self,text:&str){self.problem=None;let Some(value)=self.values.get_mut(self.focus)else{return;};let text:String=text.chars().filter(|c|!c.is_control()).take(4096).collect();if self.all{value.clear();self.caret=0;self.all=false;}if value.len()+text.len()>4096{return;}value.insert_str(self.caret,&text);self.caret+=text.len();}
    pub fn key(&mut self,key:&Key,text:Option<&str>){
        if *key==Key::Named(NamedKey::Tab){self.focus=(self.focus+1)%(self.values.len()+2);if let Some(value)=self.values.get(self.focus){self.caret=value.len();self.all=true;}return;}
        let Some(value)=self.values.get_mut(self.focus)else{return;};
        match key{
            Key::Named(NamedKey::Backspace)|Key::Named(NamedKey::Delete)=>{self.problem=None;if self.all{value.clear();self.caret=0;self.all=false;}else if *key==Key::Named(NamedKey::Backspace){if let Some((i,_))=value[..self.caret].char_indices().last(){value.drain(i..self.caret);self.caret=i;}}else if let Some(c)=value[self.caret..].chars().next(){value.drain(self.caret..self.caret+c.len_utf8());}},
            Key::Named(NamedKey::ArrowLeft)=>{self.caret=value[..self.caret].char_indices().last().map_or(0,|(i,_)|i);self.all=false;},
            Key::Named(NamedKey::ArrowRight)=>{self.caret+=value[self.caret..].chars().next().map_or(0,char::len_utf8);self.all=false;},
            Key::Named(NamedKey::Home)=>{self.caret=0;self.all=false;},Key::Named(NamedKey::End)=>{self.caret=value.len();self.all=false;},
            _=>if let Some(text)=text{self.insert(text);},
        }
    }
    pub fn draw(&self,window:[u32;2],g:&Geometry,t:&Theme,error:Option<&str>,pointer:(f64,f64),pressed:bool,measure:&mut dyn FnMut(&TextRun)->TextMetrics)->(Vec<Layer>,Vec<Hit>){
        let s=g.scale;let mut p=Paint::empty(t,measure,s,window,pointer,pressed);let mut hits=vec![];
        p.shape(g.panel,12.0*s,[0,0,0,100],None);
        if matches!(self.kind,EditKind::Color(_)|EditKind::TerminalColor(_)|EditKind::Ansi(_)){self.draw_colour(&mut p,g,t,error,&mut hits);return (p.out.scene.layers,hits);}
        // The sheet is centred in the panel, whatever the panel's height.
        let oy=((g.h-330.0)/2.0).max(8.0);
        let box_rect=g.local(220.0,oy,548.0,330.0);p.shape(box_rect,12.0*s,t.colors.card,Some(t.colors.border));
        let (title,labels)=match self.kind{EditKind::Save=>("Save as new theme",vec!["Name","File name"]),EditKind::Import=>("Import theme",vec!["Source file","File name"]),EditKind::Export=>("Export theme",vec!["Destination file"]),
            EditKind::Palette=>("Import a colour scheme",vec![""]),_=>("",vec![])};
        // The colour file prompt has a two-line note in place of a field label.
        let palette=matches!(self.kind,EditKind::Palette);
        if palette{for(i,line)in["A colour file from iTerm2, Windows Terminal, Alacritty (TOML or YAML), Kitty,","Ghostty, WezTerm, Konsole, Xresources or Ptyxis."].iter().enumerate(){p.text(g.local(240.0,oy+52.0+i as f32*18.0,508.0,18.0),line,12.0*s,t.colors.muted_foreground,400,false);}}
        p.text(g.local(240.0,oy+16.0,500.0,32.0),title,16.0*s,t.colors.foreground,600,false);
        for(i,name)in labels.iter().take(self.values.len()).enumerate(){let y=if palette{oy+100.0}else{oy+62.0}+i as f32*68.0;if !name.is_empty(){p.text(g.local(240.0,y-20.0,500.0,18.0),name,13.0*s,t.colors.muted_foreground,400,false);}let input=g.local(240.0,y,508.0,32.0);
            self.input(&mut p,input,i,t,s);hits.push(Hit{rect:input,action:Action::Field(i)});
        }
        if let Some(error)=error{p.text(g.local(240.0,oy+210.0,508.0,42.0),error,13.0*s,t.colors.err,400,false);}
        for (x,name,action)in[(568.0,"Cancel",Action::Cancel),(664.0,if palette{"Import"}else{"Apply"},Action::Confirm)]{let rect=g.local(x,oy+276.0,84.0,32.0);button(&mut p,rect,name,t,s,pointer,pressed,true);hits.push(Hit{rect,action});}
        (p.out.scene.layers,hits)
    }
    /// One text input of a sheet, with its caret when focused.
    fn input(&self,p:&mut Paint<'_>,input:PixelRect,i:usize,t:&Theme,s:f32){
        let value=&self.values[i];
        p.shape(input,8.0*s,t.colors.background,Some(if self.focus==i{t.colors.ring}else{t.colors.input}));
        let caret=if self.focus==i{self.caret}else{value.len()};let mut start=0;while p.mono_width(&value[start..caret],14.0*s,400)>input.w-20.0*s{start+=value[start..].chars().next().unwrap().len_utf8();}
        p.text(r(input.x+10.0*s,input.y,input.w-20.0*s,input.h),&value[start..],14.0*s,t.colors.foreground,400,true);
        if self.focus==i{let x=input.x+10.0*s+p.mono_width(&value[start..caret],14.0*s,400);p.shape(r(x,input.y+8.0*s,s,16.0*s),0.0,t.colors.foreground,None);}
    }
    /// The colour picker: which colour it changes, its hex value, the current and the new colour side
    /// by side, and colours to start from: shades and hues of this colour, and the theme's own.
    fn draw_colour(&self,p:&mut Paint<'_>,g:&Geometry,t:&Theme,error:Option<&str>,hits:&mut Vec<Hit>){
        let s=g.scale;let c=&t.colors;let faint=crate::calm::scene_faint(c);
        const ANSI:[&str;8]=["Black","Red","Green","Yellow","Blue","Magenta","Cyan","White"];
        let (title,note)=match self.kind{
            EditKind::Ansi(i) if i<8=>(format!("{} \u{b7} ANSI {i}",ANSI[i]),"A normal terminal colour, saved in terminal_colors.".to_owned()),
            EditKind::Ansi(i)=>(format!("Bright {} \u{b7} ANSI {i}",ANSI[i%8].to_lowercase()),"A bright terminal colour, saved in terminal_colors.".to_owned()),
            EditKind::TerminalColor(key)=>(format!("Terminal {key}"),format!("The terminal's {key}, saved in terminal_colors.")),
            EditKind::Color(token)=>(label(token),format!("A window colour of {}, saved as <colors><{token}>.",t.name)),
            _=>(String::new(),String::new())};
        let oy=((g.h-374.0)/2.0).max(8.0);
        p.shape(g.local(220.0,oy,548.0,374.0),12.0*s,c.card,Some(c.border));
        p.text(g.local(240.0,oy+16.0,508.0,24.0),&title,16.0*s,c.foreground,600,false);
        p.text(g.local(240.0,oy+40.0,508.0,18.0),&note,13.0*s,c.muted_foreground,400,false);
        p.text(g.local(240.0,oy+72.0,300.0,18.0),"Hex colour",13.0*s,c.muted_foreground,400,false);
        let input=g.local(240.0,oy+94.0,300.0,32.0);self.input(p,input,0,t,s);hits.push(Hit{rect:input,action:Action::Field(0)});
        // Current and new, as two halves of one swatch.
        let parse=|text:&str|crate::themes::parse_color(text.trim()).ok();
        let current=parse(&self.start[0]);let new=parse(&self.values[0]);
        let opaque=matches!(self.kind,EditKind::Ansi(_)|EditKind::TerminalColor(_));
        let shown=|c:Color|if opaque{[c[0],c[1],c[2],255]}else{c};
        for (i,(name,color)) in [("Current",current),("New",new)].into_iter().enumerate(){
            let x=556.0+i as f32*96.0;let sw=g.local(x,oy+94.0,92.0,32.0);
            match color{Some(color)=>p.shape(sw,8.0*s,shown(color),Some(c.input)),None=>{p.shape(sw,8.0*s,c.background,Some(c.err));let w=p.width("Not a colour",12.0*s,400);p.text(r(sw.x+(sw.w-w)/2.0,sw.y,w,sw.h),"Not a colour",12.0*s,c.err,400,false);}}
            p.text(g.local(x,oy+72.0,92.0,18.0),name,12.0*s,faint,500,false);
        }
        let base=new.or(current).unwrap_or([128,128,128,255]);
        let (hue,sat,light)=hsl(base);
        let shades:Vec<Color>=(0..9).map(|i|from_hsl(hue,sat,0.1+0.1*i as f32,base[3])).collect();
        let hues:Vec<Color>=(0..12).map(|i|from_hsl(hue+i as f32*30.0,sat.max(0.45),light.clamp(0.35,0.7),base[3])).collect();
        let own:Vec<Color>=match self.kind{
            EditKind::Color(_)=>{let mut out:Vec<Color>=Vec::new();for token in COLOR_TOKENS{if let Some(color)=c.get(token){if !out.contains(&color){out.push(color);}}}out.truncate(16);out}
            _=>t.ansi.to_vec()};
        let own_name=if matches!(self.kind,EditKind::Color(_)){format!("{}'s window colours",t.name)}else{format!("{}'s terminal colours",t.name)};
        for (row,(name,colors)) in [("Shades of this colour",shades),("Hues",hues),(own_name.as_str(),own)].into_iter().enumerate(){
            let y=oy+142.0+row as f32*54.0;
            p.text(g.local(240.0,y,508.0,16.0),name,12.0*s,faint,500,false);
            for (i,color) in colors.into_iter().enumerate(){
                let rect=g.local(240.0+i as f32*31.75,y+20.0,26.0,26.0);
                p.shape(rect,5.0*s,shown(color),Some(c.input));
                if rect.contains(p.pointer.0,p.pointer.1){p.shape(r(rect.x-2.0*s,rect.y-2.0*s,rect.w+4.0*s,rect.h+4.0*s),7.0*s,[0;4],Some(c.ring));}
                hits.push(Hit{rect,action:Action::PickColor(hex(shown(color)))});
            }
        }
        if let Some(error)=error{p.text(g.local(240.0,oy+300.0,508.0,18.0),error,13.0*s,c.err,400,false);}
        let (pointer,pressed)=(p.pointer,p.pressed);
        for (x,name,action)in[(568.0,"Cancel",Action::Cancel),(664.0,"Apply",Action::Confirm)]{let rect=g.local(x,oy+326.0,84.0,32.0);button(p,rect,name,t,s,pointer,pressed,true);hits.push(Hit{rect,action});}
    }
}
