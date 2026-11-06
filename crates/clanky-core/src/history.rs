//! Saved command history: one SQLite file the person can open, search, export and edit.
//!
//! The server owns the only writer, a thread fed by a channel, so the PTY and render
//! paths never wait on SQLite. Everything sent here is already masked; raw terminal
//! text never reaches this module. The command line reads the file directly.
use rusqlite::{Connection, ErrorCode, OpenFlags, OptionalExtension, params};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// `PRAGMA user_version` written by this build. A larger value means a newer Clanky.
pub const SCHEMA_VERSION: i64 = 1;
const DAY_MS: i64 = 86_400_000;
const RETENTION_EVERY: Duration = Duration::from_secs(24 * 60 * 60);
/// JSON keys of one row, in order (also the CSV header).
pub const ROW_KEYS: [&str; 11] = ["id", "session_id", "started_at", "cwd", "actor", "command", "exit_code",
    "duration_ms", "complete", "output", "output_truncated"];

const SCHEMA: &str = "
CREATE TABLE sessions (
  id INTEGER PRIMARY KEY,
  started_at INTEGER NOT NULL,
  ended_at INTEGER,
  host TEXT NOT NULL,
  version TEXT NOT NULL
);
CREATE TABLE commands (
  id INTEGER PRIMARY KEY,
  session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  tab_id TEXT NOT NULL,
  started_at INTEGER NOT NULL,
  cwd TEXT,
  actor TEXT NOT NULL,
  command TEXT,
  exit_code INTEGER,
  duration_ms INTEGER,
  complete INTEGER NOT NULL,
  output TEXT NOT NULL,
  output_truncated INTEGER NOT NULL
);
CREATE TABLE agent_events (
  id INTEGER PRIMARY KEY,
  session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  at INTEGER NOT NULL,
  agent TEXT NOT NULL,
  tab_id TEXT,
  action TEXT NOT NULL,
  detail TEXT NOT NULL,
  result TEXT,                      -- ok or the error code
  -- The command these actions led to; deleting the command deletes them (their text
  -- can hold the command itself).
  command_id INTEGER REFERENCES commands(id) ON DELETE CASCADE
);
CREATE INDEX commands_started ON commands(started_at);
CREATE INDEX commands_session ON commands(session_id);
CREATE INDEX agent_events_at ON agent_events(at);
CREATE INDEX agent_events_command ON agent_events(command_id);
CREATE INDEX agent_events_open ON agent_events(session_id, tab_id) WHERE command_id IS NULL;
CREATE VIRTUAL TABLE commands_fts USING fts5(command, output, content='commands', content_rowid='id');
CREATE TRIGGER commands_fts_insert AFTER INSERT ON commands BEGIN
  INSERT INTO commands_fts(rowid, command, output) VALUES (new.id, new.command, new.output);
END;
CREATE TRIGGER commands_fts_delete AFTER DELETE ON commands BEGIN
  INSERT INTO commands_fts(commands_fts, rowid, command, output) VALUES ('delete', old.id, old.command, old.output);
END;
CREATE TRIGGER commands_fts_update AFTER UPDATE OF command, output ON commands BEGIN
  INSERT INTO commands_fts(commands_fts, rowid, command, output) VALUES ('delete', old.id, old.command, old.output);
  INSERT INTO commands_fts(rowid, command, output) VALUES (new.id, new.command, new.output);
END;
";

/// The three config keys, as the server last read them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub enabled: bool,
    /// 0 keeps everything.
    pub days: u32,
    pub output_kib: u32,
}
impl Default for Settings {
    fn default() -> Self { Self { enabled: true, days: 90, output_kib: 256 } }
}

/// One sealed command block, masked and capped before it is sent.
pub struct Record {
    pub tab_id: String,
    pub started_at: i64,
    pub cwd: Option<String>,
    pub actor: String,
    pub command: Option<String>,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
    pub complete: bool,
    pub output: String,
    pub output_truncated: bool,
}

/// One audited agent request, its arguments already masked.
pub struct AgentEvent {
    pub at: i64,
    pub agent: String,
    pub tab_id: Option<String>,
    pub action: String,
    pub detail: String,
    pub result: String,
}

enum Message {
    Command(Record),
    Event(AgentEvent),
    Settings(Settings),
    Finish,
}

struct Shared {
    sender: Mutex<Option<Sender<Message>>>,
    writer: Mutex<Option<JoinHandle<()>>>,
    enabled: AtomicBool,
    cap: AtomicUsize,
}

/// The server's handle to the writer thread. Cheap to clone; sends never block.
#[derive(Clone)]
pub struct History(Arc<Shared>);

impl History {
    /// Start the writer. `path` None (no data directory) records nothing.
    /// `notice` receives at most one line per problem the person should know about.
    pub fn start(path: Option<PathBuf>, settings: Settings, notice: Box<dyn Fn(String) + Send>) -> Self {
        let (sender, receiver) = mpsc::channel();
        let shared = Arc::new(Shared {
            sender: Mutex::new(Some(sender)),
            writer: Mutex::new(None),
            enabled: AtomicBool::new(settings.enabled && path.is_some()),
            cap: AtomicUsize::new(settings.output_kib as usize * 1024),
        });
        let handle = thread::Builder::new().name("clanky-history".into())
            .spawn(move || Writer::new(path, settings, notice).run(receiver))
            .ok();
        *shared.writer.lock().unwrap() = handle;
        Self(shared)
    }
    /// False when history is off; callers then skip masking work entirely.
    pub fn enabled(&self) -> bool { self.0.enabled.load(Ordering::Acquire) }
    /// Stored output bytes per command.
    pub fn output_cap(&self) -> usize { self.0.cap.load(Ordering::Acquire) }
    pub fn configure(&self, settings: Settings) {
        self.0.cap.store(settings.output_kib as usize * 1024, Ordering::Release);
        self.0.enabled.store(settings.enabled, Ordering::Release);
        self.send(Message::Settings(settings));
    }
    pub fn command(&self, record: Record) {
        if self.enabled() { self.send(Message::Command(record)); }
    }
    pub fn agent_event(&self, event: AgentEvent) {
        if self.enabled() { self.send(Message::Event(event)); }
    }
    /// Write everything sent so far, end the session and checkpoint the WAL.
    /// Later sends are dropped.
    pub fn finish(&self) {
        self.0.enabled.store(false, Ordering::Release);
        if let Some(sender) = self.0.sender.lock().unwrap().take() {
            let _ = sender.send(Message::Finish);
        }
        if let Some(handle) = self.0.writer.lock().unwrap().take() {
            let _ = handle.join();
        }
    }
    fn send(&self, message: Message) {
        if let Some(sender) = &*self.0.sender.lock().unwrap() {
            let _ = sender.send(message);
        }
    }
}

/// Cut masked output to `cap` bytes on a UTF-8 boundary; true when anything was cut.
pub fn cap_output(output: &mut String, cap: usize) -> bool {
    if output.len() <= cap { return false; }
    let mut end = cap;
    while !output.is_char_boundary(end) { end -= 1; }
    output.truncate(end);
    true
}

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis().min(i64::MAX as u128) as i64)
}

enum State {
    Closed,
    Open { connection: Connection, session: i64 },
    /// Not writable this run (a newer file, or a file that could not be opened).
    Unusable,
}

struct Writer {
    path: Option<PathBuf>,
    settings: Settings,
    notice: Box<dyn Fn(String) + Send>,
    state: State,
    next_retention: Instant,
}

impl Writer {
    fn new(path: Option<PathBuf>, settings: Settings, notice: Box<dyn Fn(String) + Send>) -> Self {
        Self { path, settings, notice, state: State::Closed, next_retention: Instant::now() }
    }
    fn run(mut self, receiver: Receiver<Message>) {
        if self.settings.enabled { self.open(); }
        loop {
            let wait = self.next_retention.saturating_duration_since(Instant::now());
            let first = match receiver.recv_timeout(wait) {
                Ok(message) => message,
                Err(RecvTimeoutError::Timeout) => { self.retention(); continue; }
                Err(RecvTimeoutError::Disconnected) => break,
            };
            // One transaction per burst: hundreds of short commands cost one commit.
            let mut batch = vec![first];
            while batch.len() < 4096 {
                match receiver.try_recv() { Ok(message) => batch.push(message), Err(_) => break }
            }
            let finish = self.write(batch);
            if finish { break; }
            if Instant::now() >= self.next_retention { self.retention(); }
        }
        self.close();
    }
    /// Returns true after a Finish message.
    fn write(&mut self, batch: Vec<Message>) -> bool {
        let mut finish = false;
        let mut pending = Vec::new();
        for message in batch {
            match message {
                Message::Settings(settings) => {
                    self.flush(&mut pending);
                    self.apply(settings);
                }
                Message::Finish => { finish = true; break; }
                other => pending.push(other),
            }
        }
        self.flush(&mut pending);
        finish
    }
    fn flush(&mut self, pending: &mut Vec<Message>) {
        if pending.is_empty() { return; }
        let messages = std::mem::take(pending);
        let State::Open { connection, session } = &mut self.state else { return; };
        let session = *session;
        let result = (|| -> rusqlite::Result<()> {
            let tx = connection.transaction()?;
            for message in messages {
                match message {
                    Message::Command(r) => {
                        tx.prepare_cached("INSERT INTO commands (session_id, tab_id, started_at, cwd, actor, command, exit_code,
                            duration_ms, complete, output, output_truncated) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)")?
                            .execute(params![session, r.tab_id, r.started_at, r.cwd, r.actor, r.command, r.exit_code,
                                r.duration_ms.map(|d| d.min(i64::MAX as u64) as i64), r.complete, r.output, r.output_truncated])?;
                        let id = tx.last_insert_rowid();
                        tx.prepare_cached("UPDATE agent_events SET command_id = ?1
                            WHERE session_id = ?2 AND tab_id = ?3 AND command_id IS NULL")?
                            .execute(params![id, session, r.tab_id])?;
                    }
                    Message::Event(e) => {
                        tx.prepare_cached("INSERT INTO agent_events (session_id, at, agent, tab_id, action, detail, result)
                            VALUES (?1,?2,?3,?4,?5,?6,?7)")?
                            .execute(params![session, e.at, e.agent, e.tab_id, e.action, e.detail, e.result])?;
                    }
                    Message::Settings(_) | Message::Finish => (),
                }
            }
            tx.commit()
        })();
        if let Err(error) = result {
            eprintln!("clanky: saved history write failed: {error}");
        }
    }
    fn apply(&mut self, settings: Settings) {
        let old = std::mem::replace(&mut self.settings, settings);
        match (old.enabled, settings.enabled) {
            (true, false) => self.close(),
            (false, true) => self.open(),
            _ if old.days != settings.days => self.retention(),
            _ => (),
        }
    }
    fn open(&mut self) {
        if !matches!(self.state, State::Closed) { return; }
        let Some(path) = self.path.clone() else { return; };
        self.state = match open_writer(&path) {
            Ok(Opened::Ready(connection)) => {
                let session = connection.execute("INSERT INTO sessions (started_at, host, version) VALUES (?1, ?2, ?3)",
                    params![now_ms(), crate::platform::host_name(), env!("CARGO_PKG_VERSION")])
                    .map(|_| connection.last_insert_rowid());
                match session {
                    Ok(session) => State::Open { connection, session },
                    Err(error) => { (self.notice)(format!("Saved history is off for this run: {error}")); State::Unusable }
                }
            }
            Ok(Opened::Newer) => {
                (self.notice)("Saved history was written by a newer Clanky; this run leaves it untouched and saves nothing.".into());
                State::Unusable
            }
            Ok(Opened::Replaced(kept)) => {
                (self.notice)(format!("Saved history was unreadable; it was kept as {} and a new file was started.",
                    kept.file_name().map_or_else(|| kept.display().to_string(), |n| n.to_string_lossy().into_owned())));
                // The fresh file needs its session too.
                self.state = State::Closed;
                return self.open();
            }
            Err(error) => {
                (self.notice)(format!("Saved history is off for this run: {error}"));
                State::Unusable
            }
        };
        self.next_retention = Instant::now();
        self.retention();
    }
    fn close(&mut self) {
        if let State::Open { connection, session } = std::mem::replace(&mut self.state, State::Closed) {
            let _ = connection.execute("UPDATE sessions SET ended_at = ?1 WHERE id = ?2", params![now_ms(), session]);
            let _ = checkpoint(&connection);
        }
    }
    fn retention(&mut self) {
        self.next_retention = Instant::now() + RETENTION_EVERY;
        let State::Open { connection, .. } = &self.state else { return; };
        if self.settings.days == 0 { return; }
        let before = now_ms().saturating_sub(i64::from(self.settings.days) * DAY_MS);
        if let Err(error) = delete_before(connection, before) {
            eprintln!("clanky: saved history retention failed: {error}");
        }
    }
}

enum Opened {
    Ready(Connection),
    Newer,
    /// The unreadable file was renamed to this path; open again.
    Replaced(PathBuf),
}

fn open_writer(path: &Path) -> Result<Opened, String> {
    crate::platform::prepare_history_file(path).map_err(|e| format!("cannot create {}: {e}", path.display()))?;
    // Read-only first: a newer or broken file must not be changed by looking at it.
    let version = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .and_then(|c| {
            c.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))?;
            c.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
        });
    let version = match version {
        Ok(version) => version,
        Err(rusqlite::Error::SqliteFailure(e, _)) if matches!(e.code, ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt) => {
            let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
            let mut kept = path.as_os_str().to_owned();
            kept.push(format!(".broken-{secs}"));
            let kept = PathBuf::from(kept);
            std::fs::rename(path, &kept).map_err(|e| format!("cannot set aside the unreadable file: {e}"))?;
            return Ok(Opened::Replaced(kept));
        }
        Err(e) => return Err(e.to_string()),
    };
    if version > SCHEMA_VERSION { return Ok(Opened::Newer); }
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(|e| e.to_string())?;
    configure(&connection).map_err(|e| e.to_string())?;
    if version < SCHEMA_VERSION {
        let tx = connection.unchecked_transaction().map_err(|e| e.to_string())?;
        tx.execute_batch(SCHEMA).map_err(|e| e.to_string())?;
        tx.pragma_update(None, "user_version", SCHEMA_VERSION).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
    }
    Ok(Opened::Ready(connection))
}

fn configure(connection: &Connection) -> rusqlite::Result<()> {
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))?;
    connection.pragma_update(None, "secure_delete", "ON")?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    Ok(())
}

/// Delete rows started before `before` (Unix ms) and the agent actions and finished
/// sessions from before then. Deleted text is overwritten and the WAL emptied.
fn delete_before(connection: &Connection, before: i64) -> rusqlite::Result<usize> {
    let tx = connection.unchecked_transaction()?;
    let removed = tx.execute("DELETE FROM commands WHERE started_at < ?1", [before])?;
    tx.execute("DELETE FROM agent_events WHERE at < ?1", [before])?;
    tx.execute("DELETE FROM sessions WHERE ended_at IS NOT NULL AND ended_at < ?1
        AND NOT EXISTS (SELECT 1 FROM commands WHERE session_id = sessions.id)", [before])?;
    tx.commit()?;
    if removed > 0 { purge(connection)?; }
    Ok(removed)
}

/// Rewrite the search index without deleted rows, then empty the WAL, so deleted
/// text is left neither in the index pages nor in the log.
fn purge(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute("INSERT INTO commands_fts(commands_fts) VALUES ('optimize')", [])?;
    checkpoint(connection)
}

fn checkpoint(connection: &Connection) -> rusqlite::Result<()> {
    connection.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
}

/// Direct file access for `clanky history`; no server needed.
pub struct Store {
    connection: Option<Connection>,
}

impl Store {
    /// A missing file reads as empty and is not created.
    pub fn open(path: &Path) -> Result<Self, String> {
        if !path.exists() { return Ok(Self { connection: None }); }
        let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX)
            .map_err(|e| format!("cannot open {}: {e}", path.display()))?;
        connection.busy_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
        let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|e| format!("{} is not a readable history file: {e}", path.display()))?;
        if version == 0 { return Ok(Self { connection: None }); }
        Ok(Self { connection: Some(connection) })
    }
    fn writable(&self) -> Result<Option<&Connection>, String> {
        let Some(connection) = &self.connection else { return Ok(None) };
        let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0)).map_err(|e| e.to_string())?;
        if version > SCHEMA_VERSION {
            return Err("this history file was written by a newer Clanky; it is not changed".into());
        }
        connection.pragma_update(None, "secure_delete", "ON").map_err(|e| e.to_string())?;
        connection.pragma_update(None, "foreign_keys", "ON").map_err(|e| e.to_string())?;
        Ok(Some(connection))
    }
    /// Newest `limit` rows (all when None), returned oldest first.
    pub fn list(&self, limit: Option<u64>) -> Result<Vec<Value>, String> {
        self.rows("", &[], limit)
    }
    /// Full-text match on command and output, as one phrase.
    pub fn search(&self, text: &str, limit: Option<u64>) -> Result<Vec<Value>, String> {
        let phrase = format!("\"{}\"", text.replace('"', "\"\""));
        self.rows("WHERE id IN (SELECT rowid FROM commands_fts WHERE commands_fts MATCH ?)", &[&phrase], limit)
    }
    pub fn show(&self, id: i64) -> Result<Option<Value>, String> {
        let Some(connection) = &self.connection else { return Ok(None) };
        connection.query_row(&format!("SELECT {} FROM commands WHERE id = ?1", ROW_KEYS.join(", ")), [id], row_value)
            .optional().map_err(|e| e.to_string())
    }
    fn rows(&self, filter: &str, args: &[&dyn rusqlite::ToSql], limit: Option<u64>) -> Result<Vec<Value>, String> {
        let Some(connection) = &self.connection else { return Ok(Vec::new()) };
        let limit = limit.map_or(-1, |n| n.min(i64::MAX as u64) as i64);
        let sql = format!("SELECT * FROM (SELECT {} FROM commands {filter} ORDER BY started_at DESC, id DESC LIMIT {limit})
            ORDER BY started_at, id", ROW_KEYS.join(", "));
        let mut statement = connection.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = statement.query_map(args, row_value).map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
    }
    /// Returns how many of `ids` existed.
    pub fn delete(&self, ids: &[i64]) -> Result<usize, String> {
        let Some(connection) = self.writable()? else { return Ok(0) };
        let tx = connection.unchecked_transaction().map_err(|e| e.to_string())?;
        let mut removed = 0;
        for id in ids {
            removed += tx.execute("DELETE FROM commands WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        purge(connection).map_err(|e| e.to_string())?;
        Ok(removed)
    }
    /// Everything, or what started before `before` (Unix ms).
    pub fn clear(&self, before: Option<i64>) -> Result<usize, String> {
        let Some(connection) = self.writable()? else { return Ok(0) };
        let removed = delete_before(connection, before.unwrap_or(i64::MAX)).map_err(|e| e.to_string())?;
        purge(connection).map_err(|e| e.to_string())?;
        Ok(removed)
    }
}

fn row_value(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": row.get::<_, i64>(0)?,
        "session_id": row.get::<_, i64>(1)?,
        "started_at": row.get::<_, i64>(2)?,
        "cwd": row.get::<_, Option<String>>(3)?,
        "actor": row.get::<_, String>(4)?,
        "command": row.get::<_, Option<String>>(5)?,
        "exit_code": row.get::<_, Option<i64>>(6)?,
        "duration_ms": row.get::<_, Option<i64>>(7)?,
        "complete": row.get::<_, i64>(8)? != 0,
        "output": row.get::<_, String>(9)?,
        "output_truncated": row.get::<_, i64>(10)? != 0,
    }))
}
