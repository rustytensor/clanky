//! First visible calm slice: live grids and directly actionable chrome.
use crate::{look::{self,HitAction,Hotspot,Layout,LookState,PixelRect,Presentation},themes::{Appearance,Color,Theme}};
use clanky_core::{Cell,ColorSource};
use clanky_render::{Fill,GridLayer,Layer,Matrix4,PathStroke,Scene,Shape,TextMetrics,TextRun,VectorLayer,VectorPath};
use clanky_render::vector::lyon::{math::point,path::Path};

fn r(x:f32,y:f32,w:f32,h:f32)->PixelRect{PixelRect{x,y,w:w.max(0.0),h:h.max(0.0)}}
fn rgb(c:Color)->[u8;3]{[c[0],c[1],c[2]]}
fn alpha(mut c:Color,f:f32)->Color{c[3]=(c[3] as f32*f.clamp(0.0,1.0)).round() as u8;c}
fn shadow_color(theme:&Theme)->Color{match theme.appearance{Appearance::Dark=>[0,0,0,89],Appearance::Light=>[60,40,20,15]}}
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
pub(super) struct Paint<'a>{pub(super) out:Presentation,theme:&'a Theme,measure:&'a mut dyn FnMut(&TextRun)->TextMetrics,scale:f32,pointer:(f64,f64),pressed:bool,clip:[f32;4]}
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
    fn hit(&mut self,rect:PixelRect,action:HitAction){if rect.w>0.0 && rect.h>0.0{self.out.hits.push(Hotspot{rect,action});}}
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
    let mut p=Paint{out:Presentation{scene:crate::theme_scene::window_background(theme,layout.window_px,state.opacity),hits:vec![]},theme,measure,scale:s,pointer,pressed,clip:[0.0,0.0,w,h]};
    p.shape(inset,12.0*s,c.background,Some(c.border));
    if let Some(side)=layout.left_rail{
        p.shape(r(side.x+16.0*s,16.0*s,32.0*s,32.0*s),8.0*s,c.primary,None);
        let cw=p.width("C",15.0*s,600);
        p.text(r(side.x+16.0*s+(32.0*s-cw)/2.0,16.0*s,cw,32.0*s),"C",15.0*s,c.primary_foreground,600,false);
        // calm.html .sb-head .t: two 16 px lines beside the 32 px logo.
        p.text(r(side.x+58.0*s,16.0*s,side.w-74.0*s,16.0*s),"Clanky",14.0*s,c.foreground,600,false);
        p.text(r(side.x+58.0*s,32.0*s,side.w-74.0*s,16.0*s),&super::uptime_label(state.uptime_seconds),12.0*s,c.muted_foreground,400,false);
        let mut y=72.0*s;let bottom=h-108.0*s;
        for section in 0..3{
            if state.post_ai && section==1{continue;}
            if y+32.0*s>bottom{break;}
            p.text(r(side.x+16.0*s,y,side.w-32.0*s,32.0*s),["Sessions","Agents","Files"][section],12.0*s,c.muted_foreground,500,false);y+=32.0*s;
            match section{
                0=>for item in &state.sessions{if y+32.0*s>bottom{break;}
                    let n=state.session_counts.get(&item.id).copied().unwrap_or(1);
                    p.item(r(side.x+8.0*s,y,side.w-16.0*s,32.0*s),&item.title,&format!("{n} {}",if n==1{"pane"}else{"panes"}),item.active,HitAction::Session(item.id.clone()),Some("terminal"),None);y+=34.0*s;},
                1=>for item in &state.agents{if y+32.0*s>bottom{break;}
                    let name=state.agent_labels.get(&item.connection_id).cloned().unwrap_or_else(||"Agent".into());
                    let row=r(side.x+8.0*s,y,side.w-16.0*s,32.0*s);
                    p.item(row,&name,if item.running{""}else{"idle"},false,HitAction::Agent{connection_id:item.connection_id,tab_id:item.tab_id.clone()},None,Some(if item.running{c.agent}else{c.muted_foreground}));
                    // calm.html: a running agent shows the clay badge instead of plain text.
                    if item.running{let bw=(p.width("running",12.0*s,500)+16.0*s).ceil();let badge=r(row.x+row.w-8.0*s-bw,row.y+5.0*s,bw,22.0*s);
                        p.shape(badge,6.0*s,alpha(c.agent,0.1),Some(alpha(c.agent,0.35)));p.text(r(badge.x+8.0*s,badge.y,bw-16.0*s,badge.h),"running",12.0*s,c.agent,500,false);}
                    y+=34.0*s;},
                _=>for item in &state.files{if y+32.0*s>bottom{break;}
                    let count=if item.pending>0{format!("{} changes",item.pending)}else{String::new()};
                    p.item(r(side.x+8.0*s,y,side.w-16.0*s,32.0*s),&item.title,&count,item.active,HitAction::File(item.id.clone()),Some("file"),None);y+=34.0*s;},
            }
            if (section==1 && state.agents.is_empty())||(section==2 && state.files.is_empty()){
                p.text(r(side.x+16.0*s,y,side.w-32.0*s,32.0*s),if section==1{"No agents connected"}else{"No open files"},12.0*s,c.muted_foreground,400,false);y+=32.0*s;
            }
            y+=12.0*s;
        }
        p.item(r(side.x+8.0*s,h-96.0*s,side.w-16.0*s,32.0*s),"Settings","",false,HitAction::Settings,Some("settings"),None);
        p.kbd(side.x+side.w-16.0*s,h-90.0*s,&["Ctrl",","]);
        // calm.html user block: padding 8, radius 8, 32 px avatar (radius 8, muted, 12 px / 600 initials), gap 10.
        let user=r(side.x+8.0*s,h-60.0*s,side.w-16.0*s,48.0*s);
        if user.contains(pointer.0,pointer.1){p.shape(user,8.0*s,if pressed{c.pressed}else{c.sidebar_accent},None);}
        let avatar=r(user.x+8.0*s,user.y+8.0*s,32.0*s,32.0*s);p.shape(avatar,8.0*s,c.muted,None);
        let initials:String=state.user_name.split_whitespace().filter_map(|word|word.chars().next()).take(2).flat_map(char::to_uppercase).collect();
        let initials=if initials.is_empty(){"·".to_string()}else{initials};let iw=p.width(&initials,12.0*s,600);
        p.text(r(avatar.x+(avatar.w-iw)/2.0,avatar.y,iw,avatar.h),&initials,12.0*s,c.foreground,600,false);
        let name=state.user_name.split_whitespace().next().unwrap_or("You").to_string();
        p.text(r(avatar.x+42.0*s,user.y+8.0*s,user.w-58.0*s,16.0*s),&name,14.0*s,c.foreground,500,false);
        p.text(r(avatar.x+42.0*s,user.y+24.0*s,user.w-58.0*s,16.0*s),if state.masking_ready{"masking on"}else{"checking masking"},12.0*s,c.muted_foreground,400,false);
        p.hit(user,HitAction::Settings);
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
    // Search and run field (sheets: 300 x 32, radius 8, 14 px, kbd chip); it opens the palette.
    // At least the sheet's 300 px; wider when the per-key chips would clip the placeholder.
    let chips:f32=["Ctrl","Shift","P"].iter().map(|k|p.mono_width(k,12.0*s,400)+14.0*s).sum();
    let field_w=(300.0*s).max(32.0*s+p.width("Search or run a command…",14.0*s,400)+12.0*s+chips+8.0*s).ceil();
    let field=r(split.x-8.0*s-field_w,by,field_w,32.0*s);
    if field.x>toggle.x+160.0*s{
        let over=field.contains(pointer.0,pointer.1);
        if state.palette_open{p.shape(r(field.x-4.0*s,field.y-4.0*s,field.w+8.0*s,field.h+8.0*s),12.0*s,[0;4],Some(c.ring));p.shape(r(field.x-3.0*s,field.y-3.0*s,field.w+6.0*s,field.h+6.0*s),11.0*s,[0;4],Some(c.ring));}
        p.out.scene.layers.push(Layer::Shape(Shape{rect:field.array(),radius:8.0*s,fill:Fill::Solid(c.background),border:None,shadow:Some(clanky_render::Shadow{offset:[0.0,s],blur:2.0*s,spread:0.0,color:shadow_color(theme)})}));
        p.shape(field,8.0*s,c.background,Some(if over{blend(c.input,c.foreground,0.2)}else{c.input}));
        p.icon("search",field.x+10.0*s,field.y+9.0*s,c.muted_foreground);
        let hint=p.kbd(field.x+field.w-8.0*s,field.y+6.0*s,&["Ctrl","Shift","P"]);
        p.text(r(field.x+32.0*s,field.y,(hint-field.x-40.0*s).max(0.0),field.h),"Search or run a command…",14.0*s,c.muted_foreground,400,false);
        p.hit(field,HitAction::Palette);
    }
    // calm.html: toggle, a 1 x 16 separator (margin 0 4), then "session / ~/folder" crumbs, gap 8.
    let title_right=if field.x>toggle.x+160.0*s{field.x-16.0*s}else{split.x-16.0*s};
    p.shape(r(toggle.x+44.0*s,bar.y+(bar.h-16.0*s)/2.0,s,16.0*s),0.0,c.border,None);
    let folder=state.panes.iter().find(|pane|state.focused.as_ref()==Some(&pane.id)).or_else(||state.panes.first()).map(|pane|pane.title.clone()).unwrap_or_else(||"Clanky".into());
    let session=state.sessions.iter().find(|item|item.active).map(|item|item.title.clone());
    let mut x=toggle.x+57.0*s;
    if let Some(session)=session{
        let sw=p.width(&session,14.0*s,400).min(((title_right-x)*0.4).max(0.0));
        p.text(r(x,bar.y,sw,bar.h),&session,14.0*s,c.muted_foreground,400,false);x+=sw+8.0*s;
        let slash=p.width("/",14.0*s,400);p.text(r(x,bar.y,slash,bar.h),"/",14.0*s,c.muted_foreground,400,false);x+=slash+8.0*s;
    }
    p.text(r(x,bar.y,(title_right-x).max(0.0),bar.h),&folder,14.0*s,c.foreground,400,false);
    p.shape(r(bar.x+s,bar.y+bar.h-s,bar.w-2.0*s,s),0.0,c.border,None);
    for pane in &layout.panes{
        let data=state.panes.iter().find(|data|data.id==pane.id);let focus=state.focused.as_ref()==Some(&pane.id);
        let driving=data.is_some_and(|d|d.agent_running);
        // Only themes that ask for it (glow: true) glow, as v0.6.1 drew it: 6 px, no spread.
        if theme.glow && (focus || driving){
            let color=if driving{c.agent}else{alpha(c.ring,if theme.appearance==Appearance::Light{0.22}else{0.35})};
            p.out.scene.layers.push(Layer::Shape(Shape{rect:pane.rect.array(),radius:10.0*s,fill:Fill::Solid([0;4]),border:None,
                shadow:Some(clanky_render::Shadow{offset:[0.0,0.0],blur:6.0*s,spread:0.0,color})}));
        }
        p.shape(pane.rect,10.0*s,c.background,Some(if driving{blend(c.border,c.agent,0.45)}else if focus{c.ring}else{c.border}));
        p.shape(r(pane.rect.x+s,pane.rect.y+40.0*s-s,pane.rect.w-2.0*s,s),0.0,c.border,None);
        let Some(data)=data else{continue;};
        let editor=data.driver.starts_with("Local editor");
        let access=if data.allow_agent{"read + run"}else{"agents off"};let badge_width=if state.post_ai||editor{0.0}else{p.width(access,12.0*s,500)+16.0*s};
        let badge=r(pane.rect.x+pane.rect.w-8.0*s-badge_width,pane.rect.y+9.0*s,badge_width,22.0*s);
        // Sheets: off is the plain badge, allowed the neutral solid badge; clay only while an agent drives.
        if !state.post_ai && !editor {
            let (fill,border,ink)=if data.agent_running{(alpha(c.agent,0.1),Some(alpha(c.agent,0.35)),c.agent)}
                else if data.allow_agent{(c.muted,None,c.foreground)}else{([0;4],Some(c.border),c.muted_foreground)};
            p.shape(badge,6.0*s,fill,border);
            p.text(r(badge.x+8.0*s,badge.y,badge.w-16.0*s,badge.h),access,12.0*s,ink,500,false);
        }
        let available=(badge.x-pane.rect.x-24.0*s).max(0.0);let driver_width=p.width(&data.driver,12.0*s,400).min((available-p.width(&data.title,14.0*s,500)-8.0*s).max(available*0.3));
        let title_width=p.width(&data.title,14.0*s,500).min((available-driver_width-8.0*s).max(0.0));
        p.text(r(pane.rect.x+12.0*s,pane.rect.y,title_width,40.0*s),&data.title,14.0*s,c.foreground,500,false);
        p.text(r(pane.rect.x+20.0*s+title_width,pane.rect.y,driver_width,40.0*s),&data.driver,12.0*s,c.muted_foreground,400,false);
        let mut snapshot=data.snapshot.clone();let cursor=snapshot.cursor;snapshot.cursor=None;
        for cell in &mut snapshot.cells{terminal(theme,cell);}
        let origin=[pane.grid_origin[0],pane.grid_origin[1]-data.scroll_px];
        let cards:Vec<_>=data.blocks.iter().filter_map(|block|card_geometry(&mut p,layout,pane,data,block,origin,state)).collect();
        for card in &cards{card_under_grid(&mut p,card,&mut snapshot,layout,origin);}
        p.out.scene.layers.push(Layer::Grid(GridLayer{origin,clip:pane.grid_clip.array(),snapshot,underlines:data.underlines.clone(),opacity:1.0}));
        for card in &cards{card_over_grid(&mut p,card,&data.id,state);}
        if focus{if let Some((row,col))=cursor.filter(|(row,col)|*row<pane.rows && *col<pane.cols){
            let x=origin[0]+col as f32*layout.cell_px[0] as f32;let y=origin[1]+row as f32*layout.cell_px[1] as f32;
            p.shape(r(x,y+2.0*s,layout.cell_px[0] as f32,(layout.cell_px[1] as f32-4.0*s).max(0.0)),0.0,c.foreground,None);
            let ch=data.snapshot.cells[row*data.snapshot.cols+col].ch;
            if ch!=' ' {p.text(r(x,y,layout.cell_px[0] as f32,layout.cell_px[1] as f32),&ch.to_string(),font_px,c.background,400,true);}
        }}
    }
    for panel in &state.panels{if panel.id!="palette" && panel.id!="search"{calm_panel(&mut p,layout,panel,font_px);}}
    if let Some((id,query,total))=&state.find{if let Some(pane)=layout.panes.iter().find(|pane|pane.id==*id){find_bar(&mut p,pane,query,*total);}}
    if let Some(side)=layout.right_rail{
        p.shape(r(side.x,side.y,s,side.h),0.0,c.border,None);
        let x=side.x+12.0*s;let width=(side.w-24.0*s).max(0.0);let mut y=side.y+12.0*s;
        p.shape(r(x,y,width,164.0*s),10.0*s,c.card,Some(c.border));
        p.text(r(x+12.0*s,y+12.0*s,width-90.0*s,22.0*s),"Agent activity",14.0*s,c.foreground,500,false);
        // calm.html: <span class="badge muted">30 min</span>, 22 tall, radius 6, 12 px / 500, padding 0 8.
        let span_w=p.width("30 min",12.0*s,500);let span=r(x+width-12.0*s-span_w-16.0*s,y+12.0*s,span_w+16.0*s,22.0*s);
        p.shape(span,6.0*s,[0;4],Some(c.border));
        p.text(r(span.x+8.0*s,span.y,span_w,span.h),"30 min",12.0*s,c.muted_foreground,500,false);
        let total: u64=state.activity.iter().filter(|n|n.is_finite() && **n>0.0).map(|n|*n as u64).fold(0,u64::saturating_add);
        p.text(r(x+12.0*s,y+42.0*s,70.0*s,28.0*s),&total.to_string(),24.0*s,c.foreground,600,false);
        // calm.html: "47 commands", the label 6 px after the number on its baseline.
        let number_w=p.width(&total.to_string(),24.0*s,600);
        p.text(r(x+12.0*s+number_w+6.0*s,y+48.0*s,(width-30.0*s-number_w).max(0.0),22.0*s),"commands",12.0*s,c.muted_foreground,400,false);
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
            let dot=match state.timeline_exits.get(index).copied().flatten(){Some(0)=>c.ok,Some(_)=>c.err,None=>c.muted_foreground};
            p.shape(r(x+12.0*s,ey+6.0*s,8.0*s,8.0*s),4.0*s,dot,None);
            let time=state.timeline_times.get(index).map(String::as_str).unwrap_or("");let tw=p.width(time,12.0*s,400);
            let mut label_right=x+width-24.0*s-tw;
            if let Some(code)=state.timeline_exits.get(index).copied().flatten(){
                let text=code.to_string();let tone=if code==0{c.ok}else{c.err};
                let bw=(p.width(&text,12.0*s,500)+32.0*s).ceil()+s;let badge=r(label_right-bw,ey,bw,18.0*s);label_right=badge.x-8.0*s;
                exit_badge(&mut p,badge,&text,code==0,tone);
            }
            p.text(r(x+30.0*s,ey,(label_right-x-30.0*s).max(0.0),18.0*s),&event.label,14.0*s,c.foreground,400,false);
            p.text(r(x+30.0*s,ey+18.0*s,width-42.0*s,16.0*s),&event.detail,12.0*s,c.muted_foreground,400,false);
            p.text(r(x+width-12.0*s-tw,ey,tw,18.0*s),time,12.0*s,c.muted_foreground,400,false);
        }y+=height+12.0*s;
        // Sheets card: padding 12, grid gap 8, title 22, rows 32 with the switch centred.
        // Agents reach terminals only; local editor panes have no access row.
        let terminals:Vec<_>=state.panes.iter().filter(|pane|!pane.driver.starts_with("Local editor")).collect();
        let height=(46.0+40.0*terminals.len() as f32)*s;
        p.shape(r(x,y,width,height),10.0*s,c.card,Some(c.border));p.text(r(x+12.0*s,y+12.0*s,width-24.0*s,22.0*s),"Agent access",14.0*s,c.foreground,500,false);
        for (index,pane)in terminals.into_iter().enumerate(){let row=r(x+12.0*s,y+(42.0+40.0*index as f32)*s,width-24.0*s,32.0*s);
            if row.y+row.h>side.y+side.h-8.0*s{break;}
            let over=row.contains(pointer.0,pointer.1);
            if over{p.shape(r(row.x-6.0*s,row.y-4.0*s,row.w+12.0*s,row.h+8.0*s),6.0*s,if pressed{c.pressed}else{c.accent},None);}
            p.pane_label(r(row.x,row.y-1.0*s,row.w-44.0*s,18.0*s),state.pane_labels.get(&pane.id).unwrap_or(&pane.title),14.0*s,c.foreground,400);
            p.text(r(row.x,row.y+17.0*s,row.w-44.0*s,16.0*s),if pane.allow_agent{"read + run"}else{"off"},12.0*s,c.muted_foreground,400,false);
            let sw=r(row.x+row.w-32.0*s,row.y+7.0*s,32.0*s,18.0*s);
            let track=match (pane.allow_agent,over){(true,true)=>blend(c.primary,c.background,0.12),(true,false)=>c.primary,(false,true)=>blend(c.input,c.foreground,0.12),(false,false)=>c.input};
            p.shape(sw,9.0*s,track,None);
            let thumb=r(sw.x+if pane.allow_agent{16.0*s}else{2.0*s},sw.y+2.0*s,14.0*s,14.0*s);
            p.out.scene.layers.push(Layer::Shape(Shape{rect:thumb.array(),radius:7.0*s,fill:Fill::Solid([0;4]),border:None,shadow:Some(clanky_render::Shadow{offset:[0.0,s],blur:2.0*s,spread:0.0,color:[0,0,0,64]})}));
            p.shape(thumb,7.0*s,if pane.allow_agent{c.primary_foreground}else{c.background},None);
            p.hit(row,HitAction::Access(pane.id.clone()));
        }
    }
    p.out
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
fn duration_label(ms:u64)->String{
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
        for cell in &mut grid.cells{terminal(theme,cell);if cell.color_source.is_none(){cell.fg=rgb(c.foreground);cell.bg=rgb(c.background);}}
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
