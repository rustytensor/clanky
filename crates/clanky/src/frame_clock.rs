//! Shared event-driven frame scheduling for the window and headless idle gate.
use std::time::{Duration,Instant};

pub const FRAME_INTERVAL:Duration=Duration::from_millis(16);
pub struct FrameClock {
    dirty:bool,
    queued:bool,
    animation:Option<Instant>,
    force:bool,
    frames:u64,
}
impl Default for FrameClock {fn default()->Self{Self::new(false)}}
impl FrameClock {
    /// force is used only by the gate's planted continuous-redraw fault.
    pub fn new(force:bool)->Self{Self{dirty:true,queued:false,animation:None,force,frames:0}}
    pub fn invalidate(&mut self){self.dirty=true;self.queued=false;}
    pub fn deadline(&self,now:Instant)->Option<Instant>{
        if self.queued {None}else if self.dirty {Some(now)}else{self.animation}
    }
    pub fn request_due(&mut self,now:Instant)->bool{
        if self.deadline(now).is_some_and(|deadline|deadline<=now){self.queued=true;true}else{false}
    }
    pub fn begin_frame(&mut self){self.queued=false;self.dirty=false;self.animation=None;}
    pub fn presented(&mut self,now:Instant,animation_end:Option<Instant>){
        self.frames+=1;
        self.animation=if self.force {Some(now+FRAME_INTERVAL)}
            else{animation_end.filter(|end|*end>now).map(|end|end.min(now+FRAME_INTERVAL))};
    }
    pub fn frames(&self)->u64{self.frames}
}
