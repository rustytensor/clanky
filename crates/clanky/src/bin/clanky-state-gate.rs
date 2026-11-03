use clanky::config::{Config, Metrics, Zoom};
use clanky_core::{
    Engine,
    layout::{Axis, Layout, Rect},
};
use serde_json::{Value, json};
use std::{
    path::Path,
    time::{Duration, Instant},
};

type Error = Box<dyn std::error::Error>;

// Shutdown is explicit even when an observation fails or unwinds.
struct Cleanup(Engine);
impl Drop for Cleanup {
    fn drop(&mut self) {
        self.0.shutdown();
    }
}

fn metrics(value: Metrics) -> Value {
    json!({"font_px": value.font_px, "cell_width": value.cell_width,
           "cell_height": value.cell_height})
}

fn rectangles(layout: &Layout, bounds: Rect) -> Value {
    Value::Array(
        layout
            .rectangles(bounds)
            .into_iter()
            .map(|(id, r)| json!({"id": id, "rect": [r.col, r.row, r.cols, r.rows]}))
            .collect(),
    )
}

fn field(value: &Value, name: &str) -> Result<String, Error> {
    Ok(value
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing string field {name}"))?
        .to_owned())
}

fn observe(config_path: &Path) -> Result<Value, Error> {
    let config = Config::load(Some(config_path))?;
    let mut zoom = Zoom::new(config.font_size_pt);
    let initial = metrics(zoom.metrics(1.0)?);
    zoom.adjust(1.0);
    let increased = metrics(zoom.metrics(1.0)?);
    zoom.reset();
    let reset = metrics(zoom.metrics(1.0)?);
    let dpi = metrics(zoom.metrics(1.5)?);
    let invalid_zero = Config::from_json(r#"{"font_size_pt":0,"columns":80,"rows":24}"#).is_err();

    clanky_core::platform::install_shutdown_signals()?;
    let cleanup = Cleanup(Engine::new());
    let engine = &cleanup.0;
    let mut ids = Vec::new();
    for _ in 0..3 {
        ids.push(field(
            &engine.request(
                "open_tab",
                &json!({
                    "rows": config.rows, "cols": config.columns, "allow_agent": true
                }),
                false,
            )?,
            "tab_id",
        )?);
    }
    let bounds = Rect {
        col: 0,
        row: 0,
        cols: config.columns,
        rows: config.rows,
    };
    let mut layout = Layout::leaf(ids[0].clone());
    layout.split(&ids[0], ids[1].clone(), Axis::Columns, bounds.clone())?;
    let two_panes = rectangles(&layout, bounds.clone());
    layout.split(&ids[1], ids[2].clone(), Axis::Rows, bounds.clone())?;
    let three_panes = rectangles(&layout, bounds.clone());
    let hit = layout.hit_test(bounds.clone(), 41, 13);
    let divider = layout.hit_test(bounds.clone(), 39, 10);
    for (id, rect) in layout.rectangles(bounds.clone()) {
        engine.resize(&id, rect.rows, rect.cols)?;
    }

    let cwd = std::env::current_dir()?;
    let mut commands = Vec::new();
    for (id, script) in ids.iter().zip(["printf A", "printf B", "printf C"]) {
        commands.push(field(
            &engine.request(
                "run",
                &json!({
                    "tab_id": id, "argv": ["sh", "-c", script], "cwd": cwd
                }),
                false,
            )?,
            "command_id",
        )?);
    }
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut statuses = Vec::new();
    let mut outputs = Vec::new();
    for (id, command) in ids.iter().zip(&commands) {
        let status = loop {
            if clanky_core::platform::shutdown_requested() {
                return Err("state harness interrupted".into());
            }
            if Instant::now() >= deadline {
                return Err("PTY completion deadline exceeded".into());
            }
            let status = engine.request(
                "wait",
                &json!({
                    "tab_id": id, "command_id": command, "timeout_ms": 200
                }),
                false,
            )?;
            if status.get("done").and_then(Value::as_bool) == Some(true) {
                break status;
            }
        };
        statuses.push(status);
        outputs.push(engine.request(
            "get_output",
            &json!({
                "tab_id": id, "command_id": command, "offset": 0,
                "limit": 65536, "encoding": "utf8"
            }),
            false,
        )?);
    }
    engine.request("close_tab", &json!({"tab_id": ids[1]}), false)?;
    let removed = layout.remove(&ids[1]);
    let after_close = rectangles(&layout, bounds.clone());
    for (id, rect) in layout.rectangles(bounds) {
        engine.resize(&id, rect.rows, rect.cols)?;
    }
    let surviving_ids = engine.tab_ids();
    Ok(json!({
        "config": {"font_size_pt": config.font_size_pt,
                   "columns": config.columns, "rows": config.rows},
        "metrics": {"initial": initial, "zoom": increased, "reset": reset, "dpi": dpi},
        "invalid_zero": invalid_zero, "ids": ids, "two_panes": two_panes,
        "rectangles": three_panes, "hit": hit, "divider": divider,
        "removed": removed, "after_close": after_close, "surviving_ids": surviving_ids,
        "outputs": outputs, "statuses": statuses,
        "exit_codes": statuses.iter().map(|s| s["exit_code"].clone()).collect::<Vec<_>>()
    }))
}

fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let result = if args.len() == 2 && args[0] == "--config" {
        observe(Path::new(&args[1]))
    } else {
        Err("usage: clanky-state-gate --config FILE".into())
    };
    match result {
        Ok(value) => println!("{value}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
