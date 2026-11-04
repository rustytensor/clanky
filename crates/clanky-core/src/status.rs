//! Owner presentation metadata. No request arguments or terminal text are retained.
use super::*;

#[derive(Clone)]
struct Connection {
    peer_pid: u32,
    tab_id: Option<String>,
    busy: bool,
}

pub(super) struct Driver {
    command_id: String,
    connection_id: u64,
    peer_pid: u32,
}

const ACTIVITY_MINUTES: usize = 30;

// Minute boundaries are relative to server startup, never the wall clock.
struct Activity {
    buckets: [u64; ACTIVITY_MINUTES],
    minute: u64,
    current: usize,
}
impl Activity {
    fn advance(&mut self, minute: u64) {
        let elapsed = minute.saturating_sub(self.minute);
        if elapsed >= ACTIVITY_MINUTES as u64 {
            self.buckets.fill(0);
            self.current = 0;
        } else {
            for _ in 0..elapsed {
                self.current = (self.current + 1) % ACTIVITY_MINUTES;
                self.buckets[self.current] = 0;
            }
        }
        self.minute = minute;
    }
    fn snapshot(&self) -> [u64; ACTIVITY_MINUTES] {
        std::array::from_fn(|i| self.buckets[(self.current + 1 + i) % ACTIVITY_MINUTES])
    }
}

pub(super) struct Status {
    started: Instant,
    connections: Mutex<BTreeMap<u64, Connection>>,
    activity: Mutex<Activity>,
}
impl Status {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            connections: Mutex::new(BTreeMap::new()),
            activity: Mutex::new(Activity {
                buckets: [0; ACTIVITY_MINUTES], minute: 0, current: 0,
            }),
        }
    }
    pub fn close_tab(&self, tab_id: &str) {
        for peer in self.connections.lock().unwrap().values_mut() {
            if peer.tab_id.as_deref() == Some(tab_id) {
                peer.tab_id = None;
            }
        }
    }
}

impl Engine {
    /// Called on connection admission and every actual authentication role change.
    /// Owners are absent, even if they previously issued agent requests.
    pub fn agent_connection_role(&self, connection_id: u64, peer: &platform::AgentPeer, owner: bool) {
        if owner {
            self.agent_connection_closed(connection_id);
            return;
        }
        let tabs: Vec<_> = self.0.0.lock().unwrap().tabs.values().cloned().collect();
        let tab_id = tabs.iter().find_map(|tab| {
            tab.process.lock().unwrap().as_ref()
                .filter(|p| peer.terminal_roots.contains(&p.pid()))
                .map(|_| tab.id.clone())
        });
        // Serialize with tab closure so an ancestry association cannot outlive it.
        let tab = tab_id.as_ref().and_then(|id| tabs.iter().find(|t| &t.id == id));
        let _lifecycle = tab.map(|t| t.lifecycle.lock().unwrap());
        let tab_id = tab_id.filter(|_| tab.is_some_and(|t| !t.closed.load(Ordering::Acquire)));
        self.0.1.status.connections.lock().unwrap().insert(connection_id,
            Connection { peer_pid: peer.pid, tab_id, busy: false });
    }

    pub fn agent_connection_closed(&self, connection_id: u64) {
        self.0.1.status.connections.lock().unwrap().remove(&connection_id);
    }

    /// Transport request activity, not an inference from the audit log.
    pub fn agent_connection_busy(&self, connection_id: u64, busy: bool) {
        if let Some(peer) = self.0.1.status.connections.lock().unwrap().get_mut(&connection_id) {
            peer.busy = busy;
        }
    }

    /// Called once by the transport for a successful agent v1 run or v2 rerun,
    /// with the actual destination tab and generated command ID (not source args).
    /// Count dispatch success even if a competing run/close supersedes the reply;
    /// only the driver binding is omitted for an obsolete command.
    pub fn agent_run_bound(&self, connection_id: u64, peer_pid: u32, tab_id: &str, command_id: &str) {
        {
            let mut activity = self.0.1.status.activity.lock().unwrap();
            activity.advance(self.0.1.status.started.elapsed().as_secs() / 60);
            let current = activity.current;
            activity.buckets[current] = activity.buckets[current].saturating_add(1);
        }
        let Ok(tab) = self.tab(tab_id, true) else { return };
        let _lifecycle = tab.lifecycle.lock().unwrap();
        if tab.closed.load(Ordering::Acquire) { return; }
        let mut commands = tab.commands.lock().unwrap();
        if commands.current.as_deref() != Some(command_id) || !commands.all.contains_key(command_id) {
            return;
        }
        commands.driver = Some(Driver { command_id: command_id.into(), connection_id, peer_pid });
        if let Some(peer) = self.0.1.status.connections.lock().unwrap().get_mut(&connection_id) {
            peer.tab_id = Some(tab_id.into());
        }
    }

    pub(super) fn look_state(&self) -> Value {
        // Never hold the connection/event rings while acquiring tab/command locks.
        let connections = self.0.1.status.connections.lock().unwrap().clone();
        let agents: Vec<_> = connections.iter().map(|(id, peer)| {
            json!({"connection_id":id,"peer_pid":peer.peer_pid,
                "tab_id":peer.tab_id,"running":peer.busy})
        }).collect();
        let tabs: Vec<_> = self.0.0.lock().unwrap().tabs.values().cloned().collect();
        let mut drivers = Vec::new();
        for tab in tabs {
            let _lifecycle = tab.lifecycle.lock().unwrap();
            if tab.closed.load(Ordering::Acquire) { continue; }
            let commands = tab.commands.lock().unwrap();
            if let Some(driver) = &commands.driver {
                let running = commands.current.as_deref() == Some(driver.command_id.as_str())
                    && commands.all.get(&driver.command_id)
                        .is_some_and(|command| !command.state.lock().unwrap().done());
                drivers.push(json!({"tab_id":tab.id,"command_id":driver.command_id,
                    "connection_id":driver.connection_id,"peer_pid":driver.peer_pid,"running":running}));
            }
        }
        let activity = {
            let mut activity = self.0.1.status.activity.lock().unwrap();
            activity.advance(self.0.1.status.started.elapsed().as_secs() / 60);
            activity.snapshot()
        };
        json!({"uptime_ms":self.0.1.status.started.elapsed().as_millis().min(u64::MAX as u128) as u64,
            "masking_ready":self.0.1.policy.stream().is_ok(),
            "agents":agents,"drivers":drivers,"events":self.0.1.look_events(),"activity":activity})
    }
}
