//! First visible calm slice: live grids and directly actionable chrome.
use crate::{look::{self,HitAction,Hotspot,Layout,LookState,PixelRect,Presentation},themes::{Appearance,Color,Theme}};
use clanky_core::{Cell,ColorSource};
use clanky_render::{Fill,GridLayer,Layer,Matrix4,PathStroke,Scene,Shape,TextMetrics,TextRun,VectorLayer,VectorPath};
use clanky_render::vector::lyon::{math::point,path::Path};

fn r(x:f32,y:f32,w:f32,h:f32)->PixelRect{PixelRect{x,y,w:w.max(0.0),h:h.max(0.0)}}
fn rgb(c:Color)->[u8;3]{[c[0],c[1],c[2]]}
fn alpha(mut c:Color,f:f32)->Color{c[3]=(c[3] as f32*f.clamp(0.0,1.0)).round() as u8;c}
fn blend(a:Color,b:Color,t:f32)->Color{std::array::from_fn(|i|(a[i] as f32*(1.0-t)+b[i] as f32*t).round() as u8)}
/// Adapter for existing palette/search/editor panels while their calm components land.
pub fn legacy_palette(theme:&Theme)->look::Theme{
    let c=&theme.colors;let mut t=look::Theme::dark();
    t.window=c.sidebar;t.rail=c.sidebar;t.panel=c.background;t.line=c.border;t.ink=c.foreground;
    t.dim=c.muted_foreground;t.accent=c.foreground;t.violet=c.agent;t.teal=c.agent;t.focus=c.ring;
    t.ok=c.ok;t.err=c.err;t.ok_ink=c.ok;t.err_ink=c.err;t.is_light=theme.appearance==Appearance::Light;
    t.refresh_mixes();t.selection=c.accent;t.card=c.card;t
}
fn terminal(theme:&Theme,cell:&mut Cell){
    let Some(source)=cell.color_source else{return;};
    let (fg,bg)=if source.inverse{(source.bg,source.fg)}else{(source.fg,source.bg)};
    let c=&theme.colors;
    cell.fg=match fg{ColorSource::Default=>rgb(if source.inverse{c.background}else{c.foreground}),ColorSource::Indexed(i @ 0..=15)=>rgb(theme.ansi[i as usize]),_=>cell.fg};
    cell.bg=match bg{ColorSource::Default=>rgb(if source.inverse{c.foreground}else{c.background}),ColorSource::Indexed(i @ 0..=15)=>rgb(theme.ansi[i as usize]),_=>cell.bg};
}
struct Paint<'a>{out:Presentation,theme:&'a Theme,measure:&'a mut dyn FnMut(&TextRun)->TextMetrics,scale:f32,pointer:(f64,f64),pressed:bool,clip:[f32;4]}
impl Paint<'_>{
    fn shape(&mut self,rect:PixelRect,radius:f32,fill:Color,border:Option<Color>){
        if rect.w>0.0 && rect.h>0.0{self.out.scene.layers.push(Layer::Shape(Shape{rect:rect.array(),radius,
            fill:Fill::Solid(fill),border:border.map(|c|(self.scale,c)),shadow:None}));}
    }
    fn width(&mut self,text:&str,px:f32,weight:u16)->f32{(self.measure)(&TextRun{text:text.into(),px,weight,..TextRun::default()}).width}
    fn text(&mut self,rect:PixelRect,text:&str,px:f32,color:Color,weight:u16,mono:bool){
        if rect.w<=0.0 || rect.h<=0.0{return;}
        let mut run=TextRun{text:text.chars().map(|c|if c.is_control(){' '}else{c}).take(2048).collect(),px,color,weight,mono,..TextRun::default()};
        if (self.measure)(&run).width>rect.w{
            let chars:Vec<_>=run.text.chars().collect();let (mut lo,mut hi)=(0,chars.len());
            while lo<hi{let mid=(lo+hi).div_ceil(2);run.text=chars[..mid].iter().collect::<String>()+"…";
                if (self.measure)(&run).width<=rect.w{lo=mid;}else{hi=mid-1;}}
            run.text=chars[..lo].iter().collect::<String>()+"…";
        }
        let metrics=(self.measure)(&run);
        if metrics.width>rect.w{return;}
        run.origin=[rect.x,rect.y+(rect.h-metrics.line_height)/2.0];self.out.scene.layers.push(Layer::Text(run));
    }
    fn hit(&mut self,rect:PixelRect,action:HitAction){if rect.w>0.0 && rect.h>0.0{self.out.hits.push(Hotspot{rect,action});}}
    fn icon(&mut self,kind:&str,x:f32,y:f32,color:Color){
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
    fn kbd(&mut self,right:f32,y:f32,keys:&[&str])->f32{
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
    fn button(&mut self,rect:PixelRect,label:&str,action:HitAction,primary:bool){
        let c=&self.theme.colors;let hovered=rect.contains(self.pointer.0,self.pointer.1);
        let fill=if primary {
            if hovered && self.pressed{c.get("primary_pressed").unwrap_or(match self.theme.appearance{Appearance::Dark=>[214,208,200,255],Appearance::Light=>[61,56,50,255]})}
            else if hovered{blend(c.primary,c.background,0.1)}else{c.primary}
        }else if hovered{if self.pressed{c.pressed}else{c.accent}}else{c.background};
        let color=if primary{c.primary_foreground}else{c.foreground};
        self.shape(rect,8.0*self.scale,fill,(!primary).then_some(c.input));
        let width=self.width(label,14.0*self.scale,500);
        self.text(r(rect.x+(rect.w-width)/2.0,rect.y,width,rect.h),label,14.0*self.scale,color,500,false);
        self.hit(rect,action);
    }
    fn item(&mut self,rect:PixelRect,label:&str,count:&str,active:bool,action:HitAction,icon:Option<&str>,dot:Option<Color>){
        let c=&self.theme.colors;
        if active || rect.contains(self.pointer.0,self.pointer.1){self.shape(rect,6.0*self.scale,c.sidebar_accent,None);}
        let x=rect.x+8.0*self.scale;
        if let Some(icon)=icon{self.icon(icon,x,rect.y+8.0*self.scale,alpha(c.foreground,0.8));}
        if let Some(color)=dot{self.shape(r(x+4.0*self.scale,rect.y+12.0*self.scale,8.0*self.scale,8.0*self.scale),4.0*self.scale,color,None);}
        let text_x=x+if icon.is_some()||dot.is_some(){24.0*self.scale}else{0.0};
        let count_width=self.width(count,12.0*self.scale,400);
        self.text(r(text_x,rect.y,(rect.x+rect.w-8.0*self.scale-count_width-8.0*self.scale-text_x).max(0.0),rect.h),label,14.0*self.scale,c.foreground,if active{500}else{400},false);
        self.text(r(rect.x+rect.w-8.0*self.scale-count_width,rect.y,count_width,rect.h),count,12.0*self.scale,c.muted_foreground,400,false);
        self.hit(rect,action);
    }
}

pub fn presentation(layout:&Layout,state:&LookState,theme:&Theme,font_px:f32,pointer:(f64,f64),pressed:bool,measure:&mut dyn FnMut(&TextRun)->TextMetrics)->Presentation{
    let c=&theme.colors;let s=layout.scale as f32;let [w,h]=layout.window_px.map(|n|n as f32);let inset=super::inset(layout);
    let mut p=Paint{out:Presentation{scene:Scene{clear:alpha(c.sidebar,state.opacity),layers:vec![]},hits:vec![]},theme,measure,scale:s,pointer,pressed,clip:[0.0,0.0,w,h]};
    p.shape(inset,12.0*s,c.background,Some(c.border));
    if let Some(side)=layout.left_rail{
        p.shape(r(side.x+16.0*s,16.0*s,32.0*s,32.0*s),8.0*s,c.primary,None);
        let cw=p.width("C",15.0*s,600);
        p.text(r(side.x+16.0*s+(32.0*s-cw)/2.0,16.0*s,cw,32.0*s),"C",15.0*s,c.primary_foreground,600,false);
        p.text(r(side.x+58.0*s,14.0*s,side.w-74.0*s,20.0*s),"Clanky",14.0*s,c.foreground,600,false);
        p.text(r(side.x+58.0*s,34.0*s,side.w-74.0*s,16.0*s),&super::uptime_label(state.uptime_seconds),12.0*s,c.muted_foreground,400,false);
        let mut y=72.0*s;let bottom=h-92.0*s;
        for section in 0..3{
            if y+32.0*s>bottom{break;}
            p.text(r(side.x+16.0*s,y,side.w-32.0*s,32.0*s),["Sessions","Agents","Files"][section],12.0*s,c.muted_foreground,500,false);y+=32.0*s;
            match section{
                0=>for item in &state.sessions{if y+32.0*s>bottom{break;}
                    let n=state.session_counts.get(&item.id).copied().unwrap_or(1);
                    p.item(r(side.x+8.0*s,y,side.w-16.0*s,32.0*s),&item.title,&format!("{n} {}",if n==1{"pane"}else{"panes"}),item.active,HitAction::Session(item.id.clone()),Some("terminal"),None);y+=34.0*s;},
                1=>for item in &state.agents{if y+32.0*s>bottom{break;}
                    let name=state.agent_labels.get(&item.connection_id).cloned().unwrap_or_else(||format!("agent {}",item.peer_pid));
                    p.item(r(side.x+8.0*s,y,side.w-16.0*s,32.0*s),&name,if item.running{"running"}else{"idle"},false,HitAction::Agent{connection_id:item.connection_id,tab_id:item.tab_id.clone()},None,Some(if item.running{c.agent}else{c.muted_foreground}));y+=34.0*s;},
                _=>for item in &state.files{if y+32.0*s>bottom{break;}
                    let count=if item.pending>0{format!("{} changes",item.pending)}else{String::new()};
                    p.item(r(side.x+8.0*s,y,side.w-16.0*s,32.0*s),&item.title,&count,item.active,HitAction::File(item.id.clone()),Some("file"),None);y+=34.0*s;},
            }
            if (section==1 && state.agents.is_empty())||(section==2 && state.files.is_empty()){
                p.text(r(side.x+16.0*s,y,side.w-32.0*s,32.0*s),if section==1{"No agents connected"}else{"No open files"},12.0*s,c.muted_foreground,400,false);y+=32.0*s;
            }
            y+=12.0*s;
        }
        p.item(r(side.x+8.0*s,h-80.0*s,side.w-16.0*s,32.0*s),"Settings","",false,HitAction::Settings,Some("settings"),None);
        p.kbd(side.x+side.w-16.0*s,h-74.0*s,&["Ctrl",","]);
        p.text(r(side.x+16.0*s,h-40.0*s,side.w-32.0*s,32.0*s),if state.masking_ready{"masking on"}else{"checking masking"},12.0*s,c.muted_foreground,400,false);
    }
    let bar=layout.top_bar;let by=bar.y+12.0*s;
    let toggle=r(bar.x+16.0*s,by,32.0*s,32.0*s);
    if toggle.contains(pointer.0,pointer.1){p.shape(toggle,8.0*s,if pressed{c.pressed}else{c.accent},None);}
    p.icon("sidebar",toggle.x+8.0*s,toggle.y+8.0*s,c.foreground);p.hit(toggle,HitAction::ToggleRails);
    let new_width=p.width("New tab",14.0*s,500)+24.0*s;
    let new_tab=r(bar.x+bar.w-16.0*s-new_width,by,new_width,32.0*s);
    p.button(new_tab,"New tab",HitAction::NewTab,true);
    let split_width=p.width("Split",14.0*s,500)+24.0*s;
    let split=r(new_tab.x-8.0*s-split_width,by,split_width,32.0*s);
    p.button(split,"Split",HitAction::Split,false);
    let search=r((split.x-308.0*s).max(toggle.x+64.0*s),by,(split.x-8.0*s-(split.x-308.0*s).max(toggle.x+64.0*s)).max(0.0),32.0*s);
    p.shape(search,8.0*s,c.background,Some(if search.contains(pointer.0,pointer.1){c.ring}else{c.input}));
    p.icon("search",search.x+10.0*s,search.y+8.0*s,c.muted_foreground);
    let hint_left=p.kbd(search.x+search.w-8.0*s,search.y+6.0*s,&["Ctrl","Shift","P"]);
    p.text(r(search.x+32.0*s,search.y,(hint_left-search.x-40.0*s).max(0.0),search.h),"Search or run a command…",14.0*s,c.muted_foreground,400,false);p.hit(search,HitAction::Palette);
    let title=state.panes.iter().find(|pane|state.focused.as_ref()==Some(&pane.id)).or_else(||state.panes.first()).map(|pane|pane.title.as_str()).unwrap_or("Clanky");
    p.text(r(toggle.x+48.0*s,bar.y,(search.x-toggle.x-64.0*s).max(0.0),bar.h),title,14.0*s,c.foreground,400,false);
    p.shape(r(bar.x+s,bar.y+bar.h-s,bar.w-2.0*s,s),0.0,c.border,None);
    for pane in &layout.panes{
        let data=state.panes.iter().find(|data|data.id==pane.id);let focus=state.focused.as_ref()==Some(&pane.id);
        p.shape(pane.rect,10.0*s,c.background,Some(if focus{c.ring}else{c.border}));
        p.shape(r(pane.rect.x+s,pane.rect.y+40.0*s-s,pane.rect.w-2.0*s,s),0.0,c.border,None);
        let Some(data)=data else{continue;};
        let access=if data.allow_agent{"read + run"}else{"agents off"};let badge_width=p.width(access,12.0*s,500)+16.0*s;
        let badge=r(pane.rect.x+pane.rect.w-8.0*s-badge_width,pane.rect.y+9.0*s,badge_width,22.0*s);
        p.shape(badge,6.0*s,if data.allow_agent{alpha(c.agent,0.1)}else{alpha(c.background,0.0)},Some(if data.allow_agent{alpha(c.agent,0.35)}else{c.border}));
        p.text(r(badge.x+8.0*s,badge.y,badge.w-16.0*s,badge.h),access,12.0*s,if data.allow_agent{c.agent}else{c.muted_foreground},500,false);
        let available=(badge.x-pane.rect.x-24.0*s).max(0.0);let driver_width=p.width(&data.driver,12.0*s,400).min(available*0.3);
        let title_width=p.width(&data.title,14.0*s,500).min((available-driver_width-8.0*s).max(0.0));
        p.text(r(pane.rect.x+12.0*s,pane.rect.y,title_width,40.0*s),&data.title,14.0*s,c.foreground,500,false);
        p.text(r(pane.rect.x+20.0*s+title_width,pane.rect.y,driver_width,40.0*s),&data.driver,12.0*s,c.muted_foreground,400,false);
        let mut snapshot=data.snapshot.clone();let cursor=snapshot.cursor;snapshot.cursor=None;
        for cell in &mut snapshot.cells{terminal(theme,cell);}
        let origin=[pane.grid_origin[0],pane.grid_origin[1]-data.scroll_px];
        p.out.scene.layers.push(Layer::Grid(GridLayer{origin,clip:pane.grid_clip.array(),snapshot,underlines:data.underlines.clone(),opacity:1.0}));
        if focus{if let Some((row,col))=cursor.filter(|(row,col)|*row<pane.rows && *col<pane.cols){
            let x=origin[0]+col as f32*layout.cell_px[0] as f32;let y=origin[1]+row as f32*layout.cell_px[1] as f32;
            p.shape(r(x,y+2.0*s,layout.cell_px[0] as f32,(layout.cell_px[1] as f32-4.0*s).max(0.0)),0.0,c.foreground,None);
            let ch=data.snapshot.cells[row*data.snapshot.cols+col].ch;
            if ch!=' ' {p.text(r(x,y,layout.cell_px[0] as f32,layout.cell_px[1] as f32),&ch.to_string(),font_px,c.background,400,true);}
        }}
    }
    if let Some(side)=layout.right_rail{
        p.shape(r(side.x,side.y,s,side.h),0.0,c.border,None);
        let x=side.x+12.0*s;let width=(side.w-24.0*s).max(0.0);let mut y=side.y+12.0*s;
        p.shape(r(x,y,width,164.0*s),10.0*s,c.card,Some(c.border));
        p.text(r(x+12.0*s,y+12.0*s,width-90.0*s,22.0*s),"Agent activity",14.0*s,c.foreground,500,false);
        p.text(r(x+width-64.0*s,y+12.0*s,52.0*s,22.0*s),"30 min",12.0*s,c.muted_foreground,400,false);
        let total: u64=state.activity.iter().filter(|n|n.is_finite() && **n>0.0).map(|n|*n as u64).fold(0,u64::saturating_add);
        p.text(r(x+12.0*s,y+42.0*s,70.0*s,28.0*s),&total.to_string(),24.0*s,c.foreground,600,false);
        p.text(r(x+84.0*s,y+48.0*s,width-96.0*s,22.0*s),"commands",12.0*s,c.muted_foreground,400,false);
        let chart=r(x+12.0*s,y+82.0*s,width-24.0*s,40.0*s);let mut path=Path::builder();
        let samples:Vec<_>=state.activity.iter().rev().take(30).copied().collect();let max=samples.iter().filter(|v|v.is_finite()).copied().fold(1.0_f32,f32::max);
        let has_commands=samples.iter().any(|value|value.is_finite() && *value>0.0);
        if !has_commands{path.begin(point(chart.x,chart.y+chart.h-s));path.line_to(point(chart.x+chart.w,chart.y+chart.h-s));}
        else{for(i,value)in samples.iter().rev().enumerate(){let px=chart.x+i as f32*chart.w/samples.len().saturating_sub(1).max(1) as f32;let py=chart.y+chart.h-(if value.is_finite(){(*value/max).clamp(0.0,1.0)}else{0.0})*chart.h;if i==0{path.begin(point(px,py));}else{path.line_to(point(px,py));}}}path.end(false);
        p.out.scene.layers.push(Layer::Vector(VectorLayer{paths:vec![VectorPath{path:path.build(),fill:None,stroke:Some(PathStroke{width:1.5*s,color:c.muted_foreground}),transform:Matrix4::IDENTITY}],clip:chart.array(),opacity:1.0}));
        if !has_commands{p.text(r(chart.x,chart.y,chart.w,chart.h-8.0*s),"No agent commands yet",12.0*s,c.muted_foreground,400,false);}
        p.text(r(x+12.0*s,y+132.0*s,width-24.0*s,20.0*s),"Commands agents ran per minute",12.0*s,c.muted_foreground,400,false);y+=176.0*s;
        let count=state.timeline.len().min(4);let height=(48.0+36.0*count.max(1) as f32)*s;
        p.shape(r(x,y,width,height),10.0*s,c.card,Some(c.border));p.text(r(x+12.0*s,y+12.0*s,width-24.0*s,22.0*s),"Recent",14.0*s,c.foreground,500,false);
        if count==0{p.text(r(x+12.0*s,y+40.0*s,width-24.0*s,20.0*s),"No recent events",12.0*s,c.muted_foreground,400,false);}
        for (n,(index,event)) in state.timeline.iter().enumerate().rev().take(4).enumerate(){let ey=y+(40.0+n as f32*36.0)*s;
            p.shape(r(x+12.0*s,ey+6.0*s,8.0*s,8.0*s),4.0*s,c.muted_foreground,None);
            let time=state.timeline_times.get(index).map(String::as_str).unwrap_or("");let tw=p.width(time,12.0*s,400);
            p.text(r(x+30.0*s,ey,width-54.0*s-tw,18.0*s),&event.label.replace('_'," "),14.0*s,c.foreground,400,false);
            p.text(r(x+30.0*s,ey+18.0*s,width-42.0*s,16.0*s),&event.detail,12.0*s,c.muted_foreground,400,false);
            p.text(r(x+width-12.0*s-tw,ey,tw,18.0*s),time,12.0*s,c.muted_foreground,400,false);
        }y+=height+12.0*s;
        let height=(48.0+32.0*state.panes.len() as f32)*s;
        p.shape(r(x,y,width,height),10.0*s,c.card,Some(c.border));p.text(r(x+12.0*s,y+12.0*s,width-24.0*s,22.0*s),"Agent access",14.0*s,c.foreground,500,false);
        for (index,pane)in state.panes.iter().enumerate(){let row=r(x+12.0*s,y+(40.0+32.0*index as f32)*s,width-24.0*s,32.0*s);
            if row.y+row.h>side.y+side.h-8.0*s{break;}
            if row.contains(pointer.0,pointer.1){p.shape(row,6.0*s,c.accent,None);}
            p.text(r(row.x,row.y,row.w-44.0*s,18.0*s),state.pane_labels.get(&pane.id).unwrap_or(&pane.title),14.0*s,c.foreground,400,false);
            p.text(r(row.x,row.y+18.0*s,row.w-44.0*s,14.0*s),if pane.allow_agent{"read + run"}else{"off"},12.0*s,c.muted_foreground,400,false);
            let sw=r(row.x+row.w-32.0*s,row.y,32.0*s,18.0*s);
            p.shape(sw,9.0*s,if pane.allow_agent{c.primary}else{c.input},None);
            p.shape(r(sw.x+if pane.allow_agent{16.0*s}else{2.0*s},sw.y+2.0*s,14.0*s,14.0*s),7.0*s,if pane.allow_agent{c.primary_foreground}else{c.background},None);
            p.hit(row,HitAction::Access(pane.id.clone()));
        }
    }
    let panels=look::panel_presentation(layout,state,p.measure);p.out.scene.layers.extend(panels.scene.layers);p.out.hits.extend(panels.hits);
    p.out
}
