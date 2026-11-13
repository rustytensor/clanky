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
fn dot(p:&mut Paint,x:f32,y:f32,d:f32,color:Color){p.shape(r(x,y,d,d),d/2.0,color,None);}
fn hovered(p:&Paint,rect:PixelRect)->bool{rect.contains(p.pointer.0,p.pointer.1)}

/// Title bar (mockup .titlebar): 38 px on the sidebar colour with a 1 px bottom hairline.
pub(super) fn title_bar(p:&mut Paint,layout:&Layout,state:&LookState){
    let s=p.scale;let c=p.theme.colors.clone();let bar=layout.top_bar;if bar.h<=0.0{return;}
    let side_line=over(c.sidebar,c.border);let dim=c.muted_foreground;let faint=faint(p);
    p.shape(bar,0.0,c.sidebar,None);
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
        p.icon("search",field.x+9.0*s,field.y+5.5*s,faint);
        let keys="Ctrl Shift P";let kw=p.mono_width(keys,11.0*s,400);
        p.text(r(field.x+field.w-10.0*s-kw,field.y,kw,field.h),keys,11.0*s,faint,400,true);
        p.text(r(field.x+30.0*s,field.y,(field.w-50.0*s-kw).max(0.0),field.h),"Search or run a command",12.0*s,faint,400,false);
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
        let dot_color=if tab.running{c.ok}else if tab.agent{c.agent}else{faint};
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
    p.shape(col,0.0,c.sidebar,None);line(p,col.x+col.w-s,col.y,s,col.h,over(c.sidebar,c.border));
    let x=col.x+8.0*s;let w=col.w-16.0*s-s;let mut y=col.y+8.0*s;
    p.text(r(x+9.0*s,y,w-18.0*s,22.0*s),"Session 1",12.0*s,faint,500,false);y+=26.0*s;
    for tab in &state.tabs{
        let row=r(x,y,w,64.0*s);if row.y+row.h>col.y+col.h-40.0*s{break;}
        if tab.active{p.shape(row,7.0*s,c.muted,None);}else if hovered(p,row){p.shape(row,7.0*s,blend(c.sidebar,c.muted,0.6),None);}
        p.text(r(row.x+9.0*s,row.y+6.0*s,row.w-30.0*s,17.0*s),&tab.path,12.0*s,faint,400,true);
        p.text(r(row.x+9.0*s,row.y+23.0*s,row.w-18.0*s,19.0*s),if tab.last_command.is_empty(){"–"}else{&tab.last_command},13.0*s,c.foreground,400,true);
        let dot_color=if tab.running{c.ok}else if tab.agent{c.agent}else{faint};
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
    p.shape(panel,0.0,c.sidebar,None);line(p,panel.x+panel.w-s,panel.y,s,panel.h,side_line);
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
    p.text(r(head.x+4.0*s,head.y,head.w-8.0*s,head.h),title,13.0*s,c.foreground,500,false);
    let body=r(inner.x+8.0*s,head.y+head.h+6.0*s,inner.w-16.0*s,(foot_y-head.y-head.h-12.0*s).max(0.0));
    match tool{
        Tool::Files=>files(p,body,state,faint),
        Tool::Recent=>recent(p,body,state,faint),
        Tool::Agents=>agents(p,body,state,faint),
    }
}
fn files(p:&mut Paint,body:PixelRect,state:&LookState,faint:Color){
    let s=p.scale;let c=p.theme.colors.clone();let mut y=body.y;
    if state.tree.is_empty(){p.text(r(body.x+6.0*s,y,body.w-12.0*s,24.0*s),"No folder to show",12.0*s,faint,400,false);return;}
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
    p.text(r(body.x+6.0*s,y,body.w-12.0*s,18.0*s),"Agent activity",12.0*s,faint,500,false);y+=20.0*s;
    p.text(r(body.x+6.0*s,y,body.w-12.0*s,18.0*s),&format!("{total} {} in 30 min",if total==1{"command"}else{"commands"}),13.0*s,c.foreground,400,false);y+=22.0*s;
    for item in &state.agents{
        if y+22.0*s>body.y+body.h{break;}
        let name=state.agent_labels.get(&item.connection_id).cloned().unwrap_or_else(||"Agent".into());
        dot(p,body.x+6.0*s,y+7.0*s,6.0*s,if item.running{c.agent}else{faint});
        p.text(r(body.x+18.0*s,y,body.w-24.0*s,20.0*s),&format!("{name} · {}",if item.running{"running"}else{"idle"}),13.0*s,c.foreground,400,false);
        p.hit(r(body.x,y,body.w,20.0*s),HitAction::Agent{connection_id:item.connection_id,tab_id:item.tab_id.clone()});y+=22.0*s;
    }
    y+=10.0*s;
    p.text(r(body.x+6.0*s,y,body.w-12.0*s,18.0*s),"Agent access",12.0*s,faint,500,false);y+=22.0*s;
    for row in &state.access{
        let rect=r(body.x,y,body.w,40.0*s);if rect.y+rect.h>body.y+body.h{break;}
        let over_row=hovered(p,rect);
        if over_row{p.shape(rect,6.0*s,if p.pressed{c.pressed}else{c.muted},None);}
        p.pane_label(r(rect.x+6.0*s,rect.y+3.0*s,rect.w-54.0*s,18.0*s),&row.label,13.0*s,c.foreground,400);
        p.text(r(rect.x+6.0*s,rect.y+21.0*s,rect.w-54.0*s,16.0*s),if row.allowed{"read + run"}else{"off"},12.0*s,faint,400,false);
        let sw=r(rect.x+rect.w-6.0*s-28.0*s,rect.y+12.0*s,28.0*s,16.0*s);
        p.shape(sw,8.0*s,if row.allowed{c.primary}else{c.input},None);
        p.shape(r(sw.x+if row.allowed{14.0*s}else{2.0*s},sw.y+2.0*s,12.0*s,12.0*s),6.0*s,if row.allowed{c.primary_foreground}else{c.background},None);
        p.hit(rect,HitAction::Access(row.id.clone()));
        y+=42.0*s;
    }
}

/// The status line (mockup .status): session, tabs and server on the left; masking and agents on the right.
pub(super) fn status_line(p:&mut Paint,layout:&Layout,state:&LookState){
    let Some(bar)=layout.status_line else{return;};let s=p.scale;let c=p.theme.colors.clone();let faint=faint(p);
    p.shape(bar,0.0,c.sidebar,None);line(p,bar.x,bar.y,bar.w,s,over(c.sidebar,c.border));
    let y=bar.y+s;let h=bar.h-s;let gap=14.0*s;let px=12.0*s;
    let tabs=state.tabs.len().max(1);
    let left=["Session 1".to_string(),format!("{tabs} {}",if tabs==1{"tab"}else{"tabs"}),super::uptime_label(state.uptime_seconds)];
    let mut x=bar.x+12.0*s;
    for text in &left{let w=p.width(text,px,400);p.text(r(x,y,w,h),text,px,faint,400,false);x+=w+gap;}
    let mut right=vec![if state.masking_ready{"masking on".to_string()}else{"checking masking".to_string()}];
    if !state.post_ai{let n=state.agents.len();right.push(format!("{n} {}",if n==1{"agent"}else{"agents"}));}
    let mut edge=bar.x+bar.w-12.0*s;
    for text in right.iter().rev(){let w=p.width(text,px,400);if edge-w<x{break;}p.text(r(edge-w,y,w,h),text,px,faint,400,false);edge-=w+gap;}
}

/// Quiet blocks under the grid: bold the command row and keep the meta text clear of cells.
pub(super) struct Quiet{id:String,top:f32,hairline:Option<Color>,first_row:usize,meta:Vec<(String,Color,bool)>,meta_x:f32,running:bool}
pub(super) fn quiet_blocks(p:&mut Paint,layout:&Layout,pane:&look::PaneLayout,data:&look::PaneState,state:&LookState,term:&TermColors,origin:[f32;2])->Vec<Quiet>{
    let s=p.scale;let c=p.theme.colors.clone();let ch=layout.cell_px[1] as f32;let faint=faint(p);
    let mut out=Vec::new();
    let clip_top=pane.grid_clip.y;let clip_bottom=pane.grid_clip.y+pane.grid_clip.h;
    for block in &data.blocks{
        if block.first_row>=data.snapshot.rows{continue;}
        let top=origin[1]+block.first_row as f32*ch;if top<clip_top-0.5||top>=clip_bottom{continue;}
        let failed=!block.running&&block.exit_code.is_some_and(|code|code!=0);
        let hairline=(block.first_row>0).then(||if failed{blend(term.background,c.err,0.45)}else{over(term.background,c.border)});
        let mut meta=Vec::new();
        if let Some((cwd,time))=state.block_meta.get(&(data.id.clone(),block.id.clone())){
            if !cwd.is_empty(){meta.push((cwd.clone(),faint,true));}
            if !time.is_empty(){meta.push((time.clone(),faint,false));}
        }
        if failed{meta.push((format!("exit {}",block.exit_code.unwrap_or(1)),c.err,false));}
        if let Some(ms)=block.duration_ms.filter(|_|!block.running){meta.push((duration_label(ms),faint,false));}
        let right=pane.rect.x+pane.rect.w-18.0*s;let mut x=right;
        for (i,(text,_,mono)) in meta.iter().enumerate().rev(){let w=if *mono{p.mono_width(text,12.0*s,400)}else{p.width(text,12.0*s,400)};x-=w;if i>0{x-=10.0*s;}}
        out.push(Quiet{id:block.id.clone(),top,hairline,first_row:block.first_row,meta,meta_x:x,running:block.running});
    }
    out
}
pub(super) fn quiet_under(q:&Quiet,snapshot:&mut clanky_core::TerminalSnapshot,layout:&Layout,origin:[f32;2]){
    let cw=layout.cell_px[0] as f32;
    if q.first_row<snapshot.rows{
        for col in 0..snapshot.cols{
            let cell=&mut snapshot.cells[q.first_row*snapshot.cols+col];
            cell.style|=clanky_core::STYLE_BOLD;
            if !q.meta.is_empty() && origin[0]+(col+1) as f32*cw>q.meta_x-8.0{cell.ch=' ';}
        }
    }
}
pub(super) fn quiet_over(p:&mut Paint,q:&Quiet,layout:&Layout,pane:&look::PaneLayout,data:&look::PaneState,state:&LookState){
    let s=p.scale;let c=p.theme.colors.clone();let ch=layout.cell_px[1] as f32;
    if let Some(color)=q.hairline{line(p,pane.rect.x,q.top.floor(),pane.rect.w,s,color);}
    let mut x=q.meta_x;
    for (i,(text,color,mono)) in q.meta.iter().enumerate(){
        let w=if *mono{p.mono_width(text,12.0*s,400)}else{p.width(text,12.0*s,400)};
        p.text(r(x,q.top,w,ch),text,12.0*s,*color,400,*mono);x+=w+if i+1<q.meta.len(){10.0*s}else{0.0};
    }
    let _=q.running;
    // One floating bar of block actions on hover (mockup .hoverbar: right 14, top 6, radius 7).
    let key=(data.id.clone(),q.id.clone());let opacity=state.block_actions.get(&key).copied().unwrap_or(0.0);
    if opacity<=0.0{return;}
    let mut items:Vec<(&str,&str)>=vec![("Copy output","copy"),("Copy command","copy_command"),("Run again","rerun")];
    if !state.post_ai{items.push(("Ask agent","send"));}
    items.push(("⋯","more"));
    let px=12.0*s;let pad=7.0*s;let widths:Vec<f32>=items.iter().map(|(label,_)|p.width(label,px,400)+2.0*pad).collect();
    let total=widths.iter().sum::<f32>()+(items.len()-1) as f32*s+6.0*s+2.0*s;
    let bar=r((pane.rect.x+pane.rect.w-14.0*s-total).round(),(q.top.floor()+6.0*s).round(),total.round(),24.0*s);
    let bar_fill=blend(c.background,c.foreground,0.04);
    p.shape(bar,7.0*s,alpha(bar_fill,opacity),Some(alpha(over(c.background,c.border),opacity)));
    let focus=state.block_action_focus.as_ref().filter(|(tab,id,_)|*tab==data.id&&*id==q.id).map(|(_,_,i)|*i);
    let mut x=bar.x+4.0*s;
    for (i,((label,action),w)) in items.iter().zip(&widths).enumerate(){
        let span=r(x,bar.y+3.0*s,*w,bar.h-6.0*s);
        if hovered(p,span)||focus==Some(i){p.shape(span,5.0*s,alpha(if p.pressed&&hovered(p,span){c.pressed}else{c.muted},opacity),None);}
        let tw=p.width(label,px,400);
        p.text(r(span.x+(span.w-tw)/2.0,span.y,tw,span.h),label,px,alpha(if hovered(p,span){c.foreground}else{c.muted_foreground},opacity),400,false);
        p.hit(span,HitAction::Block{tab_id:data.id.clone(),block_id:q.id.clone(),action:(*action).into()});
        x+=w+s;
    }
}
/// The sticky command line (mockup .sticky) over the top of a pane whose block started above it.
pub(super) fn sticky(p:&mut Paint,layout:&Layout,pane:&look::PaneLayout,data:&look::PaneState,term:&TermColors){
    let Some(sticky)=&data.sticky else{return;};let s=p.scale;let c=p.theme.colors.clone();let faint=faint(p);
    let h=(pane.grid_origin[1]-pane.rect.y)+layout.cell_px[1] as f32;
    let band=r(pane.rect.x,pane.rect.y,pane.rect.w,h);
    p.shape(band,0.0,over(term.background,alpha(c.card,0.96)),None);
    line(p,band.x,band.y+band.h-s,band.w,s,over(term.background,c.border));
    let right=match (sticky.running_s,sticky.duration_ms){(Some(n),_)=>format!("running {n} s"),(None,Some(ms))=>duration_label(ms),_=>String::new()};
    let rw=p.width(&right,12.0*s,400);
    p.text(r(band.x+band.w-18.0*s-rw,band.y,rw,band.h-s),&right,12.0*s,faint,400,false);
    let x=pane.grid_origin[0];
    p.text(r(x,band.y,(band.x+band.w-36.0*s-rw-x).max(0.0),band.h-s),&sticky.command,12.0*s,c.foreground,600,true);
}
