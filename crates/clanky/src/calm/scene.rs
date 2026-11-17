//! First visible calm slice: live grids and directly actionable chrome.
use crate::{look::{self,HitAction,Hotspot,Layout,LookState,PixelRect,Presentation},themes::{Appearance,Color,Theme}};
use clanky_core::{Cell,ColorSource};
use clanky_render::{Fill,GridLayer,Layer,Matrix4,PathStroke,Scene,Shape,TextMetrics,TextRun,VectorLayer,VectorPath};
use clanky_render::vector::lyon::{math::point,path::Path};

fn r(x:f32,y:f32,w:f32,h:f32)->PixelRect{PixelRect{x,y,w:w.max(0.0),h:h.max(0.0)}}
pub(super) fn rgb(c:Color)->[u8;3]{[c[0],c[1],c[2]]}
pub(super) fn alpha(mut c:Color,f:f32)->Color{c[3]=(c[3] as f32*f.clamp(0.0,1.0)).round() as u8;c}
fn shadow_color(theme:&Theme)->Color{match theme.appearance{Appearance::Dark=>[0,0,0,89],Appearance::Light=>[60,40,20,15]}}
pub(super) fn blend(a:Color,b:Color,t:f32)->Color{std::array::from_fn(|i|(a[i] as f32*(1.0-t)+b[i] as f32*t).round() as u8)}
/// Adapter for existing palette/search/editor panels while their calm components land.
pub fn legacy_palette(theme:&Theme)->look::Theme{
    let c=&theme.colors;let mut t=look::Theme::dark();
    t.window=c.sidebar;t.rail=c.sidebar;t.panel=c.background;t.line=c.border;t.ink=c.foreground;
    t.dim=c.muted_foreground;t.accent=c.foreground;t.violet=c.agent;t.teal=c.agent;t.focus=c.ring;
    t.ok=c.ok;t.err=c.err;t.ok_ink=c.ok;t.err_ink=c.err;t.is_light=theme.appearance==Appearance::Light;
    t.refresh_mixes();t.selection=c.accent;t.card=c.card;t
}
/// The terminal's own colours: the config's terminal_colors, else the theme's.
pub(super) struct TermColors{pub(super) background:Color,pub(super) foreground:Color,pub(super) cursor:Color,pub(super) ansi:[Color;16]}
pub(super) fn term_colors(theme:&Theme,over:&crate::config::TerminalColors)->TermColors{
    let c=&theme.colors;
    TermColors{background:over.background.unwrap_or(c.background),foreground:over.foreground.unwrap_or(c.foreground),
        cursor:over.cursor.or(over.foreground).unwrap_or(c.foreground),ansi:over.ansi.unwrap_or(theme.ansi)}
}
fn terminal(t:&TermColors,cell:&mut Cell){
    if let Some(source)=cell.color_source{
        let (fg,bg)=if source.inverse{(source.bg,source.fg)}else{(source.fg,source.bg)};
        cell.fg=match fg{ColorSource::Default=>rgb(if source.inverse{t.background}else{t.foreground}),ColorSource::Indexed(i @ 0..=15)=>rgb(t.ansi[i as usize]),_=>cell.fg};
        cell.bg=match bg{ColorSource::Default=>rgb(if source.inverse{t.foreground}else{t.background}),ColorSource::Indexed(i @ 0..=15)=>rgb(t.ansi[i as usize]),_=>cell.bg};
    }
    // SGR 2: two thirds of the way from the background, so it stays readable on light themes.
    if cell.style&clanky_core::STYLE_FAINT!=0{
        let under=if cell.color_source.is_some_and(|s|s.bg==ColorSource::Default && !s.inverse){rgb(t.background)}else{cell.bg};
        cell.fg=std::array::from_fn(|i|(cell.fg[i] as f32*0.67+under[i] as f32*0.33).round() as u8);
    }
}
pub(super) struct Paint<'a>{pub(super) out:Presentation,pub(super) theme:&'a Theme,measure:&'a mut dyn FnMut(&TextRun)->TextMetrics,pub(super) scale:f32,pub(super) pointer:(f64,f64),pub(super) pressed:bool,pub(super) clip:[f32;4]}
impl<'a> Paint<'a>{
    pub(super) fn empty(theme:&'a Theme,measure:&'a mut dyn FnMut(&TextRun)->TextMetrics,scale:f32,window:[u32;2],pointer:(f64,f64),pressed:bool)->Self{
        Self{out:Presentation{scene:Scene{clear:[0;4],layers:vec![]},hits:vec![]},theme,measure,scale,pointer,pressed,clip:[0.0,0.0,window[0] as f32,window[1] as f32]}
    }
    pub(super) fn shape(&mut self,rect:PixelRect,radius:f32,fill:Color,border:Option<Color>){
        if rect.w>0.0 && rect.h>0.0{self.out.scene.layers.push(Layer::Shape(Shape{rect:rect.array(),radius,
            fill:Fill::Solid(fill),border:border.map(|c|(self.scale,c)),shadow:None}));}
    }
    pub(super) fn width(&mut self,text:&str,px:f32,weight:u16)->f32{(self.measure)(&TextRun{text:text.into(),px,weight,..TextRun::default()}).width}
    pub(super) fn mono_width(&mut self,text:&str,px:f32,weight:u16)->f32{(self.measure)(&TextRun{text:text.into(),px,weight,mono:true,..TextRun::default()}).width}
    pub(super) fn text(&mut self,rect:PixelRect,text:&str,px:f32,color:Color,weight:u16,mono:bool){
        if rect.w<=0.0 || rect.h<=0.0{return;}
        let mut run=TextRun{text:text.chars().map(|c|if c.is_control(){' '}else{c}).take(2048).collect(),px,color,weight,mono,..TextRun::default()};
        // Half a pixel of tolerance: controls sized to their measured label must not lose it to rounding.
        let fits=rect.w+0.5;
        if (self.measure)(&run).width>fits{
            let chars:Vec<_>=run.text.chars().collect();let (mut lo,mut hi)=(0,chars.len());
            while lo<hi{let mid=(lo+hi).div_ceil(2);run.text=chars[..mid].iter().collect::<String>()+"…";
                if (self.measure)(&run).width<=fits{lo=mid;}else{hi=mid-1;}}
            run.text=chars[..lo].iter().collect::<String>()+"…";
        }
        let metrics=(self.measure)(&run);
        if metrics.width>fits{return;}
        run.origin=[rect.x,rect.y+(rect.h-metrics.line_height)/2.0];self.out.scene.layers.push(Layer::Text(run));
    }
    pub(super) fn pane_label(&mut self,rect:PixelRect,label:&str,px:f32,color:Color,weight:u16){
        if let Some((name,number))=label.rsplit_once(" · pane "){
            let suffix=format!(" · pane {number}");let width=self.width(&suffix,px,weight).min(rect.w);
            let name_width=self.width(name,px,weight).min((rect.w-width).max(0.0));
            self.text(r(rect.x,rect.y,name_width,rect.h),name,px,color,weight,false);
            self.text(r(rect.x+name_width,rect.y,width,rect.h),&suffix,px,color,weight,false);
        }else{self.text(rect,label,px,color,weight,false);}
    }
    pub(super) fn hit(&mut self,rect:PixelRect,action:HitAction){if rect.w>0.0 && rect.h>0.0{self.out.hits.push(Hotspot{rect,action});}}
    pub(super) fn icon(&mut self,kind:&str,x:f32,y:f32,color:Color){
        let mut p=Path::builder();
        if kind=="search"{p.add_circle(point(7.0,7.0),4.5,clanky_render::vector::lyon::path::Winding::Positive);p.begin(point(10.5,10.5));p.line_to(point(13.5,13.5));p.end(false);}
        else if kind=="file"{
            p.begin(point(9.0,1.5));p.line_to(point(4.0,1.5));p.quadratic_bezier_to(point(2.5,1.5),point(2.5,3.0));p.line_to(point(2.5,13.0));p.quadratic_bezier_to(point(2.5,14.5),point(4.0,14.5));p.line_to(point(12.0,14.5));p.quadratic_bezier_to(point(13.5,14.5),point(13.5,13.0));p.line_to(point(13.5,6.0));p.close();p.begin(point(9.0,1.5));p.line_to(point(9.0,6.0));p.line_to(point(13.5,6.0));p.end(false);
        }
        else if kind=="settings"{p.add_circle(point(8.0,8.0),2.2,clanky_render::vector::lyon::path::Winding::Positive);
            for (a,b) in [([8.0,1.5],[8.0,3.5]),([8.0,12.5],[8.0,14.5]),([1.5,8.0],[3.5,8.0]),([12.5,8.0],[14.5,8.0]),([3.4,3.4],[4.8,4.8]),([11.2,11.2],[12.6,12.6]),([3.4,12.6],[4.8,11.2]),([11.2,4.8],[12.6,3.4])]{p.begin(point(a[0],a[1]));p.line_to(point(b[0],b[1]));p.end(false);}}
        else{
            p.begin(point(3.5,2.5));p.line_to(point(12.5,2.5));p.quadratic_bezier_to(point(14.5,2.5),point(14.5,4.5));p.line_to(point(14.5,11.5));
            p.quadratic_bezier_to(point(14.5,13.5),point(12.5,13.5));p.line_to(point(3.5,13.5));p.quadratic_bezier_to(point(1.5,13.5),point(1.5,11.5));
            p.line_to(point(1.5,4.5));p.quadratic_bezier_to(point(1.5,2.5),point(3.5,2.5));p.close();
            if kind=="sidebar"{p.begin(point(6.0,2.5));p.line_to(point(6.0,13.5));p.end(false);}
            else{p.begin(point(4.5,6.0));p.line_to(point(6.5,8.0));p.line_to(point(4.5,10.0));p.end(false);p.begin(point(8.0,10.0));p.line_to(point(11.5,10.0));p.end(false);}
        }
        self.out.scene.layers.push(Layer::Vector(VectorLayer{paths:vec![VectorPath{path:p.build(),fill:None,stroke:Some(PathStroke{width:1.5,color}),
            transform:Matrix4::scale(self.scale,self.scale,1.0).then(Matrix4::translate(x,y,0.0))}],clip:self.clip,opacity:1.0}));
    }
    pub(super) fn kbd(&mut self,right:f32,y:f32,keys:&[&str])->f32{
        let mut edge=right;let c=&self.theme.colors;
        for key in keys.iter().rev(){
            let width=(self.measure)(&TextRun{text:(*key).into(),px:12.0*self.scale,mono:true,..TextRun::default()}).width+10.0*self.scale;
            let chip=r(edge-width,y,width,20.0*self.scale);
            self.shape(chip,4.0*self.scale,c.muted,None);
            self.text(r(chip.x+5.0*self.scale,chip.y,chip.w-10.0*self.scale,chip.h),key,12.0*self.scale,c.muted_foreground,400,true);
            edge=chip.x-4.0*self.scale;
        }
        edge+4.0*self.scale
    }
}

pub fn presentation(layout:&Layout,state:&LookState,theme:&Theme,font_px:f32,pointer:(f64,f64),pressed:bool,measure:&mut dyn FnMut(&TextRun)->TextMetrics)->Presentation{
    let c=&theme.colors;let s=layout.scale as f32;let [w,h]=layout.window_px.map(|n|n as f32);
    let mut p=Paint{out:Presentation{scene:crate::theme_scene::window_background(theme,layout.window_px,state.opacity),hits:vec![]},theme,measure,scale:s,pointer,pressed,clip:[0.0,0.0,w,h]};
    let term=term_colors(theme,&state.terminal);
    // Each pane takes the terminal background; between panes (and between the tools panel and the
    // panes) the window's own colour shows, 8 px wide, as Ptyxis keeps its terminal apart from the frame.
    for pane in &layout.panes{
        let data=state.panes.iter().find(|data|data.id==pane.id);let focus=state.focused.as_ref()==Some(&pane.id);
        let driving=data.is_some_and(|d|d.agent_running);
        // An editor pane draws in the window theme, never the terminal colours: its padding too.
        let editor=state.files.iter().any(|file|file.id==pane.id);
        p.shape(pane.rect,0.0,if editor{c.background}else{term.background},None);
        // An agent at work: a 2 px agent-coloured edge along the pane's top.
        if driving && !state.post_ai{p.shape(r(pane.rect.x,pane.rect.y,pane.rect.w,2.0*s),0.0,c.agent,None);}
        let Some(data)=data else{continue;};
        let mut snapshot=data.snapshot.clone();let cursor=snapshot.cursor;snapshot.cursor=None;
        for cell in &mut snapshot.cells{terminal(&term,cell);}
        let origin=[pane.grid_origin[0],pane.grid_origin[1]-data.scroll_px];
        if state.block_cards{
            let cards:Vec<_>=data.blocks.iter().filter_map(|block|card_geometry(&mut p,layout,pane,data,block,origin,state)).collect();
            for card in &cards{card_under_grid(&mut p,card,&mut snapshot,layout,origin);}
            p.out.scene.layers.push(Layer::Grid(GridLayer{origin,clip:pane.grid_clip.array(),snapshot,underlines:data.underlines.clone(),opacity:1.0}));
            for card in &cards{card_over_grid(&mut p,card,&data.id,state);}
        }else{
            let quiet=super::chrome::quiet_blocks(&mut p,layout,pane,data,state,&term,origin);
            for q in &quiet{super::chrome::quiet_under(q,&mut snapshot,layout,origin);}
            match &data.breath{
                Some(breath)=>breathing_grid(&mut p,breath,pane,origin,&snapshot,&data.underlines,layout.cell_px[1] as f32),
                None=>p.out.scene.layers.push(Layer::Grid(GridLayer{origin,clip:pane.grid_clip.array(),snapshot,underlines:data.underlines.clone(),opacity:1.0})),
            }
            for q in &quiet{super::chrome::quiet_over(&mut p,q,layout,pane,data,state);}
        }
        // Block cursor in the cursor colour; the off half of a blink draws none.
        if focus && state.cursor_visible{if let Some((row,col))=cursor.filter(|(row,col)|*row<pane.rows && *col<pane.cols){
            let x=origin[0]+col as f32*layout.cell_px[0] as f32;let y=origin[1]+data.breath.as_ref().map_or(row as f32*layout.cell_px[1] as f32,|b|b.tops[row]);
            p.shape(r(x,y,layout.cell_px[0] as f32,layout.cell_px[1] as f32),0.0,term.cursor,None);
            let cell=&data.snapshot.cells[row*data.snapshot.cols+col];
            let weight=if cell.style&clanky_core::STYLE_BOLD!=0{700}else{400};
            if cell.ch!=' ' {
                p.text(r(x,y,layout.cell_px[0] as f32,layout.cell_px[1] as f32),&cell.ch.to_string(),font_px,term.background,weight,true);
                // An italic comment glyph stays italic under the cursor.
                if cell.style&clanky_core::STYLE_ITALIC!=0{if let Some(Layer::Text(run))=p.out.scene.layers.last_mut(){run.italic=true;}}
            }
        }}
        if !state.block_cards{super::chrome::sticky(&mut p,layout,pane,data,&term);}
        scrollbar(&mut p,pane,data,&term);
    }
    super::chrome::tabs_column(&mut p,layout,state);
    super::chrome::tools_panel(&mut p,layout,state);
    super::chrome::status_line(&mut p,layout,state);
    super::chrome::title_bar(&mut p,layout,state);
    for panel in &state.panels{if panel.id!="palette" && panel.id!="search"{calm_panel(&mut p,layout,panel,font_px);}}
    if let Some((id,query,total))=&state.find{if let Some(pane)=layout.panes.iter().find(|pane|pane.id==*id){find_bar(&mut p,pane,query,*total);}}
    let _=(Appearance::Dark,shadow_color as fn(&Theme)->Color);
    p.out
}

/// The overlay scrollbar (Ptyxis 50.1): 8 px wide, 7 px from the pane's right edge, round ends,
/// the foreground at 15 % over the background; its length and place follow the view in the scrollback.
fn scrollbar(p:&mut Paint,pane:&look::PaneLayout,data:&look::PaneState,term:&TermColors){
    let opacity=data.scrollbar.clamp(0.0,1.0);let snap=&data.snapshot;
    if opacity<=0.0 || snap.history==0 || snap.rows==0{return;}
    let s=p.scale;
    let w=(8.0*s).round();let x=(pane.rect.x+pane.rect.w).round()-(7.0*s).ceil()-w;
    let top=(pane.rect.y+8.0*s).round();let bottom=(pane.grid_clip.y+pane.grid_clip.h).round();
    let track=bottom-top;if track<=w{return;}
    let total=(snap.history+snap.rows) as f32;
    let len=(track*snap.rows as f32/total).max(40.0*s).min(track).round();
    let back=snap.display_offset.min(snap.history) as f32/snap.history as f32;
    let y=(top+(track-len)*(1.0-back)).round();
    let mut color=blend(term.background,term.foreground,0.15);color[3]=255;
    p.shape(r(x,y,w,len),w/2.0,alpha(color,opacity),None);
}

/// The grid in runs of rows that share one offset, each run its own grid layer, so the space
/// between quiet blocks falls between rows and never inside a cell.
fn breathing_grid(p:&mut Paint,breath:&super::breath::Breath,pane:&look::PaneLayout,origin:[f32;2],snapshot:&clanky_core::TerminalSnapshot,underlines:&[clanky_render::Underline],ch:f32){
    let clip=pane.grid_clip;let cols=snapshot.cols;
    for (start,n,top) in breath.runs(ch){
        let y=origin[1]+top;
        if y>=clip.y+clip.h||y+n as f32*ch<=clip.y{continue;}
        let part=clanky_core::TerminalSnapshot{rows:n,cols,cells:snapshot.cells[start*cols..(start+n)*cols].to_vec(),cursor:None,images:Vec::new(),
            app_cursor:snapshot.app_cursor,cursor_shape:snapshot.cursor_shape,modes:snapshot.modes,history:snapshot.history,display_offset:snapshot.display_offset,
            wrapped:snapshot.wrapped.iter().filter(|r|**r>=start&&**r<start+n).map(|r|r-start).collect()};
        let lines=underlines.iter().filter(|u|u.row>=start&&u.row<start+n).map(|u|clanky_render::Underline{row:u.row-start,..u.clone()}).collect();
        p.out.scene.layers.push(Layer::Grid(GridLayer{origin:[origin[0],y],clip:clip.array(),snapshot:part,underlines:lines,opacity:1.0}));
    }
}

/// A short message from the window (an error or a notice) as a small card at the
/// bottom right of the main area: card fill, hairline border, the sheets' shadow.
pub fn notice(layout:&Layout,theme:&Theme,text:&str,measure:&mut dyn FnMut(&TextRun)->TextMetrics)->Vec<Layer>{
    let s=layout.scale as f32;let c=&theme.colors;let area=super::inset(layout);
    let mut p=Paint::empty(theme,measure,s,layout.window_px,(-1.0,-1.0),false);
    let max=(480.0*s).min((area.w-32.0*s).max(0.0));
    let width=(p.width(text,13.0*s,400)+24.0*s).min(max).ceil();
    let card=r((area.x+area.w-16.0*s-width).round(),(area.y+area.h-16.0*s-36.0*s).round(),width,36.0*s);
    p.out.scene.layers.push(Layer::Shape(Shape{rect:card.array(),radius:8.0*s,fill:Fill::Solid(c.card),border:None,
        shadow:Some(clanky_render::Shadow{offset:[0.0,s],blur:2.0*s,spread:0.0,color:shadow_color(theme)})}));
    p.shape(card,8.0*s,c.card,Some(c.border));
    p.text(r(card.x+12.0*s,card.y,card.w-24.0*s,card.h),text,13.0*s,c.foreground,400,false);
    p.out.scene.layers
}

/// One command card over the live grid. The grid keeps its 20 px rows, so the header is
/// the command's own row (the sheet's 32 px header cannot fit without moving terminal
/// text); controls keep the sheet's order, gaps and colours at row height.
struct CardGeometry{id:String,frame:PixelRect,header:PixelRect,first_row:usize,clear_from:f32,command:String,
    badge:Option<(PixelRect,String,u8)>,duration:Option<(PixelRect,String)>,actions:Vec<(PixelRect,&'static str,&'static str)>,
    opacity:f32,hovered:bool,running:bool,failed:bool,focus:Option<usize>}
pub(super) fn duration_label(ms:u64)->String{
    if ms<1000{format!("{ms} ms")}else if ms<60_000{format!("{:.1} s",ms as f64/1000.0)}
    else{format!("{} m {} s",ms/60_000,(ms%60_000)/1000)}
}
fn card_geometry(p:&mut Paint,layout:&Layout,pane:&look::PaneLayout,data:&look::PaneState,block:&look::BlockCard,origin:[f32;2],state:&LookState)->Option<CardGeometry>{
    let s=layout.scale as f32;let ch=layout.cell_px[1] as f32;
    let rows=data.snapshot.rows;if block.first_row>=rows||block.last_row<block.first_row{return None;}
    let last=block.last_row.min(rows-1);
    let left=(origin[0]-6.0*s).max(pane.rect.x+s);let right=(origin[0]+pane.grid_clip.w+6.0*s).min(pane.rect.x+pane.rect.w-s);
    let top=origin[1]+block.first_row as f32*ch;let bottom=origin[1]+(last+1) as f32*ch-2.0*s;
    let clip_top=pane.grid_clip.y;let clip_bottom=pane.grid_clip.y+pane.grid_clip.h;
    if bottom<=clip_top||top>=clip_bottom{return None;}
    let frame=r(left,top,right-left,bottom-top);let header=r(left,top,right-left,ch.min(bottom-top));
    let key=(data.id.clone(),block.id.clone());let opacity=state.block_actions.get(&key).copied().unwrap_or(0.0);
    // Right to left, 8 px apart: exit badge, duration, then the actions (they fade in on hover).
    let mut x=right-6.0*s;let bh=18.0*s;let by=top+(ch-bh)/2.0;
    let badge=if block.running{Some(("● running".to_string(),2u8))}else{block.exit_code.map(|code|(code.to_string(),if code==0{0}else{1}))};
    let badge=badge.map(|(text,kind)|{let icon=if kind==2{0.0}else{16.0*s};let w=(p.width(&text,12.0*s,500)+16.0*s+icon).ceil()+s;x-=w;let rect=r(x,by,w,bh);x-=8.0*s;(rect,text,kind)});
    let duration=block.duration_ms.filter(|_|!block.running).map(duration_label).map(|text|{let w=p.width(&text,12.0*s,400);x-=w;let rect=r(x,top,w,ch);x-=8.0*s;(rect,text)});
    let mut actions=Vec::new();
    if opacity>0.0{
        let list:&[(&str,&str)]=if state.post_ai{&[("Copy","copy"),("Rerun","rerun")]}else{&[("Copy","copy"),("Rerun","rerun"),("Ask agent","send")]};
        let mut placed=Vec::new();
        for (label,action) in list.iter().rev(){let w=p.width(label,13.0*s,500)+16.0*s;x-=w;placed.push((r(x,top,w,ch),*label,*action));x-=2.0*s;}
        placed.reverse();actions=placed;x+=2.0*s;
    }
    let focus=state.block_action_focus.as_ref().filter(|(tab,id,_)|*tab==data.id&&*id==block.id).map(|(_,_,i)|*i);
    Some(CardGeometry{id:block.id.clone(),frame,header,first_row:block.first_row,clear_from:x-2.0*s,command:block.command.clone(),badge,duration,actions,opacity,
        hovered:frame.contains(p.pointer.0,p.pointer.1),running:block.running,failed:!block.running&&block.exit_code.is_some_and(|c|c!=0),focus})
}
fn card_under_grid(p:&mut Paint,card:&CardGeometry,snapshot:&mut clanky_core::TerminalSnapshot,layout:&Layout,origin:[f32;2]){
    let theme=p.theme;let c=&theme.colors;let s=p.scale;
    let band=if card.running{blend(c.muted,c.agent,0.07)}else{c.muted};
    // Header band: rounded top corners in the 6 px margins, square below them; the cells take the same fill.
    p.shape(card.header,8.0*s,band,None);
    p.shape(r(card.header.x,card.header.y+card.header.h/2.0,card.header.w,card.header.h/2.0),0.0,band,None);
    let background=rgb(c.background);let cw=layout.cell_px[0] as f32;
    if card.first_row<snapshot.rows{
        for col in 0..snapshot.cols{
            let cell=&mut snapshot.cells[card.first_row*snapshot.cols+col];
            if cell.bg==background{cell.bg=rgb(band);}
            // Controls sit on the right of the header row; text under them is hidden, not overdrawn.
            if origin[0]+(col+1) as f32*cw>card.clear_from{cell.ch=' ';}
        }
    }
    let _=&card.command;
}
fn card_over_grid(p:&mut Paint,card:&CardGeometry,tab:&str,state:&LookState){
    let theme=p.theme;let c=&theme.colors;let s=p.scale;
    let border=if card.failed{blend(c.border,c.err,0.30)}else if card.hovered{blend(c.border,c.foreground,0.14)}else{c.border};
    p.shape(card.frame,8.0*s,[0;4],Some(border));
    if let Some((rect,text))=&card.duration{p.text(*rect,text,12.0*s,c.muted_foreground,400,false);}
    if let Some((rect,text,kind))=&card.badge{
        let tone=match kind{0=>c.ok,1=>c.err,_=>c.agent};
        if *kind==2{
            p.shape(*rect,6.0*s,alpha(tone,0.10),Some(alpha(tone,0.35)));
            p.text(r(rect.x+8.0*s,rect.y,rect.w-15.0*s,rect.h),text,12.0*s,tone,500,false);
        }else{exit_badge(p,*rect,text,*kind==0,tone);}
    }
    for (index,(rect,label,action)) in card.actions.iter().enumerate(){
        let over=rect.contains(p.pointer.0,p.pointer.1);
        if over{p.shape(*rect,6.0*s,alpha(if p.pressed{c.pressed}else{c.accent},card.opacity),None);}
        if card.focus==Some(index){p.shape(*rect,6.0*s,[0;4],Some(alpha(c.ring,card.opacity)));p.shape(r(rect.x+s,rect.y+s,rect.w-2.0*s,rect.h-2.0*s),5.0*s,[0;4],Some(alpha(c.ring,card.opacity)));}
        p.text(r(rect.x+8.0*s,rect.y,rect.w-16.0*s,rect.h),label,13.0*s,alpha(c.foreground,card.opacity),500,false);
        p.hit(*rect,HitAction::Block{tab_id:tab.into(),block_id:card.id.clone(),action:(*action).into()});
    }
    let _=state;
}

/// The sheets' exit badge: 12 px / 500 in a tinted, bordered 6 px rounded box with a
/// check (success) or a cross (failure) before the code.
fn exit_badge(p:&mut Paint,rect:PixelRect,text:&str,ok:bool,tone:Color){
    let s=p.scale;
    p.shape(rect,6.0*s,alpha(tone,0.10),Some(alpha(tone,0.35)));
    let mut path=Path::builder();
    if ok{path.begin(point(2.5,6.2));path.line_to(point(4.8,8.5));path.line_to(point(9.5,3.6));path.end(false);}
    else{path.begin(point(3.0,3.0));path.line_to(point(9.0,9.0));path.end(false);path.begin(point(9.0,3.0));path.line_to(point(3.0,9.0));path.end(false);}
    let tx=rect.x+8.0*s;
    p.out.scene.layers.push(Layer::Vector(VectorLayer{paths:vec![VectorPath{path:path.build(),fill:None,stroke:Some(PathStroke{width:1.6,color:tone}),
        transform:Matrix4::scale(s,s,1.0).then(Matrix4::translate(tx,rect.y+(rect.h-12.0*s)/2.0,0.0))}],clip:p.clip,opacity:1.0}));
    p.text(r(tx+16.0*s,rect.y,rect.x+rect.w-7.0*s-tx-16.0*s,rect.h),text,12.0*s,tone,500,false);
}

/// Find bar over a pane's top right: the sheets' field (radius 8, input border, shadow)
/// holding the query, the match count, previous / next and close, each clickable.
fn find_bar(p:&mut Paint,pane:&look::PaneLayout,query:&str,total:usize){
    let theme=p.theme;let c=&theme.colors;let s=p.scale;
    let w=(360.0*s).min((pane.rect.w-16.0*s).max(0.0));if w<200.0*s{return;}
    let bar=r((pane.rect.x+pane.rect.w-8.0*s-w).round(),(pane.rect.y+48.0*s).round(),w,40.0*s);
    p.out.scene.layers.push(Layer::Shape(Shape{rect:bar.array(),radius:8.0*s,fill:Fill::Solid(c.background),border:None,shadow:Some(clanky_render::Shadow{offset:[0.0,2.0*s],blur:6.0*s,spread:0.0,color:shadow_color(theme)})}));
    p.shape(bar,8.0*s,c.background,Some(c.ring));
    p.icon("search",bar.x+12.0*s,bar.y+13.0*s,c.muted_foreground);
    // Right to left: close, next, previous (28 px ghost buttons, 2 px apart), then the count.
    let mut x=bar.x+bar.w-6.0*s;let mut buttons=Vec::new();
    for (kind,action) in [("close",0i8),("down",1),("up",-1)]{x-=28.0*s;buttons.push((r(x,bar.y+6.0*s,28.0*s,28.0*s),kind,action));x-=2.0*s;}
    for (rect,kind,action) in &buttons{
        if rect.contains(p.pointer.0,p.pointer.1){p.shape(*rect,6.0*s,if p.pressed{c.pressed}else{c.accent},None);}
        let mut path=Path::builder();
        match *kind{"close"=>{path.begin(point(4.0,4.0));path.line_to(point(12.0,12.0));path.end(false);path.begin(point(12.0,4.0));path.line_to(point(4.0,12.0));path.end(false);}
            "down"=>{path.begin(point(4.0,6.0));path.line_to(point(8.0,10.0));path.line_to(point(12.0,6.0));path.end(false);}
            _=>{path.begin(point(4.0,10.0));path.line_to(point(8.0,6.0));path.line_to(point(12.0,10.0));path.end(false);}}
        let disabled=*action!=0 && total==0;let color=if disabled{alpha(c.foreground,0.45)}else{c.foreground};
        p.out.scene.layers.push(Layer::Vector(VectorLayer{paths:vec![VectorPath{path:path.build(),fill:None,stroke:Some(PathStroke{width:1.5,color}),
            transform:Matrix4::scale(s,s,1.0).then(Matrix4::translate(rect.x+6.0*s,rect.y+6.0*s,0.0))}],clip:p.clip,opacity:1.0}));
        p.hit(*rect,HitAction::Find(*action));
    }
    let count=if query.is_empty(){String::new()}else if total==0{"No matches".into()}else if total==1{"1 match".into()}else{format!("{total} matches")};
    let cw=p.width(&count,12.0*s,400);x-=cw+4.0*s;p.text(r(x,bar.y,cw,bar.h),&count,12.0*s,c.muted_foreground,400,false);
    let text=r(bar.x+36.0*s,bar.y,(x-8.0*s-bar.x-36.0*s).max(0.0),bar.h);
    if query.is_empty(){p.text(text,"Find in this pane",14.0*s,c.muted_foreground,400,false);p.shape(r(text.x,bar.y+12.0*s,s,16.0*s),0.0,c.foreground,None);}
    else{p.text(text,query,14.0*s,c.foreground,400,false);let qw=p.width(query,14.0*s,400).min(text.w);p.shape(r((text.x+qw+2.0*s).round(),bar.y+12.0*s,s,16.0*s),0.0,c.foreground,None);}
}

/// Dialogs and docks that still use the panel model (open file, compose, editor review)
/// drawn as calm cards: radius 12, background, hairline border, the dialog shadow, a
/// 14 / 500 title, 14 px lines with the accent fill on the selected one. Text rows and
/// the editor grid sit exactly where the panel's keyboard and mouse code expects them.
fn calm_panel(p:&mut Paint,layout:&Layout,panel:&look::PanelState,font_px:f32){
    let theme=p.theme;let c=&theme.colors;let s=p.scale;let m=&layout.metrics;
    let rect=r(panel.rect.x,panel.rect.y+panel.rise_px,panel.rect.w,panel.rect.h);if rect.w<=0.0||rect.h<=0.0{return;}
    let fade=|color:Color|alpha(color,panel.opacity);
    let (tint,strength)=match theme.appearance{Appearance::Dark=>([0,0,0],[89u8,51u8]),Appearance::Light=>([60,40,20],[26,15])};
    for ((offset,blur),a) in [(16.0,48.0),(2.0,6.0)].into_iter().zip(strength){
        p.out.scene.layers.push(Layer::Shape(Shape{rect:rect.array(),radius:12.0*s,fill:Fill::Solid([0;4]),border:None,
            shadow:Some(clanky_render::Shadow{offset:[0.0,offset*s],blur:blur*s,spread:0.0,color:[tint[0],tint[1],tint[2],(a as f32*panel.opacity) as u8]})}));
    }
    p.shape(rect,12.0*s,fade(c.background),Some(fade(c.border)));
    let pad=m.panel_padding as f32;let header=m.panel_header as f32;let line=m.panel_line_height as f32;
    p.text(r(rect.x+pad,rect.y+pad,rect.w-2.0*pad,header),&panel.title,14.0*s,fade(c.foreground),500,false);
    let top=rect.y+pad+header;
    if panel.id=="file-prompt"{
        // The sheets' input: 32 tall, radius 8, input border, 14 px text, 1 px caret.
        let field=r(rect.x+pad,top+4.0*s,rect.w-2.0*pad,32.0*s);
        p.shape(field,8.0*s,fade(c.background),Some(fade(c.ring)));
        let text=panel.lines.first().map(|l|l.text.as_str()).unwrap_or("");let inner=r(field.x+10.0*s,field.y,field.w-20.0*s,field.h);
        // Long paths keep their end visible, where the file name is.
        let mut shown:String=text.into();while !shown.is_empty() && p.width(&shown,14.0*s,400)>inner.w-4.0*s{shown.remove(0);}
        if shown.is_empty(){p.text(inner,"~/git/clanky/README.md",14.0*s,fade(c.muted_foreground),400,false);}
        else{p.text(inner,&shown,14.0*s,fade(c.foreground),400,false);}
        let caret=if shown.is_empty(){inner.x}else{inner.x+p.width(&shown,14.0*s,400)+1.0*s};
        p.shape(r(caret.round(),field.y+8.0*s,s,16.0*s),0.0,fade(c.foreground),None);
        if let Some(hint)=panel.lines.get(1){p.text(r(rect.x+pad,field.y+field.h+8.0*s,rect.w-2.0*pad,16.0*s),&hint.text,12.0*s,fade(c.muted_foreground),400,false);}
        return;
    }
    if let Some(snapshot)=&panel.snapshot{
        let mut grid=snapshot.clone();let cursor=grid.cursor;grid.cursor=None;
        // Cells arrive already mapped to theme colours (background, selection, old / new tints).
        let panel_colors=term_colors(theme,&Default::default());for cell in &mut grid.cells{terminal(&panel_colors,cell);}
        let cw=layout.cell_px[0] as f32;let ch=layout.cell_px[1] as f32;
        let clip=r(rect.x+pad,top,(rect.w-2.0*pad).max(0.0),(rect.h-pad-header-pad).max(0.0));
        p.out.scene.layers.push(Layer::Grid(GridLayer{origin:[rect.x+pad,top],clip:clip.array(),snapshot:grid,underlines:panel.underlines.clone(),opacity:panel.opacity}));
        if let Some((row,col))=cursor{let x=rect.x+pad+col as f32*cw;let y=top+row as f32*ch;
            if y+ch<=clip.y+clip.h{p.shape(r(x,y+2.0*s,1.5*s,(ch-4.0*s).max(0.0)),0.0,fade(c.foreground),None);}}
        let _=font_px;
    }else{
        for (index,item) in panel.lines.iter().enumerate(){
            let row=r(rect.x+pad-4.0*s,top+index as f32*line,rect.w-2.0*pad+8.0*s,line);
            if row.y+row.h>rect.y+rect.h-pad{break;}
            let over=item.action.is_some()&&row.contains(p.pointer.0,p.pointer.1);
            if item.selected||over{p.shape(row,6.0*s,fade(if over&&p.pressed{c.pressed}else{c.accent}),None);}
            let muted=index>0&&item.action.is_none()&&!item.selected;
            p.text(r(row.x+4.0*s,row.y,row.w-8.0*s,row.h),&item.text,14.0*s,fade(if muted{c.muted_foreground}else{c.foreground}),400,false);
            if let Some(action)=&item.action{p.hit(row,action.clone());}
        }
    }
}

/// Review actions as the sheets' small buttons: outline for Accept, ghost for the
/// others, 13 px / 500, hover and pressed fills. Height follows the 20 px row.
/// Lays them out in `row` by measured width: Accept, Reject, Accept all, Reject all on
/// the left (8 px apart), Previous / Next on the right; returns each button's action.
pub fn review_buttons(layout:&Layout,theme:&Theme,row:PixelRect,pointer:(f64,f64),pressed:bool,measure:&mut dyn FnMut(&TextRun)->TextMetrics)->(Vec<Layer>,Vec<(PixelRect,&'static str)>){
    let s=layout.scale as f32;let c=&theme.colors;let mut p=Paint::empty(theme,measure,s,layout.window_px,pointer,pressed);
    let mut placed=Vec::new();let mut x=row.x;
    for (label,action,primary) in [("Accept","accept",true),("Reject","reject",false),("Accept all","accept_all",true),("Reject all","reject_all",false)]{
        let w=(p.width(label,13.0*s,500)+20.0*s).ceil();if x+w>row.x+row.w{break;}placed.push((r(x,row.y,w,row.h),label,action,primary));x+=w+8.0*s;
    }
    let mut right=row.x+row.w;
    for (label,action) in [("Next","review_next"),("Previous","review_previous")]{
        let w=(p.width(label,13.0*s,500)+20.0*s).ceil();if right-w<x{break;}right-=w;placed.push((r(right,row.y,w,row.h),label,action,false));right-=4.0*s;
    }
    let mut hits=Vec::new();
    for (rect,label,action,primary) in &placed{
        let face=r(rect.x,rect.y-2.0*s,rect.w,rect.h+1.0*s);hits.push((face,*action));
        let over=face.contains(pointer.0,pointer.1);
        let fill=if over{if pressed{c.pressed}else{c.accent}}else if *primary{c.background}else{[0;4]};
        if *primary{
            p.out.scene.layers.push(Layer::Shape(Shape{rect:face.array(),radius:6.0*s,fill:Fill::Solid(fill),border:None,shadow:Some(clanky_render::Shadow{offset:[0.0,s],blur:2.0*s,spread:0.0,color:shadow_color(theme)})}));
            p.shape(face,6.0*s,fill,Some(c.input));
        }else if over{p.shape(face,6.0*s,fill,None);}
        let w=p.width(label,13.0*s,500);
        p.text(r(face.x+(face.w-w)/2.0,face.y,w,face.h),label,13.0*s,c.foreground,500,false);
    }
    (p.out.scene.layers,hits)
}
