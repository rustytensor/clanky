//! Real process/session fixture for the fixed M4 gate.
use clanky::session;
use clanky_api::Client;
use clanky_core::platform;
use serde_json::{Value, json};
use std::{
    env, fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

fn call(c: &Client, v: u64, op: &str, args: Value) -> Result<Value, String> {
    c.exchange(v, op, &args)
        .map_err(|e| format!("{op}: {}", e.code))
}
fn text(v: &Value, key: &str) -> Result<String, String> {
    v[key]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("missing {key}"))
}
fn private_json(path: &Path, value: &Value) -> Result<(), String> {
    let pending = path.with_extension("pending");
    let mut file =
        platform::create_private_file(&pending).map_err(|_| "fixture file creation failed")?;
    serde_json::to_writer(&mut file, value)
        .map_err(|_| "fixture serialization failed".to_owned())?;
    file.flush()
        .map_err(|_| "fixture flush failed".to_owned())?;
    drop(file);
    fs::rename(pending, path).map_err(|_| "fixture publication failed".to_owned())
}
fn wait(c: &Client, tab: &str, command: &str) -> Result<Value, String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let s = call(
            c,
            1,
            "wait",
            json!({"tab_id":tab,"command_id":command,"timeout_ms":100}),
        )?;
        if s["done"] == true {
            return Ok(s);
        }
    }
    Err("command deadline".into())
}
fn raw(c: &Client, tab: &str, command: &str) -> Result<Value, String> {
    call(
        c,
        1,
        "get_output",
        json!({"tab_id":tab,"command_id":command,"offset":0,"limit":65536,"encoding":"base64"}),
    )
}
fn screen(c: &Client, tab: &str) -> Result<Value, String> {
    let s = c.snapshot(tab).map_err(|e| e.code)?;
    let lines: Vec<String> = s
        .cells
        .chunks(s.cols)
        .map(|r| {
            r.iter()
                .map(|c| c.ch)
                .collect::<String>()
                .trim_end_matches(' ')
                .to_owned()
        })
        .collect();
    Ok(json!({"rows":s.rows,"cols":s.cols,"lines":lines,"cursor":s.cursor}))
}
fn quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}
fn probe(socket: &Path, dir: &Path, tab: &str, command: &str) -> Result<(), String> {
    // This process is a child of B, not an outside privileged helper. Read the
    // actual capability through the platform API; never serialize it to a file.
    let token = platform::owner_token(socket, false).map_err(|_| "probe token unavailable")?;
    let app = env::current_exe()
        .map_err(|_| "probe executable")?
        .with_file_name("clanky");
    let child = platform::spawn_pipe(
        app.as_os_str(),
        &[
            "--stdio".into(),
            "--socket".into(),
            socket.as_os_str().into(),
        ],
    )
    .map_err(|_| "probe bridge spawn")?;
    let c = Client::from_child(child).map_err(|_| "probe bridge unavailable")?;
    let auth = c.exchange(3, "auth", &json!({"token":token}));
    let read = c.exchange(
        1,
        "get_output",
        &json!({"tab_id":tab,"command_id":command,"offset":0,"limit":64,"encoding":"base64"}),
    );
    // Only fixed error codes and booleans leave this process. In particular no
    // reply message, request or credential can reach the shell transcript.
    let denied = |r: &Result<Value, clanky_core::CoreError>| matches!(r, Err(e) if e.code == "permission_denied");
    let a = denied(&auth);
    let r = denied(&read);
    private_json(
        &dir.join("probe.json"),
        &json!({"auth_code":if a {"permission_denied"} else {"unexpected"},
        "read_code":if r {"permission_denied"} else {"unexpected"},"auth_denied":a,"read_denied":r,"bridge_continued":r}),
    )
}
fn seed(socket: &Path, dir: &Path) -> Result<Value, String> {
    let c = session::connect_or_start(socket)?;
    let hello = call(&c, 3, "hello", json!({}))?;
    private_json(&dir.join("server.json"), &hello)?;
    let a = text(
        &call(
            &c,
            1,
            "open_tab",
            json!({"rows":3,"cols":20,"allow_agent":true}),
        )?,
        "tab_id",
    )?;
    let ca = text(
        &call(
            &c,
            1,
            "run",
            json!({"tab_id":a,"argv":["sh",dir.join("a.sh")],"cwd":dir}),
        )?,
        "command_id",
    )?;
    let status_a = wait(&c, &a, &ca)?;
    c.set_agent_permission(&a, false).map_err(|e| e.code)?;
    let b = text(
        &call(
            &c,
            1,
            "open_tab",
            json!({"rows":3,"cols":20,"allow_agent":true}),
        )?,
        "tab_id",
    )?;
    let helper = env::current_exe().map_err(|_| "helper path")?;
    let script = format!(
        "umask 077\nprintf '%s' \"$$\" > {}\nprintf '\\033]133;C\\007start\\n'\n{} --probe --socket {} --dir {} --tab '{}' --command '{}' > {} 2>&1\nwhile [ ! -f {} ]; do sleep 0.02; done\nprintf 'done\\n\\033]133;D;7\\007'\nexit 7\n",
        quote(&dir.join("b.pid")),
        quote(&helper),
        quote(socket),
        quote(dir),
        a,
        ca,
        quote(&dir.join("probe.log")),
        quote(&dir.join("release"))
    );
    fs::write(dir.join("b.sh"), script).map_err(|_| "B script write")?;
    let cb = text(
        &call(
            &c,
            1,
            "run",
            json!({"tab_id":b,"argv":["sh",dir.join("b.sh")],"cwd":dir}),
        )?,
        "command_id",
    )?;
    let deadline = Instant::now() + Duration::from_secs(10);
    while !dir.join("probe.json").exists() {
        if Instant::now() >= deadline {
            return Err("inside-B probe deadline".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let probe: Value =
        serde_json::from_slice(&fs::read(dir.join("probe.json")).map_err(|_| "probe read")?)
            .map_err(|_| "probe JSON")?;
    let mut workspace = serde_json::to_value(c.workspace_get().map_err(|e| e.code)?)
        .map_err(|_| "workspace encode")?;
    workspace["groups"] = json!([{"layout":{"Split":{"axis":"Columns","first":{"Leaf":a},"second":{"Leaf":b}}},"focus":b}]);
    workspace["active"] = json!(0);
    let workspace = call(&c, 3, "workspace_set", workspace)?;
    let view = call(&c, 2, "view", json!({"tab_id":a}))?;
    let block = text(&view["blocks"][0], "block_id")?;
    call(
        &c,
        2,
        "select",
        json!({"tab_id":a,"block_id":block,"start":0,"end":3}),
    )?;
    call(
        &c,
        2,
        "collapse",
        json!({"tab_id":a,"block_id":block,"collapsed":true}),
    )?;
    let task = call(
        &c,
        2,
        "send",
        json!({"tab_id":a,"block_id":block,"label":"Persist"}),
    )?;
    c.scroll(&a, i32::MAX).map_err(|e| e.code)?;
    let o = json!({"hello":hello,"a":a,"b":b,"ca":ca,"cb":cb,"status_a":status_a,
        "status_b":call(&c,1,"wait",json!({"tab_id":b,"command_id":cb,"timeout_ms":1}))?,
        "raw_a":raw(&c,&a,&ca)?,"raw_b":raw(&c,&b,&cb)?,"workspace":workspace,"probe":probe,
        "view_a":call(&c,2,"view",json!({"tab_id":a}))?,
        "view_b":call(&c,2,"view",json!({"tab_id":b}))?,"task":task,
        "history":call(&c,3,"history",json!({"tab_id":a}))?,"screen":screen(&c,&a)?,
        "permission_a":c.agent_permission(&a).map_err(|e|e.code)?,"permission_b":c.agent_permission(&b).map_err(|e|e.code)?});
    println!("{o}");
    io::stdout().flush().map_err(|_| "seed stdout")?;
    // Keep the real owner connection alive until Python kills this process.
    let mut byte = [0u8];
    let _ = io::stdin().read(&mut byte);
    drop(c);
    Ok(Value::Null)
}
fn inspect(socket: &Path, dir: &Path) -> Result<Value, String> {
    let seed: Value =
        serde_json::from_slice(&fs::read(dir.join("seed.json")).map_err(|_| "seed read")?)
            .map_err(|_| "seed JSON")?;
    let a = text(&seed, "a")?;
    let b = text(&seed, "b")?;
    let ca = text(&seed, "ca")?;
    let cb = text(&seed, "cb")?;
    let c = session::connect_pipe(socket)?;
    let agent = Client::connect_agent(socket).map_err(|_| "agent connection")?;
    let denied = matches!(agent.exchange(1,"get_output",&json!({"tab_id":a,"command_id":ca,"offset":0,"limit":64,"encoding":"base64"})), Err(e) if e.code == "permission_denied");
    drop(agent);
    let o = json!({"hello":call(&c,3,"hello",json!({}))?,"tabs":c.tab_ids().map_err(|e|e.code)?,
        "workspace":c.workspace_get().map_err(|e|e.code)?,"agent_a_denied":denied,
        "raw_a":raw(&c,&a,&ca)?,"raw_b":raw(&c,&b,&cb)?,
        "status_a":wait(&c,&a,&ca)?,"status_b":wait(&c,&b,&cb)?,
        "view_a":call(&c,2,"view",json!({"tab_id":a}))?,"view_b":call(&c,2,"view",json!({"tab_id":b}))?,
        "task":call(&c,2,"task_get",json!({"tab_id":a,"task_id":seed["task"]["task_id"]}))?,
        "history":call(&c,3,"history",json!({"tab_id":a}))?,"screen":screen(&c,&a)?,
        "permission_a":c.agent_permission(&a).map_err(|e|e.code)?,"permission_b":c.agent_permission(&b).map_err(|e|e.code)?});
    call(&c, 1, "close_tab", json!({"tab_id":a}))?;
    call(&c, 1, "close_tab", json!({"tab_id":b}))?;
    Ok(o)
}
fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    // Session factories launch current_exe. Delegate those real CLI modes to
    // the captured app without inserting another process into its ancestry.
    if args
        .first()
        .is_some_and(|s| s == "--server" || s == "--stdio")
    {
        let app = env::current_exe()
            .map_err(|_| "app path")?
            .with_file_name("clanky");
        if args[0] == "--server" {
            let socket = args
                .windows(2)
                .find(|w| w[0] == "--socket")
                .ok_or("server socket argument")?;
            let parent = Path::new(&socket[1])
                .parent()
                .ok_or("server socket parent")?;
            private_json(
                &parent.join("launcher.json"),
                &json!({"server_pid":std::process::id()}),
            )?;
        }
        let _ = platform::exec_program(&app, &args);
        return Err("app exec failed".into());
    }
    let arg = |name: &str| -> Result<String, String> {
        args.windows(2)
            .find(|w| w[0] == name)
            .map(|w| w[1].clone())
            .ok_or_else(|| format!("missing {name}"))
    };
    let socket = PathBuf::from(arg("--socket")?);
    let dir = PathBuf::from(arg("--dir")?);
    match args.first().map(String::as_str) {
        Some("--seed") => {
            seed(&socket, &dir)?;
        }
        Some("--inspect") => println!("{}", inspect(&socket, &dir)?),
        Some("--probe") => probe(&socket, &dir, &arg("--tab")?, &arg("--command")?)?,
        _ => return Err("unknown helper mode".into()),
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        // Helpers return static stage labels or protocol error codes, never
        // credential bytes, requests or unfiltered auth response messages.
        eprintln!("session fixture failed: {error}");
        std::process::exit(1);
    }
}
