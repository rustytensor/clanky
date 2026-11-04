//! Observation-only palette driver. The Python gate owns all expected values.
use clanky_core::Engine;
use serde_json::{Value, json};
use std::{
    error::Error,
    path::Path,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

// Explicit shutdown also runs on early errors and unwinding.
struct Session(Engine);
impl Drop for Session {
    fn drop(&mut self) {
        self.0.shutdown();
    }
}

fn field(value: &Value, key: &str) -> Result<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("missing observation field {key}").into())
}

fn v1(engine: &Engine, op: &str, args: Value) -> Result<Value> {
    Ok(engine.request(op, &args, true)?)
}

fn action(engine: &Engine, tab: &str, op: &str, mut args: Value) -> Result<Value> {
    args["tab_id"] = json!(tab);
    Ok(engine.palette_action(op, &args, true)?)
}

fn envelope(result: std::result::Result<Value, clanky_core::CoreError>) -> Value {
    match result {
        Ok(value) => json!({"ok":true,"result":value}),
        Err(error) => json!({"ok":false,"error":{"code":error.code,"message":error.message}}),
    }
}

fn wait(engine: &Engine, tab: &str, command: &str) -> Result<Value> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let remaining = deadline
            .saturating_duration_since(Instant::now())
            .as_millis()
            .min(1000) as u64;
        let value = v1(
            engine,
            "wait",
            json!({"tab_id":tab,"command_id":command,"timeout_ms":remaining}),
        )?;
        if value["done"] == true {
            return Ok(value);
        }
    }
    Err("command completion exceeded 10 seconds".into())
}

fn output(engine: &Engine, tab: &str, command: &str) -> Result<Value> {
    v1(
        engine,
        "get_output",
        json!({"tab_id":tab,"command_id":command,"offset":0,"limit":65536,"encoding":"base64"}),
    )
}

fn observe(transcript: &Path) -> Result<Value> {
    let session = Session(Engine::new());
    let engine = &session.0;
    // Shut down PTYs before the caller's 30-second hard deadline, including
    // unexpected stalls. Dropping the sender cancels this watchdog on return.
    let (_finished, receiver) = std::sync::mpsc::channel::<()>();
    let watched = engine.clone();
    std::thread::spawn(move || {
        if matches!(
            receiver.recv_timeout(Duration::from_secs(25)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ) {
            watched.shutdown();
        }
    });
    let cwd = transcript.parent().ok_or("transcript has no parent")?;
    let mut o = json!({});
    o["actions"] = engine.palette_action("actions", &json!({}), true)?;
    let opened = v1(
        engine,
        "open_tab",
        json!({"rows":24,"cols":80,"allow_agent":true}),
    )?;
    let tab = field(&opened, "tab_id")?;
    let started = v1(
        engine,
        "run",
        json!({"tab_id":tab,"argv":["sh",transcript],"cwd":cwd}),
    )?;
    let command = field(&started, "command_id")?;
    o["tab_id"] = json!(tab);
    o["command_id"] = json!(command);
    o["status"] = wait(engine, &tab, &command)?;
    o["initial"] = action(engine, &tab, "view", json!({}))?;
    o["raw_initial"] = output(engine, &tab, &command)?;
    let block = o["initial"]["blocks"]
        .as_array()
        .and_then(|b| b.first())
        .ok_or("fixture returned no block observations")?;
    let block = field(block, "block_id")?;
    o["selection"] = action(
        engine,
        &tab,
        "select",
        json!({"block_id":block,"start":0,"end":3}),
    )?;
    o["copy"] = action(engine, &tab, "copy", json!({"block_id":block}))?;
    o["collapse"] = action(
        engine,
        &tab,
        "collapse",
        json!({"block_id":block,"collapsed":true}),
    )?;
    o["collapsed"] = action(engine, &tab, "view", json!({}))?;
    o["raw_collapsed"] = output(engine, &tab, &command)?;
    o["expand"] = action(
        engine,
        &tab,
        "collapse",
        json!({"block_id":block,"collapsed":false}),
    )?;
    o["expanded"] = action(engine, &tab, "view", json!({}))?;
    o["tabs_before"] = v1(engine, "list_tabs", json!({}))?;
    o["missing_argv"] = envelope(engine.palette_action(
        "rerun",
        &json!({"tab_id":tab,"block_id":block,"cwd":cwd}),
        true,
    ));
    o["tabs_after"] = v1(engine, "list_tabs", json!({}))?;
    o["rerun"] = action(
        engine,
        &tab,
        "rerun",
        json!({"block_id":block,"argv":["sh","-c","printf again"],"cwd":cwd}),
    )?;
    let new_tab = field(&o["rerun"], "tab_id")?;
    let new_command = field(&o["rerun"], "command_id")?;
    o["rerun_status"] = wait(engine, &new_tab, &new_command)?;
    o["rerun_output"] = output(engine, &new_tab, &new_command)?;
    o["after_rerun"] = action(engine, &tab, "view", json!({}))?;
    o["raw_after_rerun"] = output(engine, &tab, &command)?;
    o["queued"] = action(
        engine,
        &tab,
        "send",
        json!({"block_id":block,"label":"Review first"}),
    )?;
    let task = field(&o["queued"], "task_id")?;
    o["running"] = action(
        engine,
        &tab,
        "task_update",
        json!({"task_id":task,"state":"running"}),
    )?;
    o["done"] = action(
        engine,
        &tab,
        "task_update",
        json!({"task_id":task,"state":"done"}),
    )?;
    o["tasks"] = action(engine, &tab, "tasks", json!({}))?;
    o["task_get"] = action(engine, &tab, "task_get", json!({"task_id":task}))?;
    let denied = v1(engine, "open_tab", json!({"allow_agent":false}))?;
    o["denied"] =
        envelope(engine.palette_action("view", &json!({"tab_id":field(&denied,"tab_id")?}), false));
    Ok(o)
}

fn run() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 || args[0] != "--transcript" {
        return Err("usage: clanky-actions-gate --transcript FILE".into());
    }
    let transcript = std::fs::canonicalize(&args[1])?;
    let observations = observe(&transcript)?;
    println!("{}", serde_json::to_string(&observations)?);
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
