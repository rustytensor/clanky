//! Settings › Appearance: the Layout group and the theme picker with colour dots
//! (mockup approved 2026-11-13, sections 4 and 5).
use super::{scene::Paint,settings_frame::{self,Action,Geometry,Hit}};
use crate::{config::{BlockStyle,CommandInput,Config,TabStyle,TerminalColors,ThemePreference,Tool,TOOLS},look::PixelRect,theme_store::ThemeStore,themes::{Appearance,Color,Theme}};

fn r(x:f32,y:f32,w:f32,h:f32)->PixelRect{PixelRect{x,y,w:w.max(0.0),h:h.max(0.0)}}
/// Display order of the built-in themes in the picker (the mockup's columns).
pub const PICKER_DARK:[&str;9]=["graphite-dark","ink","stone-dark","paper-dark","catppuccin-mocha","catppuccin-macchiato","catppuccin-frappe","ai-bedazzled","unicorn-vomit"];
pub const PICKER_LIGHT:[&str;5]=["graphite-light","stone-light","paper-light","catppuccin-latte","unicorn-tears"];

#[derive(Clone)]
pub enum PickKind{Theme{id:String,appearance:Appearance,theme:Box<Theme>},Palette{name:String,value:serde_json::Value,colors:TerminalColors}}
/// One picker row, in keyboard order, with its top in page coordinates (before scrolling).
#[derive(Clone)]
pub struct PickItem{pub kind:PickKind,pub y:f32}

/// Ptyxis palettes imported into ~/.local/share/org.gnome.Ptyxis/palettes: name and [Dark] colours.
pub fn scan_palettes()->Vec<(String,serde_json::Value)>{
    let Some(home)=std::env::var_os("HOME") else{return vec![];};
    let dir=std::path::PathBuf::from(home).join(".local/share/org.gnome.Ptyxis/palettes");
    let Ok(entries)=std::fs::read_dir(&dir) else{return vec![];};
    let mut out=Vec::new();
    let mut paths:Vec<_>=entries.filter_map(Result::ok).map(|e|e.path()).filter(|p|p.extension().is_some_and(|e|e=="palette")).collect();
    paths.sort();
    for path in paths.into_iter().take(64){
        if std::fs::metadata(&path).map(|m|m.len()>64*1024).unwrap_or(true){continue;}
        let Ok(text)=std::fs::read_to_string(&path) else{continue;};
        let Ok(value)=crate::config::import_ptyxis_palette(&text) else{continue;};
        let mut section="";let mut name=None;
        for line in text.lines().map(str::trim){
            if line.starts_with('[') && line.ends_with(']'){section=&line[1..line.len()-1];continue;}
            if section=="Palette"{if let Some(v)=line.strip_prefix("Name="){name=Some(v.trim().to_owned());}}
        }
        let name=name.filter(|n|!n.is_empty()).unwrap_or_else(||path.file_stem().map(|s|s.to_string_lossy().into_owned()).unwrap_or_default());
        out.push((name.chars().filter(|c|!c.is_control()).take(80).collect(),value));
    }
    out
}

/// Greedy word wrap of `text` at `px` into lines no wider than `width`.
fn wrap(p:&mut Paint<'_>,text:&str,px:f32,width:f32)->Vec<String>{
    let mut lines:Vec<String>=Vec::new();let mut line=String::new();
    for word in text.split(' '){
        let candidate=if line.is_empty(){word.to_owned()}else{format!("{line} {word}")};
        if !line.is_empty() && p.width(&candidate,px,400)>width{lines.push(std::mem::replace(&mut line,word.to_owned()));}else{line=candidate;}
    }
    if !line.is_empty(){lines.push(line);}
    lines
}
/// Description lines shift everything under the first one down by this much (page px).
const LINE:f32=17.0;
/// A Layout row (mockup .row): label, the full description wrapped onto as many lines as it
/// needs, and a segmented control (mockup .segc) on the right. Returns the row's height.
#[allow(clippy::too_many_arguments)]
fn segments(p:&mut Paint<'_>,g:&Geometry,t:&Theme,y:f32,extra:f32,options:&[(&str,&'static str,&'static str)],selected:&str,hits:&mut Vec<Hit>,label:&str,description:&str,last:bool)->f32{
    let s=g.scale;let c=&t.colors;
    let widths:Vec<f32>=options.iter().map(|(name,_,_)|p.width(name,13.0*s,500)/s+26.0).collect();
    let total=widths.iter().sum::<f32>()+6.0;
    let text_width=522.0-total-24.0;
    let lines=wrap(p,description,13.0*s,text_width*s);
    let height=(if last{67.84375}else{68.84375})+extra+LINE*(lines.len().max(1)-1) as f32;
    if !settings_frame::row_fits(g,y){return height;}
    p.pane_label(g.local(229.0,y+14.0,text_width,18.0),label,14.0*s,c.foreground,500);
    for (i,text) in lines.iter().enumerate(){p.text(g.local(229.0,y+35.0+LINE*i as f32,text_width,18.84375),text,13.0*s,c.muted_foreground,400,false);}
    if !last{p.shape(g.local(229.0,y+height-1.0,522.0,1.0),0.0,c.border,None);}
    // The control sits level with the label when the row holds more under its text (the tool
    // chips, mockup align-items:start); otherwise it is centred, as the mockup's other rows.
    let grown=extra>0.0;
    let control=g.local(751.0-total,if grown{y+18.0}else{y+(height-if last{0.0}else{1.0}-32.0)/2.0},total,32.0);
    p.shape(control,8.0*s,c.card,Some(c.border));
    let mut x=control.x+3.0*s;
    for ((name,key,value),w) in options.iter().zip(widths){
        let b=r(x,control.y+3.0*s,w*s,control.h-6.0*s);let on=selected==*value;
        if on{p.shape(b,6.0*s,c.pressed,None);}else if b.contains(p.pointer.0,p.pointer.1){p.shape(b,6.0*s,c.accent,None);}
        let tw=p.width(name,13.0*s,500);
        p.text(r(b.x+(b.w-tw)/2.0,b.y,tw,b.h),name,13.0*s,if on{c.foreground}else{c.muted_foreground},500,false);
        hits.push(Hit{rect:b,action:Action::Choice(key,value)});x+=w*s;
    }
    height
}

/// The Layout group; returns the page y where it ends.
pub(super) fn layout_group(p:&mut Paint<'_>,g0:&Geometry,t:&Theme,cfg:&Config,hits:&mut Vec<Hit>)->f32{
    let s=g0.scale;let c=&t.colors;let g=g0;
    p.text(g.local(229.0,85.0,522.0,16.0),"Layout",12.0*s,c.muted_foreground,500,false);
    let tab=match cfg.tab_style{TabStyle::Horizontal=>"horizontal",TabStyle::Vertical=>"vertical"};
    let mut y=105.0;
    y+=segments(p,g,t,y,0.0,&[("Horizontal","tab_style","horizontal"),("Vertical","tab_style","vertical")],tab,hits,
        "Tab style","Tabs across the top, or a list down the left with folder, branch and last command.",false);
    let tools_y=y;
    y+=segments(p,g,t,y,35.15625,&[("On","tools_panel","on"),("Off","tools_panel","off")],if cfg.tools_panel{"on"}else{"off"},hits,
        "Tools panel","A panel on the left. Ctrl+Shift+B shows it or puts it away.",false);
    // Tool chips (mockup .tok): a check in the ok colour when the tool is in the panel.
    let mut x=229.0;
    for (tool,_,name) in TOOLS{
        let on=cfg.tools.contains(&tool);
        let tw=p.width(name,12.0*s,400)/s;let w=tw+20.0+if on{16.0}else{0.0};
        let chip=g.local(x,tools_y+62.0,w,24.0);
        p.shape(chip,6.0*s,[0;4],Some(if on{c.input}else{c.border}));
        let mut tx=chip.x+10.0*s;
        if on{settings_frame::icon(p,"check",r(tx-1.0*s,chip.y+5.0*s,14.0*s,14.0*s),c.ok,s*0.85);tx+=16.0*s;}
        let mut ink=c.foreground;if !on{ink=crate::calm::scene_faint(c);}
        p.text(r(tx,chip.y,tw*s+1.0,chip.h),name,12.0*s,ink,400,false);
        hits.push(Hit{rect:chip,action:Action::ToolChip(tool)});x+=w+6.0;
    }
    let input=match cfg.command_input{CommandInput::Editor=>"editor",CommandInput::Classic=>"classic"};
    y+=segments(p,g,t,y,0.0,&[("Editor","command_input","editor"),("Classic","command_input","classic")],input,hits,
        "Command input","An editor at the bottom of each pane, or the shell's own prompt.",false);
    let blocks=match cfg.block_style{BlockStyle::Quiet=>"quiet",BlockStyle::Cards=>"cards"};
    y+=segments(p,g,t,y,0.0,&[("Quiet","block_style","quiet"),("Cards","block_style","cards")],blocks,hits,
        "Blocks","Quiet hairlines between commands, or framed blocks.",false);
    y+=segments(p,g,t,y,0.0,&[("Show","status_line","show"),("Hide","status_line","hide")],if cfg.status_line{"show"}else{"hide"},hits,
        "Status line","Session, server and masking, in one row at the bottom.",true);
    y
}

/// One picker row (mockup .th): name, and a chip of the theme's background with four dots
/// in its ansi 1, 2, 4, 3 (red, green, blue, yellow).
fn theme_row(p:&mut Paint<'_>,rect:PixelRect,t:&Theme,name:&str,chip_bg:Color,dots:[Color;4],saved:bool,lit:bool,s:f32){
    let c=&t.colors;
    p.shape(rect,8.0*s,c.card,Some(c.border));
    if saved||lit{p.shape(rect,8.0*s,[0;4],Some(c.ring));p.shape(r(rect.x+s,rect.y+s,rect.w-2.0*s,rect.h-2.0*s),7.0*s,[0;4],Some(c.ring));}
    let chip=r((rect.x+rect.w-12.0*s-68.0*s).round(),(rect.y+(rect.h-21.0*s)/2.0).round(),68.0*s,21.0*s);
    p.text(r(rect.x+12.0*s,rect.y,(chip.x-rect.x-20.0*s).max(0.0),rect.h),name,13.0*s,c.foreground,400,false);
    p.shape(chip,chip.h/2.0,chip_bg,Some(c.border));
    for (i,color) in dots.into_iter().enumerate(){
        let cx=chip.x+7.0*s+(5.5+14.0*i as f32)*s;let cy=chip.y+chip.h/2.0;
        p.shape(r(cx-5.0*s,cy-5.0*s,10.0*s,10.0*s),5.0*s,color,None);
    }
}

/// The Theme group from page y `top`; returns where it ends and the rows in keyboard order.
#[allow(clippy::too_many_arguments)]
pub(super) fn theme_group(p:&mut Paint<'_>,g:&Geometry,t:&Theme,cfg:&Config,store:&ThemeStore,palettes:&[(String,serde_json::Value)],system_light:bool,highlight:Option<usize>,hits:&mut Vec<Hit>,top:f32)->Result<(f32,Vec<PickItem>),String>{
    let s=g.scale;let c=&t.colors;
    p.text(g.local(229.0,top,522.0,16.0),"Theme",12.0*s,c.muted_foreground,500,false);
    let follow=cfg.theme==ThemePreference::System;
    let row_y=top+20.0;
    let options=[("On","","on"),("Off","","off")];
    let widths:Vec<f32>=options.iter().map(|(name,_,_)|p.width(name,13.0*s,500)/s+26.0).collect();
    let total=widths.iter().sum::<f32>()+6.0;
    let control=settings_frame::row(p,g,t,row_y,total,"Follow the system","Use one theme in dark mode and another in light mode.",false);
    p.shape(control,8.0*s,c.card,Some(c.border));
    let mut x=control.x+3.0*s;
    for ((name,_,value),w) in options.iter().zip(widths){
        let b=r(x,control.y+3.0*s,w*s,control.h-6.0*s);let on=(*value=="on")==follow;
        if on{p.shape(b,6.0*s,c.pressed,None);}else if b.contains(p.pointer.0,p.pointer.1){p.shape(b,6.0*s,c.accent,None);}
        let tw=p.width(name,13.0*s,500);
        p.text(r(b.x+(b.w-tw)/2.0,b.y,tw,b.h),name,13.0*s,if on{c.foreground}else{c.muted_foreground},500,false);
        hits.push(Hit{rect:b,action:Action::Follow(*value=="on")});x+=w*s;
    }
    let entries=store.picker(None)?;
    let ordered=|list:&[&str],appearance:Appearance|{
        let mut out:Vec<_>=list.iter().filter_map(|id|entries.iter().find(|e|e.id==*id)).cloned().collect();
        out.extend(entries.iter().filter(|e|!e.builtin && e.appearance==appearance).cloned());out
    };
    let dark=ordered(&PICKER_DARK,Appearance::Dark);let light=ordered(&PICKER_LIGHT,Appearance::Light);
    let mut items=Vec::new();
    let mut y=row_y+68.84375+14.0;
    let faint=crate::calm::scene_faint(c);
    let saved_dark=cfg.theme_dark.as_str();let saved_light=cfg.theme_light.as_str();let saved_one=cfg.selected_theme_id(system_light);
    let push_theme=|p:&mut Paint<'_>,hits:&mut Vec<Hit>,items:&mut Vec<PickItem>,entry:&crate::theme_store::PickerEntry,x:f32,w:f32,y:f32|{
        let Ok(theme)=store.get(&entry.id) else{return;};
        let index=items.len();let rect=g.local(x,y,w,38.0);
        let saved=if follow{entry.id==if entry.appearance==Appearance::Dark{saved_dark}else{saved_light}}else{entry.id==saved_one};
        let a=entry.swatches.ansi;
        theme_row(p,rect,t,&entry.name,entry.swatches.background,[a[1],a[2],a[4],a[3]],saved && highlight.is_none(),highlight==Some(index),s);
        hits.push(Hit{rect,action:Action::Pick(index)});
        items.push(PickItem{kind:PickKind::Theme{id:entry.id.clone(),appearance:entry.appearance,theme:Box::new(theme)},y});
    };
    if follow{
        p.text(g.local(231.0,y,248.0,16.0),"In dark mode",12.0*s,faint,500,false);
        p.text(g.local(503.0,y,248.0,16.0),"In light mode",12.0*s,faint,500,false);
        y+=22.0;
        let mut yd=y;for entry in &dark{push_theme(p,hits,&mut items,entry,229.0,250.0,yd);yd+=44.0;}
        let mut yl=y;for entry in &light{push_theme(p,hits,&mut items,entry,501.0,250.0,yl);yl+=44.0;}
        y=yd.max(yl);
    }else{
        for entry in dark.iter().chain(&light){push_theme(p,hits,&mut items,entry,229.0,522.0,y);y+=44.0;}
    }
    if !palettes.is_empty(){
        y+=10.0;
        p.text(g.local(231.0,y,520.0,16.0),"Terminal colours from Ptyxis",12.0*s,faint,500,false);y+=22.0;
        for (name,value) in palettes{
            let Ok(colors)=TerminalColors::from_value(value) else{continue;};
            let index=items.len();let rect=g.local(229.0,y,522.0,38.0);
            let ansi=colors.ansi.unwrap_or(t.ansi);let bg=colors.background.unwrap_or(t.colors.background);
            let saved=cfg.terminal_colors==colors;
            theme_row(p,rect,t,name,bg,[ansi[1],ansi[2],ansi[4],ansi[3]],saved && highlight.is_none(),highlight==Some(index),s);
            hits.push(Hit{rect,action:Action::Pick(index)});
            items.push(PickItem{kind:PickKind::Palette{name:name.clone(),value:value.clone(),colors},y});y+=44.0;
        }
    }
    y+=6.0;
    // Mockup .th-foot, wrapped to the page width.
    for text in wrap(p,"Up and Down move, Enter applies. Moving the highlight previews the theme in the window behind Settings; Escape puts the old one back.",12.0*s,522.0*s){
        p.text(g.local(229.0,y,522.0,18.0),&text,12.0*s,faint,400,false);y+=18.0;
    }
    y+=8.0;
    let _=Tool::Files;
    Ok((y,items))
}
