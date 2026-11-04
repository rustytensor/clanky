//! Role-neutral, bounded RPC client. Dropping a client never closes a session.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use clanky_core::{
    CoreError, TerminalSnapshot, Workspace,
    platform::{self, LocalStream},
};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    io::{self, BufRead, BufReader, Read, Write},
    path::Path,
    process::Child,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
    },
    thread,
    time::Duration,
};

const DEADLINE: Duration = Duration::from_secs(10);
const PIXEL_BUDGET: usize = 64 * 1024 * 1024;
fn fail(code: &str, message: &str) -> CoreError {
    CoreError {
        code: code.into(),
        message: message.into(),
    }
}
fn io_error(e: CoreError) -> io::Error {
    io::Error::new(
        if e.code == "permission_denied" {
            io::ErrorKind::PermissionDenied
        } else {
            io::ErrorKind::Other
        },
        "Client connection failed",
    )
}
struct Call {
    version: u64,
    op: String,
    args: Value,
    deadline: Duration,
    reply: SyncSender<Result<Value, CoreError>>,
}
struct Control {
    socket: Option<LocalStream>,
    child: Mutex<Option<Child>>,
    closed: AtomicBool,
}
impl Control {
    fn close(&self) {
        self.closed.store(true, Ordering::Release);
        if let Some(s) = &self.socket {
            let _ = s.shutdown();
        }
        if let Some(mut child) = self.child.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
#[derive(Default)]
struct Cache {
    snapshots: HashMap<String, (u64, TerminalSnapshot)>,
    pixels: HashMap<u64, Arc<Vec<u8>>>,
}
struct Inner {
    sender: Option<SyncSender<Call>>,
    control: Arc<Control>,
    worker: Option<thread::JoinHandle<()>>,
    cache: Mutex<Cache>,
}
impl Drop for Inner {
    fn drop(&mut self) {
        self.sender.take();
        self.control.close();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
#[derive(Clone)]
pub struct Client(Arc<Inner>);
impl Client {
    pub fn connect_agent(path: &Path) -> io::Result<Self> {
        let stream = LocalStream::connect(path)?;
        stream.set_read_timeout(Some(DEADLINE))?;
        stream.set_write_timeout(Some(DEADLINE))?;
        let writer = stream.try_clone()?;
        let control = Arc::new(Control {
            socket: Some(stream.try_clone()?),
            child: Mutex::new(None),
            closed: AtomicBool::new(false),
        });
        Self::start(Box::new(stream), Box::new(writer), control)
    }
    pub fn connect_owner(path: &Path) -> io::Result<Self> {
        let client = Self::connect_agent(path)?;
        let token = platform::owner_token(path, false)?;
        let hello = client.exchange(3, "hello", &json!({})).map_err(io_error)?;
        if hello["version"] != 3 {
            return Err(io::Error::other("Incompatible server"));
        }
        let result = client
            .exchange(3, "auth", &json!({"token":token}))
            .map_err(io_error)?;
        if result["authenticated"] != true {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Owner authentication denied",
            ));
        }
        Ok(client)
    }
    pub fn from_child(mut child: Child) -> io::Result<Self> {
        let pipes = child.stdout.take().zip(child.stdin.take());
        let Some((reader, writer)) = pipes else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::other("Child pipes unavailable"));
        };
        let control = Arc::new(Control {
            socket: None,
            child: Mutex::new(Some(child)),
            closed: AtomicBool::new(false),
        });
        let client = Self::start(Box::new(reader), Box::new(writer), control)?;
        let hello = client.exchange(3, "hello", &json!({})).map_err(io_error)?;
        if hello["version"] != 3 {
            return Err(io::Error::other("Incompatible pipe server"));
        }
        Ok(client)
    }
    pub fn connect_ssh(host: &str) -> io::Result<Self> {
        Self::from_child(platform::spawn_ssh(host)?)
    }
    fn start(
        reader: Box<dyn Read + Send>,
        writer: Box<dyn Write + Send>,
        control: Arc<Control>,
    ) -> io::Result<Self> {
        let (tx, rx) = mpsc::sync_channel::<Call>(8);
        let worker_control = control.clone();
        let worker = match thread::Builder::new()
            .name("session-io".into())
            .spawn(move || {
                let mut reader = BufReader::new(reader);
                let mut writer = writer;
                let mut sequence = 0u64;
                while let Ok(call) = rx.recv() {
                    if worker_control.closed.load(Ordering::Acquire) {
                        break;
                    }
                    let mut remote_error = false;
                    let result = (|| {
                        if let Some(socket) = &worker_control.socket {
                            socket.set_read_timeout(Some(call.deadline)).map_err(|_| {
                                fail("transport_error", "Cannot set reply deadline")
                            })?;
                        }
                        sequence = sequence
                            .checked_add(1)
                            .ok_or_else(|| fail("transport_error", "Request ids exhausted"))?;
                        let id = sequence.to_string();
                        let mut request = call.args.as_object().cloned().ok_or_else(|| {
                            fail("invalid_request", "Arguments must be an object")
                        })?;
                        request.insert("v".into(), json!(call.version));
                        request.insert("op".into(), json!(call.op));
                        request.insert("id".into(), json!(id));
                        let mut bytes = serde_json::to_vec(&request)
                            .map_err(|_| fail("invalid_request", "Invalid arguments"))?;
                        bytes.push(b'\n');
                        if bytes.len() > super::MAX_LINE {
                            return Err(fail("invalid_request", "Request exceeds 1 MiB"));
                        }
                        writer
                            .write_all(&bytes)
                            .and_then(|_| writer.flush())
                            .map_err(|_| fail("transport_error", "Request write failed"))?;
                        let mut line = Vec::new();
                        loop {
                            let available = reader
                                .fill_buf()
                                .map_err(|_| fail("transport_error", "Reply read failed"))?;
                            if available.is_empty() {
                                return Err(fail("transport_error", "Connection closed"));
                            }
                            let n = available
                                .iter()
                                .position(|b| *b == b'\n')
                                .map_or(available.len(), |n| n + 1);
                            if line.len() + n > super::MAX_REPLY {
                                return Err(fail("transport_error", "Reply exceeds limit"));
                            }
                            let complete = available[n - 1] == b'\n';
                            line.extend_from_slice(&available[..n]);
                            reader.consume(n);
                            if complete {
                                break;
                            }
                        }
                        let reply: Value = serde_json::from_slice(&line)
                            .map_err(|_| fail("transport_error", "Invalid reply"))?;
                        if reply["id"] != id {
                            return Err(fail("transport_error", "Reply id mismatch"));
                        }
                        match reply["ok"].as_bool() {
                            Some(true) => reply
                                .get("result")
                                .cloned()
                                .ok_or_else(|| fail("transport_error", "Missing result")),
                            Some(false) => {
                                remote_error = true;
                                Err(fail(
                                    reply["error"]["code"].as_str().unwrap_or("remote_error"),
                                    reply["error"]["message"]
                                        .as_str()
                                        .unwrap_or("Request denied"),
                                ))
                            }
                            None => Err(fail("transport_error", "Invalid reply status")),
                        }
                    })();
                    let broken = !remote_error
                        && result
                            .as_ref()
                            .err()
                            .is_some_and(|e| e.code == "transport_error");
                    let _ = call.reply.send(result);
                    if broken {
                        break;
                    }
                }
                worker_control.close();
            }) {
            Ok(worker) => worker,
            Err(e) => {
                control.close();
                return Err(e);
            }
        };
        Ok(Self(Arc::new(Inner {
            sender: Some(tx),
            control,
            worker: Some(worker),
            cache: Mutex::new(Cache::default()),
        })))
    }
    pub fn exchange(&self, version: u64, op: &str, args: &Value) -> Result<Value, CoreError> {
        if self.is_closed() {
            return Err(fail("transport_error", "Connection closed"));
        }
        let encoded_size = serde_json::to_vec(args)
            .map_err(|_| fail("invalid_request", "Invalid arguments"))?
            .len();
        if encoded_size.saturating_add(op.len()).saturating_add(128) > super::MAX_LINE {
            return Err(fail("invalid_request", "Request exceeds 1 MiB"));
        }
        let deadline = if (version == 1 && op == "wait")
            || (version == 4 && matches!(op, "wait_input" | "events"))
        {
            args["timeout_ms"]
                .as_u64()
                .filter(|n| *n <= 30_000)
                .map(|n| DEADLINE.max(Duration::from_millis(n + 2_000)))
                .unwrap_or(DEADLINE)
        } else {
            DEADLINE
        };
        let (tx, rx) = mpsc::sync_channel(1);
        self.0
            .sender
            .as_ref()
            .unwrap()
            .try_send(Call {
                version,
                op: op.into(),
                args: args.clone(),
                deadline,
                reply: tx,
            })
            .map_err(|_| fail("transport_error", "Request queue unavailable"))?;
        match rx.recv_timeout(deadline) {
            Ok(result) => result,
            Err(_) => {
                self.0.control.close();
                Err(fail(
                    "transport_error",
                    "Request timed out; connection closed without replay",
                ))
            }
        }
    }
    pub fn request(&self, op: &str, args: &Value, _trusted: bool) -> Result<Value, CoreError> {
        let (v, op) = if let Some(op) = op.strip_prefix("v2.") {
            (2, op)
        } else if let Some(op) = op.strip_prefix("v3.") {
            (3, op)
        } else if let Some(op) = op.strip_prefix("v4.") {
            (4, op)
        } else {
            (1, op)
        };
        let value = self.exchange(v, op, args)?;
        if v == 1 && op == "close_tab" {
            if let Some(tab) = args["tab_id"].as_str() {
                let mut cache = self.0.cache.lock().unwrap();
                cache.snapshots.remove(tab);
                let retained: HashSet<u64> = cache
                    .snapshots
                    .values()
                    .flat_map(|(_, s)| s.images.iter().map(|i| i.image_key))
                    .collect();
                cache.pixels.retain(|key, _| retained.contains(key));
            }
        }
        Ok(value)
    }
    pub fn palette_action(
        &self,
        action: &str,
        args: &Value,
        _trusted: bool,
    ) -> Result<Value, CoreError> {
        self.exchange(2, action, args)
    }
    pub fn is_closed(&self) -> bool {
        self.0.control.closed.load(Ordering::Acquire)
    }
    pub fn input(&self, tab: &str, bytes: &[u8]) -> Result<(), CoreError> {
        if bytes.len() > 64 * 1024 {
            return Err(fail("invalid_request", "Input exceeds 64 KiB"));
        }
        self.exchange(
            3,
            "input",
            &json!({"tab_id":tab,"data":STANDARD.encode(bytes)}),
        )
        .map(|_| ())
    }
    pub fn resize(&self, tab: &str, rows: usize, cols: usize) -> Result<(), CoreError> {
        self.exchange(3, "resize", &json!({"tab_id":tab,"rows":rows,"cols":cols}))
            .map(|_| ())
    }
    pub fn scroll(&self, tab: &str, delta: i32) -> Result<(), CoreError> {
        self.exchange(3, "scroll", &json!({"tab_id":tab,"delta":delta}))
            .map(|_| ())
    }
    pub fn agent_permission(&self, tab: &str) -> Result<bool, CoreError> {
        self.exchange(3, "permission_get", &json!({"tab_id":tab}))?["allow_agent"]
            .as_bool()
            .ok_or_else(|| fail("transport_error", "Invalid permission"))
    }
    pub fn set_agent_permission(&self, tab: &str, allow: bool) -> Result<(), CoreError> {
        self.exchange(
            3,
            "permission_set",
            &json!({"tab_id":tab,"allow_agent":allow}),
        )
        .map(|_| ())
    }
    pub fn tab_ids(&self) -> Result<Vec<String>, CoreError> {
        let workspace = self.workspace_get()?;
        Ok(workspace
            .groups
            .iter()
            .flat_map(|g| g.layout.leaves())
            .collect())
    }
    pub fn workspace_get(&self) -> Result<Workspace, CoreError> {
        serde_json::from_value(self.exchange(3, "workspace_get", &json!({}))?)
            .map_err(|_| fail("transport_error", "Invalid workspace"))
    }
    pub fn workspace_set(&self, workspace: &Workspace) -> Result<Workspace, CoreError> {
        serde_json::from_value(
            self.exchange(
                3,
                "workspace_set",
                &serde_json::to_value(workspace)
                    .map_err(|_| fail("invalid_request", "Invalid workspace"))?,
            )?,
        )
        .map_err(|_| fail("transport_error", "Invalid workspace"))
    }
    pub fn snapshot(&self, tab: &str) -> Result<TerminalSnapshot, CoreError> {
        let mut cache = self.0.cache.lock().unwrap();
        let generation = cache.snapshots.get(tab).map(|(g, _)| *g);
        let known: Vec<u64> = cache.pixels.keys().copied().take(4096).collect();
        let mut args = json!({"tab_id":tab,"known_images":known});
        if let Some(generation) = generation {
            args["known_generation"] = json!(generation);
        }
        let result = self.exchange(3, "snapshot", &args)?;
        let generation = result["generation"]
            .as_u64()
            .ok_or_else(|| fail("transport_error", "Invalid snapshot generation"))?;
        if result["unchanged"] == true {
            return cache
                .snapshots
                .get(tab)
                .filter(|(g, _)| *g == generation)
                .map(|(_, s)| s.clone())
                .ok_or_else(|| fail("transport_error", "Missing cached snapshot"));
        }
        if result["unchanged"] != false {
            return Err(fail("transport_error", "Invalid snapshot status"));
        }
        let mut snapshot: TerminalSnapshot = serde_json::from_value(result["snapshot"].clone())
            .map_err(|_| fail("transport_error", "Invalid snapshot"))?;
        if snapshot.rows == 0
            || snapshot.cols == 0
            || snapshot.rows > 1000
            || snapshot.cols > 1000
            || snapshot
                .rows
                .checked_mul(snapshot.cols)
                .filter(|n| *n <= 250000)
                != Some(snapshot.cells.len())
            || snapshot
                .cursor
                .is_some_and(|(r, c)| r >= snapshot.rows || c >= snapshot.cols)
        {
            return Err(fail("transport_error", "Invalid snapshot dimensions"));
        }
        let pixels = result["pixels"]
            .as_array()
            .ok_or_else(|| fail("transport_error", "Invalid pixels"))?;
        let mut received = HashSet::new();
        let mut incoming = HashMap::new();
        let mut size = 0usize;
        for pixel in pixels {
            let key = pixel["image_key"]
                .as_u64()
                .filter(|k| *k > 0)
                .ok_or_else(|| fail("transport_error", "Invalid image key"))?;
            if !received.insert(key) {
                return Err(fail("transport_error", "Duplicate immutable image key"));
            }
            let bytes = STANDARD
                .decode(
                    pixel["data"]
                        .as_str()
                        .ok_or_else(|| fail("transport_error", "Invalid pixels"))?,
                )
                .map_err(|_| fail("transport_error", "Invalid pixel encoding"))?;
            size = size
                .checked_add(bytes.len())
                .filter(|n| *n <= PIXEL_BUDGET)
                .ok_or_else(|| fail("transport_error", "Image budget exceeded"))?;
            if cache
                .pixels
                .get(&key)
                .is_some_and(|cached| cached.as_slice() != bytes.as_slice())
            {
                return Err(fail("transport_error", "Immutable image key changed"));
            }
            incoming.insert(key, Arc::new(bytes));
        }
        let mut used = HashMap::new();
        for image in &mut snapshot.images {
            let expected = (image.width as usize)
                .checked_mul(image.height as usize)
                .and_then(|n| n.checked_mul(4))
                .filter(|n| *n > 0 && *n <= PIXEL_BUDGET)
                .ok_or_else(|| fail("transport_error", "Invalid image dimensions"))?;
            let bytes = incoming
                .get(&image.image_key)
                .or_else(|| cache.pixels.get(&image.image_key))
                .ok_or_else(|| fail("transport_error", "Missing image pixels"))?;
            if bytes.len() != expected {
                return Err(fail("transport_error", "Invalid RGBA length"));
            }
            image.rgba = bytes.clone();
            used.insert(image.image_key, bytes.clone());
        }
        // Bound both retained headers and unique decoded image buffers. Eviction
        // removes cached generations too, so unchanged replies never need lost pixels.
        if used.values().map(|bytes| bytes.len()).sum::<usize>() > PIXEL_BUDGET {
            return Err(fail("transport_error", "Image budget exceeded"));
        }
        let mut keys = HashMap::new();
        for (_, s) in cache.snapshots.values() {
            for image in &s.images {
                keys.insert(image.image_key, image.rgba.len());
            }
        }
        for (key, bytes) in &used {
            keys.insert(*key, bytes.len());
        }
        if keys.values().sum::<usize>() > PIXEL_BUDGET || cache.snapshots.len() >= 64 {
            cache.snapshots.clear();
            cache.pixels.clear();
        }
        cache
            .snapshots
            .insert(tab.into(), (generation, snapshot.clone()));
        let retained: HashSet<u64> = cache
            .snapshots
            .values()
            .flat_map(|(_, s)| s.images.iter().map(|i| i.image_key))
            .collect();
        cache.pixels.retain(|key, _| retained.contains(key));
        cache.pixels.extend(used);
        Ok(snapshot)
    }
}
