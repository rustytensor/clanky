//! Readiness checks run on activity and one quiet-period deadline, never an idle tick.
use super::*;

pub(super) struct ReadinessSchedule {
    pending:Mutex<BTreeMap<String,(Instant,Option<Instant>)>>,
    changed:Condvar,
}
impl ReadinessSchedule {
    pub fn new()->Self{Self{pending:Mutex::new(BTreeMap::new()),changed:Condvar::new()}}
    pub fn activity(&self,tab:&str){
        let now=Instant::now();
        self.pending.lock().unwrap().insert(tab.into(),(now,Some(now+Duration::from_millis(200))));
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
                    if let Some((_,Some(next)))=pending.remove(id) {
                        if next>now{pending.insert(id.clone(),(next,None));}
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
