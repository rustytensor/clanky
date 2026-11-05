//! Opt-in latency/idle diagnostics. Records timestamps and counts, never key text.
use std::{fs::{File,OpenOptions},io::Write,time::Instant};

pub struct FrameTrace {
    file:Option<File>,
    started:Instant,
    sequence:u64,
    pending:Vec<(u64,u128)>,
}
impl FrameTrace {
    pub fn from_env()->Result<Self,String>{
        let file=std::env::var_os("CLANKY_FRAME_TRACE").map(|path|
            OpenOptions::new().create_new(true).write(true).open(path).map_err(|e|e.to_string())).transpose()?;
        Ok(Self{file,started:Instant::now(),sequence:0,pending:Vec::new()})
    }
    pub fn key_event(&mut self){
        if let Some(file)=&mut self.file{
            self.sequence+=1;let time=self.started.elapsed().as_micros();
            let _=writeln!(file,"key,{},{time}",self.sequence);
            self.pending.push((self.sequence,time));
        }
    }
    pub fn presented(&mut self){
        if let Some(file)=&mut self.file{
            let time=self.started.elapsed().as_micros();
            let _=writeln!(file,"frame,0,{time}");
            for (sequence,key_time) in self.pending.drain(..){let _=writeln!(file,"latency,{sequence},{key_time},{time},{}",time-key_time);}
        }
    }
}
