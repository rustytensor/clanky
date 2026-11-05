//! An independent owner wait lane wakes winit without polling terminal snapshots.
use clanky_api::Client;
use serde_json::json;
use std::{sync::{Arc,atomic::{AtomicBool,Ordering}},thread};
use winit::event_loop::EventLoopProxy;

#[derive(Debug)]
pub enum WindowNotice { Changed, Unavailable(String) }
pub struct WindowEvents {
    client:Client,
    stop:Arc<AtomicBool>,
    pending:Arc<AtomicBool>,
    worker:Option<thread::JoinHandle<()>>,
}
impl WindowEvents {
    pub fn start(client:&Client,proxy:EventLoopProxy<WindowNotice>)->Result<Self,String>{
        let client=client.fork_owner().map_err(|e|e.to_string())?;
        let stop=Arc::new(AtomicBool::new(false));let pending=Arc::new(AtomicBool::new(false));
        let lane=client.clone();let stopping=stop.clone();let queued=pending.clone();
        let worker=thread::Builder::new().name("window-changes".into()).spawn(move||{
            let mut revision=0;
            while !stopping.load(Ordering::Acquire){
                match lane.exchange(3,"window_changes",&json!({"after":revision,"timeout_ms":1000})) {
                    Ok(result)=>{
                        let Some(next)=result["revision"].as_u64() else{let _=proxy.send_event(WindowNotice::Unavailable("Invalid window change notice".into()));break;};
                        if next!=revision && !queued.swap(true,Ordering::AcqRel){
                            if proxy.send_event(WindowNotice::Changed).is_err(){break;}
                        }
                        revision=next;
                    },
                    Err(error)=>{if !stopping.load(Ordering::Acquire){let _=proxy.send_event(WindowNotice::Unavailable(error.message));}break;},
                }
            }
        }).map_err(|e|e.to_string())?;
        Ok(Self{client,stop,pending,worker:Some(worker)})
    }
    pub fn received(&self){self.pending.store(false,Ordering::Release);}
}
impl Drop for WindowEvents {
    fn drop(&mut self){
        self.stop.store(true,Ordering::Release);self.client.disconnect();
        if let Some(worker)=self.worker.take(){let _=worker.join();}
    }
}
