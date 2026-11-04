//! Private-pipe RPC fixture for the M5 gate. Never exports an owner capability.
use clanky_api::Client;
use serde_json::{Value, json};
use std::{
    env,
    io::{self, BufRead, Write},
    path::Path,
};

fn serve() -> Result<(), ()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        return Err(());
    }
    let c = match args[1].as_str() {
        "owner" => Client::connect_owner(Path::new(&args[2])),
        "agent" => Client::connect_agent(Path::new(&args[2])),
        _ => return Err(()),
    }
    .map_err(|_| ())?;
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    loop {
        let mut frame = Vec::new();
        use std::io::Read;
        let n = input
            .by_ref()
            .take(1024 * 1024 + 1)
            .read_until(b'\n', &mut frame)
            .map_err(|_| ())?;
        if n == 0 {
            break;
        }
        if frame.len() > 1024 * 1024 || frame.last() != Some(&b'\n') {
            return Err(());
        }
        let q: Value = serde_json::from_slice(&frame).map_err(|_| ())?;
        let op = q["op"].as_str().ok_or(())?;
        let result = c.exchange(q["v"].as_u64().ok_or(())?, op, &q["args"]);
        let reply = match result {
            Ok(value) => json!({"ok":true,"result":value}),
            Err(e) => json!({"ok":false,"code":e.code}),
        };
        serde_json::to_writer(&mut output, &reply).map_err(|_| ())?;
        output.write_all(b"\n").map_err(|_| ())?;
        output.flush().map_err(|_| ())?;
    }
    Ok(())
}
fn main() {
    if serve().is_err() {
        eprintln!("FAIL: agent gate helper transport");
        std::process::exit(1);
    }
}
