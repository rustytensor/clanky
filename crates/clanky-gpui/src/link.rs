//! The pane's connection to the session server. A background thread waits for the server's change notices
//! and fetches the pane's (masked) snapshot itself, so the window thread never waits on the socket.
//!
//! Output floods are batched the way Zed batches its terminal events: the first change after a quiet spell is
//! fetched at once, for the lowest latency; after that, changes are gathered for 4 ms (or 100 notices) and one
//! snapshot covers them all.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use clanky_api::Client;
use clanky_core::TerminalSnapshot;
use serde_json::json;

const GATHER: Duration = Duration::from_millis(4);
const GATHER_NOTICES: usize = 100;
/// How long the server holds a change request open before answering "nothing new".
const WAIT_MS: u64 = 1000;

pub enum Update {
    Snapshot(Arc<TerminalSnapshot>),
    /// The server went away or refused us; the window shows the reason and stops updating.
    Lost(String),
    /// The shell the window started in the pane has exited.
    Finished,
}

/// How often the thread asks whether the pane's shell has exited.
const EXIT_CHECK: Duration = Duration::from_millis(250);

pub struct Link {
    stop: Arc<AtomicBool>,
    lane: Client,
    worker: Option<thread::JoinHandle<()>>,
}

impl Link {
    /// Starts watching `tab`; updates arrive on the returned channel.
    /// With `watch_exit`, an `Update::Finished` is sent once the pane's shell exits.
    pub fn start(
        client: &Client,
        tab: String,
        watch_exit: bool,
    ) -> Result<(Self, async_channel::Receiver<Update>), String> {
        let lane = client
            .fork_owner()
            .map_err(|error| format!("opening a second server connection: {error}"))?;
        let fetch = client
            .fork_owner()
            .map_err(|error| format!("opening a third server connection: {error}"))?;
        let (sender, receiver) = async_channel::unbounded();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let waiter = lane.clone();
        let worker = thread::Builder::new()
            .name("pane-updates".into())
            .spawn(move || watch(&waiter, &fetch, &tab, watch_exit, &sender, &stopping))
            .map_err(|error| format!("starting the update thread: {error}"))?;
        Ok((
            Self {
                stop,
                lane,
                worker: Some(worker),
            },
            receiver,
        ))
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.lane.disconnect();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn watch(
    lane: &Client,
    fetch: &Client,
    tab: &str,
    watch_exit: bool,
    sender: &async_channel::Sender<Update>,
    stop: &AtomicBool,
) {
    let mut revision = 0;
    let mut exit_checked = Instant::now();
    let mut quiet_since = Instant::now();
    let send = |update: Update| sender.send_blocking(update).is_ok();
    // The first screen, before any change.
    match fetch.snapshot(tab) {
        Ok(snapshot) => {
            if !send(Update::Snapshot(Arc::new(snapshot))) {
                return;
            }
        }
        Err(error) => {
            send(Update::Lost(format!("reading the pane: {}", error.message)));
            return;
        }
    }
    while !stop.load(Ordering::Acquire) {
        if watch_exit && exit_checked.elapsed() >= EXIT_CHECK {
            exit_checked = Instant::now();
            if shell_finished(fetch, tab) {
                send(Update::Finished);
                return;
            }
        }
        let wait_ms = if watch_exit {
            u64::try_from(EXIT_CHECK.as_millis()).unwrap_or(WAIT_MS)
        } else {
            WAIT_MS
        };
        let next = match wait_change(lane, revision, wait_ms) {
            Ok(next) => next,
            Err(message) => {
                if !stop.load(Ordering::Acquire) {
                    send(Update::Lost(message));
                }
                return;
            }
        };
        if next == revision {
            continue;
        }
        revision = next;
        // A change after a quiet spell is drawn at once; inside a flood, gather before fetching.
        if quiet_since.elapsed() < GATHER {
            revision = gather(lane, revision);
        }
        quiet_since = Instant::now();
        // The change notice is the whole session's: when this pane's screen is the one already held (the server
        // answers "unchanged" for its generation), there is nothing to draw.
        let held = fetch.cached_generation(tab);
        match fetch.snapshot(tab) {
            Ok(_) if held.is_some() && fetch.cached_generation(tab) == held => {}
            Ok(snapshot) => {
                if !send(Update::Snapshot(Arc::new(snapshot))) {
                    return;
                }
            }
            Err(error) => {
                send(Update::Lost(format!("reading the pane: {}", error.message)));
                return;
            }
        }
    }
}

fn shell_finished(client: &Client, tab: &str) -> bool {
    // A failed check is not an exit: the next change notice reports a lost server.
    client
        .exchange(3, "finished_shells", &json!({}))
        .is_ok_and(|finished| {
            finished["tab_ids"]
                .as_array()
                .is_some_and(|ids| ids.iter().any(|id| id.as_str() == Some(tab)))
        })
}

fn wait_change(lane: &Client, after: u64, timeout_ms: u64) -> Result<u64, String> {
    let result = lane
        .exchange(
            3,
            "window_changes",
            &json!({"after": after, "timeout_ms": timeout_ms}),
        )
        .map_err(|error| format!("waiting for pane changes: {}", error.message))?;
    result["revision"]
        .as_u64()
        .ok_or_else(|| "the server sent an invalid change notice".to_owned())
}

/// Takes further change notices for up to 4 ms or 100 notices; returns the newest revision seen.
fn gather(lane: &Client, mut revision: u64) -> u64 {
    let end = Instant::now() + GATHER;
    for _ in 0..GATHER_NOTICES {
        let left = end.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        // The server's wait has millisecond resolution; one millisecond is the shortest wait it takes.
        let wait_ms = u64::try_from(left.as_millis()).unwrap_or(1).max(1);
        match wait_change(lane, revision, wait_ms) {
            Ok(next) if next != revision => revision = next,
            _ => break,
        }
    }
    revision
}
