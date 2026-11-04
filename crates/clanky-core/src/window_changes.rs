//! Owner-only invalidation notices: no terminal bytes and no polling of the grid.
use super::*;

pub(super) struct WindowSignal { revision:Mutex<u64>, changed:Condvar }
impl WindowSignal {
    pub fn new()->Self{Self{revision:Mutex::new(0),changed:Condvar::new()}}
    pub fn notify(&self){
        let mut revision=self.revision.lock().unwrap();
        *revision=revision.wrapping_add(1);
        self.changed.notify_all();
    }
    pub fn wait(&self,after:u64,timeout:Duration)->u64{
        let revision=self.revision.lock().unwrap();
        let (revision,_)=self.changed.wait_timeout_while(revision,timeout,|revision|*revision==after).unwrap();
        *revision
    }
}
