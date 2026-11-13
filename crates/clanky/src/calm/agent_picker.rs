//! "Start an agent": the picker opened by the Agent button, the palette or Ctrl+Shift+G.
//! It lists the configured tools (not-installed ones greyed, never hidden), where the new
//! pane opens, its folder and its agent access. Enter starts the highlighted tool.
use super::scene::Paint;
use crate::{look::{Layout,PixelRect},themes::{Appearance,Theme}};
use clanky_render::{Fill,Layer,Shadow,Shape,TextMetrics,TextRun};
use winit::keyboard::{Key,NamedKey};

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Choice { pub name:String, pub command:String, pub found:bool }

/// What the window knows when the picker opens.
#[derive(Clone,Debug,Default)]
pub struct Context { pub tools:Vec<Choice>, pub folder:String, pub access:bool }

#[derive(Debug,PartialEq,Eq)]
pub enum Effect { None, Close, Start { index:usize, tab:bool } }

const DIALOG_W:f32=430.0;const ROW:f32=36.0;const TITLE_H:f32=44.0;const OPT:f32=24.0;

#[derive(Default)]
pub struct AgentPicker {
    pub open:bool,
    selected:usize,
    /// Open in a new tab instead of a split to the right.
    pub tab:bool,
    dialog:Option<PixelRect>,
    rows:Vec<(PixelRect,usize)>,
    open_in:Option<PixelRect>,
}

impl AgentPicker {
    /// Open with the first installed tool highlighted.
    pub fn show(&mut self,context:&Context){
        self.open=true;self.tab=false;
        self.selected=context.tools.iter().position(|t|t.found).unwrap_or(0);
    }
    pub fn close(&mut self){self.open=false;self.dialog=None;self.rows.clear();self.open_in=None;}
    fn step(&mut self,context:&Context,forward:bool){
        let n=context.tools.len();
        for k in 1..=n{
            let i=if forward{(self.selected+k)%n}else{(self.selected+n-k%n)%n};
            if context.tools[i].found{self.selected=i;return;}
        }
    }
    fn start(&mut self,context:&Context,index:usize)->Effect{
        if context.tools.get(index).is_some_and(|t|t.found){let tab=self.tab;self.close();Effect::Start{index,tab}}else{Effect::None}
    }
    pub fn key(&mut self,key:&Key,context:&Context)->Effect{
        match key {
            Key::Named(NamedKey::Escape)=>{self.close();Effect::Close}
            Key::Named(NamedKey::ArrowDown)=>{self.step(context,true);Effect::None}
            Key::Named(NamedKey::ArrowUp)=>{self.step(context,false);Effect::None}
            Key::Named(NamedKey::Tab|NamedKey::ArrowLeft|NamedKey::ArrowRight)=>{self.tab=!self.tab;Effect::None}
            Key::Named(NamedKey::Enter)=>self.start(context,self.selected),
            _=>Effect::None,
        }
    }
    pub fn click(&mut self,x:f64,y:f64,context:&Context)->Effect{
        if let Some(&(_,index))=self.rows.iter().find(|(rect,_)|rect.contains(x,y)){return self.start(context,index);}
        if self.open_in.is_some_and(|rect|rect.contains(x,y)){self.tab=!self.tab;return Effect::None;}
        if self.dialog.is_some_and(|dialog|!dialog.contains(x,y)){self.close();return Effect::Close;}
        Effect::None
    }
    pub fn pointer(&mut self,x:f64,y:f64,context:&Context){
        if let Some(&(_,index))=self.rows.iter().find(|(rect,_)|rect.contains(x,y)){
            if context.tools.get(index).is_some_and(|t|t.found){self.selected=index;}
        }
    }

    pub fn draw(&mut self,layout:&Layout,theme:&Theme,context:&Context,pointer:(f64,f64),pressed:bool,measure:&mut dyn FnMut(&TextRun)->TextMetrics)->Vec<Layer>{
        if !self.open{return vec![];}
        let s=layout.scale as f32;let c=&theme.colors;let bar=layout.top_bar;
        let width=(DIALOG_W*s).min((bar.w-32.0*s).max(0.0));
        let list_h=context.tools.len().max(1) as f32*ROW*s;
        let height=TITLE_H*s+list_h+(4.0*OPT+20.0)*s;
        let dialog=PixelRect{x:(bar.x+(bar.w-width)/2.0).round(),y:(bar.y+bar.h+12.0*s).round(),w:width.round(),h:height.round()};
        let mut p=Paint::empty(theme,measure,s,layout.window_px,pointer,pressed);
        let (tint,strength)=match theme.appearance{Appearance::Dark=>([0,0,0],[89,51]),Appearance::Light=>([60,40,20],[26,15])};
        for ((offset,blur),alpha) in [(16.0,48.0),(2.0,6.0)].into_iter().zip(strength){
            p.out.scene.layers.push(Layer::Shape(Shape{rect:dialog.array(),radius:10.0*s,fill:Fill::Solid([0;4]),border:None,
                shadow:Some(Shadow{offset:[0.0,offset*s],blur:blur*s,spread:0.0,color:[tint[0],tint[1],tint[2],alpha]})}));
        }
        p.shape(dialog,10.0*s,c.card,Some(c.border));
        p.text(PixelRect{x:dialog.x+14.0*s,y:dialog.y+4.0*s,w:dialog.w-28.0*s,h:TITLE_H*s-4.0*s},"Start an agent",14.0*s,c.foreground,600,false);
        self.dialog=Some(dialog);self.rows.clear();
        let mut y=dialog.y+TITLE_H*s;
        if context.tools.is_empty(){
            p.text(PixelRect{x:dialog.x+14.0*s,y,w:dialog.w-28.0*s,h:ROW*s},"No agent tools in the config (\"agents\").",14.0*s,c.muted_foreground,400,false);
        }
        for (index,tool) in context.tools.iter().enumerate(){
            let row=PixelRect{x:dialog.x,y,w:dialog.w,h:ROW*s};
            p.shape(PixelRect{x:row.x+s,y:row.y,w:row.w-2.0*s,h:s},0.0,c.border,None);
            if index==self.selected && tool.found{p.shape(PixelRect{x:row.x+s,y:row.y+s,w:row.w-2.0*s,h:row.h-s},0.0,if pressed && row.contains(pointer.0,pointer.1){c.pressed}else{c.accent},None);}
            let ink=if tool.found{c.foreground}else{c.muted_foreground};
            p.shape(PixelRect{x:row.x+14.0*s,y:row.y+(ROW*s-8.0*s)/2.0,w:8.0*s,h:8.0*s},4.0*s,if tool.found{c.agent}else{c.muted},None);
            let detail=if tool.found{tool.command.as_str()}else{"not installed"};
            let dw=if tool.found{p.mono_width(detail,12.0*s,400)}else{p.width(detail,12.0*s,400)}.min(row.w*0.45);
            p.text(PixelRect{x:row.x+row.w-14.0*s-dw,y:row.y,w:dw,h:row.h},detail,12.0*s,c.muted_foreground,400,tool.found);
            p.text(PixelRect{x:row.x+32.0*s,y:row.y,w:(row.w-60.0*s-dw).max(0.0),h:row.h},&tool.name,14.0*s,ink,400,false);
            self.rows.push((row,index));
            y+=ROW*s;
        }
        if context.tools.is_empty(){y+=ROW*s;}
        p.shape(PixelRect{x:dialog.x+s,y,w:dialog.w-2.0*s,h:s},0.0,c.border,None);
        y+=10.0*s;
        let access=if context.access{"Read + run, this pane only"}else{"Off"};
        let options=[("Open in",if self.tab{"New tab ▾"}else{"Split right ▾"},true),("Folder",context.folder.as_str(),true),
            ("Agent access",access,true),("New git worktree","Coming next",false)];
        for (label,value,live) in options{
            let rect=PixelRect{x:dialog.x+14.0*s,y,w:dialog.w-28.0*s,h:OPT*s};
            let vw=p.width(value,13.0*s,500).min(rect.w*0.65);
            p.text(PixelRect{w:rect.w-vw-8.0*s,..rect},label,13.0*s,c.muted_foreground,400,false);
            p.text(PixelRect{x:rect.x+rect.w-vw,w:vw,..rect},value,13.0*s,if live{c.foreground}else{c.muted_foreground},500,false);
            if label=="Open in"{self.open_in=Some(rect);}
            y+=OPT*s;
        }
        p.out.scene.layers
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    fn context()->Context{
        let t=|name:&str,found|Choice{name:name.into(),command:name.to_lowercase(),found};
        Context{tools:vec![t("Aider",false),t("Codex",true),t("Pi",true)],folder:"~/git/weather".into(),access:true}
    }
    #[test]
    fn enter_starts_the_first_installed_tool_and_skips_missing_ones(){
        let context=context();let mut picker=AgentPicker::default();
        picker.show(&context);
        // Worked out by hand: Aider (index 0) is not installed, so Codex (1) is first; Down goes to Pi, Down wraps past Aider.
        picker.key(&Key::Named(NamedKey::ArrowDown),&context);
        picker.key(&Key::Named(NamedKey::ArrowDown),&context);
        picker.key(&Key::Named(NamedKey::Tab),&context);
        assert_eq!(picker.key(&Key::Named(NamedKey::Enter),&context),Effect::Start{index:1,tab:true});
        assert!(!picker.open);
    }
}
