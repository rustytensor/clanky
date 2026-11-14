//! The quiet window around the panes (mockup approved 2026-11-13, sections 1 and 2): the title
//! bar with tabs, search, settings and the tools toggle; the vertical tab list; the tools panel
//! (Files, Recent, Agents); the status line; and quiet command blocks over the live grid.
use super::scene::{Paint,TermColors,alpha,blend,duration_label};
use crate::{config::Tool,look::{self,HitAction,Layout,LookState,PixelRect},themes::Color};
use clanky_render::{Layer,Matrix4,PathStroke,VectorLayer,VectorPath};
use clanky_render::vector::lyon::{math::point,path::Path};

fn r(x:f32,y:f32,w:f32,h:f32)->PixelRect{PixelRect{x,y,w:w.max(0.0),h:h.max(0.0)}}
/// An opaque colour: `top` (with its alpha) laid over `under`.
pub(super) fn over(under:Color,top:Color)->Color{
    let a=top[3] as f32/255.0;let mut c=blend(under,top,a);c[3]=255;c
}
/// The mockup's faint text (#6b6b6b on Graphite): muted text halfway to the background.
pub(super) fn faint(p:&Paint)->Color{super::scene_faint(&p.theme.colors)}
fn line(p:&mut Paint,x:f32,y:f32,w:f32,h:f32,color:Color){p.shape(r(x,y,w,h),0.0,color,None);}
fn stroke(p:&mut Paint,path:Path,x:f32,y:f32,color:Color,width:f32){
    let s=p.scale;let clip=p.clip;
    p.out.scene.layers.push(Layer::Vector(VectorLayer{paths:vec![VectorPath{path,fill:None,stroke:Some(PathStroke{width,color}),
        transform:Matrix4::scale(s,s,1.0).then(Matrix4::translate(x,y,0.0))}],clip,opacity:1.0}));
}
fn close_mark(p:&mut Paint,x:f32,y:f32,size:f32,color:Color){
    let mut path=Path::builder();path.begin(point(0.0,0.0));path.line_to(point(size,size));path.end(false);
    path.begin(point(size,0.0));path.line_to(point(0.0,size));path.end(false);stroke(p,path.build(),x,y,color,1.4);
}
fn plus_mark(p:&mut Paint,x:f32,y:f32,size:f32,color:Color){
    let h=size/2.0;let mut path=Path::builder();path.begin(point(h,0.0));path.line_to(point(h,size));path.end(false);
    path.begin(point(0.0,h));path.line_to(point(size,h));path.end(false);stroke(p,path.build(),x,y,color,1.5);
}
fn chevron(p:&mut Paint,x:f32,y:f32,open:bool,color:Color){
    let mut path=Path::builder();
    if open{path.begin(point(0.0,2.0));path.line_to(point(3.5,5.5));path.line_to(point(7.0,2.0));}else{path.begin(point(2.0,0.0));path.line_to(point(5.5,3.5));path.line_to(point(2.0,7.0));}
    path.end(false);stroke(p,path.build(),x,y,color,1.3);
}
/// Text drawn word by word with a slightly wider space (`extra`), so small labels keep their word breaks.
fn spaced_width(p:&mut Paint,text:&str,px:f32,weight:u16,extra:f32)->f32{
    let space=p.width(" ",px,weight)+extra*p.scale;
    text.split(' ').map(|w|p.width(w,px,weight)).sum::<f32>()+space*(text.split(' ').count()-1) as f32
}
fn spaced_by(p:&mut Paint,x:f32,y:f32,h:f32,text:&str,px:f32,color:Color,weight:u16,extra:f32){
    let space=p.width(" ",px,weight)+extra*p.scale;let mut x=x;
    for word in text.split(' '){let w=p.width(word,px,weight);p.text(r(x,y,w+0.5,h),word,px,color,weight,false);x+=w+space;}
}
fn spaced(p:&mut Paint,x:f32,y:f32,h:f32,text:&str,px:f32,color:Color,weight:u16){
    let space=p.width(" ",px,weight)+2.0*p.scale;let mut x=x;
    for word in text.split(' '){let w=p.width(word,px,weight);p.text(r(x,y,w+0.5,h),word,px,color,weight,false);x+=w+space;}
}
/// Mono text one character per advance, as the terminal grid draws it: a command such as
/// "vtlastcmd" never turns into a font ligature.
fn mono_chars(p:&mut Paint,rect:PixelRect,text:&str,px:f32,color:Color,weight:u16){
    let advance=p.mono_width("M",px,weight).max(1.0);let mut x=rect.x;
    let count=text.chars().count();let fit=((rect.w/advance).floor() as usize).max(1);
    for (i,ch) in text.chars().enumerate(){
        if x+advance>rect.x+rect.w+0.5{break;}
        let shown=if count>fit && i+1==fit{'…'}else if ch.is_control(){' '}else{ch};
        if shown!=' '{p.text(r(x,rect.y,advance+1.0,rect.h),&shown.to_string(),px,color,weight,true);}
        x+=advance;
    }
}
fn dot(p:&mut Paint,x:f32,y:f32,d:f32,color:Color){p.shape(r(x,y,d,d),d/2.0,color,None);}
fn hovered(p:&Paint,rect:PixelRect)->bool{rect.contains(p.pointer.0,p.pointer.1)}
/// A sidebar-coloured surface (title bar, tab column, tools panel, status line). The window
/// backdrop is already the sidebar colour, or the theme's window gradient, which must show here.
fn side(p:&mut Paint,rect:PixelRect){if p.theme.window_gradient.is_none(){let c=p.theme.colors.sidebar;p.shape(rect,0.0,c,None);}}

/// Title bar (mockup .titlebar): 38 px on the sidebar colour with a 1 px bottom hairline.
pub(super) fn title_bar(p:&mut Paint,layout:&Layout,state:&LookState){
    let s=p.scale;let c=p.theme.colors.clone();let bar=layout.top_bar;if bar.h<=0.0{return;}
    let side_line=over(c.sidebar,c.border);let dim=c.muted_foreground;let faint=faint(p);
    side(p,bar);
    line(p,bar.x,bar.y+bar.h-s,bar.w,s,side_line);
    let cy=bar.y+(bar.h-s)/2.0;
    // Tools toggle (.ico, 26 x 24, radius 6): filled while the tools panel shows.
    let toggle=r(bar.x+10.0*s,cy-12.0*s,26.0*s,24.0*s);
    if layout.rails_shown{p.shape(toggle,6.0*s,c.muted,None);}else if hovered(p,toggle){p.shape(toggle,6.0*s,if p.pressed{c.pressed}else{c.muted},None);}
    p.icon("sidebar",toggle.x+5.0*s,toggle.y+4.0*s,if layout.rails_shown{c.foreground}else{dim});
    p.hit(toggle,HitAction::ToggleRails);
    // Settings gear at the right, then the search field (.search 260 x 26) left of it.
    let gear=r(bar.x+bar.w-10.0*s-26.0*s,cy-12.0*s,26.0*s,24.0*s);
    if hovered(p,gear){p.shape(gear,6.0*s,if p.pressed{c.pressed}else{c.muted},None);}
    p.icon("settings",gear.x+5.0*s,gear.y+4.0*s,dim);p.hit(gear,HitAction::Settings);
    let field=r((gear.x-4.0*s-260.0*s).round(),(cy-13.0*s).round(),260.0*s,26.0*s);
    let mut right_edge=bar.x+bar.w;
    if field.x>toggle.x+toggle.w+120.0*s{
        let edge=if state.palette_open{c.ring}else if hovered(p,field){blend(side_line,c.foreground,0.15)}else{side_line};
        p.shape(field,5.0*s,c.card,Some(edge));
        p.icon("search",field.x+8.0*s,field.y+5.5*s,faint);
        let keys="Ctrl Shift P";let kw=p.mono_width(keys,12.0*s,400);
        p.text(r(field.x+field.w-9.0*s-kw,field.y,kw,field.h),keys,12.0*s,faint,400,true);
        p.text(r(field.x+28.0*s,field.y,(field.w-45.0*s-kw).max(0.0),field.h),"Search or run a command",12.0*s,faint,400,false);
        p.hit(field,HitAction::Palette);
        right_edge=field.x;
    }
    // "program has the mouse" for the focused pane (a right-click goes to the program).
    if let Some(owner)=&state.mouse_owner{
        let w=p.width(owner,12.0*s,500).min(320.0*s);let chip=r(right_edge-12.0*s-w-16.0*s,cy-11.0*s,w+16.0*s,22.0*s);
        if chip.x>toggle.x+200.0*s{p.shape(chip,6.0*s,c.muted,None);p.text(r(chip.x+8.0*s,chip.y,w,chip.h),owner,12.0*s,c.foreground,500,false);right_edge=chip.x;}
    }
    if layout.tabs_column.is_some(){return;}
    // Horizontal tabs (.tab): padding 4 x 11, radius 7, gap 2; a 6 px state dot, 7 px gaps,
    // the folder name, and a close mark on the active or hovered tab; "+" after the last.
    let mut x=toggle.x+toggle.w+12.0*s;let limit=right_edge-16.0*s;
    let count=state.tabs.len().max(1);
    let each=((limit-x-30.0*s)/count as f32-2.0*s).max(60.0*s);
    for tab in &state.tabs{
        let title_px=13.0*s;let natural=p.width(&tab.title,title_px,if tab.active{500}else{400});
        let pill_h=26.0*s;let pill_y=(cy-pill_h/2.0).round();
        let reserve=if tab.active{15.0*s}else{0.0};
        let width=(11.0*s+6.0*s+7.0*s+natural+reserve+11.0*s).min(each.max(80.0*s));
        if x+width>limit{break;}
        let pill=r(x,pill_y,width,pill_h);let over_tab=hovered(p,pill);let show_x=tab.active||over_tab;
        if tab.active{p.shape(pill,7.0*s,c.muted,None);}else if over_tab{p.shape(pill,7.0*s,blend(c.sidebar,c.muted,0.6),None);}
        // An agent's program always runs in the foreground, so agent comes before running.
        let dot_color=if tab.agent{c.agent}else if tab.running{c.ok}else{faint};
        dot(p,pill.x+11.0*s,pill.y+(pill_h-6.0*s)/2.0,6.0*s,dot_color);
        let tx=pill.x+11.0*s+6.0*s+7.0*s;let tw=(pill.x+pill.w-11.0*s-if show_x{15.0*s}else{0.0}-tx).max(0.0);
        p.text(r(tx,pill.y,tw,pill.h),&tab.title,title_px,if tab.active{c.foreground}else{dim},if tab.active{500}else{400},false);
        p.hit(pill,HitAction::Session(tab.id.clone()));
        if show_x{
            let mx=tx+natural.min(tw)+7.0*s;let my=pill.y+(pill_h-7.0*s)/2.0;
            let mark=r(mx-3.0*s,my-3.0*s,13.0*s,13.0*s);
            close_mark(p,mx,my,7.0,if hovered(p,mark){c.foreground}else{faint});
            p.hit(mark,HitAction::CloseTab(tab.panes.clone()));
        }
        x+=width+2.0*s;
    }
    let plus=r(x+4.0*s,cy-12.0*s,24.0*s,24.0*s);
    if hovered(p,plus){p.shape(plus,6.0*s,c.muted,None);}
    plus_mark(p,plus.x+7.0*s,plus.y+7.0*s,10.0,dim);p.hit(plus,HitAction::NewTab);
}

/// The vertical tab list (mockup picture 2, .vtabs): folder and branch, last command, state.
pub(super) fn tabs_column(p:&mut Paint,layout:&Layout,state:&LookState){
    let Some(col)=layout.tabs_column else{return;};let s=p.scale;let c=p.theme.colors.clone();let faint=faint(p);
    side(p,col);line(p,col.x+col.w-s,col.y,s,col.h,over(c.sidebar,c.border));
    let x=col.x+8.0*s;let w=col.w-16.0*s-s;let mut y=col.y+8.0*s;
    p.text(r(x+9.0*s,y,w-18.0*s,22.0*s),"Session 1",12.0*s,faint,500,false);y+=26.0*s;
    for tab in &state.tabs{
        let row=r(x,y,w,64.0*s);if row.y+row.h>col.y+col.h-40.0*s{break;}
        // A light fill (60 % of the way to muted) keeps the row's small text crisp.
        if tab.active{p.shape(row,7.0*s,blend(c.sidebar,c.muted,0.6),None);}else if hovered(p,row){p.shape(row,7.0*s,blend(c.sidebar,c.muted,0.35),None);}
        p.text(r(row.x+9.0*s,row.y+6.0*s,row.w-30.0*s,17.0*s),&tab.path,12.0*s,faint,400,true);
        mono_chars(p,r(row.x+9.0*s,row.y+23.0*s,row.w-18.0*s,19.0*s),if tab.last_command.is_empty(){"–"}else{&tab.last_command},13.0*s,c.foreground,400);
        // An agent's program always runs in the foreground, so agent comes before running.
        let dot_color=if tab.agent{c.agent}else if tab.running{c.ok}else{faint};
        dot(p,row.x+9.0*s,row.y+48.0*s,6.0*s,dot_color);
        p.text(r(row.x+21.0*s,row.y+42.0*s,row.w-30.0*s,17.0*s),&tab.state,12.0*s,faint,400,false);
        p.hit(row,HitAction::Session(tab.id.clone()));
        if tab.active || hovered(p,row){
            let mark=r(row.x+row.w-20.0*s,row.y+6.0*s,14.0*s,14.0*s);
            close_mark(p,mark.x+3.5*s,mark.y+3.5*s,7.0,faint);p.hit(mark,HitAction::CloseTab(tab.panes.clone()));
        }
        y+=68.0*s;
    }
    let add=r(x,y,w,30.0*s);if hovered(p,add){p.shape(add,7.0*s,blend(c.sidebar,c.muted,0.6),None);}
    plus_mark(p,add.x+10.0*s,add.y+10.0*s,10.0,c.muted_foreground);
    p.text(r(add.x+28.0*s,add.y,add.w-36.0*s,add.h),"New tab",13.0*s,c.muted_foreground,400,false);p.hit(add,HitAction::NewTab);
}

/// The tools panel (mockup .tools): a head, the chosen tool, and a footer to switch tools.
pub(super) fn tools_panel(p:&mut Paint,layout:&Layout,state:&LookState){
    let Some(panel)=layout.left_rail else{return;};let s=p.scale;let c=p.theme.colors.clone();let faint=faint(p);
    let side_line=over(c.sidebar,c.border);
    side(p,panel);line(p,panel.x+panel.w-s,panel.y,s,panel.h,side_line);
    let inner=r(panel.x,panel.y,panel.w-s,panel.h);
    let tools:Vec<Tool>=state.tools.iter().copied().filter(|t|!(state.post_ai && *t==Tool::Agents)).collect();
    let foot_h=if tools.is_empty(){0.0}else{42.0*s};
    let foot_y=inner.y+inner.h-foot_h;
    if !tools.is_empty(){
        line(p,inner.x,foot_y,inner.w,s,side_line);
        let gap=4.0*s;let seg_w=(inner.w-16.0*s-gap*(tools.len()-1) as f32)/tools.len() as f32;
        for (i,tool) in tools.iter().enumerate(){
            let seg=r(inner.x+8.0*s+i as f32*(seg_w+gap),foot_y+8.0*s,seg_w,26.0*s);let on=state.tool==Some(*tool);
            if on{p.shape(seg,6.0*s,c.muted,None);}else if hovered(p,seg){p.shape(seg,6.0*s,blend(c.sidebar,c.muted,0.6),None);}
            let label=tool.label();let tw=p.width(label,12.0*s,on.then_some(500).unwrap_or(400));
            p.text(r(seg.x+(seg.w-tw)/2.0,seg.y,tw,seg.h),label,12.0*s,if on{c.foreground}else{c.muted_foreground},if on{500}else{400},false);
            p.hit(seg,HitAction::Tool(*tool));
        }
    }
    let Some(tool)=state.tool.filter(|t|tools.contains(t)) else{return;};
    let head=r(inner.x+8.0*s,inner.y+8.0*s,inner.w-16.0*s,24.0*s);
    let title=match tool{Tool::Files=>state.tree_title.as_str(),Tool::Recent=>"Recent",Tool::Agents=>"Agents"};
    // Files head (mockup .tools-head): the folder name, then search and + icons at the right.
    let mut name_w=head.w-8.0*s;
    if tool==Tool::Files{
        let plus=r(head.x+head.w-24.0*s,head.y,24.0*s,24.0*s);let find=r(plus.x-2.0*s-24.0*s,head.y,24.0*s,24.0*s);
        for (rect,action) in [(find,HitAction::Palette),(plus,HitAction::NewTab)]{
            let over_icon=hovered(p,rect);if over_icon{p.shape(rect,6.0*s,if p.pressed{c.pressed}else{c.muted},None);}
            let ink=if over_icon{c.foreground}else{c.muted_foreground};
            if matches!(action,HitAction::Palette){p.icon("search",rect.x+4.5*s,rect.y+4.5*s,ink);}else{plus_mark(p,rect.x+7.0*s,rect.y+7.0*s,10.0,ink);}
            p.hit(rect,action);
        }
        name_w=(find.x-4.0*s-head.x-4.0*s).max(0.0);
    }
    p.text(r(head.x+4.0*s,head.y,name_w,head.h),title,13.0*s,c.foreground,500,false);
    let body=r(inner.x+8.0*s,head.y+head.h+6.0*s,inner.w-16.0*s,(foot_y-head.y-head.h-12.0*s).max(0.0));
    match tool{
        Tool::Files=>files(p,body,state,faint),
        Tool::Recent=>recent(p,body,state,faint),
        Tool::Agents=>agents(p,body,state,faint),
    }
}
fn files(p:&mut Paint,body:PixelRect,state:&LookState,faint:Color){
    let s=p.scale;let c=p.theme.colors.clone();let mut y=body.y;
    if state.tree.is_empty(){p.text(r(body.x+6.0*s,y,body.w-12.0*s,24.0*s),if state.tree_title.is_empty(){"No folder to show"}else{"This folder is empty"},12.0*s,faint,400,false);return;}
    for row in &state.tree{
        let rect=r(body.x,y,body.w,23.0*s);if rect.y+rect.h>body.y+body.h{break;}
        if hovered(p,rect){p.shape(rect,5.0*s,if p.pressed{c.pressed}else{c.muted},None);}
        let indent=row.depth as f32*12.0*s;
        if row.dir{chevron(p,rect.x+6.0*s+indent,rect.y+8.0*s,row.open,faint);}
        let tx=rect.x+6.0*s+indent+if row.dir{14.0*s}else{14.0*s};
        p.text(r(tx,rect.y,(rect.x+rect.w-6.0*s-tx).max(0.0),rect.h),&row.name,13.0*s,if row.dir{c.foreground}else{c.muted_foreground},400,false);
        p.hit(rect,if row.dir{HitAction::TreeToggle(row.path.clone())}else{HitAction::OpenPath(row.path.clone())});
        y+=24.0*s;
    }
}
fn recent(p:&mut Paint,body:PixelRect,state:&LookState,faint:Color){
    let s=p.scale;let c=p.theme.colors.clone();let mut y=body.y;
    if state.timeline.is_empty(){p.text(r(body.x+6.0*s,y,body.w-12.0*s,24.0*s),"No recent events",12.0*s,faint,400,false);return;}
    for (index,event) in state.timeline.iter().enumerate().rev(){
        let rect=r(body.x,y,body.w,40.0*s);if rect.y+rect.h>body.y+body.h{break;}
        let tone=match state.timeline_exits.get(index).copied().flatten(){Some(0)=>c.ok,Some(_)=>c.err,None=>faint};
        dot(p,rect.x+6.0*s,rect.y+8.0*s,6.0*s,tone);
        let time=state.timeline_times.get(index).map(String::as_str).unwrap_or("");let tw=p.width(time,12.0*s,400);
        p.text(r(rect.x+rect.w-6.0*s-tw,rect.y+1.0*s,tw,18.0*s),time,12.0*s,faint,400,false);
        p.text(r(rect.x+18.0*s,rect.y+1.0*s,(rect.w-30.0*s-tw).max(0.0),18.0*s),&event.label,13.0*s,c.foreground,400,false);
        p.text(r(rect.x+18.0*s,rect.y+19.0*s,rect.w-24.0*s,16.0*s),&event.detail,12.0*s,faint,400,false);
        y+=42.0*s;
    }
}
fn agents(p:&mut Paint,body:PixelRect,state:&LookState,faint:Color){
    let s=p.scale;let c=p.theme.colors.clone();let mut y=body.y;
    let total:u64=state.activity.iter().filter(|n|n.is_finite() && **n>0.0).map(|n|*n as u64).fold(0,u64::saturating_add);
    spaced(p,body.x+6.0*s,y,18.0*s,"Agent activity",12.0*s,c.muted_foreground,500);y+=20.0*s;
    p.text(r(body.x+6.0*s,y,body.w-12.0*s,18.0*s),&format!("{total} {} in 30 min",if total==1{"command"}else{"commands"}),13.0*s,c.foreground,400,false);y+=22.0*s;
    for item in &state.agents{
        if y+22.0*s>body.y+body.h{break;}
        let name=state.agent_labels.get(&item.connection_id).cloned().unwrap_or_else(||"Agent".into());
        dot(p,body.x+6.0*s,y+7.0*s,6.0*s,if item.running{c.agent}else{faint});
        p.text(r(body.x+18.0*s,y,body.w-24.0*s,20.0*s),&format!("{name} · {}",if item.running{"running"}else{"idle"}),13.0*s,c.foreground,400,false);
        p.hit(r(body.x,y,body.w,20.0*s),HitAction::Agent{connection_id:item.connection_id,tab_id:item.tab_id.clone()});y+=22.0*s;
    }
    y+=10.0*s;
    spaced(p,body.x+6.0*s,y,18.0*s,"Agent access",12.0*s,c.muted_foreground,500);y+=22.0*s;
    for row in &state.access{
        let rect=r(body.x,y,body.w,40.0*s);if rect.y+rect.h>body.y+body.h{break;}
        let over_row=hovered(p,rect);
        if over_row{p.shape(rect,6.0*s,if p.pressed{c.pressed}else{c.muted},None);}
        p.pane_label(r(rect.x+6.0*s,rect.y+3.0*s,rect.w-54.0*s,18.0*s),&row.label,13.0*s,c.foreground,400);
        p.text(r(rect.x+6.0*s,rect.y+21.0*s,rect.w-54.0*s,16.0*s),if row.allowed{"read + run"}else{"off"},12.0*s,c.muted_foreground,400,false);
        let sw=r(rect.x+rect.w-6.0*s-28.0*s,rect.y+12.0*s,28.0*s,16.0*s);
        p.shape(sw,8.0*s,if row.allowed{c.primary}else{c.input},None);
        p.shape(r(sw.x+if row.allowed{14.0*s}else{2.0*s},sw.y+2.0*s,12.0*s,12.0*s),6.0*s,if row.allowed{c.primary_foreground}else{c.background},None);
        p.hit(rect,HitAction::Access(row.id.clone()));
        y+=42.0*s;
    }
}

/// The status line (mockup .status): session, tabs and server on the left; masking and agents on the right.
pub(super) fn status_line(p:&mut Paint,layout:&Layout,state:&LookState){
    let Some(bar)=layout.status_line else{return;};let s=p.scale;let c=p.theme.colors.clone();
    side(p,bar);line(p,bar.x,bar.y,bar.w,s,over(c.sidebar,c.border));
    let y=bar.y+s;let h=bar.h-s;let gap=16.0*s;let px=12.0*s;let faint=c.muted_foreground;
    let tabs=state.tabs.len().max(1);
    let left=["Session 1".to_string(),format!("{tabs} {}",if tabs==1{"tab"}else{"tabs"}),super::uptime_label(state.uptime_seconds)];
    let mut x=bar.x+12.0*s;
    for text in &left{
        // Words go out one by one with a clear gap, so a count never runs into its noun.
        for (i,word) in text.split(' ').enumerate(){
            if i>0{x+=p.mono_width(" ",px,400)+3.0*s;}
            let w=p.mono_width(word,px,400);p.text(r(x,y,w,h),word,px,faint,400,true);x+=w;
        }
        x+=gap;
    }
    let mut right=vec![if state.masking_ready{"masking on".to_string()}else{"checking masking".to_string()}];
    if !state.post_ai{let n=state.agents.len();right.push(format!("{n} {}",if n==1{"agent"}else{"agents"}));}
    let mut edge=bar.x+bar.w-12.0*s;
    for text in right.iter().rev(){let w=p.mono_width(text,px,400);if edge-w<x{break;}p.text(r(edge-w,y,w,h),text,px,faint,400,true);edge-=w+gap;}
}

/// Quiet blocks under the grid: bold the command row and keep the meta text clear of cells.
/// With `breath` (Q4) the meta text has its own row above the command, in the space between
/// blocks; without it (classic input) the meta text sits at the right end of the command row.
pub(super) struct Quiet{id:String,top:f32,hairline:Option<(f32,Color)>,first_row:usize,meta:Vec<(String,Color,bool,f32)>,meta_x:f32,meta_y:f32,meta_h:f32,bar_y:f32,breath:bool,spread:f32,running:bool}
pub(super) fn quiet_blocks(p:&mut Paint,layout:&Layout,pane:&look::PaneLayout,data:&look::PaneState,state:&LookState,term:&TermColors,origin:[f32;2])->Vec<Quiet>{
    let s=p.scale;let c=p.theme.colors.clone();let ch=layout.cell_px[1] as f32;let faint=faint(p);
    let mut out=Vec::new();
    let clip_top=pane.grid_clip.y;let clip_bottom=pane.grid_clip.y+pane.grid_clip.h;
    let breath=data.breath.as_ref();
    for block in &data.blocks{
        if block.first_row>=data.snapshot.rows{continue;}
        let top=origin[1]+breath.map_or(block.first_row as f32*ch,|b|b.tops[block.first_row]);
        if breath.is_some(){
            let last=block.last_row.min(data.snapshot.rows-1);
            let bottom=origin[1]+breath.map_or(0.0,|b|b.tops[last])+ch;
            if top>=clip_bottom||bottom<=clip_top{continue;}
        }else if top<clip_top-0.5||top>=clip_bottom{continue;}
        let failed=!block.running&&block.exit_code.is_some_and(|code|code!=0);
        let color=if failed{blend(term.background,c.err,0.45)}else{over(term.background,c.border)};
        let hairline=match breath{
            Some(b)=>b.hairline(block.first_row,ch,s).map(|y|(origin[1]+y,color)).filter(|(y,_)|*y>=clip_top&&*y<clip_bottom),
            None=>(block.first_row>0).then(||(top.floor(),color)),
        };
        let mut left=Vec::new();let mut right=Vec::new();
        let mut running_s=None;
        if let Some((cwd,time,running))=state.block_meta.get(&(data.id.clone(),block.id.clone())){
            // The folder stands in for the shell's prompt (the command input hides PS1). In its own meta
            // row it is faint like the rest of the row (mockup .meta); next to the command row it keeps
            // the chips' dim colour.
            if !cwd.is_empty(){left.push((cwd.clone(),if breath.is_some(){faint}else{c.muted_foreground},true));}
            if !time.is_empty(){left.push((time.clone(),faint,false));}
            running_s=*running;
        }
        if let Some(n)=running_s.filter(|_|block.running){right.push((format!("running {n} s"),c.ok,true));}
        if failed{right.push((format!("exit {}",block.exit_code.unwrap_or(1)),c.err,false));}
        // Durations keep fixed-width digits (mockup .meta .right, tabular-nums), faint like the whole meta row.
        if let Some(ms)=block.duration_ms.filter(|_|!block.running){right.push((duration_label(ms),faint,true));}
        // In the meta row words keep a gap 2 px wider than the font's, so "2.3 s" never reads as one word.
        let spread=if breath.is_some(){META_SPREAD}else{0.0};
        let width=|p:&mut Paint,text:&str,color:Color,mono:bool|meta_width(p,text,12.0*s,if color==c.err{600}else{400},mono,spread);
        let edge=pane.rect.x+pane.rect.w-18.0*s;
        let mut meta=Vec::new();
        let (meta_y,meta_h,bar_y);
        if let Some(b)=breath{
            // Mockup .meta: folder then HH:MM from the grid's first column; exit, then duration, at the right edge.
            meta_y=origin[1]+b.meta_top(block.first_row,s);meta_h=super::breath::META_H*s;bar_y=meta_y+s;
            let mut x=origin[0];
            // Whole pixels: small faint text keeps the same glyph shapes wherever its width puts it.
            for (text,color,mono) in left{let w=width(p,&text,color,mono);meta.push((text,color,mono,x.round()));x+=w+10.0*s;}
            let mut x=edge;let mut placed=Vec::new();
            for (text,color,mono) in right.into_iter().rev(){let w=width(p,&text,color,mono);x-=w;placed.push((text,color,mono,x.round()));x-=10.0*s;}
            meta.extend(placed.into_iter().rev());
            // Meta text above the pane's top (rows lifted out of view) is not drawn over the title bar.
            if meta_y<pane.rect.y{meta.clear();}
        }else{
            meta_y=top;meta_h=ch;bar_y=top.floor()+6.0*s;
            let all:Vec<_>=left.into_iter().chain(right).collect();
            let mut x=edge;let mut placed=Vec::new();
            for (i,(text,color,mono)) in all.into_iter().enumerate().rev(){let w=width(p,&text,color,mono);x-=w;placed.push((text,color,mono,x));let _=i;x-=10.0*s;}
            meta.extend(placed.into_iter().rev());
        }
        let meta_x=meta.first().map_or(edge,|m|m.3);
        out.push(Quiet{id:block.id.clone(),top,hairline,first_row:block.first_row,meta,meta_x,meta_y,meta_h,bar_y,breath:breath.is_some(),spread,running:block.running});
    }
    out
}
const META_SPREAD:f32=2.0;
fn meta_width(p:&mut Paint,text:&str,px:f32,weight:u16,mono:bool,spread:f32)->f32{
    let w=|p:&mut Paint,t:&str|if mono{p.mono_width(t,px,weight)}else{p.width(t,px,weight)};
    if spread==0.0{return w(p,text);}
    let space=w(p," ")+spread*p.scale;
    text.split(' ').map(|word|w(p,word)).sum::<f32>()+space*(text.split(' ').count()-1) as f32
}
pub(super) fn quiet_under(q:&Quiet,snapshot:&mut clanky_core::TerminalSnapshot,layout:&Layout,origin:[f32;2]){
    let cw=layout.cell_px[0] as f32;
    if q.first_row<snapshot.rows{
        for col in 0..snapshot.cols{
            let cell=&mut snapshot.cells[q.first_row*snapshot.cols+col];
            cell.style|=clanky_core::STYLE_BOLD;
            if !q.breath && !q.meta.is_empty() && origin[0]+(col+1) as f32*cw>q.meta_x-8.0{cell.ch=' ';}
        }
    }
}
pub(super) fn quiet_over(p:&mut Paint,q:&Quiet,layout:&Layout,pane:&look::PaneLayout,data:&look::PaneState,state:&LookState){
    let s=p.scale;let c=p.theme.colors.clone();let _=layout;
    if let Some((y,color))=q.hairline{line(p,pane.rect.x,y.floor(),pane.rect.w,s,color);}
    let key=(data.id.clone(),q.id.clone());let opacity=state.block_actions.get(&key).copied().unwrap_or(0.0).clamp(0.0,1.0);
    let mut items:Vec<(&str,&str)>=vec![("Copy output","copy"),("Copy command","copy_command"),("Run again","rerun")];
    if !state.post_ai{items.push(("Ask agent","send"));}
    items.push(("⋯","more"));
    let px=12.0*s;let pad=7.0*s;let widths:Vec<f32>=items.iter().map(|(label,_)|spaced_width(p,label,px,400,2.0)+2.0*pad).collect();
    let total=widths.iter().sum::<f32>()+(items.len()-1) as f32*s+6.0*s+2.0*s;
    let bar=r((pane.rect.x+pane.rect.w-14.0*s-total).round(),q.bar_y.round(),total.round(),24.0*s);
    // The hover bar is an overlay at the top right (mockup .hoverbar): only the meta words under it
    // fade out; the folder and the time on the left stay.
    {
        for (text,color,mono,x) in &q.meta{
            let weight=if *color==c.err{600}else{400};
            let covered=*x+meta_width(p,text,12.0*s,weight,*mono,q.spread)>bar.x-4.0*s;
            let opacity=if covered{opacity}else{0.0};
            if opacity>=1.0{continue;}
            // Sans text sits 1 px lower so its digits line up with the mono folder beside it.
            let y=q.meta_y+if *mono{0.0}else{s};
            if q.spread==0.0{
                let w=meta_width(p,text,12.0*s,weight,*mono,0.0);
                p.text(r(*x,y,w+0.5,q.meta_h),text,12.0*s,alpha(*color,1.0-opacity),weight,*mono);
            }else{
                let space=meta_width(p," ",12.0*s,weight,*mono,0.0)+q.spread*s;let mut x=*x;
                for word in text.split(' '){let w=meta_width(p,word,12.0*s,weight,*mono,0.0);p.text(r(x.round(),y,w+0.5,q.meta_h),word,12.0*s,alpha(*color,1.0-opacity),weight,*mono);x+=w+space;}
            }
        }
    }
    let _=(q.running,q.top);
    // One floating bar of block actions on hover (mockup .hoverbar: right 14, top 6, radius 7,
    // a soft drop shadow, the first item filled until another is pointed at).
    if opacity<=0.0{return;}
    let bar_fill=blend(c.background,c.foreground,0.04);
    p.out.scene.layers.push(clanky_render::Layer::Shape(clanky_render::Shape{rect:bar.array(),radius:7.0*s,fill:clanky_render::Fill::Solid([0;4]),border:None,
        shadow:Some(clanky_render::Shadow{offset:[0.0,4.0*s],blur:7.0*s,spread:0.0,color:[0,0,0,(102.0*opacity) as u8]})}));
    p.shape(bar,7.0*s,alpha(bar_fill,opacity),Some(alpha(over(c.background,c.border),opacity)));
    let focus=state.block_action_focus.as_ref().filter(|(tab,id,_)|*tab==data.id&&*id==q.id).map(|(_,_,i)|*i);
    // Labels keep a word gap 2 px wider than the font's: at 12 px in the dim colour a plain gap
    // reads as one word ("Copycommand") to OCR, and the Q1 gate reads this bar.
    let spans:Vec<PixelRect>={let mut x=bar.x+4.0*s;widths.iter().map(|w|{let span=r(x,bar.y+3.0*s,*w,bar.h-6.0*s);x+=w+s;span}).collect()};
    let pointed=spans.iter().position(|span|hovered(p,*span));
    let lit=pointed.or(focus).unwrap_or(0);
    for (i,((label,action),span)) in items.iter().zip(&spans).enumerate(){
        let span=*span;let on=i==lit;
        if on{p.shape(span,5.0*s,alpha(if p.pressed&&pointed==Some(i){c.pressed}else{c.muted},opacity),None);}
        let tw=spaced_width(p,label,px,400,2.0);
        spaced_by(p,span.x+(span.w-tw)/2.0,span.y,span.h,label,px,alpha(if on{c.foreground}else{c.muted_foreground},opacity),400,2.0);
        p.hit(span,HitAction::Block{tab_id:data.id.clone(),block_id:q.id.clone(),action:(*action).into()});
    }
}
/// The sticky command line (mockup .sticky) over the top of a pane whose block started above it.
pub(super) fn sticky(p:&mut Paint,layout:&Layout,pane:&look::PaneLayout,data:&look::PaneState,term:&TermColors){
    let Some(sticky)=&data.sticky else{return;};let s=p.scale;let c=p.theme.colors.clone();
    let h=(pane.grid_origin[1]-pane.rect.y)+layout.cell_px[1] as f32;
    let band=r(pane.rect.x,pane.rect.y,pane.rect.w,h);
    p.shape(band,0.0,over(term.background,alpha(c.card,0.96)),None);
    line(p,band.x,band.y+band.h-s,band.w,s,over(term.background,c.border));
    let right=match (sticky.running_s,sticky.duration_ms){(Some(n),_)=>format!("running {n} s"),(None,Some(ms))=>duration_label(ms),_=>String::new()};
    let rw=p.mono_width(&right,12.0*s,400);
    p.text(r(band.x+band.w-18.0*s-rw,band.y,rw,band.h-s),&right,12.0*s,c.muted_foreground,400,true);
    let x=pane.grid_origin[0];
    mono_chars(p,r(x,band.y,(band.x+band.w-36.0*s-rw-x).max(0.0),band.h-s),&sticky.command,13.0*s,c.foreground,600);
}
