//! Settings pages General, Terminal, Editor, History and the agent list: every setting of
//! docs/config.md has its row here or on Appearance, and each change is saved at once.
use super::{scene::Paint,settings_form::{self as form,Control,Editing,Row,row},settings_frame::{Action,Geometry,Hit}};
use crate::{config::{Config,CursorBlink},look::PixelRect,themes::{Color,Theme}};

fn r(x:f32,y:f32,w:f32,h:f32)->PixelRect{PixelRect{x,y,w:w.max(0.0),h:h.max(0.0)}}
/// A decimal setting as config.xml writes it (1, 1.25, 0.85).
pub(super) fn decimal(v:f32)->String{format!("{v}")}

pub(super) fn general(p:&mut Paint<'_>,g:&Geometry,t:&Theme,cfg:&Config,forced_post_ai:bool,editing:Option<Editing<'_>>,hits:&mut Vec<Hit>)->f32{
    let first=hits.len();
    let post_ai=if forced_post_ai{"Restart the server without --post-ai to turn this off."}else{"A plain terminal with tabs and splits. No agents, no AI features."};
    let rows:Vec<Row>=vec![
        row("Post-AI mode",post_ai,Control::Switch("post_ai",cfg.post_ai)),
        row("Restore layout","Reopen the last tabs and splits when Clanky starts.",Control::Switch("restore_layout",cfg.restore_layout)),
        row("Robot greeting","The robot says hello when a window opens.",Control::Switch("startup_animation",cfg.startup_animation)),
        row("Animations","Motion in the window, the robot's greeting included.",Control::Switch("animations",cfg.animations)),
        row("Notify after","A notice when a command runs this long, in seconds.",Control::Field("notify_after_seconds",cfg.notify_after_seconds.to_string(),80.0,true)),
        row("Columns","Width of a new window in characters. Applies to new windows.",Control::Field("columns",cfg.columns.to_string(),80.0,true)),
        row("Rows","Height of a new window in lines. Applies to new windows.",Control::Field("rows",cfg.rows.to_string(),80.0,true)),
    ];
    let end=form::rows(p,g,t,65.0,&rows,editing,hits);
    if forced_post_ai{let mut i=first;while i<hits.len(){if matches!(hits[i].action,Action::Toggle("post_ai",_)){hits.remove(i);}else{i+=1;}}}
    end
}

fn color_row(p:&mut Paint<'_>,g:&Geometry,t:&Theme,y:f32,name:&str,color:Color,action:Action,hits:&mut Vec<Hit>){
    let s=g.scale;let c=&t.colors;
    form::swatch(p,g.local(229.0,y+10.0,20.0,20.0),color,t,s);p.text(g.local(259.0,y,260.0,40.0),name,14.0*s,c.foreground,400,false);
    let value=format!("#{:02x}{:02x}{:02x}",color[0],color[1],color[2]);let width=p.mono_width(&value,13.0*s,400);let local=g.local(751.0,y,0.0,40.0);
    p.text(r(local.x-width,local.y,width,40.0*s),&value,13.0*s,c.muted_foreground,400,true);
    hits.push(Hit{rect:g.local(229.0,y,522.0,40.0),action});
    p.shape(g.local(229.0,y+39.0,522.0,1.0),0.0,c.border,None);
}

pub(super) fn terminal(p:&mut Paint<'_>,g:&Geometry,t:&Theme,cfg:&Config,editing:Option<Editing<'_>>,hits:&mut Vec<Hit>)->f32{
    let s=g.scale;let c=&t.colors;
    let blink=match cfg.cursor_blink{CursorBlink::System=>"system",CursorBlink::On=>"on",CursorBlink::Off=>"off"};
    let mut y=form::heading(p,g,t,85.0,"Cursor");
    y=form::rows(p,g,t,y,&[
        row("Cursor blink","System follows the GNOME setting.",Control::Segments(vec![("System","cursor_blink","system"),("On","cursor_blink","on"),("Off","cursor_blink","off")],blink)),
        row("Blink cycle","One full blink, shown and hidden, in milliseconds.",Control::Field("cursor_blink_time_ms",cfg.cursor_blink_time_ms.to_string(),80.0,true)),
        row("Stop blinking after","Seconds after the last key press.",Control::Field("cursor_blink_timeout_s",cfg.cursor_blink_timeout_s.to_string(),80.0,true)),
    ],editing,hits);
    y=form::heading(p,g,t,y+20.0,"Clipboard");
    y=form::rows(p,g,t,y,&[
        row("Clipboard writes","Programs in the terminal may copy text (OSC 52).",Control::Switch("osc52_write",cfg.osc52_write)),
        row("Clipboard reads","Programs in the terminal may read what you copied.",Control::Switch("osc52_read",cfg.osc52_read)),
    ],editing,hits);
    y=form::heading(p,g,t,y+20.0,"Colours");
    let tc=&cfg.terminal_colors;
    for (key,name,color) in [("background","Background",tc.background.unwrap_or(c.background)),("foreground","Foreground",tc.foreground.unwrap_or(c.foreground)),("cursor","Cursor",tc.cursor.or(tc.foreground).unwrap_or(c.foreground))]{
        color_row(p,g,t,y,name,color,Action::TerminalColor(key),hits);y+=40.0;
    }
    let ansi=tc.ansi.unwrap_or(t.ansi);
    for (row_index,name) in ["Normal","Bright"].into_iter().enumerate(){
        let top=y+12.0;
        p.text(g.local(229.0,top,90.0,30.0),name,14.0*s,c.foreground,400,false);
        for i in 0..8{
            let index=row_index*8+i;let rect=g.local(331.0+i as f32*52.5,top,30.0,30.0);
            form::swatch(p,rect,ansi[index],t,s);
            if rect.contains(p.pointer.0,p.pointer.1){p.shape(r(rect.x-3.0*s,rect.y-3.0*s,rect.w+6.0*s,rect.h+6.0*s),8.0*s,[0;4],Some(c.ring));}
            hits.push(Hit{rect,action:Action::AnsiColor(index)});
        }
        y+=42.0;
    }
    p.text(g.local(229.0,y+6.0,522.0,18.0),"Normal and bright are ANSI colours 0 to 7 and 8 to 15. Click one to change it.",12.0*s,crate::calm::scene_faint(c),400,false);
    y+=30.0;
    let mut x=229.0;
    for(name,action)in[("Import Ptyxis palette…",Action::ImportPalette),("Use theme colours",Action::ThemeTerminalColors)]{
        let b=form::button(p,g,t,x,y+8.0,name);hits.push(Hit{rect:b,action});x+=b.w/s+8.0;
    }
    y+48.0
}

pub(super) fn editor(p:&mut Paint<'_>,g:&Geometry,t:&Theme,cfg:&Config,editing:Option<Editing<'_>>,hits:&mut Vec<Hit>)->f32{
    let y=form::heading(p,g,t,85.0,"Compose");
    form::rows(p,g,t,y,&[
        row("Dictionary","The spelling language of the compose editor.",Control::Field("compose_dictionary",cfg.compose_dictionary.clone(),120.0,true)),
        row("Dictionary folder","Where the spelling dictionaries are.",Control::Field("compose_dictionary_dir",cfg.compose_dictionary_dir.to_string_lossy().into_owned(),240.0,true)),
        row("Enter after send","Press Enter in the pane after a draft is sent.",Control::Switch("compose_send_enter",cfg.compose_send_enter)),
    ],editing,hits)
}

pub(super) fn history(p:&mut Paint<'_>,g:&Geometry,t:&Theme,cfg:&Config,editing:Option<Editing<'_>>,hits:&mut Vec<Hit>)->f32{
    form::rows(p,g,t,65.0,&[
        row("Save history","Keep commands and their output on this computer.",Control::Switch("history",cfg.history)),
        row("Keep for","Days of saved history. Zero keeps everything.",Control::Field("history_days",cfg.history_days.to_string(),80.0,true)),
        row("Output per command","The output saved with each command, in KiB.",Control::Field("history_output_kib",cfg.history_output_kib.to_string(),80.0,true)),
    ],editing,hits)
}

/// The agent list (agent_pane_access, agents); returns where the access rows start.
pub(super) fn agents(p:&mut Paint<'_>,g:&Geometry,t:&Theme,cfg:&Config,editing:Option<Editing<'_>>,adding:Option<&[String]>,hits:&mut Vec<Hit>)->f32{
    let s=g.scale;let c=&t.colors;
    let mut y=65.0;
    if cfg.post_ai{
        p.text(g.local(229.0,y+8.0,522.0,18.0),"Post-AI mode is on, so no agent runs. These settings apply once it is off.",13.0*s,c.muted_foreground,400,false);
        y+=36.0;
    }
    let mut y=form::rows(p,g,t,y,&[row("New agent panes","What an agent started in a new pane may do.",
        Control::Segments(vec![("Read + run","agent_pane_access","read+run"),("Off","agent_pane_access","off")],if cfg.agent_pane_access{"read+run"}else{"off"}))],None,hits);
    y=form::heading(p,g,t,y+20.0,"Agent tools");
    for (i,agent) in cfg.agents.iter().enumerate(){
        if !super::settings_frame::row_fits(g,y){y+=form::ROW;continue;}
        let w=p.width("Remove",13.0*s,500)/s+20.0;
        let control=super::settings_frame::row(p,g,t,y,w,&agent.name,"",false);
        let mut cmd=c.muted_foreground;cmd[3]=cmd[3].max(200);
        p.text(g.local(229.0,y+35.0,522.0-w-24.0,18.84375),&agent.command,13.0*s,cmd,400,true);
        let b=r(control.x,control.y+2.0*s,control.w,28.0*s);
        let fill=if b.contains(p.pointer.0,p.pointer.1){if p.pressed{c.pressed}else{c.accent}}else{c.background};p.shape(b,6.0*s,fill,Some(c.input));
        let tw=p.width("Remove",13.0*s,500);p.text(r(b.x+(b.w-tw)/2.0,b.y,tw,b.h),"Remove",13.0*s,c.foreground,500,false);
        hits.push(Hit{rect:b,action:Action::RemoveAgent(i)});
        y+=form::ROW;
    }
    if let Some(values)=adding{
        let faint=crate::calm::scene_faint(c);
        p.text(g.local(229.0,y+10.0,200.0,16.0),"Name",12.0*s,faint,500,false);
        p.text(g.local(441.0,y+10.0,310.0,16.0),"Command",12.0*s,faint,500,false);
        for (index,x,w) in [(0usize,229.0,200.0),(1,441.0,310.0)]{
            let rect=g.local(x,y+30.0,w,32.0);
            let edit=editing.filter(|e|e.key=="agents" && e.index==index);
            form::field(p,rect,values.get(index).map_or("",String::as_str),index==1,edit,t,s,if index==0{"Claude Code"}else{"claude --flags"});
            hits.push(Hit{rect,action:Action::Field(index)});
        }
        p.text(g.local(229.0,y+70.0,522.0,18.0),"Tab moves to the command. Enter adds the agent; Escape cancels.",12.0*s,faint,400,false);
        y+=100.0;
    }else if cfg.agents.len()<32{
        let b=form::button(p,g,t,229.0,y+12.0,"Add agent");hits.push(Hit{rect:b,action:Action::AddAgent});
        y+=52.0;
    }
    if cfg.post_ai{return y;}
    form::heading(p,g,t,y+16.0,"Access")
}
