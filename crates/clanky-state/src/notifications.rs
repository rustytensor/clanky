//! Window-local completion tracking shared by the window and notification gate.
//! No wall clock, command text or output participates in a decision.
use serde_json::Value;
use std::collections::HashMap;

/// A newly eligible completion. Contains metadata only, never terminal text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notification {
    pub tab_id: String,
    pub block_id: String,
    pub exit_code: Option<i32>,
}

impl Notification {
    /// Best effort, nonblocking, through the platform boundary.
    pub fn send(&self) {
        clanky_core::platform::send_command_finished_notification(self.exit_code);
    }
}

#[derive(Default)]
struct TabObservation {
    initialized: bool,
    completed_through: u64,
}

/// One tracker per attached window, retained across focus/config changes.
///
/// Feed each terminal's v2 `view` result (the result object, not the protocol
/// envelope), including background terminals. Walk its pages in chronological
/// order. On the initial observation of each tab, all pages through
/// `has_more: false` form a baseline and produce no notifications. A block
/// observed active during that baseline can notify when it later completes.
/// Subsequent polls must likewise cover all new completions; polling only the
/// first 64 retained blocks cannot discover completions on later pages.
///
/// Block IDs are the engine's monotonically increasing `bN` IDs. Completed
/// blocks are consumed even while disabled, focused or below threshold, so a
/// focus/threshold change never replays them. Recreate the tracker when
/// attaching to a different server (whose tab/block IDs may restart).
#[derive(Default)]
pub struct NotificationTracker {
    tabs: HashMap<String, TabObservation>,
}

impl NotificationTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// The latest completed retained block; active blocks must be queried again.
    pub fn cursor(&self, tab_id: &str) -> Option<String> {
        self.tabs.get(tab_id).filter(|state| state.initialized && state.completed_through > 0)
            .map(|state| format!("b{}", state.completed_through))
    }

    /// Discard a cursor after eviction or when a terminal closes.
    pub fn forget(&mut self, tab_id: &str) { self.tabs.remove(tab_id); }

    pub fn retain(&mut self, ids: &[String]) {
        self.tabs.retain(|id, _| ids.contains(id));
    }

    /// Poll owner-only metadata, without transferring terminal text or output.
    pub fn poll_client(&mut self, client: &clanky_api::Client, ids: &[String], threshold: u64, window_focused: bool, focused: Option<&str>) -> Result<(), String> {
        self.retain(ids);
        for id in ids {
            let mut after = self.cursor(id);
            let mut reset = false;
            for _ in 0..=1024 {
                let result = client.request("v3.block_events", &serde_json::json!({"tab_id":id,"after":after,"limit":64}), true);
                let page = match result {
                    Ok(page) => page,
                    Err(e) if e.code == "not_found" && after.is_some() && !reset => {
                        self.forget(id);
                        after = None;
                        reset = true;
                        continue;
                    }
                    Err(e) if e.code == "not_found" => break,
                    Err(e) => return Err(e.message),
                };
                let more = page["has_more"].as_bool().ok_or("invalid block event page")?;
                for notice in self.poll(id, &page, threshold, window_focused, focused) { notice.send(); }
                if !more { break; }
                let next = page["next_after"].as_str().ok_or("missing block event cursor")?.to_owned();
                if after.as_deref() == Some(&next) { return Err("block event cursor did not advance".into()); }
                after = Some(next);
            }
        }
        Ok(())
    }

    /// Pure deterministic decision: does not spawn helpers or read a clock.
    /// Invalid view objects are ignored without advancing the tab baseline.
    /// Missing/invalid durations never qualify; missing exit codes are unknown.
    pub fn poll(
        &mut self,
        tab_id: &str,
        view: &Value,
        threshold_seconds: u64,
        _window_focused: bool,
        _focused_tab_id: Option<&str>,
    ) -> Vec<Notification> {
        let Some(blocks) = view.get("blocks").and_then(Value::as_array) else {
            return Vec::new();
        };
        let Some(has_more) = view.get("has_more").and_then(Value::as_bool) else {
            return Vec::new();
        };
        // Retain references/metadata only; never clone command or output bytes.
        let mut completed: Vec<_> = blocks.iter().filter_map(|block| {
            if block.get("complete").and_then(Value::as_bool) != Some(true) {
                return None;
            }
            let id = block.get("block_id")?.as_str()?;
            let sequence = id.strip_prefix('b')?.parse::<u64>().ok()?;
            Some((sequence, id, block))
        }).collect();
        completed.sort_unstable_by_key(|(sequence, _, _)| *sequence);

        let state = self.tabs.entry(tab_id.to_owned()).or_default();
        let eligible = state.initialized && threshold_seconds != 0;
        let threshold_ms = u128::from(threshold_seconds) * 1000;
        let mut notifications = Vec::new();
        for (sequence, id, block) in completed {
            if sequence <= state.completed_through {
                continue;
            }
            state.completed_through = sequence;
            let focus = block.get("focused_tab_at_completion");
            if !matches!(focus, Some(Value::Null) | Some(Value::String(_)))
                || focus.and_then(Value::as_str) == Some(tab_id)
                || !eligible || !block.get("duration_ms")
                .and_then(Value::as_u64)
                .is_some_and(|ms| u128::from(ms) >= threshold_ms)
            {
                continue;
            }
            let exit_code = block.get("exit_code")
                .and_then(Value::as_i64)
                .and_then(|code| i32::try_from(code).ok());
            notifications.push(Notification {
                tab_id: tab_id.to_owned(),
                block_id: id.to_owned(),
                exit_code,
            });
        }
        if !has_more {
            state.initialized = true;
        }
        notifications
    }
}
