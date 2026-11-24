//! Readiness checks run on activity and one quiet-period deadline, never an idle tick.
use super::*;

pub(super) struct ReadinessSchedule {
    pending:Mutex<BTreeMap<String,(Instant,Vec<Instant>)>>,
    changed:Condvar,
}
impl ReadinessSchedule {
    pub fn new()->Self{Self{pending:Mutex::new(BTreeMap::new()),changed:Condvar::new()}}
    /// Checks the tab now, after the 200 ms quiet period, and once more after the
    /// longer quiet spell an event-loop wait needs.
    pub fn activity(&self,tab:&str){
        let now=Instant::now();
        self.pending.lock().unwrap().insert(tab.into(),(now,vec![now+Duration::from_millis(200),now+super::agent::POLL_QUIET]));
        self.changed.notify_one();
    }
    pub fn wake(&self){let _pending=self.pending.lock().unwrap();self.changed.notify_all();}
    pub fn due(&self,stop:&AtomicBool)->Vec<String>{
        let mut pending=self.pending.lock().unwrap();
        loop {
            if stop.load(Ordering::Acquire){return Vec::new();}
            let now=Instant::now();
            let due:Vec<_>=pending.iter().filter(|(_, (when,_))|*when<=now).map(|(id,_)|id.clone()).collect();
            if !due.is_empty(){
                for id in &due {
                    if let Some((_,mut later))=pending.remove(id) {
                        later.retain(|when|*when>now);
                        if !later.is_empty(){let next=later.remove(0);pending.insert(id.clone(),(next,later));}
                    }
                }
                return due;
            }
            pending=if let Some(next)=pending.values().map(|(when,_)|*when).min(){
                self.changed.wait_timeout(pending,next.saturating_duration_since(now)).unwrap().0
            }else{self.changed.wait(pending).unwrap()};
        }
    }
}
