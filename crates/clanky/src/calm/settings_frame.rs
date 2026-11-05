//! Settings frame measured against the approved HTML, not the legacy modal.
use super::scene::Paint;
use crate::{config::Config, look::PixelRect, themes::{Color, Theme}};
use clanky_render::{Fill,Layer,Matrix4,PathStroke,Scene,Shadow,Shape,TextMetrics,TextRun,VectorLayer,VectorPath};
use clanky_render::vector::lyon::{math::{point,Box2D},path::{Path,Winding,builder::BorderRadii}};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Section { General, Appearance, Terminal, Agents, Keys }
impl Section {
    pub const ALL:[Self;5]=[Self::General,Self::Appearance,Self::Terminal,Self::Agents,Self::Keys];
    pub fn title(self)->&'static str {match self{Self::General=>"General",Self::Appearance=>"Appearance",Self::Terminal=>"Terminal",Self::Agents=>"Agents",Self::Keys=>"Keys"}}
}
#[derive(Clone,Debug,PartialEq)]
pub enum Action { Section(Section),Close,OpenFile,FontMenu,FontChoice(String),FontSize(f32),Opacity,Toggle(&'static str,bool),Mode(&'static str),Theme(String),Color(&'static str),ResetColor(&'static str),SaveTheme,ImportTheme,ExportTheme,Field(usize),Confirm,Cancel,PickColor(String),Access(String),StopAgents,ResumeAgents }
#[derive(Clone,Debug)]
pub struct Hit {pub rect:PixelRect,pub action:Action}
pub struct Frame {pub scene:Scene,pub hits:Vec<Hit>,pub geometry:Geometry}
#[derive(Clone,Debug)]
pub struct Geometry {pub panel:PixelRect,pub nav:PixelRect,pub header:PixelRect,pub body:PixelRect,pub footer:PixelRect,pub scale:f32}
fn r(x:f32,y:f32,w:f32,h:f32)->PixelRect{PixelRect{x,y,w,h}}
impl Geometry {
    pub fn local(&self,x:f32,y:f32,w:f32,h:f32)->PixelRect{let s=self.scale;r(self.panel.x+x*s,self.panel.y+y*s,w*s,h*s)}
}
pub fn geometry(window:[u32;2],scale:f32)->Geometry{
    let x=(window[0] as f32-780.0*scale)/2.0;let y=(window[1] as f32-600.0*scale)/2.0;
    let rect=|a,b,w,h|r(x+a*scale,y+b*scale,w*scale,h*scale);
    Geometry{panel:rect(0.0,0.0,780.0,600.0),nav:rect(1.0,1.0,200.0,598.0),header:rect(201.0,1.0,578.0,56.0),body:rect(201.0,57.0,578.0,486.0),footer:rect(201.0,543.0,578.0,56.0),scale}
}
pub(super) fn icon(p:&mut Paint<'_>,kind:&str,rect:PixelRect,mut color:Color,s:f32){
    let mut path=Path::builder();let mut fill=None;
    let rounded=|p:&mut clanky_render::vector::lyon::path::Builder,x,y,w,h,radius|p.add_rounded_rectangle(&Box2D::new(point(x,y),point(x+w,y+h)),&BorderRadii{top_left:radius,top_right:radius,bottom_left:radius,bottom_right:radius},Winding::Positive);
    match kind{
        "General"=>{path.add_circle(point(8.0,8.0),2.2,Winding::Positive);for (a,b) in [([8.0,1.8],[8.0,3.4]),([8.0,12.6],[8.0,14.2]),([1.8,8.0],[3.4,8.0]),([12.6,8.0],[14.2,8.0]),([3.6,3.6],[4.7,4.7]),([11.3,11.3],[12.4,12.4]),([3.6,12.4],[4.7,11.3]),([11.3,4.7],[12.4,3.6])]{path.begin(point(a[0],a[1]));path.line_to(point(b[0],b[1]));path.end(false);}},
        "Appearance"=>{path.add_circle(point(8.0,8.0),6.0,Winding::Positive);let mut half=Path::builder();half.begin(point(8.0,2.0));half.cubic_bezier_to(point(4.686292,2.0),point(2.0,4.686292),point(2.0,8.0));half.cubic_bezier_to(point(2.0,11.313708),point(4.686292,14.0),point(8.0,14.0));half.close();fill=Some(half.build());},
        "Terminal"=>{rounded(&mut path,1.8,2.8,12.4,10.4,2.0);path.begin(point(4.5,6.5));path.line_to(point(6.5,8.0));path.line_to(point(4.5,9.5));path.end(false);path.begin(point(8.5,10.0));path.line_to(point(11.5,10.0));path.end(false);},
        "Agents"=>{rounded(&mut path,3.0,5.0,10.0,8.0,2.0);for(a,b)in[([8.0,5.0],[8.0,2.5]),([6.0,9.0],[6.01,9.0]),([10.0,9.0],[10.01,9.0])]{path.begin(point(a[0],a[1]));path.line_to(point(b[0],b[1]));path.end(false);}},
        "Keys"=>{rounded(&mut path,1.8,4.0,12.4,8.0,1.6);for x in[4.5,7.0,9.5,12.0]{path.begin(point(x,7.0));path.line_to(point(x+0.01,7.0));path.end(false);}path.begin(point(5.0,9.5));path.line_to(point(11.0,9.5));path.end(false);},
        "check"=>{path.begin(point(3.5,8.5));path.line_to(point(6.5,11.5));path.line_to(point(12.5,4.5));path.end(false);},
        "close"=>{path.begin(point(4.0,4.0));path.line_to(point(12.0,12.0));path.end(false);path.begin(point(12.0,4.0));path.line_to(point(4.0,12.0));path.end(false);},
        _=>{path.begin(point(4.5,6.5));path.line_to(point(8.0,10.0));path.line_to(point(11.5,6.5));path.end(false);}
    }
    if Section::ALL.iter().any(|section|section.title()==kind){color[3]=(color[3] as f32*0.8).round() as u8;}
    let transform=Matrix4::scale(s,s,1.0).then(Matrix4::translate(rect.x,rect.y,0.0));
    let stroke=PathStroke{width:if kind=="check"{2.0}else if kind=="close" || kind=="chevron"{1.6}else{1.4},color};
    let mut paths=vec![VectorPath{path:path.build(),fill:None,stroke:Some(stroke.clone()),transform}];
    if let Some(path)=fill{paths.push(VectorPath{path,fill:Some(color),stroke:Some(stroke),transform});}
    p.out.scene.layers.push(Layer::Vector(VectorLayer{paths,clip:rect.array(),opacity:1.0}));
}
fn heading(p:&mut Paint<'_>,rect:PixelRect,label:&str,theme:&Theme,s:f32){
    p.text(rect,label,16.0*s,theme.colors.foreground,600,false);
    if let Some(Layer::Text(run))=p.out.scene.layers.last_mut(){run.letter_spacing=-0.16*s;}
}
fn hover(p:&mut Paint<'_>,rect:PixelRect,pointer:(f64,f64),pressed:bool,theme:&Theme,s:f32,radius:f32){
    if rect.contains(pointer.0,pointer.1){p.shape(rect,radius*s,if pressed{theme.colors.pressed}else{theme.colors.accent},None);}
}
/// Frame and Terminal controls. Other pages are supplied by the Settings controller;
/// the live entry remains hidden until that controller is complete.
pub fn draw(window:[u32;2],scale:f32,section:Section,theme:&Theme,config:&Config,pointer:(f64,f64),pressed:bool,measure:&mut dyn FnMut(&TextRun)->TextMetrics)->Frame{
    draw_pages(window,scale,section,theme,config,pointer,pressed,measure,&Section::ALL)
}
pub fn draw_live(window:[u32;2],scale:f32,section:Section,theme:&Theme,config:&Config,pointer:(f64,f64),pressed:bool,measure:&mut dyn FnMut(&TextRun)->TextMetrics)->Frame{
    draw_pages(window,scale,section,theme,config,pointer,pressed,measure,&Section::ALL)
}
fn draw_pages(window:[u32;2],scale:f32,section:Section,theme:&Theme,config:&Config,pointer:(f64,f64),pressed:bool,measure:&mut dyn FnMut(&TextRun)->TextMetrics,ready:&[Section])->Frame{
    let g=geometry(window,scale);let s=scale;let c=&theme.colors;let mut p=Paint::empty(theme,measure,s,window,pointer,pressed);let mut hits=Vec::new();
    for (offset,blur,alpha) in [(16.0,48.0,89),(2.0,6.0,51)]{p.out.scene.layers.push(Layer::Shape(Shape{rect:g.panel.array(),radius:12.0*s,fill:Fill::Solid([0;4]),border:None,shadow:Some(Shadow{offset:[0.0,offset*s],blur:blur*s,spread:0.0,color:[0,0,0,alpha]})}));}
    p.shape(g.panel,12.0*s,c.background,Some(c.border));
    p.shape(g.nav,11.0*s,c.sidebar,None);p.shape(g.local(190.0,1.0,11.0,598.0),0.0,c.sidebar,None);p.shape(g.local(200.0,1.0,1.0,598.0),0.0,c.border,None);
    heading(&mut p,g.local(17.0,21.0,167.0,21.0),"Settings",theme,s);
    for (i,item) in Section::ALL.into_iter().enumerate(){if !ready.contains(&item) || (config.post_ai && item==Section::Agents){continue;}let y=58.0+i as f32*34.0;let rect=g.local(9.0,y,183.0,32.0);let selected=item==section;let over=rect.contains(pointer.0,pointer.1);
        if selected || over{p.shape(rect,6.0*s,if over && pressed{c.pressed}else{c.sidebar_accent},None);}
        let color=if selected || over{c.foreground}else{c.muted_foreground};icon(&mut p,item.title(),g.local(17.0,y+8.0,16.0,16.0),color,s);
        p.text(g.local(41.0,y,143.0,32.0),item.title(),14.0*s,color,if selected{500}else{400},false);hits.push(Hit{rect,action:Action::Section(item)});
    }
    heading(&mut p,g.local(229.0,1.0,490.0,56.0),section.title(),theme,s);
    p.shape(g.local(201.0,56.0,578.0,1.0),0.0,c.border,None);
    let close=g.local(735.0,13.0,32.0,32.0);hover(&mut p,close,pointer,pressed,theme,s,8.0);icon(&mut p,"close",g.local(743.0,21.0,16.0,16.0),c.foreground,s);hits.push(Hit{rect:close,action:Action::Close});
    if section==Section::General{
        p.text(g.local(229.0,79.0,466.0,18.0),"Post-AI mode",14.0*s,c.foreground,500,false);
        for (i,line) in ["A plain terminal with tabs and splits. No agents, no AI features.","For coding and AI purists."].into_iter().enumerate(){p.text(g.local(229.0,100.0+i as f32*18.85,466.0,18.85),line,13.0*s,c.muted_foreground,400,false);}
        let sw=g.local(719.0,108.0,32.0,18.0);
        p.shape(sw,9.0*s,if config.post_ai{c.primary}else{c.input},None);
        p.shape(r(sw.x+if config.post_ai{16.0*s}else{2.0*s},sw.y+2.0*s,14.0*s,14.0*s),7.0*s,if config.post_ai{c.primary_foreground}else{c.background},None);
        hits.push(Hit{rect:g.local(229.0,65.0,522.0,104.0),action:Action::Toggle("post_ai",!config.post_ai)});
    }
    if section==Section::Terminal{terminal(&mut p,&g,theme,config,pointer,pressed,&mut hits);}
    p.shape(g.local(201.0,543.0,578.0,1.0),0.0,c.border,None);
    let label="Open settings file";let width=p.width(label,13.0*s,500)+20.0*s;let button=r(g.panel.x+767.0*s-width,g.panel.y+557.0*s,width,28.0*s);
    hover(&mut p,button,pointer,pressed,theme,s,6.0);p.text(r(button.x+10.0*s,button.y,button.w-20.0*s,button.h),label,13.0*s,c.foreground,500,false);hits.push(Hit{rect:button,action:Action::OpenFile});
    p.text(r(g.panel.x+229.0*s,g.footer.y,(button.x-g.panel.x-237.0*s).max(0.0),g.footer.h),"Changes apply and save as you go.",13.0*s,c.muted_foreground,400,false);
    Frame{scene:p.out.scene,hits,geometry:g}
}
pub(super) fn row(p:&mut Paint<'_>,g:&Geometry,theme:&Theme,y:f32,width:f32,label:&str,description:&str,last:bool)->PixelRect{
    let s=g.scale;let height=if last{67.84375}else{68.84375};let text_width=522.0-width-24.0;
    p.pane_label(g.local(229.0,y+14.0,text_width,18.0),label,14.0*s,theme.colors.foreground,500);
    p.text(g.local(229.0,y+35.0,text_width,18.84375),description,13.0*s,theme.colors.muted_foreground,400,false);
    if !last{p.shape(g.local(229.0,y+height-1.0,522.0,1.0),0.0,theme.colors.border,None);}
    g.local(751.0-width,y+(height-if last{0.0}else{1.0}-32.0)/2.0,width,32.0)
}
fn terminal(p:&mut Paint<'_>,g:&Geometry,theme:&Theme,config:&Config,pointer:(f64,f64),pressed:bool,hits:&mut Vec<Hit>){
    let s=g.scale;let c=&theme.colors;
    for(label,y)in[("Text",85.0),("Window",262.6875)]{p.text(g.local(229.0,y,522.0,16.0),label,12.0*s,c.muted_foreground,500,false);}
    let font=row(p,g,theme,105.0,180.0,"Font","Used in every terminal pane.",false);
    p.shape(font,8.0*s,c.background,Some(c.input));p.text(r(font.x+10.0*s,font.y,font.w-40.0*s,font.h),&config.font_family,14.0*s,c.foreground,400,false);let mut arrow=c.foreground;arrow[3]=153;icon(p,"chevron",r(font.x+font.w-23.0*s,font.y+9.0*s,14.0*s,14.0*s),arrow,s*0.875);hits.push(Hit{rect:font,action:Action::FontMenu});
    let step=row(p,g,theme,173.84375,110.0,"Size","Ctrl + and Ctrl − change it for one window.",false);p.shape(step,8.0*s,c.background,Some(c.input));
    for (x,label,delta) in [(step.x+s,"−",-1.0),(step.x+77.0*s,"+",1.0)]{let rect=r(x,step.y+s,32.0*s,30.0*s);hover(p,rect,pointer,pressed,theme,s,6.0);let w=p.width(label,14.0*s,400);p.text(r(x+(32.0*s-w)/2.0,rect.y,w,rect.h),label,14.0*s,c.muted_foreground,400,false);hits.push(Hit{rect,action:Action::FontSize(delta)});}
    for x in[step.x+33.0*s,step.x+76.0*s]{p.shape(r(x,step.y+s,s,30.0*s),0.0,c.border,None);}
    let label=format!("{}",config.font_size);let width=p.mono_width(&label,14.0*s,400).min(42.0*s);p.text(r(step.x+34.0*s+(42.0*s-width)/2.0,step.y,width,32.0*s),&label,14.0*s,c.foreground,400,true);
    let slider=row(p,g,theme,282.6875,220.0,"Opacity","How much of the desktop shows through.",false);let y=slider.y+16.0*s;let progress=((config.opacity-0.5)/0.5).clamp(0.0,1.0);let track=r(slider.x,y-2.0*s,168.0*s,4.0*s);
    p.shape(track,2.0*s,c.muted,None);p.shape(r(track.x,track.y,track.w*progress,track.h),2.0*s,c.foreground,None);p.shape(r(track.x+track.w*progress-8.0*s,y-8.0*s,16.0*s,16.0*s),8.0*s,c.background,Some(c.input));
    let label=format!("{:.0}%",config.opacity*100.0);let width=p.mono_width(&label,13.0*s,400).min(40.0*s);p.text(r(slider.x+220.0*s-width,slider.y,width,slider.h),&label,13.0*s,c.muted_foreground,400,true);hits.push(Hit{rect:r(track.x-8.0*s,slider.y,track.w+16.0*s,slider.h),action:Action::Opacity});
    for (y,label,description,key,on,last) in [(351.53125,"Side columns","The sidebar and the agent column beside the panes.","rails",config.rails,false),(420.375,"Animations","Includes the robot at start-up.","animations",config.animations,true)]{
        let control=row(p,g,theme,y,32.0,label,description,last);let switch=r(control.x,control.y+7.0*s,32.0*s,18.0*s);p.shape(switch,9.0*s,if on{c.primary}else{c.input},None);p.shape(r(switch.x+if on{16.0*s}else{2.0*s},switch.y+2.0*s,14.0*s,14.0*s),7.0*s,if on{c.primary_foreground}else{c.background},None);hits.push(Hit{rect:g.local(229.0,y,522.0,if last{67.84375}else{68.84375}),action:Action::Toggle(key,!on)});
    }
}
#[cfg(test)]
#[test]
fn settings_frame_geometry(){
    // Independent Chrome bounds from the approved 780×600 HTML panel.
    let mut g=geometry([1240,760],1.0);
    if std::env::var_os("CLANKY_SETTINGS_FRAME_FAULT").is_some(){g.panel.w+=1.0;}
    assert_eq!(g.panel.array(),[230.0,80.0,780.0,600.0]);
    assert_eq!(g.nav.array(),[231.0,81.0,200.0,598.0]);
    assert_eq!(g.header.array(),[431.0,81.0,578.0,56.0]);
    assert_eq!(g.body.array(),[431.0,137.0,578.0,486.0]);
    assert_eq!(g.footer.array(),[431.0,623.0,578.0,56.0]);
}
