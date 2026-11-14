//! Settings › Terminal › Colour scheme: a search field, "Follow the window theme" and the built-in
//! schemes as theme-picker rows (name, four dots of ANSI 1, 2, 4, 3 on a chip of the scheme's
//! background), and an Import button. The highlighted row previews in the live terminal behind Settings.
use super::{scene::Paint,settings_frame::{Action,Geometry,Hit},settings_layout::theme_row};
use crate::{config::{Config,TerminalColors},look::PixelRect,terminal_schemes::{self,Scheme},themes::{Color,Theme}};

fn r(x:f32,y:f32,w:f32,h:f32)->PixelRect{PixelRect{x,y,w:w.max(0.0),h:h.max(0.0)}}
/// Page y of the group's heading: below the Terminal colours rows, at a whole number of wheel
/// steps (48 px) from the page top plus 20, so a wheel step never leaves the heading half hidden.
pub const TOP:f32=884.0;
/// Row pitch and height (the theme picker's).
const PITCH:f32=44.0;const ROW_H:f32=38.0;const LIST_W:f32=340.0;
/// The list keeps room for this many rows, so the page does not jump while the search narrows it.
const MIN_ROWS:usize=4;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Row{Follow,Scheme(usize)}
#[derive(Default)]
pub struct Picker{pub query:String,pub focused:bool,pub highlight:Option<usize>,pub rows:Vec<(Row,f32)>}

impl Picker{
    /// The rows the search keeps: Follow while the search is empty, then every scheme whose name holds the text.
    pub fn shown(&self)->Vec<Row>{
        let q=self.query.trim().to_lowercase();
        let schemes=terminal_schemes::builtin().iter().enumerate().filter(|(_,s)|q.is_empty()||fold(&s.name).contains(&fold(&q))).map(|(i,_)|Row::Scheme(i));
        (q.is_empty()).then_some(Row::Follow).into_iter().chain(schemes).collect()
    }
    /// The colours a row stands for (Follow: none, the window theme's).
    pub fn colors(row:Row)->TerminalColors{match row{Row::Follow=>TerminalColors::default(),Row::Scheme(i)=>terminal_schemes::builtin()[i].colors.clone()}}
    /// The shown row the saved colours equal, if any.
    pub fn saved(&self,cfg:&Config)->Option<usize>{
        self.rows.iter().position(|(row,_)|match row{Row::Follow=>cfg.terminal_colors==TerminalColors::default(),Row::Scheme(i)=>terminal_schemes::builtin()[*i].colors==cfg.terminal_colors})
    }
}
/// Case and accents folded, so "rose" finds Rosé Pine.
fn fold(text:&str)->String{text.to_lowercase().replace('é',"e")}

fn dots(c:&TerminalColors,t:&Theme)->(Color,[Color;4]){
    let a=c.ansi.unwrap_or(t.ansi);(c.background.unwrap_or(t.colors.background),[a[1],a[2],a[4],a[3]])
}

/// Draw the group on `g` (already scrolled); returns the page y where it ends.
#[allow(clippy::too_many_arguments)]
pub(super) fn group(p:&mut Paint<'_>,g:&Geometry,t:&Theme,cfg:&Config,picker:&mut Picker,hits:&mut Vec<Hit>)->f32{
    let s=g.scale;let c=&t.colors;let faint=crate::calm::scene_faint(c);
    p.text(g.local(229.0,TOP,522.0,16.0),"Colour scheme",12.0*s,c.muted_foreground,500,false);
    // Search field: 32 tall, radius 8, filled with the muted colour (as Appearance's segmented
    // controls), a ring while focused.
    let field=g.local(229.0,TOP+20.0,LIST_W,32.0);
    p.shape(field,8.0*s,c.muted,picker.focused.then_some(c.ring));
    let inner=r(field.x+10.0*s,field.y,field.w-20.0*s,field.h);
    // The sheets' input text: 14 px.
    if picker.query.is_empty(){p.text(inner,"Search",14.0*s,c.muted_foreground,400,false);}
    else{p.text(inner,&picker.query,14.0*s,c.foreground,400,false);}
    if picker.focused{
        let x=if picker.query.is_empty(){inner.x}else{inner.x+p.width(&picker.query,14.0*s,400).min(inner.w)+s};
        p.shape(r(x.round(),field.y+8.0*s,s,16.0*s),0.0,c.foreground,None);
    }
    hits.push(Hit{rect:field,action:Action::SchemeSearch});
    // Import, at the right of the search field: a quiet button that fills on hover.
    let label="Import…";let width=p.width(label,13.0*s,500)/s+20.0;let b=g.local(751.0-width,TOP+22.0,width,28.0);
    if b.contains(p.pointer.0,p.pointer.1){p.shape(b,6.0*s,if p.pressed{c.pressed}else{c.accent},None);}
    let w=p.width(label,13.0*s,500);p.text(r(b.x+(b.w-w)/2.0,b.y,w,b.h),label,13.0*s,c.foreground,500,false);
    hits.push(Hit{rect:b,action:Action::SchemeImport});
    // The rows.
    let shown=picker.shown();let top=TOP+64.0;
    picker.rows=shown.iter().enumerate().map(|(i,row)|(*row,top+i as f32*PITCH)).collect();
    let saved=picker.saved(cfg);
    let list:&[Scheme]=terminal_schemes::builtin();
    for (i,(row,y)) in picker.rows.iter().enumerate(){
        let rect=g.local(229.0,*y,LIST_W,ROW_H);
        let colors=Picker::colors(*row);let (bg,d)=dots(&colors,t);
        let name=match row{Row::Follow=>"Follow the window theme",Row::Scheme(i)=>list[*i].name.as_str()};
        theme_row(p,rect,t,name,bg,d,saved==Some(i)&&picker.highlight.is_none(),picker.highlight==Some(i),s);
        hits.push(Hit{rect,action:Action::SchemeRow(i)});
    }
    if shown.is_empty(){p.text(g.local(231.0,top,LIST_W,ROW_H),"No scheme has that name.",13.0*s,faint,400,false);}
    let end=top+shown.len().max(MIN_ROWS) as f32*PITCH;
    let mut y=end+2.0;
    for text in ["Up and Down preview a scheme in the terminal, Enter keeps it,","Escape puts the saved colours back."]{
        p.text(g.local(229.0,y,522.0,18.0),text,12.0*s,faint,400,false);y+=18.0;
    }
    y+8.0
}
