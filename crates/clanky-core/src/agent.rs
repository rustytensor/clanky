//! Version four operations and permission leases shared with legacy requests.
use super::*;

pub(super) fn optional_number(
    args: &Value,
    key: &str,
    min: u64,
    max: u64,
    default: u64,
) -> Result<u64, CoreError> {
    if args.get(key).is_some() {
        number(args, key, min, max)
    } else {
        Ok(default)
    }
}
impl Tab {
    fn input_state(&self) -> Value {
        let command = {
            let commands = self.commands.lock().unwrap();
            commands
                .current
                .as_ref()
                .and_then(|id| commands.all.get(id))
                .cloned()
        };
        let exited = self.closed.load(Ordering::Acquire)
            || command
                .as_ref()
                .is_some_and(|c| c.state.lock().unwrap().done());
        let quiet = self.last_output.lock().unwrap().elapsed() >= Duration::from_millis(200);
        let input_time = *self.last_input.lock().unwrap();
        let after_input =
            input_time.is_none_or(|time| time.elapsed() >= Duration::from_millis(200));
        let prompt = !exited
            && quiet
            && after_input
            && self.terminal.lock().unwrap().prompt_waiting()
            && input_time.is_none_or(|time| *self.last_output.lock().unwrap() > time);
        // An explicit quiet shell prompt is already sufficient evidence; do not
        // scan every process on the desktop ten times a second to confirm it.
        let blocked = !prompt && !exited && quiet && after_input
            && self.process.lock().unwrap().as_ref().is_some_and(Process::waiting_for_input);
        let waiting = blocked || prompt;
        let bracketed_paste = self.terminal.lock().unwrap().bracketed_paste();
        json!({"state":if exited {"exited"} else if waiting {"waiting"} else {"running"},
            "waiting":waiting,"bracketed_paste":bracketed_paste,"evidence":if blocked {"pty_read"} else if prompt {"prompt"} else {"none"},
            "command_id":command.as_ref().map(|c| c.id.clone())})
    }
    pub(super) fn observe_input(&self) -> Value {
        let state = self.input_state();
        let waiting = state["waiting"].as_bool().unwrap_or(false);
        if self.was_waiting.swap(waiting, Ordering::AcqRel) != waiting {
            let mut event = state.clone();
            event["kind"] = json!("waiting_for_input");
            self.safety.publish(&self.id, &self.allowed, event);
        }
        state
    }
}
impl Engine {
    pub(super) fn lease(&self, tab: &Tab) -> (u64, u64) {
        let _authority = self.0.1.authority.lock().unwrap();
        (
            self.0.1.epoch.load(Ordering::Acquire),
            tab.permission.load(Ordering::Acquire),
        )
    }
    pub(super) fn check_lease(
        &self,
        tab: &Tab,
        lease: (u64, u64),
        trusted: bool,
    ) -> Result<(), CoreError> {
        if !trusted
            && (!tab.allowed.load(Ordering::Acquire)
                || (
                    self.0.1.epoch.load(Ordering::Acquire),
                    tab.permission.load(Ordering::Acquire),
                ) != lease)
        {
            return Err(error("permission_denied", "agent permission changed"));
        }
        Ok(())
    }
    pub(super) fn start_monitor(&self) {
        let weak = Arc::downgrade(&self.0);
        let safety = self.0.1.clone();
        let handle = std::thread::spawn(move || {
            while !safety.stop.load(Ordering::Acquire) {
                // No Engine reference is held while parked, and there is no
                // periodic scan after the quiet-period observation completes.
                let due=safety.readiness.due(&safety.stop);
                if safety.stop.load(Ordering::Acquire){break;}
                let Some(inner) = weak.upgrade() else { break };
                let tabs:Vec<_>={let state=inner.0.lock().unwrap();due.iter().filter_map(|id|state.tabs.get(id).cloned()).collect()};
                drop(inner);
                for tab in tabs {tab.observe_input();}
            }
        });
        *self.0.2.lock().unwrap() = Some(handle);
    }
    /// Sanitize a final transport error without exposing policy diagnostics.
    pub fn mask_message(&self, text: &str) -> String {
        self.0
            .1
            .policy
            .text(text, true)
            .unwrap_or_else(|_| "agent request failed".into())
    }
    /// Register server-owned credentials before any terminal can be created.
    /// This is deliberately not a wire operation.
    pub fn add_mask_literal(&self, label: &str, value: &str) -> Result<(), CoreError> {
        let state = self.0.0.lock().unwrap();
        if state.next_tab != 0 {
            return Err(error("conflict", "masking policy is already in use"));
        }
        self.0.1.policy.add_literal(label, value)
    }
    /// The transport passes its authenticated role, never a role from request JSON.
    pub fn agent_request(&self, op: &str, args: &Value, trusted: bool) -> Result<Value, CoreError> {
        if matches!(op, "audit" | "revoke_all" | "resume_agents") && !trusted {
            return Err(error("permission_denied", "owner operation"));
        }
        match op {
            "audit" => return self.audit_request(args),
            "events" => return self.events_request(args, trusted),
            "revoke_all" => {
                fields(args, &[])?;
                // State lock serializes tab creation and the creation latch. The
                // epoch invalidates every outstanding agent lease in one change.
                let _authority = self.0.1.authority.lock().unwrap();
                let state = self.0.0.lock().unwrap();
                self.0.1.latched.store(true, Ordering::Release);
                let epoch = self.0.1.epoch.fetch_add(1, Ordering::AcqRel) + 1;
                let mut revoked = 0;
                for tab in state.tabs.values() {
                    if tab.allowed.swap(false, Ordering::AcqRel) {
                        revoked += 1;
                    }
                    tab.permission.fetch_add(1, Ordering::AcqRel);
                    for c in tab.commands.lock().unwrap().all.values() {
                        c.changed.notify_all();
                    }
                }
                self.0.1.deny_retained_events();
                self.0.1.changed.notify_all();
                return Ok(json!({"revoked":revoked,"epoch":epoch}));
            }
            "resume_agents" => {
                fields(args, &[])?;
                let _state = self.0.0.lock().unwrap();
                self.0.1.latched.store(false, Ordering::Release);
                return Ok(json!({"resumed":true}));
            }
            _ => (),
        }
        let allowed: &[&str] = match op {
            "input_state" => &["tab_id"],
            "wait_input" => &["tab_id", "timeout_ms"],
            "screen" => &["tab_id", "history_offset", "history_limit"],
            "send_key" => &["tab_id", "key"],
            "paste" => &["tab_id", "text"],
            _ => return Err(error("unknown_operation", "unknown operation")),
        };
        fields(args, allowed)?;
        let id = string(args, "tab_id")?;
        let tab = self.tab(id, trusted)?;
        let lease = self.lease(&tab);
        let result = match op {
            "input_state" => tab.observe_input(),
            "wait_input" => {
                let timeout = number(args, "timeout_ms", 0, 30000)?;
                let deadline = Instant::now() + Duration::from_millis(timeout);
                loop {
                    self.check_lease(&tab, lease, trusted)?;
                    let mut value = tab.observe_input();
                    let waiting = value["waiting"] == true;
                    if waiting || Instant::now() >= deadline {
                        value["timed_out"] = json!(!waiting);
                        break value;
                    }
                    self.0.1.wait_tick(
                        deadline
                            .saturating_duration_since(Instant::now())
                            .min(Duration::from_millis(100)),
                    );
                }
            }
            "send_key" | "paste" => {
                let _authority = self.0.1.authority.lock().unwrap();
                let _guard = tab.lifecycle.lock().unwrap();
                self.check_lease(&tab, lease, trusted)?;
                let bytes = {
                    let mut terminal = tab.terminal.lock().unwrap();
                    let bytes = if op == "send_key" {
                        terminal
                            .key_bytes(string(args, "key")?)
                            .map_err(|_| invalid("unsupported named key"))?
                    } else {
                        let text = string(args, "text")?;
                        if text.len() > 65536 {
                            return Err(error("resource_limit", "paste exceeds 64 KiB"));
                        }
                        terminal.paste_bytes(text)
                    };
                    terminal.input_submitted();
                    bytes
                };
                let process = tab.process.lock().unwrap();
                let process = process
                    .as_ref()
                    .ok_or_else(|| error("not_found", "no active process"))?;
                let sent = bytes.len();
                *tab.last_input.lock().unwrap() = Some(Instant::now());
                process
                    .input(bytes)
                    .map_err(|_| error("resource_limit", "PTY input unavailable"))?;
                self.0.1.readiness.activity(id);
                json!({"sent":sent})
            }
            "screen" => {
                let offset =
                    optional_number(args, "history_offset", 0, usize::MAX as u64, 0)? as usize;
                let limit = optional_number(args, "history_limit", 1, 64, 64)? as usize;
                let (raw, total, display_offset, mut all) = {
                    let terminal = tab.terminal.lock().unwrap();
                    let (total, display_offset, _, _, _) = terminal.text_context();
                    let all = self.0.1.policy.terminal_lines(&terminal)?;
                    (
                        terminal.snapshot(),
                        total,
                        display_offset,
                        all,
                    )
                };
                for line in &mut all {
                    *line = line.trim_end_matches(' ').to_owned();
                }
                let lines = all
                    .iter()
                    .skip(total.saturating_sub(display_offset))
                    .take(raw.rows)
                    .cloned()
                    .collect::<Vec<_>>();
                let history = all
                    .into_iter()
                    .take(total)
                    .skip(offset)
                    .take(limit)
                    .collect::<Vec<_>>();
                if offset > total {
                    return Err(invalid("offset exceeds history"));
                }
                json!({"lines":lines,"cursor":raw.cursor,"rows":raw.rows,"cols":raw.cols,
                    "history":{"total":total,"offset":offset,"next_offset":offset+history.len(),"lines":history}})
            }
            _ => unreachable!(),
        };
        self.check_lease(&tab, lease, trusted)?;
        Ok(result)
    }
}

impl super::masking::Policy {
    pub(super) fn mask_lines(
        &self,
        lines: &mut [String],
        wraps: &[bool],
        cursor: Option<(usize, usize)>,
    ) -> Result<(), CoreError> {
        let original = lines.to_vec();
        let mut merged: Vec<Vec<u8>> = original.iter().map(|s| s.as_bytes().to_vec()).collect();
        // Consider both literal trailing spaces and terminal padding. Wrapped
        // rows always retain spaces; hard line breaks may have padding stripped.
        for trim_padding in [false, true] {
            let mut context = String::new();
            let mut ranges = Vec::with_capacity(lines.len());
            for (index, line) in original.iter().enumerate() {
                let wrapped = wraps.get(index).copied().unwrap_or(false);
                let text = if trim_padding && !wrapped {
                    line.trim_end_matches(' ')
                } else {
                    line
                };
                let start = context.len();
                context.push_str(text);
                ranges.push(start..context.len());
                if index + 1 < lines.len() && !wrapped {
                    context.push('\n');
                }
            }
            let safe = self.bytes(context.as_bytes(), false)?;
            // Blank cells after the live cursor are not received delimiters:
            // do not release an undecided prefix just because they are present.
            let cut =
                cursor.and_then(|(row, byte)| ranges.get(row).map(|r| r.start + byte.min(r.len())));
            let prefix = cut
                .map(|cut| self.bytes(&context.as_bytes()[..cut], false))
                .transpose()?;
            for ((out, original), range) in merged.iter_mut().zip(&original).zip(ranges) {
                for index in 0..range.len() {
                    let position = range.start + index;
                    let pending = cut.is_some_and(|cut| position < cut)
                        && prefix.as_ref().is_some_and(|safe| {
                            safe.get(position)
                                .is_none_or(|b| *b != original.as_bytes()[index])
                        });
                    if pending
                        || safe
                            .get(position)
                            .is_none_or(|b| *b != original.as_bytes()[index])
                    {
                        out[index] = b'*';
                    }
                }
            }
        }
        for (line, bytes) in lines.iter_mut().zip(merged) {
            *line = String::from_utf8_lossy(&bytes).into_owned();
        }
        Ok(())
    }
}
