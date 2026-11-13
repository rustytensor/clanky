//! Command palette: every window command a person can click, findable by typing.
//! Opened from the top-bar field or Ctrl+Shift+P. Built from the component sheets'
//! parts: the search field, 12 px group labels, 32 px items with the accent fill,
//! and muted keyboard chips.
use super::scene::Paint;
use crate::{look::{Layout,PixelRect},themes::{Appearance,Theme}};
use clanky_render::{Fill,Layer,Shadow,Shape,TextMetrics,TextRun};
use winit::keyboard::{Key,NamedKey};

#[derive(Clone,Debug,PartialEq,Eq)]
pub enum Command {
    NewTab, SplitRight, SplitDown, ClosePane, ToggleAgentAccess, ToggleSidebars,
    ZoomIn, ZoomOut, ZoomReset, Settings, StopAgents, Session(String), Run(String),
    OpenFile, LoadChanges,
    /// Open the agent picker (None) or start a configured tool directly.
    StartAgent(Option<usize>),
}

#[derive(Clone,Debug,PartialEq)]
pub struct Entry { pub group:&'static str, pub label:String, pub keys:&'static [&'static str], pub command:Command }

/// What the window knows when the palette lists its commands.
#[derive(Clone,Debug,Default)]
pub struct Context {
    /// Sessions in sidebar order: (first pane id, title, active).
    pub sessions:Vec<(String,String,bool)>,
    pub post_ai:bool,
    /// Label of the focused terminal pane, when a terminal has focus.
    pub terminal:Option<String>,
    /// A file editor has focus (Load changes applies to it).
    pub editor:bool,
    /// Configured agent tools: (name, installed).
    pub agents:Vec<(String,bool)>,
}

pub enum Effect { None, Close, Run(Command) }

const DIALOG_W:f32=560.0;const INPUT_H:f32=48.0;const ROW:f32=32.0;const PAD:f32=6.0;const LIST_ROWS:usize=10;

#[derive(Default)]
pub struct Palette {
    pub open:bool,
    query:String,
    preedit:String,
    selected:usize,
    /// First visible list line; the list scrolls by whole 32 px rows.
    scroll:usize,
    /// Geometry of the last drawn frame, for the mouse: dialog, then (item rect, entry index).
    dialog:Option<PixelRect>,
    rows:Vec<(PixelRect,usize)>,
    list:Option<PixelRect>,
    pointer:(f64,f64),
    caret:Option<[f32;2]>,
}

fn base(context:&Context)->Vec<Entry>{
    let e=|group,label:&str,keys,command|Entry{group,label:label.into(),keys,command};
    let mut out=vec![
        e("Terminal","New tab",&["Ctrl","Shift","T"],Command::NewTab),
        e("Terminal","Split right",&["Ctrl","Shift","D"],Command::SplitRight),
        e("Terminal","Split down",&["Ctrl","Shift","E"],Command::SplitDown),
        e("Terminal","Close pane",&["Ctrl","Shift","W"],Command::ClosePane),
        e("Files","Open file…",&["Ctrl","Shift","O"],Command::OpenFile),
    ];
    if context.editor { out.push(e("Files","Load proposed changes…",&[],Command::LoadChanges)); }
    if !context.post_ai && context.terminal.is_some() {
        out.push(e("Agents","Allow or block agents in this pane",&["Ctrl","Shift","A"],Command::ToggleAgentAccess));
    }
    if !context.post_ai {
        out.push(e("Agents","Start an agent…",&["Ctrl","Shift","G"],Command::StartAgent(None)));
        for (index,(name,found)) in context.agents.iter().enumerate() {
            if *found { out.push(Entry{group:"Agents",label:format!("Start agent {name}"),keys:&[],command:Command::StartAgent(Some(index))}); }
        }
        out.push(e("Agents","Stop every agent now",&["Ctrl","Shift","Esc"],Command::StopAgents));
    }
    out.extend([
        e("View","Show or hide sidebars",&["Ctrl","Shift","B"],Command::ToggleSidebars),
        e("View","Zoom in",&["Ctrl","+"],Command::ZoomIn),
        e("View","Zoom out",&["Ctrl","-"],Command::ZoomOut),
        e("View","Reset zoom",&["Ctrl","0"],Command::ZoomReset),
        e("View","Settings",&["Ctrl",","],Command::Settings),
    ]);
    for (id,title,active) in &context.sessions {
        if !active { out.push(Entry{group:"Sessions",label:format!("Go to {title}"),keys:&[],command:Command::Session(id.clone())}); }
    }
    out
}

/// 0 label starts with the query, 1 a word starts with it, 2 it appears inside,
/// 3 its letters appear in order. None: no match. Case is ignored.
fn rank(label:&str,query:&str)->Option<u8>{
    let label=label.to_lowercase();let query=query.trim().to_lowercase();
    if query.is_empty(){return Some(0);}
    if label.starts_with(&query){return Some(0);}
    if label.split(|c:char|!c.is_alphanumeric()).any(|word|word.starts_with(&query)){return Some(1);}
    if label.contains(&query){return Some(2);}
    let mut letters=label.chars();
    query.chars().filter(|c|!c.is_whitespace()).all(|q|letters.any(|c|c==q)).then_some(3)
}

/// The entries shown for a query, best match first, stable within a rank. A typed
/// query that names no command still offers to run it in the focused terminal.
pub fn entries(context:&Context,query:&str)->Vec<Entry>{
    let mut ranked:Vec<_>=base(context).into_iter().filter_map(|entry|rank(&entry.label,query).map(|r|(r,entry))).collect();
    ranked.sort_by_key(|(r,_)|*r);
    let mut out:Vec<_>=ranked.into_iter().map(|(_,entry)|entry).collect();
    let typed=query.trim();
    if !typed.is_empty() && !typed.chars().any(char::is_control) {
        if let Some(pane)=&context.terminal {
            out.push(Entry{group:"Run",label:format!("Run “{typed}” in {pane}"),keys:&[],command:Command::Run(typed.into())});
        }
    }
    out
}

impl Palette {
    pub fn show(&mut self){self.open=true;self.query.clear();self.preedit.clear();self.selected=0;self.scroll=0;}
    pub fn close(&mut self){self.open=false;self.query.clear();self.preedit.clear();self.dialog=None;self.rows.clear();self.caret=None;}
    pub fn insert(&mut self,text:&str){
        if self.query.len()+text.len()<=256 && !text.chars().any(char::is_control){self.query.push_str(text);self.selected=0;self.scroll=0;}
        self.preedit.clear();
    }
    pub fn preedit(&mut self,text:&str){self.preedit=text.chars().take(64).collect();}
    pub fn ime_position(&self)->Option<[f32;2]>{self.caret}
    pub fn key(&mut self,key:&Key,text:Option<&str>,composing:bool,context:&Context)->Effect{
        let count=entries(context,&self.query).len();
        match key {
            Key::Named(NamedKey::Escape)=>{self.close();return Effect::Close;}
            Key::Named(NamedKey::ArrowDown) if count>0=>self.selected=(self.selected+1)%count,
            Key::Named(NamedKey::ArrowUp) if count>0=>self.selected=(self.selected+count-1)%count,
            Key::Named(NamedKey::Home) if count>0=>self.selected=0,
            Key::Named(NamedKey::End) if count>0=>self.selected=count-1,
            Key::Named(NamedKey::Enter) if !composing=>{
                return match entries(context,&self.query).into_iter().nth(self.selected) {
                    Some(entry)=>{self.close();Effect::Run(entry.command)}
                    None=>Effect::None,
                };
            }
            Key::Named(NamedKey::Backspace) if !composing=>{self.query.pop();self.selected=0;self.scroll=0;}
            _ if !composing=>{if let Some(text)=text{self.insert(text);}}
            _=>{}
        }
        Effect::None
    }
    /// The pointer selects what it rests on, as in the sheets' sidebar items.
    pub fn pointer(&mut self,x:f64,y:f64){
        if (x,y)==self.pointer{return;}
        self.pointer=(x,y);
        if let Some(&(_,index))=self.rows.iter().find(|(rect,_)|rect.contains(x,y)){self.selected=index;}
    }
    pub fn click(&mut self,x:f64,y:f64,context:&Context)->Effect{
        if let Some(&(_,index))=self.rows.iter().find(|(rect,_)|rect.contains(x,y)){
            if let Some(entry)=entries(context,&self.query).into_iter().nth(index){self.close();return Effect::Run(entry.command);}
        }
        if self.dialog.is_some_and(|dialog|!dialog.contains(x,y)){self.close();return Effect::Close;}
        Effect::None
    }
    pub fn wheel(&mut self,lines:i32){self.scroll=self.scroll.saturating_add_signed(-(lines as isize));}

    pub fn draw(&mut self,layout:&Layout,theme:&Theme,context:&Context,pointer:(f64,f64),pressed:bool,measure:&mut dyn FnMut(&TextRun)->TextMetrics)->Vec<Layer>{
        if !self.open{return vec![];}
        let s=layout.scale as f32;let c=&theme.colors;let bar=layout.top_bar;
        let shown=entries(context,&self.query);
        if self.selected>=shown.len(){self.selected=0;}
        let width=(DIALOG_W*s).min((bar.w-32.0*s).max(0.0));
        // Group labels appear once per group, in entry order.
        let mut lines:Vec<(Option<&'static str>,Option<usize>)>=Vec::new();let mut group="";
        for (index,entry) in shown.iter().enumerate(){
            if entry.group!=group{group=entry.group;lines.push((Some(group),None));}
            lines.push((None,Some(index)));
        }
        let room=((layout.window_px[1] as f32-bar.y-bar.h-48.0*s-INPUT_H*s-2.0*PAD*s)/(ROW*s)).floor().max(1.0) as usize;
        let visible=lines.len().min(LIST_ROWS).min(room);
        let list_h=if shown.is_empty(){64.0*s}else{visible as f32*ROW*s};
        let height=INPUT_H*s+1.0*s+list_h+2.0*PAD*s;
        let dialog=PixelRect{x:(bar.x+(bar.w-width)/2.0).round(),y:(bar.y+bar.h+24.0*s).round(),w:width.round(),h:height.round()};
        let mut p=Paint::empty(theme,measure,s,layout.window_px,pointer,pressed);
        // Dark: the Settings dialog's shadow. Light: the sheets' warm shadow tint, lighter.
        let (tint,strength)=match theme.appearance{Appearance::Dark=>([0,0,0],[89,51]),Appearance::Light=>([60,40,20],[26,15])};
        for ((offset,blur),alpha) in [(16.0,48.0),(2.0,6.0)].into_iter().zip(strength){
            p.out.scene.layers.push(Layer::Shape(Shape{rect:dialog.array(),radius:12.0*s,fill:Fill::Solid([0;4]),border:None,
                shadow:Some(Shadow{offset:[0.0,offset*s],blur:blur*s,spread:0.0,color:[tint[0],tint[1],tint[2],alpha]})}));
        }
        p.shape(dialog,12.0*s,c.background,Some(c.border));
        // Input row: the sheets' field without its own frame, the dialog is the frame.
        p.icon("search",dialog.x+16.0*s,dialog.y+16.0*s,c.muted_foreground);
        let text_x=dialog.x+44.0*s;let text_w=(dialog.w-60.0*s).max(0.0);
        let row=PixelRect{x:text_x,y:dialog.y,w:text_w,h:INPUT_H*s};
        let typed=self.query.clone()+&self.preedit;
        if typed.is_empty(){
            p.text(row,"Type a command or search…",14.0*s,c.muted_foreground,400,false);
            self.caret=Some([text_x,dialog.y+16.0*s]);
        }else{
            p.text(row,&typed,14.0*s,c.foreground,400,false);
            let typed_w=p.width(&typed,14.0*s,400).min(text_w);
            self.caret=Some([text_x+typed_w+2.0*s,dialog.y+16.0*s]);
        }
        if let Some([x,y])=self.caret{p.shape(PixelRect{x:x.round(),y,w:1.0*s,h:16.0*s},0.0,c.foreground,None);}
        p.shape(PixelRect{x:dialog.x+1.0*s,y:dialog.y+INPUT_H*s,w:dialog.w-2.0*s,h:1.0*s},0.0,c.border,None);
        let list=PixelRect{x:dialog.x+PAD*s,y:dialog.y+INPUT_H*s+1.0*s+PAD*s,w:dialog.w-2.0*PAD*s,h:list_h};
        self.dialog=Some(dialog);self.list=Some(list);self.rows.clear();
        if shown.is_empty(){
            let message="No matching commands.";let mw=p.width(message,14.0*s,400);
            p.text(PixelRect{x:list.x+(list.w-mw)/2.0,y:list.y,w:mw,h:list.h},message,14.0*s,c.muted_foreground,400,false);
            return p.out.scene.layers;
        }
        // Keep the selected entry in view, and its group label when it is first.
        let line=lines.iter().position(|(_,i)|*i==Some(self.selected)).unwrap_or(0);
        let first=if line>0 && lines[line-1].0.is_some(){line-1}else{line};
        self.scroll=self.scroll.min(lines.len().saturating_sub(visible));
        if first<self.scroll{self.scroll=first;}
        if line>=self.scroll+visible{self.scroll=line+1-visible;}
        for (n,(label,index)) in lines.iter().enumerate().skip(self.scroll).take(visible){
            let rect=PixelRect{x:list.x,y:list.y+(n-self.scroll) as f32*ROW*s,w:list.w,h:ROW*s};
            if let Some(label)=label{
                p.text(PixelRect{x:rect.x+8.0*s,w:rect.w-16.0*s,..rect},label,12.0*s,c.muted_foreground,500,false);
                continue;
            }
            let index=index.unwrap();let entry=&shown[index];
            if index==self.selected{
                let over=rect.contains(pointer.0,pointer.1);
                p.shape(rect,6.0*s,if over && pressed{c.pressed}else{c.accent},None);
            }
            let keys_left=if entry.keys.is_empty(){rect.x+rect.w-8.0*s}else{p.kbd(rect.x+rect.w-8.0*s,rect.y+6.0*s,entry.keys)-8.0*s};
            p.text(PixelRect{x:rect.x+8.0*s,y:rect.y,w:(keys_left-rect.x-8.0*s).max(0.0),h:rect.h},&entry.label,14.0*s,c.foreground,400,false);
            self.rows.push((rect,index));
        }
        p.out.scene.layers
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    fn labels(context:&Context,query:&str)->Vec<String>{entries(context,query).into_iter().map(|e|e.label).collect()}
    #[test]
    fn typing_finds_commands_and_offers_to_run_the_rest(){
        let context=Context{sessions:vec![("a".into(),"clanky".into(),true),("b".into(),"larra".into(),false)],post_ai:false,terminal:Some("clanky · pane 1".into()),editor:false,agents:vec![("Pi".into(),true)]};
        // Worked out by hand from the command list: prefix matches first, then word starts.
        assert_eq!(labels(&context,"spl"),["Split right","Split down","Run “spl” in clanky · pane 1"]);
        assert_eq!(labels(&context,"zoom"),["Zoom in","Zoom out","Reset zoom","Run “zoom” in clanky · pane 1"]);
        assert_eq!(labels(&context,"larra"),["Go to larra","Run “larra” in clanky · pane 1"]);
        assert_eq!(labels(&context,"git log"),["Run “git log” in clanky · pane 1"]);
        // Post-AI hides agent commands; without a focused terminal nothing is run.
        let quiet=Context{post_ai:true,terminal:None,..context};
        assert_eq!(labels(&quiet,"agent"),Vec::<String>::new());
    }
}
