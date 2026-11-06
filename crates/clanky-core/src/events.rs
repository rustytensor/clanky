//! Bounded in-memory notifications and owner audit records.
use super::*;
use std::sync::atomic::AtomicU64;

struct Event {
    seq: u64,
    recorded: Instant,
    tab_id: String,
    allowed: Arc<AtomicBool>,
    value: Value,
    encoded_len: usize,
}
#[derive(Default)]
struct EventRing {
    next: u64,
    items: VecDeque<Event>,
}
struct RecentEvent { recorded:Instant, value:Value }
#[derive(Default)]
struct RecentRing { next:u64, items:VecDeque<RecentEvent> }
#[derive(Default)]
struct AuditRing {
    records: VecDeque<Value>,
    evicted: u64,
    last: u64,
}
#[derive(Default)]
struct Audits {
    next: u64,
    tabs: BTreeMap<String, AuditRing>,
}
/// Agent requests that only read; saved history keeps the ones that act.
const READ_ONLY_OPS: &[&str] = &["hello", "input_state", "wait_input", "screen", "list_tabs", "list_blocks",
    "read_output", "events", "wait", "block_events", "terminal_info", "terminal_cwd", "snapshot", "permission_get",
    "audit", "look_state", "shell_defaults", "history", "window_changes", "window_search", "clipboard_events"];
pub(super) struct Safety {
    pub policy: masking::Policy,
    pub status: status::Status,
    pub authority: Mutex<()>,
    pub epoch: AtomicU64,
    pub latched: AtomicBool,
    pub post_ai: AtomicBool,
    pub post_ai_forced: AtomicBool,
    pub post_ai_epoch: AtomicU64,
    pub stop: AtomicBool,
    events: Mutex<EventRing>,
    recent: Mutex<RecentRing>,
    pub changed: Condvar,
    pub window_signal: window_changes::WindowSignal,
    pub readiness: readiness::ReadinessSchedule,
    audit: Mutex<Audits>,
    pub history: std::sync::OnceLock<crate::history::History>,
}
impl Safety {
    pub fn new() -> Self {
        Self {
            policy: masking::Policy::capture(),
            status: status::Status::new(),
            authority: Mutex::new(()),
            epoch: AtomicU64::new(0),
            latched: AtomicBool::new(false),
            post_ai: AtomicBool::new(false),
            post_ai_forced: AtomicBool::new(false),
            post_ai_epoch: AtomicU64::new(0),
            stop: AtomicBool::new(false),
            events: Mutex::new(EventRing::default()),
            recent: Mutex::new(RecentRing::default()),
            changed: Condvar::new(),
            window_signal: window_changes::WindowSignal::new(),
            readiness: readiness::ReadinessSchedule::new(),
            audit: Mutex::new(Audits::default()),
            history: std::sync::OnceLock::new(),
        }
    }
    /// Project only protocol metadata; never clone output or other event payloads.
    pub fn look_events(&self) -> Vec<Value> {
        let ring = self.events.lock().unwrap();
        ring.items.iter().skip(ring.items.len().saturating_sub(20)).map(|event| {
            json!({"seq":event.seq,"kind":event.value["kind"],"age_ms":event.recorded.elapsed().as_millis().min(u64::MAX as u128) as u64,
                "tab_id":event.tab_id,"command_id":event.value["command_id"]})
        }).collect()
    }
    /// Owner-only human activity, separate from the high-volume replay stream.
    pub fn record_recent(&self,kind:&str,tab_id:&str,mut value:Value){
        let mut ring=self.recent.lock().unwrap();ring.next+=1;
        value["seq"]=json!(ring.next);value["kind"]=json!(kind);value["tab_id"]=json!(tab_id);
        ring.items.push_back(RecentEvent{recorded:Instant::now(),value});
        while ring.items.len()>20{ring.items.pop_front();}
    }
    pub fn look_recent(&self)->Vec<Value>{
        self.recent.lock().unwrap().items.iter().map(|event|{
            let mut value=event.value.clone();value["age_ms"]=json!(event.recorded.elapsed().as_millis().min(u64::MAX as u128) as u64);value
        }).collect()
    }
    pub fn deny_retained_events(&self) {
        let ring = self.events.lock().unwrap();
        for event in &ring.items {
            event.allowed.store(false, Ordering::Release);
        }
    }
    pub fn wait_tick(&self, duration: Duration) {
        let ring = self.events.lock().unwrap();
        drop(self.changed.wait_timeout(ring, duration).unwrap());
    }
    pub fn publish(&self, tab_id: &str, allowed: &Arc<AtomicBool>, mut value: Value) {
        // Raw PTY feed notifies only after the grid is updated; masked output
        // can be buffered and must not be the window's typing wakeup source.
        let window_change=!matches!(value["kind"].as_str(),Some("output_appended"|"waiting_for_input"));
        if value["kind"]=="command_finished"{
            self.readiness.activity(tab_id);
            self.record_recent("command_finished",tab_id,json!({"exit_code":value["exit_code"]}));
        }
        if value["kind"]=="terminal_closed"{self.record_recent("pane_closed",tab_id,json!({}));}
        let mut ring = self.events.lock().unwrap();
        ring.next += 1;
        let seq = ring.next;
        value["seq"] = json!(seq);
        value["tab_id"] = json!(tab_id);
        let encoded_len = serde_json::to_vec(&value).map_or(65536, |v| v.len());
        ring.items.push_back(Event {
            seq,
            recorded: Instant::now(),
            tab_id: tab_id.into(),
            allowed: allowed.clone(),
            value,
            encoded_len,
        });
        while ring.items.len() > 1024 {
            ring.items.pop_front();
        }
        self.changed.notify_all();
        drop(ring);
        if window_change{self.window_signal.notify();}
    }
    pub fn output(
        &self,
        tab_id: &str,
        allowed: &Arc<AtomicBool>,
        command_id: &str,
        bytes: &[u8],
        start: u64,
    ) {
        for (i, chunk) in bytes.chunks(45 * 1024).enumerate() {
            self.publish(
                tab_id,
                allowed,
                json!({"kind":"output_appended", "command_id":command_id,
                "data":base64::engine::general_purpose::STANDARD.encode(chunk), "encoding":"base64",
                "offset":start + (i * 45 * 1024 + chunk.len()) as u64}),
            );
        }
    }
}
impl Engine {
    pub(super) fn events_request(&self, args: &Value, trusted: bool) -> Result<Value, CoreError> {
        fields(args, &["after", "limit", "timeout_ms", "tab_id"])?;
        let after = number(args, "after", 0, u64::MAX)?;
        let limit = optional_number(args, "limit", 1, 128, 128)? as usize;
        let timeout = optional_number(args, "timeout_ms", 0, 30000, 0)?;
        let tab_id = args
            .get("tab_id")
            .map(|_| string(args, "tab_id"))
            .transpose()?;
        let tab = tab_id.map(|id| self.tab(id, trusted)).transpose()?;
        let safety = &self.0.1;
        let epoch = {
            let _authority = safety.authority.lock().unwrap();
            safety.epoch.load(Ordering::Acquire)
        };
        let permission = tab.as_ref().map(|t| t.permission.load(Ordering::Acquire));
        let deadline = Instant::now() + Duration::from_millis(timeout);
        // An explicit event request (and its timeout) may refresh readiness.
        // Never run this while holding the ring: observations can publish events.
        let observe=|| {
            let tabs=if let Some(tab)=&tab {vec![tab.clone()]} else {
                self.0.0.lock().unwrap().tabs.values().filter(|tab|trusted || tab.allowed.load(Ordering::Acquire)).cloned().collect()
            };
            for tab in tabs{tab.observe_input();}
        };
        observe();
        let mut ring = safety.events.lock().unwrap();
        loop {
            if !trusted
                && (epoch != safety.epoch.load(Ordering::Acquire)
                    || tab.as_ref().is_some_and(|t| {
                        !t.allowed.load(Ordering::Acquire)
                            || Some(t.permission.load(Ordering::Acquire)) != permission
                    }))
            {
                return Err(error("permission_denied", "agent permission changed"));
            }
            let oldest = ring.items.front().map_or(ring.next + 1, |e| e.seq);
            let gap = after < oldest.saturating_sub(1) || after > ring.next;
            let mut page = Vec::new();
            let mut cursor = after.min(ring.next);
            let mut budget = 512 * 1024;
            for event in ring.items.iter().filter(|e| e.seq > after) {
                if tab_id.is_some_and(|id| id != event.tab_id)
                    || (!trusted && !event.allowed.load(Ordering::Acquire))
                {
                    cursor = event.seq;
                    continue;
                }
                if event.encoded_len > budget {
                    break;
                }
                budget -= event.encoded_len;
                cursor = event.seq;
                page.push(event.value.clone());
                if page.len() == limit {
                    break;
                }
            }
            if !page.is_empty() || gap || Instant::now() >= deadline {
                // Recheck while retaining the ring lock: revoke publishes under it.
                if !trusted
                    && (epoch != safety.epoch.load(Ordering::Acquire)
                        || tab.as_ref().is_some_and(|t| {
                            !t.allowed.load(Ordering::Acquire)
                                || Some(t.permission.load(Ordering::Acquire)) != permission
                        }))
                {
                    return Err(error("permission_denied", "agent permission changed"));
                }
                if !trusted {
                    page.retain(|value| {
                        ring.items
                            .iter()
                            .find(|event| value["seq"] == event.seq)
                            .is_some_and(|event| event.allowed.load(Ordering::Acquire))
                    });
                }
                if trusted {
                    // Never acquire a command lock while holding the event ring:
                    // producers append under the command lock before publishing.
                    drop(ring);
                    for event in &mut page {
                        if event["kind"] != "output_appended" {
                            continue;
                        }
                        let length = event["data"]
                            .as_str()
                            .and_then(|s| base64::engine::general_purpose::STANDARD.decode(s).ok())
                            .map_or(0, |b| b.len());
                        let end = event["offset"].as_u64().unwrap_or(0);
                        let start = end.saturating_sub(length as u64);
                        let output = self.request(
                            "get_output",
                            &json!({
                            "tab_id":event["tab_id"],"command_id":event["command_id"],
                            "offset":start,"limit":length,"encoding":"base64"}),
                            true,
                        );
                        match output {
                            Ok(output) => event["data"] = output["data"].clone(),
                            Err(_) => {
                                event.as_object_mut().unwrap().remove("data");
                                event["data_unavailable"] = json!(true);
                            }
                        }
                    }
                }
                return Ok(
                    json!({"events":page,"next_cursor":cursor,"oldest_cursor":oldest,"gap":gap}),
                );
            }
            let (next, timed) = safety.changed.wait_timeout(ring,deadline.saturating_duration_since(Instant::now())).unwrap();
            ring = next;
            if timed.timed_out(){drop(ring);observe();ring=safety.events.lock().unwrap();}
        }
    }
    pub fn record_agent(
        &self,
        peer: &platform::AgentPeer,
        connection_id: u64,
        version: u64,
        op: &str,
        args: &Value,
        result_code: &str,
    ) {
        let terminal_ancestry: Vec<_> = {
            let s = self.0.0.lock().unwrap();
            s.tabs
                .iter()
                .filter_map(|(id, tab)| {
                    let p = tab.process.lock().unwrap();
                    p.as_ref()
                        .filter(|p| peer.terminal_roots.contains(&p.pid()))
                        .map(|_| id.clone())
                })
                .collect()
        };
        let masked = self
            .0
            .1
            .policy
            .value(args)
            .unwrap_or_else(|_| json!({"unavailable":true}));
        let encoded = serde_json::to_string(&masked).unwrap_or_default();
        let masked = if encoded.len() > 8192 {
            let mut end = 8192;
            loop {
                while !encoded.is_char_boundary(end) {
                    end -= 1;
                }
                let value = json!({"truncated":true,"text":&encoded[..end]});
                let size = serde_json::to_vec(&value).map_or(usize::MAX, |v| v.len());
                if size <= 8192 {
                    break value;
                }
                end = end.saturating_sub((size - 8192).div_ceil(2).max(1));
            }
        } else {
            masked
        };
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let active = self.tab_ids();
        let mut audit = self.0.1.audit.lock().unwrap();
        let requested = args.get("tab_id").and_then(Value::as_str);
        let tab_id = requested
            .filter(|id| {
                active.iter().any(|live| live == id)
                    || audit.tabs.contains_key(*id)
                    || (op == "close_tab" && result_code == "ok")
            })
            .or_else(|| terminal_ancestry.first().map(String::as_str))
            .unwrap_or("_connection");
        audit.next += 1;
        let seq = audit.next;
        if tab_id != "_connection"
            && !audit.tabs.contains_key(tab_id)
            && audit
                .tabs
                .keys()
                .filter(|id| id.as_str() != "_connection")
                .count()
                >= 64
        {
            let oldest = audit
                .tabs
                .iter()
                .filter(|(id, _)| id.as_str() != "_connection")
                .min_by_key(|(id, ring)| (active.contains(id), ring.last))
                .map(|(id, _)| id.clone());
            if let Some(id) = oldest {
                audit.tabs.remove(&id);
            }
        }
        if let Some(history) = self.0.1.history.get().filter(|h| h.enabled() && !READ_ONLY_OPS.contains(&op)) {
            let agent = self.agent_name(connection_id);
            history.agent_event(crate::history::AgentEvent {
                at: time.min(i64::MAX as u128) as i64, agent,
                tab_id: (tab_id != "_connection").then(|| tab_id.to_owned()),
                action: self.0.1.policy.text(op, true).unwrap_or_else(|_| "unavailable".into()),
                detail: serde_json::to_string(&masked).unwrap_or_default(),
                result: result_code.into(),
            });
        }
        let ring = audit.tabs.entry(tab_id.into()).or_default();
        ring.last = seq;
        ring.records.push_back(json!({"seq":seq,"time_unix_ms":time,"connection_id":connection_id,
            "peer_pid":peer.pid,"ancestry":peer.ancestry,"terminal_ancestry":terminal_ancestry,
            "version":version,"op":self.0.1.policy.text(op,true).unwrap_or_else(|_| "unavailable".into()),"args":masked,"result_code":result_code}));
        while ring.records.len() > 512 {
            ring.records.pop_front();
            ring.evicted += 1;
        }
    }
    pub(super) fn audit_request(&self, args: &Value) -> Result<Value, CoreError> {
        fields(args, &["tab_id", "after", "limit"])?;
        let id = string(args, "tab_id")?;
        let after = optional_number(args, "after", 0, u64::MAX, 0)?;
        let limit = optional_number(args, "limit", 1, 128, 128)? as usize;
        let audit = self.0.1.audit.lock().unwrap();
        let ring = audit.tabs.get(id);
        let mut records = Vec::new();
        let mut budget = 512 * 1024;
        for record in ring
            .into_iter()
            .flat_map(|r| &r.records)
            .filter(|r| r["seq"].as_u64().unwrap_or(0) > after)
            .take(limit)
        {
            let size = serde_json::to_vec(record).map_or(budget + 1, |v| v.len());
            if size > budget {
                break;
            }
            budget -= size;
            records.push(record.clone());
        }
        let next = records
            .last()
            .and_then(|r| r["seq"].as_u64())
            .unwrap_or(after);
        Ok(
            json!({"records":records,"oldest_cursor":ring.and_then(|r| r.records.front()).map(|v| v["seq"].clone()).unwrap_or(json!(audit.next+1)),
            "evicted":ring.map_or(0,|r|r.evicted),"next_cursor":next}),
        )
    }
}
