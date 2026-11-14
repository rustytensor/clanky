//! Settings frame measured against the approved HTML, not the legacy modal.
use super::scene::Paint;
use crate::{config::Config, look::PixelRect, themes::{Color, Theme}};
use clanky_render::{Fill,Layer,Matrix4,PathStroke,Scene,Shadow,Shape,TextMetrics,TextRun,VectorLayer,VectorPath};
use clanky_render::vector::lyon::{math::{point,Box2D},path::{Path,Winding,builder::BorderRadii}};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Section { General, Appearance, Terminal, Editor, Agents, History, Keys }
impl Section {
    pub const ALL:[Self;7]=[Self::General,Self::Appearance,Self::Terminal,Self::Editor,Self::Agents,Self::History,Self::Keys];
    pub fn title(self)->&'static str {match self{Self::General=>"General",Self::Appearance=>"Appearance",Self::Terminal=>"Terminal",Self::Editor=>"Editor",Self::Agents=>"Agents",Self::History=>"History",Self::Keys=>"Keys"}}
}
#[derive(Clone,Debug,PartialEq)]
pub enum Action { Section(Section),Close,OpenFile,FontMenu,FontChoice(String),FontSize(f32),Opacity,Toggle(&'static str,bool),Mode(&'static str),Theme(String),Color(&'static str),ResetColor(&'static str),SaveTheme,ImportTheme,ExportTheme,Field(usize),Confirm,Cancel,PickColor(String),Access(String),StopAgents,ResumeAgents,
    /// Terminal page: line height step, cursor blink mode, one terminal colour, back to theme colours, Ptyxis import.
    CellHeight(f32),Blink(&'static str),TerminalColor(&'static str),ThemeTerminalColors,ImportPalette,
    /// Appearance › Layout: one segment (config key, value), a tool chip; Theme: Follow the system, a picker row.
    Choice(&'static str,&'static str),ToolChip(crate::config::Tool),Follow(bool),Pick(usize),
    /// A text field edited in place (its config key); an ANSI colour swatch; the agent list.
    Edit(&'static str),AnsiColor(usize),AddAgent,RemoveAgent(usize) }
#[derive(Clone,Debug)]
pub struct Hit {pub rect:PixelRect,pub action:Action}
pub struct Frame {pub scene:Scene,pub hits:Vec<Hit>,pub geometry:Geometry,pub max_scroll:f32}
#[derive(Clone,Debug)]
pub struct Geometry {pub panel:PixelRect,pub nav:PixelRect,pub header:PixelRect,pub body:PixelRect,pub footer:PixelRect,pub scale:f32,
    /// The panel's height in page px: 600, or less in a window too short for it (the page scrolls).
    pub h:f32}
fn r(x:f32,y:f32,w:f32,h:f32)->PixelRect{PixelRect{x,y,w,h}}
impl Geometry {
    /// Panel-relative rectangle snapped to whole device pixels, so 1 px dividers stay crisp
    /// at every scale (the layout's fractional Chrome offsets would smear them over 3 px at 2x).
    pub fn local(&self,x:f32,y:f32,w:f32,h:f32)->PixelRect{let s=self.scale;
        let (x0,y0)=((self.panel.x+x*s).round(),(self.panel.y+y*s).round());
        let (x1,y1)=((self.panel.x+(x+w)*s).round(),(self.panel.y+(y+h)*s).round());
        r(x0,y0,(x1-x0).max(if w>0.0{1.0}else{0.0}),(y1-y0).max(if h>0.0{1.0}else{0.0}))}
}
pub fn geometry(window:[u32;2],scale:f32)->Geometry{fit(window,scale,0.0)}
/// The panel inside the window below its title bar (`top` px tall), with a 12 px margin: 780 x 600
/// centred when there is room; shorter in a short window (the page scrolls), and scaled down as a
/// whole in a window narrower than the panel, so the header, nav, controls and footer stay in view.
pub fn fit(window:[u32;2],scale:f32,top:f32)->Geometry{
    let (ww,wh)=(window[0] as f32,window[1] as f32);let m=12.0*scale;
    let s=scale*((ww-2.0*m)/(780.0*scale)).clamp(0.6,1.0);
    let room=wh-top-2.0*m;
    let h=(room/s).clamp(300.0,600.0).floor();
    let x=((ww-780.0*s)/2.0).floor();
    let centred=(wh-600.0*s)/2.0;
    let y=if h>=600.0 && centred>=top+m{centred}else{(top+m+((room-h*s)/2.0).max(0.0)).floor()};
    let rect=|a,b,w,h|r(x+a*s,y+b*s,w*s,h*s);
    Geometry{panel:rect(0.0,0.0,780.0,h),nav:rect(1.0,1.0,200.0,h-2.0),header:rect(201.0,1.0,578.0,56.0),body:rect(201.0,57.0,578.0,h-114.0),footer:rect(201.0,h-57.0,578.0,56.0),scale:s,h}
}
pub(super) fn icon(p:&mut Paint<'_>,kind:&str,rect:PixelRect,mut color:Color,s:f32){
    let mut path=Path::builder();let mut fill=None;
    let rounded=|p:&mut clanky_render::vector::lyon::path::Builder,x,y,w,h,radius|p.add_rounded_rectangle(&Box2D::new(point(x,y),point(x+w,y+h)),&BorderRadii{top_left:radius,top_right:radius,bottom_left:radius,bottom_right:radius},Winding::Positive);
    match kind{
        "General"=>{path.add_circle(point(8.0,8.0),2.2,Winding::Positive);for (a,b) in [([8.0,1.8],[8.0,3.4]),([8.0,12.6],[8.0,14.2]),([1.8,8.0],[3.4,8.0]),([12.6,8.0],[14.2,8.0]),([3.6,3.6],[4.7,4.7]),([11.3,11.3],[12.4,12.4]),([3.6,12.4],[4.7,11.3]),([11.3,4.7],[12.4,3.6])]{path.begin(point(a[0],a[1]));path.line_to(point(b[0],b[1]));path.end(false);}},
        "Appearance"=>{path.add_circle(point(8.0,8.0),6.0,Winding::Positive);let mut half=Path::builder();half.begin(point(8.0,2.0));half.cubic_bezier_to(point(4.686292,2.0),point(2.0,4.686292),point(2.0,8.0));half.cubic_bezier_to(point(2.0,11.313708),point(4.686292,14.0),point(8.0,14.0));half.close();fill=Some(half.build());},
        "Terminal"=>{rounded(&mut path,1.8,2.8,12.4,10.4,2.0);path.begin(point(4.5,6.5));path.line_to(point(6.5,8.0));path.line_to(point(4.5,9.5));path.end(false);path.begin(point(8.5,10.0));path.line_to(point(11.5,10.0));path.end(false);},
        "Agents"=>{rounded(&mut path,3.0,5.0,10.0,8.0,2.0);for(a,b)in[([8.0,5.0],[8.0,2.5]),([6.0,9.0],[6.01,9.0]),([10.0,9.0],[10.01,9.0])]{path.begin(point(a[0],a[1]));path.line_to(point(b[0],b[1]));path.end(false);}},
        "Editor"=>{path.begin(point(3.0,13.0));path.line_to(point(3.6,10.4));path.line_to(point(10.8,3.2));path.line_to(point(12.8,5.2));path.line_to(point(5.6,12.4));path.close();path.begin(point(9.4,4.6));path.line_to(point(11.4,6.6));path.end(false);},
        "History"=>{path.add_circle(point(8.0,8.0),6.0,Winding::Positive);path.begin(point(8.0,4.6));path.line_to(point(8.0,8.0));path.line_to(point(10.4,9.4));path.end(false);},
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
    draw_pages(geometry(window,scale),window,section,theme,config,pointer,pressed,measure,&Section::ALL)
}
pub fn draw_live(window:[u32;2],scale:f32,section:Section,theme:&Theme,config:&Config,pointer:(f64,f64),pressed:bool,measure:&mut dyn FnMut(&TextRun)->TextMetrics)->Frame{
    draw_pages(geometry(window,scale),window,section,theme,config,pointer,pressed,measure,&Section::ALL)
}
/// The live panel, fitted below the window's title bar (`top` px).
pub fn draw_fitted(window:[u32;2],scale:f32,top:f32,section:Section,theme:&Theme,config:&Config,pointer:(f64,f64),pressed:bool,measure:&mut dyn FnMut(&TextRun)->TextMetrics)->Frame{
    draw_pages(fit(window,scale,top),window,section,theme,config,pointer,pressed,measure,&Section::ALL)
}
#[allow(clippy::too_many_arguments)]
fn draw_pages(g:Geometry,window:[u32;2],section:Section,theme:&Theme,config:&Config,pointer:(f64,f64),pressed:bool,measure:&mut dyn FnMut(&TextRun)->TextMetrics,ready:&[Section])->Frame{
    let s=g.scale;let h=g.h;let c=&theme.colors;let mut p=Paint::empty(theme,measure,s,window,pointer,pressed);let mut hits=Vec::new();
    // The deep shadow stays within 33 px of the panel, so the window behind it keeps its pixels.
    for (offset,blur,alpha) in [(16.0,32.0,89),(2.0,6.0,51)]{p.out.scene.layers.push(Layer::Shape(Shape{rect:g.panel.array(),radius:12.0*s,fill:Fill::Solid([0;4]),border:None,shadow:Some(Shadow{offset:[0.0,offset*s],blur:blur*s,spread:0.0,color:[0,0,0,alpha]})}));}
    p.shape(g.panel,12.0*s,c.background,Some(c.border));
    p.shape(g.nav,11.0*s,c.sidebar,None);p.shape(g.local(190.0,1.0,11.0,h-2.0),0.0,c.sidebar,None);p.shape(g.local(200.0,1.0,1.0,h-2.0),0.0,c.border,None);
    heading(&mut p,g.local(17.0,21.0,167.0,21.0),"Settings",theme,s);
    for (i,item) in Section::ALL.into_iter().enumerate(){if !ready.contains(&item){continue;}let y=58.0+i as f32*34.0;let rect=g.local(9.0,y,183.0,32.0);let selected=item==section;let over=rect.contains(pointer.0,pointer.1);
        if selected || over{p.shape(rect,6.0*s,if over && pressed{c.pressed}else{c.sidebar_accent},None);}
        let color=if selected || over{c.foreground}else{c.muted_foreground};icon(&mut p,item.title(),g.local(17.0,y+8.0,16.0,16.0),color,s);
        p.text(g.local(41.0,y,143.0,32.0),item.title(),14.0*s,color,if selected{500}else{400},false);hits.push(Hit{rect,action:Action::Section(item)});
    }
    // The page title is centred in the header, as a GNOME header bar's.
    let title_w=p.width(section.title(),16.0*s,600)/s+1.0;heading(&mut p,g.local(490.0-title_w/2.0,1.0,title_w,56.0),section.title(),theme,s);
    p.shape(g.local(201.0,56.0,578.0,1.0),0.0,c.border,None);
    let close=g.local(735.0,13.0,32.0,32.0);hover(&mut p,close,pointer,pressed,theme,s,8.0);icon(&mut p,"close",g.local(743.0,21.0,16.0,16.0),c.foreground,s);hits.push(Hit{rect:close,action:Action::Close});
    let _=config;let max_scroll=0.0;
    p.shape(g.local(201.0,h-57.0,578.0,1.0),0.0,c.border,None);
    let label="Open settings file";let width=p.width(label,13.0*s,500)+20.0*s;let button=r(g.panel.x+767.0*s-width,g.panel.y+(h-43.0)*s,width,28.0*s);
    hover(&mut p,button,pointer,pressed,theme,s,6.0);p.text(r(button.x+10.0*s,button.y,button.w-20.0*s,button.h),label,13.0*s,c.foreground,500,false);hits.push(Hit{rect:button,action:Action::OpenFile});
    p.text(r(g.panel.x+229.0*s,g.footer.y,(button.x-g.panel.x-237.0*s).max(0.0),g.footer.h),"Changes apply and save as you go.",13.0*s,c.muted_foreground,400,false);
    Frame{scene:p.out.scene,hits,geometry:g,max_scroll}
}
/// A text field sits level with its row's label (its text on the label's line), 7 px above centre.
pub(super) fn level(g:&Geometry,control:PixelRect)->PixelRect{PixelRect{y:control.y-7.0*g.scale,..control}}
/// Whether a row starting at page y shows its control whole above the page's bottom edge. A row
/// that does not is left out until it scrolls into view, so a label never shows without its control.
pub(super) fn row_fits(g:&Geometry,y:f32)->bool{fits(g,y,54.0)}
/// Whether `h` page px from page y end above the page's bottom edge (a heading, a card, a line of text).
pub(super) fn fits(g:&Geometry,y:f32,h:f32)->bool{g.panel.y+(y+h)*g.scale<=g.body.y+g.body.h+0.5}
pub(super) fn row(p:&mut Paint<'_>,g:&Geometry,theme:&Theme,y:f32,width:f32,label:&str,description:&str,last:bool)->PixelRect{
    row_noted(p,g,theme,y,width,label,description,None,last)
}
/// As `row`; `problem`, when given, takes the description's place in the error colour.
#[allow(clippy::too_many_arguments)]
pub(super) fn row_noted(p:&mut Paint<'_>,g:&Geometry,theme:&Theme,y:f32,width:f32,label:&str,description:&str,problem:Option<&str>,last:bool)->PixelRect{
    let s=g.scale;let (description,ink)=match problem{Some(text)=>(text,theme.colors.err),None=>(description,theme.colors.muted_foreground)};let height=if last{67.84375}else{68.84375};let text_width=522.0-width-24.0;
    if !row_fits(g,y){return g.local(751.0-width,y+(height-if last{0.0}else{1.0}-32.0)/2.0,width,32.0);}
    p.pane_label(g.local(229.0,y+14.0,text_width,18.0),label,14.0*s,theme.colors.foreground,500);
    p.text(g.local(229.0,y+35.0,text_width,18.84375),description,13.0*s,ink,400,false);
    if !last{p.shape(g.local(229.0,y+height-1.0,522.0,1.0),0.0,theme.colors.border,None);}
    g.local(751.0-width,y+(height-if last{0.0}else{1.0}-32.0)/2.0,width,32.0)
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
