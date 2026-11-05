//! Opt-in latency/idle diagnostics. Records timestamps and counts, never key text.
use std::{fs::{File,OpenOptions},io::Write,time::Instant};

pub struct FrameTrace {
    file:Option<File>,
    started:Instant,
    sequence:u64,
    pending:Vec<(u64,u128)>,
    pending_echo:Vec<(u64,u128)>,
}
impl FrameTrace {
    pub fn from_env()->Result<Self,String>{
        let file=std::env::var_os("CLANKY_FRAME_TRACE").map(|path|
            OpenOptions::new().create_new(true).write(true).open(path).map_err(|e|e.to_string())).transpose()?;
        let mut trace=Self{file,started:Instant::now(),sequence:0,pending:Vec::new(),pending_echo:Vec::new()};
        if let Some(file)=&mut trace.file{
            // Wall clock at the trace's zero, so a driver can line up its own timestamps.
            let unix=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0,|d|d.as_micros());
            let _=writeln!(file,"clock,{unix},0");
        }
        Ok(trace)
    }
    pub fn enabled(&self)->bool{self.file.is_some()}
    /// One line per drawn frame: total and per-phase microseconds, and whether terminal output changed.
    pub fn phases(&mut self,total:u64,app:[u64;4],gpu:[u64;4],dirty:bool,snapshot:u64){
        if let Some(file)=&mut self.file{
            let time=self.started.elapsed().as_micros();
            let _=writeln!(file,"phases,{time},{total},{},{},{},{},{},{},{},{},{},{snapshot}",app[0],app[1],app[2],app[3],gpu[0],gpu[1],gpu[2],gpu[3],u8::from(dirty));
            if dirty{for (sequence,key_time) in self.pending_echo.drain(..){let _=writeln!(file,"echo,{sequence},{key_time},{time},{}",time-key_time);}}
        }
    }
    pub fn gate_frame(&mut self,elapsed_ms:Option<u64>){
        if let (Some(file),Some(time))=(&mut self.file,elapsed_ms){
            let _=writeln!(file,"gate_frame,{time},{}",self.started.elapsed().as_micros());
        }
    }
    pub fn robot_started(&mut self,duration_ms:u64){
        if let Some(file)=&mut self.file{
            let _=writeln!(file,"robot_start,{duration_ms},{}",self.started.elapsed().as_micros());
        }
    }
    pub fn robot_sample(&mut self,elapsed_ms:u64){
        if let Some(file)=&mut self.file{
            let _=writeln!(file,"robot_sample,{elapsed_ms},{}",self.started.elapsed().as_micros());
        }
    }
    pub fn robot_stopped(&mut self,skipped:bool){
        if let Some(file)=&mut self.file{
            let event=if skipped {"robot_skip"}else{"robot_end"};
            let _=writeln!(file,"{event},0,{}",self.started.elapsed().as_micros());
        }
    }
    pub fn key_event(&mut self){
        if let Some(file)=&mut self.file{
            self.sequence+=1;let time=self.started.elapsed().as_micros();
            let _=writeln!(file,"key,{},{time}",self.sequence);
            self.pending.push((self.sequence,time));
            self.pending_echo.push((self.sequence,time));
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
