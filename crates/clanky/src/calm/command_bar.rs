//! The command input at the bottom of a terminal pane (mockup sections 1 and 2, CSS .input
//! .chips .chip .line .ghost .t-* .mode .pop): a hairline, a row of chips (folder, git branch,
//! shell, agent access), then the line being written in the terminal font with bash colours,
//! grey ghost text from history and a Shell | Agent switch. While a command runs, on the
//! alternate screen or without shell marks it stays drawn, dimmed, and says where keys go.
use super::scene::{Paint,alpha,blend,term_colors};
use crate::{command_input::Tok,look::{HitAction,Hotspot,Layout,PixelRect},themes::{Color,Theme}};
use clanky_render::{Fill,GridLayer,Layer,Shadow,Shape,TextMetrics,TextRun};

fn r(x:f32,y:f32,w:f32,h:f32)->PixelRect{PixelRect{x,y,w:w.max(0.0),h:h.max(0.0)}}
/// Lines of text the input shows at most; more scroll.
pub const MAX_LINES:usize=8;
/// Chips and the Shell | Agent switch: 32 px tall (Marc's R1 of 2026-11-14: controls at least 32 px).
pub const CHIP_H:f32=32.0;
/// The input's height for `lines` lines of text: border 1, padding 8, chips 32, gap 7, the
/// lines in terminal rows, padding 10 (mockup .input, .chip, at R1's control height).
pub fn height(cell_h:f32,scale:f32,lines:usize)->f32{((1.0+8.0+CHIP_H+7.0+10.0)*scale).round()+cell_h*lines.clamp(1,MAX_LINES) as f32}
/// Where the input's parts sit in a pane.
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct Geometry{pub area:PixelRect,pub chips_y:f32,pub line_y:f32,pub text_x:f32,pub cell:[f32;2],pub cols:usize,pub lines:usize}
pub fn geometry(layout:&Layout,pane:PixelRect,lines:usize)->Geometry{
    let s=layout.scale as f32;let cell=[layout.cell_px[0] as f32,layout.cell_px[1] as f32];
    let lines=lines.clamp(1,MAX_LINES);
    let h=height(cell[1],s,lines).min(pane.h);
    let top=(pane.y+pane.h-h).round();
    let text_x=(pane.x+14.0*s+cell[0]+9.0*s).round();
    let mode_w=130.0*s;
    let cols=((pane.x+pane.w-14.0*s-mode_w-text_x)/cell[0]).floor().max(1.0) as usize;
    Geometry{area:r(pane.x,top,pane.w,pane.y+pane.h-top),chips_y:(top+9.0*s).round(),line_y:(top+(9.0+CHIP_H+7.0)*s).round(),text_x,cell,cols,lines}
}
#[derive(Clone,Debug,Default)]
pub struct Popup{pub items:Vec<(String,&'static str)>,pub selected:usize,
    /// The visible column where the word being completed starts; the list sits under it.
    pub col:usize}
/// One pane's input as the window has it now.
#[derive(Clone,Debug,Default)]
pub struct Bar{
    pub pane:String,
    pub geometry:Option<Geometry>,
    /// The text's lines, each character with its token; the first visible line and column.
    pub lines:Vec<Vec<(char,Tok,bool)>>,
    pub top_line:usize,pub left_col:usize,
    pub ghost:String,
    /// The caret's line and column in the text, when shown.
    pub caret:Option<(usize,usize)>,
    /// The caret belongs to this input (blink aside): the ghost text starts after its block.
    pub caret_here:bool,
    pub folder:String,pub branch:Option<String>,pub shell:String,
    /// Agent access for the pane, when the window may show it.
    pub access:Option<bool>,
    /// Why the input stepped aside (keys go to the program), dimmed.
    pub aside:Option<&'static str>,
    pub notice:Option<String>,
    pub popup:Option<Popup>,
}
/// Layers and hit areas for one pane's input, drawn over the pane's bottom rows.
pub fn draw(layout:&Layout,theme:&Theme,terminal:&crate::config::TerminalColors,font_px:f32,bar:&Bar,pointer:(f64,f64),pressed:bool,
    measure:&mut dyn FnMut(&TextRun)->TextMetrics)->(Vec<Layer>,Vec<Hotspot>){
    let Some(g)=bar.geometry else{return (Vec::new(),Vec::new());};
    let s=layout.scale as f32;let c=theme.colors.clone();let term=term_colors(theme,terminal);
    let mut p=Paint::empty(theme,measure,s,layout.window_px,pointer,pressed);
    let o=if bar.aside.is_some(){0.55}else{1.0};
    let fade=|color:Color|alpha(color,o);
    let faint=crate::calm::scene_faint(&c);
    p.shape(g.area,0.0,term.background,None);
    p.shape(r(g.area.x,g.area.y.round(),g.area.w,super::chrome::hair(s)),0.0,super::chrome::over(term.background,c.border),None);
    // Chips: folder, branch, shell, agent access (mockup .chip, 32 px tall, radius 8, 14 px labels).
    let mut x=g.area.x+14.0*s;let y=g.chips_y;let ch=(CHIP_H*s).round();let px=14.0*s;
    let mut chips:Vec<(String,bool)>=vec![(bar.folder.clone(),true)];
    if let Some(branch)=&bar.branch{chips.push((format!("⎇ {branch}"),true));}
    if !bar.shell.is_empty(){chips.push((bar.shell.clone(),true));}
    let right_edge=g.area.x+g.area.w-14.0*s;
    // A faint raised fill (mockup .chip): the background 6 % of the way to the foreground, so a chip shows at rest
    // on every theme (a theme's card colour can equal its background: Graphite dark #111111 on #111111).
    let chip_fill=blend(term.background,term.foreground,0.06);
    for (text,mono) in &chips{
        let w=(if *mono{p.mono_width(text,px,400)}else{p.width(text,px,400)}+24.0*s).ceil();
        if x+w>right_edge{break;}
        // Its fill only, the border while the pointer is on it.
        let chip=r(x.round(),y,w,ch);
        p.shape(chip,8.0*s,fade(chip_fill),super::chrome::hovered(&p,chip).then(||fade(c.border)));
        p.text(r(x+12.0*s,y,w-12.0*s,ch),text,px,fade(c.muted_foreground),400,*mono);
        x+=w+6.0*s;
    }
    if let (Some(on),None)=(bar.access,bar.aside){
        let label="Agent access";let tw=p.width(label,px,400);let w=(tw+24.0*s+8.0*s+22.0*s).ceil();
        if x+w<=right_edge{
            let chip=r(x.round(),y,w,ch);
            p.shape(chip,8.0*s,chip_fill,super::chrome::hovered(&p,chip).then_some(c.border));
            p.text(r(x+12.0*s,y,tw+1.0,ch),label,px,c.muted_foreground,400,false);
            let sw=r((x+12.0*s+tw+8.0*s).round(),(y+(ch-10.0*s)/2.0).round(),18.0*s,10.0*s);
            p.shape(sw,5.0*s,if on{blend(c.background,c.ok,0.55)}else{c.muted},Some(c.input));
            let knob=6.0*s;let kx=if on{sw.x+sw.w-knob-2.0*s}else{sw.x+2.0*s};
            p.shape(r(kx,sw.y+(sw.h-knob)/2.0,knob,knob),knob/2.0,if on{c.foreground}else{c.muted_foreground},None);
            p.hit(chip,HitAction::Access(bar.pane.clone()));
        }
    }
    // The line: a blue ❯, then the text in the terminal font (or where keys go, dimmed).
    let lh=g.cell[1];
    p.text(r(g.area.x+14.0*s,g.line_y,g.cell[0]+2.0,lh),"❯",font_px,fade(term.ansi[4]),400,true);
    if let Some(why)=bar.aside{
        p.text(r(g.text_x,g.line_y,(right_edge-g.text_x).max(0.0),lh),why,12.0*s,fade(faint),400,false);
    }else{
        let rows=g.lines;let cols=g.cols;
        let bg=[term.background[0],term.background[1],term.background[2]];
        let colour=|t:Tok|->Color{match t{
            Tok::Plain=>term.foreground,Tok::Function=>term.ansi[4],Tok::Str=>term.ansi[2],Tok::Property=>term.ansi[6],
            Tok::Constant|Tok::Keyword=>term.ansi[5],Tok::Number=>term.ansi[1],Tok::Comment=>c.muted_foreground,Tok::Operator=>faint}};
        let selection=blend(term.background,term.ansi[4],0.35);
        let blank=clanky_core::Cell{ch:' ',fg:[term.foreground[0],term.foreground[1],term.foreground[2]],bg,color_source:None,style:0};
        let mut snapshot=clanky_core::TerminalSnapshot{rows,cols,cells:vec![blank;rows*cols],..Default::default()};
        for (row,line) in bar.lines.iter().skip(bar.top_line).take(rows).enumerate(){
            let mut col=0;
            for (i,(ch,tok,selected)) in line.iter().enumerate(){
                if i<bar.left_col{continue;}
                if col>=cols{break;}
                let cell=&mut snapshot.cells[row*cols+col];
                cell.ch=if ch.is_control(){' '}else{*ch};let fg=colour(*tok);cell.fg=[fg[0],fg[1],fg[2]];
                if *selected{cell.bg=[selection[0],selection[1],selection[2]];}
                col+=1;
            }
            // Ghost text follows the last line (it is only offered with the caret at the end),
            // after the caret's block when this input has the caret (mockup .caret then .ghost).
            if !bar.ghost.is_empty() && bar.top_line+row+1==bar.lines.len(){
                if bar.caret_here{col+=1;}
                let ghost=blend(term.background,term.foreground,0.26);
                for ch in bar.ghost.chars(){
                    if col>=cols{break;}
                    let cell=&mut snapshot.cells[row*cols+col];cell.ch=ch;cell.fg=[ghost[0],ghost[1],ghost[2]];col+=1;
                }
            }
        }
        let clip=r(g.text_x,g.line_y,cols as f32*g.cell[0],rows as f32*lh);
        p.out.scene.layers.push(Layer::Grid(GridLayer{origin:[g.text_x,g.line_y],clip:clip.array(),snapshot,underlines:Vec::new(),opacity:1.0}));
        if let Some((line,col))=bar.caret{
            if line>=bar.top_line && line<bar.top_line+rows && col>=bar.left_col && col-bar.left_col<=cols{
                // A filled block in the foreground colour (mockup .caret: 7.5 x 16 px); the character
                // under it, if any, is drawn in the background colour.
                let cx=g.text_x+(col-bar.left_col) as f32*g.cell[0];let cy=g.line_y+(line-bar.top_line) as f32*lh;
                let (cw,chh)=((7.5*s).min(g.cell[0]),(16.0*s).min(lh));
                let block=r(cx.round(),(cy+(lh-chh)/2.0).round(),cw,chh);
                let under=bar.lines.get(line).and_then(|l|l.get(col)).map(|c|c.0).filter(|c|!c.is_control()).unwrap_or(' ');
                let fg=[term.foreground[0],term.foreground[1],term.foreground[2]];
                let cell=clanky_core::Cell{ch:under,fg:bg,bg:fg,color_source:None,style:0};
                let one=clanky_core::TerminalSnapshot{rows:1,cols:1,cells:vec![cell],..Default::default()};
                p.out.scene.layers.push(Layer::Grid(GridLayer{origin:[cx,cy],clip:block.array(),snapshot:one,underlines:Vec::new(),opacity:1.0}));
            }
        }
        if let Some(notice)=&bar.notice{
            let w=p.width(notice,11.0*s,400);
            p.text(r((right_edge-132.0*s-w).max(g.text_x),g.chips_y,w+1.0,ch),notice,11.0*s,c.err,400,false);
        }
    }
    // Shell | Agent (mockup .mode): Agent opens the agent picker; Clanky makes no AI calls.
    if bar.access.is_some(){
        let mpx=14.0*s;let pad=10.0*s;let mh=(CHIP_H*s).round();
        let labels=["Shell","Agent"];let widths:Vec<f32>=labels.iter().map(|l|(p.width(l,mpx,400)+2.0*pad).ceil()).collect();
        let mut mx=(right_edge-widths.iter().sum::<f32>()-2.0*s).round();
        let my=(g.line_y+(lh-mh)/2.0).round();
        for (i,(label,w)) in labels.iter().zip(&widths).enumerate(){
            let span=r(mx,my,*w,mh);
            if i==0{p.shape(span,8.0*s,fade(c.muted),None);}
            p.text(r(span.x+pad,span.y,span.w-pad,span.h),label,mpx,fade(if i==0{c.foreground}else{faint}),400,false);
            if i==1 && bar.aside.is_none(){p.hit(span,HitAction::StartAgent);}
            mx+=w+2.0*s;
        }
    }
    if let (Some(popup),None)=(&bar.popup,bar.aside){completion(&mut p,&g,popup,&c,faint);}
    (p.out.scene.layers,p.out.hits)
}
/// The completion list above the input (mockup .pop): name and kind, the highlighted item,
/// and the footer naming the keys.
fn completion(p:&mut Paint,g:&Geometry,popup:&Popup,c:&crate::themes::Colors,faint:Color){
    let s=p.scale;let px=12.0*s;let row=22.0*s;let shown=popup.items.len().min(8);
    if shown==0{return;}
    let first=popup.selected.saturating_sub(shown-1).min(popup.items.len()-shown);
    let name_w=popup.items.iter().skip(first).take(shown).map(|(n,_)|p.mono_width(n,px,400)).fold(0.0f32,f32::max);
    let foot="Tab completes · Enter runs · Esc closes";let foot_w=p.width(foot,10.5*s,400);
    let w=(name_w+60.0*s).max(foot_w+18.0*s).max(290.0*s).min(g.area.w-20.0*s);
    let h=4.0*s+shown as f32*row+4.0*s+22.0*s+4.0*s;
    // Under the word being completed, item text in line with it; the bottom covers the input's
    // hairline and the top edge of the chip row (mockup .pop bottom 50 over .chip).
    let x=(g.text_x+popup.col as f32*g.cell[0]-13.0*s).min(g.area.x+g.area.w-6.0*s-w).max(g.area.x+6.0*s);
    let y=(g.chips_y+2.0*s-h).max(0.0);
    let rect=r(x,y,w,h);
    p.out.scene.layers.push(Layer::Shape(Shape{rect:rect.array(),radius:9.0*s,fill:Fill::Solid([0;4]),border:None,
        shadow:Some(Shadow{offset:[0.0,12.0*s],blur:32.0*s,spread:0.0,color:[0,0,0,140]})}));
    p.shape(rect,9.0*s,blend(c.background,c.foreground,0.03),Some(c.input));
    for (i,(name,kind)) in popup.items.iter().enumerate().skip(first).take(shown){
        let span=r(x+4.0*s,y+4.0*s+(i-first) as f32*row,w-8.0*s,row);
        let on=i==popup.selected;
        if on{p.shape(span,5.0*s,c.pressed,None);}
        let kw=p.width(kind,10.5*s,400);
        p.text(r(span.x+9.0*s,span.y,span.w-kw-26.0*s,span.h),name,px,if on{c.foreground}else{c.muted_foreground},400,true);
        p.text(r(span.x+span.w-9.0*s-kw,span.y,kw+1.0,span.h),kind,10.5*s,faint,400,false);
    }
    let fy=y+4.0*s+shown as f32*row+3.0*s;
    p.shape(r(x+4.0*s,fy,w-8.0*s,s),0.0,super::chrome::over(blend(c.background,c.foreground,0.03),c.border),None);
    p.text(r(x+13.0*s,fy+2.0*s,w-22.0*s,20.0*s),foot,10.5*s,faint,400,false);
}
