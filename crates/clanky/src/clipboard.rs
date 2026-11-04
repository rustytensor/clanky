//! Owner-only OSC 52 bridge. Clipboard values never enter diagnostics.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use clanky_api::Client;
use clanky_core::platform::{self, Clipboard};
use serde_json::json;
use std::{
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

/// Drain one owner event using the same path as the window's background worker.
pub fn handle_events(
    client: &Client,
    tab: &str,
    write: bool,
    read: bool,
    mut copy: impl FnMut(&[u8]) -> Result<(), String>,
    mut paste: impl FnMut() -> Result<Vec<u8>, String>,
) -> Result<(), String> {
    let result = client.exchange(3, "clipboard_events", &json!({"tab_id":tab,"write":write,"read":read}))
        .map_err(|_| "Clipboard event request failed".to_owned())?;
    let events = result["events"].as_array().filter(|events| events.len() <= 1)
        .ok_or_else(|| "Invalid clipboard event response".to_owned())?;
    for event in events {
        let id = event["id"].as_u64().ok_or_else(|| "Invalid clipboard event id".to_owned())?;
        match event["kind"].as_str() {
            Some("write") if write => {
                let data = event["data"].as_str().filter(|data| data.len() <= (1024 * 1024usize).div_ceil(3) * 4)
                    .ok_or_else(|| "Invalid clipboard write".to_owned())?;
                let bytes = STANDARD.decode(data).map_err(|_| "Invalid clipboard write".to_owned())?;
                if bytes.len() > 1024 * 1024 { return Err("Clipboard write exceeds 1 MiB".into()); }
                if std::str::from_utf8(&bytes).is_err() { return Err("Invalid clipboard text".into()); }
                copy(&bytes).map_err(|_| "Clipboard copy failed".to_owned())?;
            }
            Some("read") if read => {
                let bytes = paste().map_err(|_| "Clipboard read failed".to_owned())?;
                if bytes.len() > 65536 { return Err("Clipboard read exceeds 64 KiB".into()); }
                client.exchange(3, "clipboard_reply", &json!({"tab_id":tab,"request_id":id,"data":STANDARD.encode(&bytes)}))
                    .map_err(|_| "Clipboard reply rejected".to_owned())?;
            }
            // Policy is enforced locally too, never trust a server to invoke a helper.
            Some("write" | "read") => (),
            _ => return Err("Invalid clipboard event kind".into()),
        }
    }
    Ok(())
}

#[derive(Default)]
struct State {
    tabs: Vec<String>,
    stop: bool,
    error: Option<String>,
}

/// Polls tabs round-robin; the GUI never waits for desktop helpers.
pub struct ClipboardBridge {
    shared: Arc<(Mutex<State>, Condvar)>,
    worker: Option<JoinHandle<()>>,
}
impl ClipboardBridge {
    pub fn new(client: Client, write: bool, read: bool) -> Self {
        let shared = Arc::new((Mutex::new(State::default()), Condvar::new()));
        let state = shared.clone();
        let client = client.clone();
        let worker = thread::Builder::new().name("clipboard-bridge".into()).spawn(move || {
            let mut clipboard = Clipboard::new();
            let mut next = 0usize;
            loop {
                let tab = {
                    let (lock, wake) = &*state;
                    let mut guard = lock.lock().unwrap();
                    if guard.stop { break; }
                    if guard.tabs.is_empty() {
                        guard = wake.wait(guard).unwrap();
                        if guard.stop { break; }
                        if guard.tabs.is_empty() { continue; }
                    }
                    let tab = guard.tabs[next % guard.tabs.len()].clone();
                    next = next.wrapping_add(1);
                    tab
                };
                let result = handle_events(&client, &tab, write, read,
                    |bytes| clipboard.copy(bytes).map_err(|_| "Clipboard copy failed".into()),
                    || platform::clipboard_read().map_err(|_| "Clipboard read failed".into()));
                let (lock, wake) = &*state;
                let mut guard = lock.lock().unwrap();
                if let Err(error) = result { guard.error = Some(error); }
                if client.is_closed() {
                    guard.error = Some("Clipboard connection closed".into());
                    break;
                }
                if guard.stop { break; }
                drop(wake.wait_timeout(guard, Duration::from_millis(50)).unwrap());
            }
        });
        let worker = match worker {
            Ok(worker) => Some(worker),
            Err(_) => {
                shared.0.lock().unwrap().error = Some("Clipboard worker unavailable".into());
                None
            }
        };
        Self { shared, worker }
    }
    pub fn set_tabs(&self, tabs: Vec<String>) {
        let mut state = self.shared.0.lock().unwrap();
        state.tabs = tabs.into_iter().take(64).collect();
        self.shared.1.notify_one();
    }
    pub fn take_error(&self) -> Option<String> {
        self.shared.0.lock().unwrap().error.take()
    }
}
impl Drop for ClipboardBridge {
    fn drop(&mut self) {
        self.shared.0.lock().unwrap().stop = true;
        self.shared.1.notify_one();
        if let Some(worker) = self.worker.take() { let _ = worker.join(); }
    }
}
