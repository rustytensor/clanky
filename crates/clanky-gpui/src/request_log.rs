//! An opt-in log of the server requests the window sends: with `CLANKY_REQUEST_LOG=<path>` each request the window's
//! status read, its panes' links and its actions send appends one line holding the request's operation name, so a
//! gate can count what one status tick costs. Without the variable nothing is opened or written.

use std::{
    fs::{File, OpenOptions},
    io::Write,
    sync::{Mutex, OnceLock},
};

use clanky_api::Client;
use clanky_core::CoreError;
use serde_json::Value;

/// The environment variable that names the log file.
const LOG_VARIABLE: &str = "CLANKY_REQUEST_LOG";

/// The log file, opened on first use; None when the variable is unset or the file cannot be opened.
fn log() -> Option<&'static Mutex<File>> {
    static LOG: OnceLock<Option<Mutex<File>>> = OnceLock::new();
    LOG.get_or_init(|| {
        let path = std::env::var_os(LOG_VARIABLE)?;
        match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(file) => Some(Mutex::new(file)),
            Err(error) => {
                eprintln!(
                    "opening the request log {}: {error}",
                    std::path::Path::new(&path).display()
                );
                None
            }
        }
    })
    .as_ref()
}

/// Notes one request by its operation name. The panes' threads and the window's status read write from different
/// threads; each line is written whole under the lock.
pub fn note(op: &str) {
    let Some(file) = log() else { return };
    let Ok(mut file) = file.lock() else { return };
    // A full disk must not stop the window; the log only counts.
    let _ = writeln!(file, "{op}");
}

/// `Client::exchange`, noted in the log first.
pub fn exchange(client: &Client, version: u64, op: &str, args: &Value) -> Result<Value, CoreError> {
    note(op);
    client.exchange(version, op, args)
}
