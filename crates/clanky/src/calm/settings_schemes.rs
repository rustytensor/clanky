//! Settings › Terminal › Colour scheme: the heading names the scheme in use; a search field and the
//! list on the left (theme-picker rows: name, four dots of ANSI 1, 2, 4, 3 on a chip of the scheme's
//! background); on the right an Import button and a sample terminal in the highlighted (or saved)
//! scheme's colours, with its 16 ANSI colours, that stays in view while the list scrolls. The
//! highlighted row also previews in the live terminal behind Settings. Imported files are kept next
//! to the config file and listed after "Follow the window theme".
use super::{scene::Paint,settings_frame::{Action,Geometry,Hit}};
use crate::{config::{Config,TerminalColors},look::PixelRect,terminal_schemes::{self,Scheme},themes::{Color,Theme}};
use std::path::PathBuf;

fn r(x:f32,y:f32,w:f32,h:f32)->PixelRect{PixelRect{x,y,w:w.max(0.0),h:h.max(0.0)}}
/// Row pitch and height (the theme picker's). The list takes the left 340 px of the 522 px column;
/// the sample terminal and Import take the right 170 px, 12 px apart.
const PITCH:f32=44.0;const ROW_H:f32=38.0;const LIST_W:f32=340.0;const SIDE_X:f32=581.0;const SIDE_W:f32=170.0;
/// The sample terminal's height.
const CARD_H:f32=166.0;
/// The list keeps room for this many rows, so the page does not jump while the search narrows it.
const MIN_ROWS:usize=4;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Row{Follow,Imported(usize),Scheme(usize)}
#[derive(Default)]
pub struct Picker{pub query:String,pub focused:bool,pub highlight:Option<usize>,pub rows:Vec<(Row,f32)>,
    /// The config file whose folder keeps imported schemes, and those schemes (read once per opening).
    pub config:Option<PathBuf>,imported:Option<Vec<Scheme>>,
    /// The name of the scheme just imported, for the heading's "Added" note until the next change.
    pub added:Option<String>}

impl Picker{
    fn imported(&mut self)->&[Scheme]{
        if self.imported.is_none(){self.imported=Some(self.config.as_deref().map(terminal_schemes::imported).unwrap_or_default());}
        self.imported.as_deref().unwrap_or_default()
    }
    /// Read the imported schemes again (after an import).
    pub fn rescan(&mut self){self.imported=None;}
    /// The rows the search keeps: Follow while the search is empty, then the imported schemes and
    /// the built-in ones whose name holds the text.
    pub fn shown(&mut self)->Vec<Row>{
        let q=fold(self.query.trim());
        let keep=|s:&Scheme|q.is_empty()||fold(&s.name).contains(&q);
        let imported:Vec<Row>=self.imported().iter().enumerate().filter(|(_,s)|keep(s)).map(|(i,_)|Row::Imported(i)).collect();
        let schemes=terminal_schemes::builtin().iter().enumerate().filter(|(_,s)|keep(s)).map(|(i,_)|Row::Scheme(i));
        q.is_empty().then_some(Row::Follow).into_iter().chain(imported).chain(schemes).collect()
    }
    /// The colours a row stands for (Follow: none, the window theme's).
    pub fn colors(&mut self,row:Row)->TerminalColors{match row{Row::Follow=>TerminalColors::default(),
        Row::Imported(i)=>self.imported().get(i).map(|s|s.colors.clone()).unwrap_or_default(),Row::Scheme(i)=>terminal_schemes::builtin()[i].colors.clone()}}
    fn name(&mut self,row:Row)->String{match row{Row::Follow=>"Follow the window theme".into(),
        Row::Imported(i)=>self.imported().get(i).map(|s|s.name.clone()).unwrap_or_default(),Row::Scheme(i)=>terminal_schemes::builtin()[i].name.clone()}}
    /// The shown row the saved colours equal, if any.
    pub fn saved(&mut self,cfg:&Config)->Option<usize>{
        let rows:Vec<Row>=self.rows.iter().map(|(row,_)|*row).collect();
        rows.into_iter().position(|row|self.colors(row)==cfg.terminal_colors)
    }
    /// What the heading says is in use, whatever the search shows.
    fn in_use(&mut self,cfg:&Config)->String{
        let t=&cfg.terminal_colors;
        if *t==TerminalColors::default(){return "In use: the window theme's colours".into();}
        let name=self.imported().iter().chain(terminal_schemes::builtin()).find(|s|s.colors==*t).map(|s|s.name.clone());
        match (name,&self.added){(Some(name),Some(added)) if name==*added=>format!("Added and in use: {name}"),
            (Some(name),_)=>format!("In use: {name}"),(None,_)=>"In use: your own colours".into()}
    }
}
/// Case and accents folded, so "rose" finds Rosé Pine.
fn fold(text:&str)->String{text.to_lowercase().replace('é',"e")}

/// The 20 colours a row stands for, the window theme's where the scheme leaves one unset.
struct Full{bg:Color,fg:Color,cursor:Color,selection:Color,ansi:[Color;16]}
fn full(c:&TerminalColors,t:&Theme)->Full{
    let bg=c.background.unwrap_or(t.colors.background);let fg=c.foreground.unwrap_or(t.colors.foreground);
    Full{bg,fg,cursor:c.cursor.unwrap_or(fg),selection:c.selection.unwrap_or_else(||terminal_schemes::default_selection(bg,fg)),ansi:c.ansi.unwrap_or(t.ansi)}
}

/// One list row (the theme picker's look): name, and a chip of the scheme's background with four
/// dots. The highlighted row is filled and double-ringed; the saved row is double-ringed, or ringed
/// once while another row is highlighted, so it stays findable during a preview.
#[allow(clippy::too_many_arguments)]
fn row(p:&mut Paint<'_>,rect:PixelRect,t:&Theme,name:&str,chip_bg:Color,dots:[Color;4],saved:bool,lit:bool,previewing:bool,s:f32){
    let c=&t.colors;
    p.shape(rect,8.0*s,if lit{c.accent}else{c.card},Some(c.border));
    if lit||(saved&&!previewing){p.shape(rect,8.0*s,[0;4],Some(c.ring));p.shape(r(rect.x+s,rect.y+s,rect.w-2.0*s,rect.h-2.0*s),7.0*s,[0;4],Some(c.ring));}
    else if saved{p.shape(rect,8.0*s,[0;4],Some(c.ring));}
    let chip=r((rect.x+rect.w-12.0*s-68.0*s).round(),(rect.y+(rect.h-21.0*s)/2.0).round(),68.0*s,21.0*s);
    p.text(r(rect.x+12.0*s,rect.y,(chip.x-rect.x-20.0*s).max(0.0),rect.h),name,13.0*s,c.foreground,400,false);
    p.shape(chip,chip.h/2.0,chip_bg,Some(c.border));
    for (i,color) in dots.into_iter().enumerate(){
        let cx=chip.x+7.0*s+(5.5+14.0*i as f32)*s;let cy=chip.y+chip.h/2.0;
        p.shape(r(cx-5.0*s,cy-5.0*s,10.0*s,10.0*s),5.0*s,color,None);
    }
}

/// A small terminal in the scheme's colours: a prompt, git and ls output in six of the ANSI colours,
/// a selected word, the cursor, then all 16 ANSI colours as two rows of swatches.
fn sample(p:&mut Paint<'_>,card:PixelRect,t:&Theme,f:&Full,s:f32){
    p.shape(card,8.0*s,f.bg,Some(t.colors.border));
    let a=&f.ansi;let px=12.0*s;let lh=17.0*s;let x0=card.x+10.0*s;let mut y=card.y+8.0*s;
    let lines:[&[(&str,Color)];5]=[
        &[("$ ",a[2]),("git status",f.fg)],
        &[("modified: main.rs",a[1])],
        &[("new file: notes.md",a[2])],
        &[("$ ",a[2]),("ls",f.fg)],
        &[("src ",a[4]),("docs ",a[6]),("run.sh ",a[2]),("tags",a[5])],
    ];
    let right=card.x+card.w-8.0*s;
    for line in lines{
        let mut x=x0;
        for (text,color) in line{let w=p.mono_width(text,px,400);p.text(r(x,y,(right-x).min(w+1.0),lh),text,px,*color,400,true);x+=w;}
        y+=lh;
    }
    // "$ cat todo" with "todo" selected, then the block cursor.
    let mut x=x0;
    for (text,color,selected) in [("$ ",a[2],false),("cat ",f.fg,false),("todo",f.fg,true)]{
        let w=p.mono_width(text,px,400);
        if selected{p.shape(r(x,y+s,w,lh-2.0*s),0.0,f.selection,None);}
        p.text(r(x,y,w+1.0,lh),text,px,color,400,true);x+=w;
    }
    let cell=p.mono_width("m",px,400);p.shape(r((x+cell).round(),y+s,cell.round(),lh-2.0*s),0.0,f.cursor,None);
    // The 16 ANSI colours: 0-7, then 8-15.
    let sw=((card.w-20.0*s-7.0*2.0*s)/8.0).floor();let top=card.y+card.h-10.0*s-2.0*14.0*s-4.0*s;
    for (i,color) in a.iter().enumerate(){
        let (col,line)=((i%8) as f32,(i/8) as f32);
        p.shape(r(x0+col*(sw+2.0*s),top+line*18.0*s,sw,14.0*s),3.0*s,*color,None);
    }
}

/// Draw the group on `g` (already scrolled) with its heading at page y `head`; returns the page y where it ends.
#[allow(clippy::too_many_arguments)]
pub(super) fn group(p:&mut Paint<'_>,g:&Geometry,t:&Theme,cfg:&Config,picker:&mut Picker,head:f32,hits:&mut Vec<Hit>)->f32{
    let s=g.scale;let c=&t.colors;let faint=crate::calm::scene_faint(c);
    let previewing=picker.highlight.is_some();
    p.text(g.local(229.0,head,260.0,16.0),"Colour scheme",12.0*s,c.muted_foreground,500,false);
    // The scheme in use, at the right of the heading, so it shows whatever the search or preview does.
    let in_use=picker.in_use(cfg);let w=p.width(&in_use,12.0*s,400).min(400.0*s);
    p.text(r(g.local(751.0,head,0.0,0.0).x-w,g.local(0.0,head,0.0,16.0).y,w,16.0*s),&in_use,12.0*s,c.muted_foreground,400,false);
    // Search field: 32 tall, radius 8, filled with the muted colour (as Appearance's segmented
    // controls). While it has the keys it gets one ring and the caret; while Up/Down preview a row,
    // the row has them (filled, two rings) and the field shows neither.
    let field=g.local(229.0,head+20.0,LIST_W,32.0);
    let typing=picker.focused&&!previewing;
    p.shape(field,8.0*s,c.muted,typing.then_some(c.ring));
    let inner=r(field.x+10.0*s,field.y,field.w-20.0*s,field.h);
    // The sheets' input text: 14 px.
    if picker.query.is_empty(){p.text(inner,"Search",14.0*s,c.muted_foreground,400,false);}
    else{p.text(inner,&picker.query,14.0*s,c.foreground,400,false);}
    if typing{
        let x=if picker.query.is_empty(){inner.x}else{inner.x+p.width(&picker.query,14.0*s,400).min(inner.w)+s};
        p.shape(r(x.round(),field.y+8.0*s,s,16.0*s),0.0,c.foreground,None);
    }
    hits.push(Hit{rect:field,action:Action::SchemeSearch});
    // Import, on the field's line in the right column: an outlined button that fills on hover.
    let label="Import…";let b=g.local(SIDE_X,head+20.0,SIDE_W,32.0);
    let fill=if b.contains(p.pointer.0,p.pointer.1){if p.pressed{c.pressed}else{c.accent}}else{c.background};
    p.shape(b,8.0*s,fill,Some(c.input));
    let w=p.width(label,13.0*s,500);p.text(r(b.x+(b.w-w)/2.0,b.y,w,b.h),label,13.0*s,c.foreground,500,false);
    hits.push(Hit{rect:b,action:Action::SchemeImport});
    // The rows.
    let shown=picker.shown();let top=head+64.0;
    picker.rows=shown.iter().enumerate().map(|(i,row)|(*row,top+i as f32*PITCH)).collect();
    let saved=picker.saved(cfg);
    for (i,(kind,y)) in picker.rows.clone().into_iter().enumerate(){
        let rect=g.local(229.0,y,LIST_W,ROW_H);
        let f=full(&picker.colors(kind),t);let name=picker.name(kind);
        row(p,rect,t,&name,f.bg,[f.ansi[1],f.ansi[2],f.ansi[4],f.ansi[3]],saved==Some(i),picker.highlight==Some(i),previewing,s);
        hits.push(Hit{rect,action:Action::SchemeRow(i)});
    }
    if shown.is_empty(){p.text(g.local(231.0,top,LIST_W,ROW_H),"No scheme has that name.",13.0*s,faint,400,false);}
    let end=top+shown.len().max(MIN_ROWS) as f32*PITCH;
    // The sample terminal: the highlighted row's colours, else the saved ones. It stays below the
    // top of the page body while the list scrolls under it, and never past the list's end.
    let colors=match picker.highlight.and_then(|i|picker.rows.get(i)).map(|(row,_)|*row){Some(row)=>picker.colors(row),None=>cfg.terminal_colors.clone()};
    let f=full(&colors,t);
    let list_top=g.local(0.0,top,0.0,0.0).y;let list_end=g.local(0.0,end-(PITCH-ROW_H),0.0,0.0).y;
    let y=(g.body.y+8.0*s).max(list_top).min((list_end-CARD_H*s).max(list_top));
    let card=r(g.local(SIDE_X,0.0,0.0,0.0).x,y.round(),SIDE_W*s,CARD_H*s);
    sample(p,card,t,&f,s);
    let mut y=end+2.0;
    for text in ["Up and Down preview a scheme in the terminal, Enter keeps it,","Escape puts the saved colours back."]{
        p.text(g.local(229.0,y,522.0,18.0),text,12.0*s,faint,400,false);y+=18.0;
    }
    y+8.0
}
