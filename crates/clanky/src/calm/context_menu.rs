//! The right-click menu over a pane, as in the mockup approved on 2026-11-13: Ptyxis's
//! rows (links, copy and paste, find, reset) plus Clanky's own (splits, the agent, agent
//! access, close pane). It opens at the pointer, flipped up or left to stay in the window.
//! Keys: Down and Up move the highlight (none at first; greyed rows are skipped), Enter
//! runs it, Escape closes the menu.
use super::scene::Paint;
use crate::{look::{Layout,PixelRect},themes::Theme};
use clanky_render::{Fill,Layer,Shadow,Shape,TextMetrics,TextRun};
use winit::keyboard::{Key,NamedKey};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Item { OpenLink, CopyLink, Cut, Copy, Paste, SelectAll, CopyCommand, CopyOutput, RunAgain, Find,
    SplitRight, SplitBelow, StartAgent, AgentAccess, Reset, ResetClear, ClosePane }

/// What is under the pointer when the menu opens.
#[derive(Clone,Debug,Default)]
pub struct Context {
    pub editor:bool,
    pub link:Option<String>,
    /// A finished command block, by id.
    pub block:Option<String>,
    pub selection:bool,
    pub post_ai:bool,
    pub access:bool,
}

#[derive(Clone,Debug)]
struct Row { item:Item, label:&'static str, keys:&'static str, enabled:bool }

#[derive(Debug,PartialEq,Eq)]
pub enum Effect { None, Close, Run(Item) }

const WIDTH:f32=252.0;const ROW:f32=26.0;const PAD:f32=4.0;const RULE:f32=9.0;const HEAD:f32=21.0;
/// The shadow reaches this far past the card: left/right, up, down.
const SHADOW_SIDE:f32=6.0;const SHADOW_UP:f32=2.0;const SHADOW_DOWN:f32=10.0;

#[derive(Default)]
pub struct ContextMenu {
    pub open:bool,
    /// The pane it was opened on, and what it was opened over.
    pub pane:String,
    pub context:Context,
    groups:Vec<(Option<&'static str>,Vec<Row>)>,
    selected:Option<usize>,
    at:(f32,f32),
    opened:Option<std::time::Instant>,
    card:Option<PixelRect>,
    rows:Vec<(PixelRect,usize)>,
}

fn row(item:Item,label:&'static str,keys:&'static str,enabled:bool)->Row{Row{item,label,keys,enabled}}

impl ContextMenu {
    pub fn show(&mut self,pane:String,context:Context,at:(f64,f64)){
        let c=&context;let mut groups=Vec::new();
        if c.editor{
            groups.push((None,vec![row(Item::Cut,"Cut","Ctrl+X",c.selection),row(Item::Copy,"Copy","Ctrl+C",c.selection),
                row(Item::Paste,"Paste","Ctrl+V",true),row(Item::SelectAll,"Select all","Ctrl+A",true)]));
            groups.push((None,vec![row(Item::Find,"Find…","Ctrl+F",true),row(Item::SplitRight,"Split right","Ctrl+Shift+D",true),
                row(Item::SplitBelow,"Split below","Ctrl+Shift+E",true)]));
        }else{
            if c.link.is_some(){groups.push((None,vec![row(Item::OpenLink,"Open link","Ctrl+click",true),row(Item::CopyLink,"Copy link","",true)]));}
            groups.push((None,vec![row(Item::Copy,"Copy","Ctrl+Shift+C",c.selection),row(Item::Paste,"Paste","Ctrl+Shift+V",true),
                row(Item::SelectAll,"Select all","",true)]));
            if c.block.is_some(){groups.push((Some("This command"),vec![row(Item::CopyCommand,"Copy command","",true),
                row(Item::CopyOutput,"Copy output","",true),row(Item::RunAgain,"Run again","",true)]));}
            let mut tools=vec![row(Item::Find,"Find…","Ctrl+Shift+F",true),row(Item::SplitRight,"Split right","Ctrl+Shift+D",true),
                row(Item::SplitBelow,"Split below","Ctrl+Shift+E",true)];
            if !c.post_ai{tools.push(row(Item::StartAgent,"Start an agent here…","Ctrl+Shift+G",true));tools.push(row(Item::AgentAccess,"Agent access","",true));}
            groups.push((None,tools));
            groups.push((None,vec![row(Item::Reset,"Reset terminal","",true),row(Item::ResetClear,"Reset and clear","",true)]));
        }
        groups.push((None,vec![row(Item::ClosePane,"Close pane","Ctrl+Shift+W",true)]));
        *self=Self{open:true,pane,context,groups,selected:None,at:(at.0 as f32,at.1 as f32),opened:Some(std::time::Instant::now()),card:None,rows:Vec::new()};
    }
    pub fn close(&mut self){*self=Self::default();}
    fn flat(&self)->Vec<&Row>{self.groups.iter().flat_map(|(_,rows)|rows).collect()}
    pub fn items(&self)->Vec<(Item,bool)>{self.flat().into_iter().map(|r|(r.item,r.enabled)).collect()}
    fn step(&mut self,forward:bool){
        let rows=self.flat();let n=rows.len();
        let start=match(self.selected,forward){(Some(i),_)=>i,(None,true)=>n-1,(None,false)=>0};
        for k in 1..=n{
            let i=if forward{(start+k)%n}else{(start+n-k%n)%n};
            if rows[i].enabled{self.selected=Some(i);return;}
        }
    }
    fn run(&mut self,index:usize)->Effect{
        match self.flat().get(index){Some(r) if r.enabled=>{let item=r.item;Effect::Run(item)}_=>Effect::None}
    }
    pub fn key(&mut self,key:&Key)->Effect{
        match key{
            Key::Named(NamedKey::Escape)=>Effect::Close,
            Key::Named(NamedKey::ArrowDown)=>{self.step(true);Effect::None}
            Key::Named(NamedKey::ArrowUp)=>{self.step(false);Effect::None}
            Key::Named(NamedKey::Enter)=>self.selected.map_or(Effect::None,|i|self.run(i)),
            _=>Effect::None,
        }
    }
    /// A left click: a row runs, the rest of the card does nothing, outside closes.
    pub fn click(&mut self,x:f64,y:f64)->Effect{
        if let Some(&(_,index))=self.rows.iter().find(|(rect,_)|rect.contains(x,y)){return self.run(index);}
        if self.card.is_some_and(|card|card.contains(x,y)){Effect::None}else{Effect::Close}
    }
    /// The highlight follows the pointer over rows that can act.
    pub fn pointer(&mut self,x:f64,y:f64){
        let enabled=self.flat().iter().map(|r|r.enabled).collect::<Vec<_>>();
        self.selected=self.rows.iter().find(|(rect,index)|rect.contains(x,y) && enabled[*index]).map(|(_,index)|*index);
    }
    /// When the fade-in ends, for the frame clock.
    pub fn animation_deadline(&self,animations:bool)->Option<std::time::Instant>{
        self.opened.filter(|_|animations && self.open).map(|t|t+std::time::Duration::from_millis(120))
    }
    fn size(&self,s:f32)->(f32,f32){
        let rows:f32=self.groups.iter().map(|(head,rows)|rows.len() as f32*ROW+if head.is_some(){HEAD}else{0.0}).sum();
        (WIDTH*s,(rows+RULE*(self.groups.len().saturating_sub(1)) as f32+2.0*PAD)*s)
    }
    /// The card's place: its top-left just past the pointer, or flipped so that it and its
    /// shadow end just before the pointer when the window has no room right or below.
    fn place(&self,window:[u32;2],s:f32)->PixelRect{
        let (w,h)=self.size(s);let (x,y)=self.at;let (ww,wh)=(window[0] as f32,window[1] as f32);
        let mut left=x+(SHADOW_SIDE-3.0)*s;let mut top=y+s;
        if left+w+SHADOW_SIDE*s>ww{left=x-w-(SHADOW_SIDE+1.0)*s;}
        if top+h+SHADOW_DOWN*s>wh{top=y-h-(SHADOW_DOWN+1.0)*s;}
        PixelRect{x:left.max(0.0).round(),y:top.max(0.0).round(),w:w.round(),h:h.round()}
    }
    pub fn draw(&mut self,layout:&Layout,theme:&Theme,animations:bool,measure:&mut dyn FnMut(&TextRun)->TextMetrics)->Vec<Layer>{
        if !self.open{return vec![];}
        let s=layout.scale as f32;let c=&theme.colors;
        let fade=self.opened.map_or(1.0,|t|crate::look::ease_out_cubic(0.0,1.0,t.elapsed().as_secs_f32()*1000.0,120.0,animations));
        let a=|mut color:[u8;4]|{color[3]=(color[3] as f32*fade).round() as u8;color};
        let card=self.place(layout.window_px,s);
        let mut p=Paint::empty(theme,measure,s,layout.window_px,(-1.0,-1.0),false);
        p.out.scene.layers.push(Layer::Shape(Shape{rect:card.array(),radius:8.0*s,fill:Fill::Solid([0;4]),border:None,
            shadow:Some(Shadow{offset:[0.0,4.0*s],blur:6.0*s,spread:0.0,color:a([0,0,0,120])})}));
        p.shape(card,8.0*s,a(c.card),Some(a(c.input)));
        let dim=|t:f32,color:[u8;4]|->[u8;4]{std::array::from_fn(|i|if i==3{color[3]}else{(color[i] as f32*(1.0-t)+c.card[i] as f32*t).round() as u8})};
        self.card=Some(card);self.rows.clear();
        let mut y=card.y+PAD*s;let mut index=0;
        let groups=std::mem::take(&mut self.groups);
        for (g,(head,rows)) in groups.iter().enumerate(){
            if g>0{p.shape(PixelRect{x:card.x+6.0*s,y:y+4.0*s,w:card.w-12.0*s,h:s},0.0,a(c.border),None);y+=RULE*s;}
            if let Some(head)=head{
                p.text(PixelRect{x:card.x+(PAD+8.0)*s,y:y+4.0*s,w:card.w-2.0*(PAD+8.0)*s,h:HEAD*s-4.0*s},head,11.0*s,a(c.muted_foreground),500,false);
                y+=HEAD*s;
            }
            for r in rows{
                let rect=PixelRect{x:card.x+PAD*s,y,w:card.w-2.0*PAD*s,h:ROW*s};
                if self.selected==Some(index){p.shape(rect,5.0*s,a(c.pressed),None);}
                let ink=if !r.enabled{dim(0.6,c.foreground)}else if r.item==Item::StartAgent{c.agent}else{c.foreground};
                let mut right=rect.x+rect.w-8.0*s;
                if r.item==Item::AgentAccess{
                    let on=self.context.access;
                    let pill=PixelRect{x:right-26.0*s,y:rect.y+(ROW-15.0)/2.0*s,w:26.0*s,h:15.0*s};
                    p.shape(pill,7.5*s,a(if on{c.agent}else{c.muted}),Some(a(c.input)));
                    p.shape(PixelRect{x:if on{pill.x+pill.w-11.0*s}else{pill.x+2.0*s},y:pill.y+3.0*s,w:9.0*s,h:9.0*s},4.5*s,a(if on{c.card}else{c.muted_foreground}),None);
                    right=pill.x-8.0*s;
                }else if !r.keys.is_empty(){
                    let kw=p.mono_width(r.keys,11.5*s,400);
                    let key_ink=if r.enabled{c.muted_foreground}else{dim(0.6,c.muted_foreground)};
                    p.text(PixelRect{x:right-kw,y:rect.y,w:kw,h:rect.h},r.keys,11.5*s,a(key_ink),400,true);
                    right-=kw+10.0*s;
                }
                p.text(PixelRect{x:rect.x+8.0*s,y:rect.y,w:(right-rect.x-8.0*s).max(0.0),h:rect.h},r.label,13.0*s,a(ink),400,false);
                self.rows.push((rect,index));
                index+=1;y+=ROW*s;
            }
        }
        self.groups=groups;
        p.out.scene.layers
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    fn keys(menu:&mut ContextMenu,keys:&[NamedKey])->Effect{
        let mut last=Effect::None;for k in keys{last=menu.key(&Key::Named(*k));}last
    }
    #[test]
    fn keys_skip_greyed_rows_and_post_ai_drops_the_agent_rows(){
        use NamedKey::{ArrowDown as D,ArrowUp as U,Enter as E};
        let mut menu=ContextMenu::default();
        // Worked out from the approved rows: with nothing selected, Copy is greyed, so Down x4 is
        // Paste, Select all, Find…, Split right; Up from no highlight is Close pane.
        menu.show("t1".into(),Context::default(),(0.0,0.0));
        assert_eq!(keys(&mut menu,&[D,D,D,D,E]),Effect::Run(Item::SplitRight));
        menu.show("t1".into(),Context::default(),(0.0,0.0));
        assert_eq!(keys(&mut menu,&[U,E]),Effect::Run(Item::ClosePane));
        menu.show("t1".into(),Context{selection:true,block:Some("b1".into()),..Context::default()},(0.0,0.0));
        assert_eq!(keys(&mut menu,&[D,D,D,D,D,E]),Effect::Run(Item::CopyOutput));
        // Post-AI: Paste, Select all, Find…, Split right, Split below, Reset terminal, Reset and clear.
        menu.show("t1".into(),Context{post_ai:true,..Context::default()},(0.0,0.0));
        assert_eq!(keys(&mut menu,&[D,D,D,D,D,D,D,E]),Effect::Run(Item::ResetClear));
    }
}
