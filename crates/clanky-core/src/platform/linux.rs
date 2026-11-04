use clanky_vt::{
    event::{OnResize, WindowSize},
    tty::{self, ChildEvent, EventedPty, EventedReadWrite},
};
use std::{
    ffi::{CString, OsStr, OsString},
    fs::{self, File},
    io::{self, Read, Seek, Write},
    net::Shutdown,
    os::{
        fd::AsRawFd,
        unix::{
            ffi::OsStrExt,
            fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
            net::{UnixListener, UnixStream},
            process::{CommandExt, ExitStatusExt},
        },
    },
    path::{Path, PathBuf},
    process::{Child, Command as HelperCommand, Stdio},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};

/// Owns the foreground Linux clipboard helper until replacement or shutdown.
#[derive(Default)]
pub struct Clipboard {
    child: Option<Child>,
}

impl Clipboard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Copies at most 1 MiB without writing into a potentially blocked pipe.
    /// Success means the helper was spawned, not that the display accepted it.
    pub fn copy(&mut self, bytes: &[u8]) -> io::Result<()> {
        if bytes.len() > 1024 * 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "clipboard input exceeds 1 MiB",
            ));
        }
        let mut command = if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            let mut command = HelperCommand::new("wl-copy");
            command.args(["--foreground", "--type", "text/plain;charset=utf-8"]);
            command
        } else {
            let mut command = HelperCommand::new("xclip");
            command.args(["-selection", "clipboard", "-in", "-quiet"]);
            command
        };

        // Anonymous RAM-backed input: no clipboard bytes touch a named disk file.
        use std::os::fd::FromRawFd;
        let name = c"clanky-clipboard";
        let fd = unsafe { libc::memfd_create(name.as_ptr(), libc::MFD_CLOEXEC) };
        if fd < 0 { return Err(io::Error::other("could not create clipboard memory input")); }
        let mut input = unsafe { File::from_raw_fd(fd) };
        input.write_all(bytes)?;
        input.rewind()?;

        self.stop()?;
        self.child = Some(
            command
                .stdin(Stdio::from(input))
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|error| {
                    io::Error::new(
                        error.kind(),
                        "clipboard helper unavailable",
                    )
                })?,
        );
        Ok(())
    }

    fn stop(&mut self) -> io::Result<()> {
        if let Some(child) = self.child.as_mut() {
            if child.try_wait()?.is_none() {
                child.kill()?;
                child.wait()?;
            }
            self.child = None;
        }
        Ok(())
    }
}

impl Drop for Clipboard {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Read only the clipboard selection with a bounded capture and helper lifetime.
/// Helpers' stderr and clipboard bytes are never used in diagnostics.
pub fn clipboard_read() -> io::Result<Vec<u8>> {
    const LIMIT: usize = 64 * 1024;
    let mut command = if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        let mut command = HelperCommand::new("wl-paste");
        command.args(["--no-newline", "--type", "text/plain;charset=utf-8"]);
        command
    } else {
        let mut command = HelperCommand::new("xclip");
        command.args(["-selection", "clipboard", "-out"]);
        command
    };
    let mut child = command.stdin(Stdio::null()).stdout(Stdio::piped())
        .stderr(Stdio::null()).spawn().map_err(|_| io::Error::other("clipboard read helper unavailable"))?;
    let result = (|| {
        let mut output = child.stdout.take().ok_or_else(|| io::Error::other("clipboard capture unavailable"))?;
        let fd = output.as_raw_fd();
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(io::Error::other("clipboard capture unavailable"));
        }
        let deadline = Instant::now() + Duration::from_millis(750);
        let mut bytes = Vec::new();
        let mut buffer = [0u8; 4096];
        let mut eof = false;
        loop {
            if Instant::now() >= deadline { return Err(io::Error::new(io::ErrorKind::TimedOut, "clipboard read timed out")); }
            if !eof {
                // At the bound probe one byte, but never retain an oversized capture.
                let count = (LIMIT - bytes.len()).clamp(1, buffer.len());
                match output.read(&mut buffer[..count]) {
                    Ok(0) => eof = true,
                    Ok(n) => {
                        if bytes.len() + n > LIMIT { return Err(io::Error::other("clipboard read exceeds 64 KiB")); }
                        bytes.extend_from_slice(&buffer[..n]);
                        continue;
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => (),
                    Err(_) => return Err(io::Error::other("clipboard capture failed")),
                }
            }
            if eof {
                if let Some(status) = child.try_wait().map_err(|_| io::Error::other("clipboard helper status unavailable"))? {
                    return if status.success() { Ok(bytes) } else { Err(io::Error::other("clipboard read helper failed")) };
                }
            }
            thread::sleep(Duration::from_millis(5));
        }
    })();
    // Only this owned PID is stopped/reaped, including capture failures/timeouts.
    if !matches!(child.try_wait(), Ok(Some(_))) {
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}

static SHUTDOWN: AtomicBool = AtomicBool::new(false);
extern "C" fn signal_shutdown(_: libc::c_int) {
    SHUTDOWN.store(true, Ordering::Relaxed);
}
pub fn install_shutdown_signals() -> io::Result<()> {
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = signal_shutdown as *const () as usize;
        libc::sigemptyset(&mut action.sa_mask);
        for sig in [libc::SIGINT, libc::SIGTERM] {
            if libc::sigaction(sig, &action, std::ptr::null_mut()) < 0 {
                return Err(io::Error::last_os_error());
            }
        }
    }
    Ok(())
}
pub fn shutdown_requested() -> bool {
    SHUTDOWN.load(Ordering::Relaxed)
}
fn uid() -> u32 {
    unsafe { libc::geteuid() }
}
pub struct LocalListener {
    inner: UnixListener,
    path: PathBuf,
    dev: u64,
    ino: u64,
}
pub struct LocalStream(UnixStream, Option<ProcessIdentity>);
impl LocalListener {
    pub fn bind(path: &Path) -> io::Result<Self> {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or_else(|| io::Error::other("socket requires private parent directory"))?;
        match fs::symlink_metadata(parent) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let mut b = fs::DirBuilder::new();
                b.mode(0o700);
                b.create(parent)?;
            }
            Err(e) => return Err(e),
            _ => (),
        }
        let meta = fs::symlink_metadata(parent)?;
        if !meta.is_dir() || meta.uid() != uid() || meta.mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "socket parent must be owned and private",
            ));
        }
        // Never remove a pre-existing socket, even if apparently stale.
        let inner = UnixListener::bind(path)?;
        let meta = fs::symlink_metadata(path)?;
        let listener = Self {
            inner,
            path: path.into(),
            dev: meta.dev(),
            ino: meta.ino(),
        };
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        listener.inner.set_nonblocking(true)?;
        Ok(listener)
    }
    pub fn accept(&self) -> io::Result<LocalStream> {
        let (stream, _) = self.inner.accept()?;
        let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
        let mut len = std::mem::size_of_val(&cred) as libc::socklen_t;
        let result = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                &mut cred as *mut _ as *mut _,
                &mut len,
            )
        };
        if result < 0 {
            return Err(io::Error::last_os_error());
        }
        if len as usize != std::mem::size_of_val(&cred) || cred.uid != uid() || cred.pid < 1 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "peer UID differs",
            ));
        }
        // Pin the kernel peer's lifetime before any authentication. A reused
        // PID must not turn an inherited connection into an owner connection.
        let peer = process_identity(cred.pid).ok();
        Ok(LocalStream(stream, peer))
    }
}
impl Drop for LocalListener {
    fn drop(&mut self) {
        if let Ok(m) = fs::symlink_metadata(&self.path) {
            if m.dev() == self.dev && m.ino() == self.ino && m.uid() == uid() {
                let _ = fs::remove_file(&self.path);
            }
        }
    }
}
impl LocalStream {
    pub fn connect(path: &Path) -> io::Result<Self> {
        use std::os::unix::fs::FileTypeExt;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or_else(|| io::Error::other("socket requires private parent directory"))?;
        private_directory(parent, false)?;
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.file_type().is_socket() || metadata.uid() != uid() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "unsafe server socket",
            ));
        }
        let stream = UnixStream::connect(path)?;
        let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
        let mut len = std::mem::size_of_val(&cred) as libc::socklen_t;
        if unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                &mut cred as *mut _ as *mut _,
                &mut len,
            )
        } < 0
        {
            return Err(io::Error::last_os_error());
        }
        if len as usize != std::mem::size_of_val(&cred) || cred.uid != uid() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "server UID differs",
            ));
        }
        Ok(Self(stream, process_identity(cred.pid).ok()))
    }
    pub fn try_clone(&self) -> io::Result<Self> {
        self.0.try_clone().map(|stream| Self(stream, self.1))
    }
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.0.set_read_timeout(timeout)
    }
    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.0.set_write_timeout(timeout)
    }
    pub fn shutdown(&self) -> io::Result<()> {
        self.0.shutdown(Shutdown::Both)
    }
}
impl Read for LocalStream {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.0.read(b)
    }
}
impl Write for LocalStream {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.0.write(b)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

pub(crate) enum Control {
    Input(Vec<u8>),
    Resize(usize, usize),
}
pub(crate) struct Process {
    cwd_tty: Option<File>,
    shell: Option<ProcessIdentity>,
    pub tx: SyncSender<Control>,
    queued: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    join: Mutex<Option<thread::JoinHandle<()>>>,
}
impl Process {
    /// Read live kernel state, never the directory supplied at spawn time.
    pub fn cwd(&self) -> Option<String> {
        if self.stop.load(Ordering::Acquire) {
            return None;
        }
        let shell = self.shell?;
        if let Some(tty) = &self.cwd_tty {
            let group = unsafe { libc::tcgetpgrp(tty.as_raw_fd()) };
            if group > 1 {
                if let Ok(identity) = process_identity(group) {
                    if identity.group == group && identity.session == shell.session
                        && identity.start >= shell.start
                    {
                        if let Some(cwd) = identity_cwd(identity) {
                            if unsafe { libc::tcgetpgrp(tty.as_raw_fd()) } == group
                                && process_identity(group).ok() == Some(identity)
                            {
                                return Some(cwd);
                            }
                        }
                    }
                }
            }
        }
        identity_cwd(shell)
    }
    pub fn input(&self, bytes: Vec<u8>) -> io::Result<()> {
        let n = bytes.len();
        self.queued
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |q| {
                q.checked_add(n).filter(|v| *v <= 1024 * 1024)
            })
            .map_err(|_| io::Error::other("PTY input exceeds 1 MiB queued"))?;
        if self.tx.try_send(Control::Input(bytes)).is_err() {
            self.queued.fetch_sub(n, Ordering::AcqRel);
            return Err(io::Error::other("PTY input queue unavailable"));
        }
        Ok(())
    }
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Release);
        if let Some(j) = self.join.lock().unwrap().take() {
            let _ = j.join();
        }
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        self.stop();
    }
}
fn size(rows: usize, cols: usize) -> WindowSize {
    WindowSize {
        num_lines: rows as u16,
        num_cols: cols as u16,
        cell_width: 16,
        cell_height: 32,
    }
}
fn group_signal(pid: i32, signal: i32) {
    // tty::new calls setsid; child PID is the session's original PGID.
    if pid > 1 {
        unsafe {
            libc::kill(-pid, signal);
        }
    }
}
pub(crate) fn spawn(
    argv: &[String],
    cwd: &Path,
    rows: usize,
    cols: usize,
    command: Arc<super::super::Command>,
    tab: Arc<super::super::Tab>,
) -> io::Result<Process> {
    let options = tty::Options {
        shell: Some(tty::Shell::new(argv[0].clone(), argv[1..].to_vec())),
        working_directory: Some(cwd.into()),
        drain_on_exit: true,
        env: [
            ("TERM".into(), "xterm-256color".into()),
            ("COLORTERM".into(), "truecolor".into()),
        ]
        .into(),
        ..Default::default()
    };
    // Reserve registry space before creating any child. Never discard roots:
    // descendants can outlive both their shell and its terminal tab.
    let mut roots = pty_roots()
        .lock()
        .map_err(|_| io::Error::other("PTY registry unavailable"))?;
    if roots.len() >= MAX_PTY_ROOTS {
        return Err(io::Error::other("PTY ancestry registry full"));
    }
    let mut pty = tty::new(&options, size(rows, cols), 0)?;
    let pid = pty.child().id() as i32;
    let shell = process_identity(pid).ok();
    // Only the worker reads/writes the PTY. This duplicate is used for ioctl only.
    let cwd_tty = pty.file().try_clone().ok();
    // tty::from_fd completes setsid before spawn returns, so PGID and SID
    // are known even if a very short-lived child already exited.
    roots.push(PtyRoot {
        pid,
        start: process_identity(pid).ok().map(|p| p.start),
    });
    drop(roots);
    let (tx, rx) = mpsc::sync_channel(64);
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = stop.clone();
    let queued = Arc::new(AtomicUsize::new(0));
    let pending_count = queued.clone();
    let join = thread::spawn(move || {
        let mut eof = false;
        let mut status = false;
        let mut deadline = None;
        let mut killed = false;
        let mut foreground = None;
        let mut pending = std::collections::VecDeque::<u8>::new();
        let mut buffer = [0u8; 16384];
        loop {
            if stopping.load(Ordering::Acquire) && deadline.is_none() {
                let pgid = unsafe { libc::tcgetpgrp(pty.file().as_raw_fd()) };
                if pgid > 1 && pgid != unsafe { libc::getpgrp() } {
                    foreground = Some(pgid);
                }
                if let Some(fg) = foreground {
                    group_signal(fg, libc::SIGHUP);
                }
                group_signal(pid, libc::SIGHUP);
                deadline = Some(Instant::now() + Duration::from_millis(200));
            }
            if let Some(d) = deadline {
                if Instant::now() >= d && !killed {
                    if let Some(fg) = foreground {
                        group_signal(fg, libc::SIGKILL);
                    }
                    group_signal(pid, libc::SIGKILL);
                    killed = true;
                }
            }
            for _ in 0..64 {
                match rx.try_recv() {
                    Ok(Control::Input(bytes)) => {
                        pending.extend(bytes);
                    }
                    Ok(Control::Resize(r, c)) => pty.on_resize(size(r, c)),
                    Err(_) => break,
                }
            }
            if !pending.is_empty() {
                let (a, _) = pending.as_slices();
                match pty.writer().write(a) {
                    Ok(n) => {
                        pending.drain(..n);
                        pending_count.fetch_sub(n, Ordering::AcqRel);
                    }
                    Err(e)
                        if e.kind() == io::ErrorKind::WouldBlock
                            || e.kind() == io::ErrorKind::Interrupted =>
                    {
                        ()
                    }
                    Err(_) => {
                        pending_count.fetch_sub(pending.len(), Ordering::AcqRel);
                        pending.clear();
                    }
                }
            }
            if !eof {
                for _ in 0..16 {
                    match pty.reader().read(&mut buffer) {
                        Ok(0) => {
                            eof = true;
                            break;
                        }
                        Ok(n) => {
                            command.append(&buffer[..n]);
                            tab.feed(&command.id, &buffer[..n]);
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                        Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                        Err(e) if e.raw_os_error() == Some(libc::EIO) => {
                            eof = true;
                            break;
                        }
                        Err(e) => {
                            command.failed(format!("PTY read failed: {e}"));
                            eof = true;
                            break;
                        }
                    }
                }
            }
            if !status {
                if let Some(ChildEvent::Exited(Some(s))) = pty.next_child_event() {
                    command.exited(s.code(), s.signal());
                    status = true;
                }
                // Exited(None) is also emitted for an unrelated child's SIGCHLD.
                // Poll as a fallback: a consumed/missed notification must not lose status.
                if !status {
                    let mut raw = 0;
                    let got = unsafe { libc::waitpid(pid, &mut raw, libc::WNOHANG) };
                    if got == pid {
                        let s = std::process::ExitStatus::from_raw(raw);
                        command.exited(s.code(), s.signal());
                        status = true;
                    } else if got < 0
                        && io::Error::last_os_error().raw_os_error() == Some(libc::ECHILD)
                    {
                        command.failed("child status unavailable".into());
                        command.exited(None, None);
                        status = true;
                    }
                }
            }
            if eof && status && (deadline.is_none() || killed) {
                break;
            }
            // During forced shutdown never wait indefinitely for an unrelated slave holder.
            if killed
                && deadline.is_some_and(|d| Instant::now() > d + Duration::from_millis(500))
                && status
            {
                if !eof {
                    command.failed("PTY forcibly closed before EOF".into());
                }
                break;
            }
            thread::sleep(Duration::from_millis(3));
        }
        tab.eof(&command.id);
        command.drained();
        // Retain Pty until its child has been reaped by next_child_event. Its Drop
        // calls Child::wait; dropping it earlier would block and discard status.
        drop(pty);
    });
    Ok(Process {
        cwd_tty,
        shell,
        tx,
        queued,
        stop,
        join: Mutex::new(Some(join)),
    })
}

const MAX_PTY_ROOTS: usize = 65_536;
struct PtyRoot {
    pid: i32,
    start: Option<u64>,
}
fn pty_roots() -> &'static Mutex<Vec<PtyRoot>> {
    static ROOTS: OnceLock<Mutex<Vec<PtyRoot>>> = OnceLock::new();
    ROOTS.get_or_init(|| Mutex::new(Vec::new()))
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct ProcessIdentity {
    pid: i32,
    parent: i32,
    group: i32,
    session: i32,
    start: u64,
}
fn process_identity(pid: i32) -> io::Result<ProcessIdentity> {
    if pid < 1 {
        return Err(io::Error::other("invalid process identity"));
    }
    let text = fs::read_to_string(format!("/proc/{pid}/stat"))?;
    // comm may contain spaces, parentheses and newlines. Its closing ')' is
    // the last one; all subsequent stat fields are kernel-formatted numbers.
    let (head, tail) = text
        .rsplit_once(')')
        .ok_or_else(|| io::Error::other("invalid process stat"))?;
    let actual = head
        .split_once(' ')
        .and_then(|(p, _)| p.parse::<i32>().ok());
    let fields: Vec<_> = tail.split_whitespace().collect();
    let invalid = || io::Error::other("invalid process stat");
    let number = |i: usize| {
        fields
            .get(i)
            .ok_or_else(invalid)?
            .parse::<i32>()
            .map_err(|_| invalid())
    };
    if actual != Some(pid) {
        return Err(invalid());
    }
    let start = fields
        .get(19)
        .ok_or_else(invalid)?
        .parse::<u64>()
        .map_err(|_| invalid())?;
    let identity = ProcessIdentity {
        pid,
        parent: number(1)?,
        group: number(2)?,
        session: number(3)?,
        start,
    };
    if identity.group < 1 || identity.session < 1 {
        return Err(invalid());
    }
    Ok(identity)
}

/// Kernel-proven owner provenance. A token alone is never owner authority.
/// Unknown, changing or excessively deep ancestry is deliberately ineligible.
pub fn owner_eligible(stream: &LocalStream) -> io::Result<bool> {
    let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of_val(&cred) as libc::socklen_t;
    if unsafe {
        libc::getsockopt(
            stream.0.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut cred as *mut _ as *mut _,
            &mut len,
        )
    } < 0
    {
        return Err(io::Error::last_os_error());
    }
    if len as usize != std::mem::size_of_val(&cred) || cred.uid != uid() || cred.pid < 1 {
        return Ok(false);
    }
    // Synchronize with spawn registration; the server ancestor check also
    // denies children during the interval before a root can be recorded.
    let roots = pty_roots()
        .lock()
        .map_err(|_| io::Error::other("PTY registry unavailable"))?;
    let Some(peer) = stream.1 else {
        return Ok(false);
    };
    if peer.pid != cred.pid || process_identity(cred.pid).ok() != Some(peer) {
        return Ok(false);
    }
    let server = process_identity(std::process::id() as i32)?;
    let mut chain: Vec<ProcessIdentity> = Vec::new();
    let mut pid = cred.pid;
    for _ in 0..256 {
        let current = match process_identity(pid) {
            Ok(p) => p,
            Err(_) => return Ok(false),
        };
        if current.pid == server.pid || chain.iter().any(|p| p.pid == pid) {
            return Ok(false);
        }
        if roots.iter().any(|root| {
            // Group/session numbers remain reserved security roots even after
            // leader exit. Numeric reuse is ambiguous and must deny, not allow.
            current.group == root.pid
                || current.session == root.pid
                || (current.pid == root.pid && root.start.is_none_or(|s| s == current.start))
        }) {
            return Ok(false);
        }
        if let Some(child) = chain.last() {
            if current.start > child.start {
                return Ok(false);
            }
        }
        chain.push(current);
        if current.pid == 1 && current.parent == 0 {
            // Re-read every link to catch PID reuse, reparenting, setsid and
            // process-group changes during the walk. Never accept a torn chain.
            for identity in &chain {
                if process_identity(identity.pid).ok() != Some(*identity) {
                    return Ok(false);
                }
            }
            return Ok(true);
        }
        if current.parent < 1 {
            return Ok(false);
        }
        pid = current.parent;
    }
    Ok(false)
}

fn private_directory(path: &Path, create: bool) -> io::Result<File> {
    if create {
        match fs::DirBuilder::new().mode(0o700).create(path) {
            Ok(()) => (),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e),
        }
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    let meta = file.metadata()?;
    if !meta.is_dir() || meta.uid() != uid() || meta.mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "runtime directory must be owned and private",
        ));
    }
    Ok(file)
}
fn sidecar(socket: &Path, suffix: &str, create_parent: bool) -> io::Result<(File, CString)> {
    let parent = socket
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| io::Error::other("socket requires private parent directory"))?;
    let directory = private_directory(parent, create_parent)?;
    let mut name = socket
        .file_name()
        .ok_or_else(|| io::Error::other("invalid socket path"))?
        .to_os_string();
    name.push(suffix);
    let name =
        CString::new(name.as_bytes()).map_err(|_| io::Error::other("invalid socket path"))?;
    Ok((directory, name))
}
fn open_sidecar(directory: &File, name: &CString, flags: i32) -> io::Result<File> {
    use std::os::fd::FromRawFd;
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
            0o600,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let file = unsafe { File::from_raw_fd(fd) };
    let meta = file.metadata()?;
    if !meta.is_file() || meta.uid() != uid() || meta.mode() & 0o7777 != 0o600 || meta.nlink() != 1
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "unsafe runtime file",
        ));
    }
    Ok(file)
}

/// The lock file is permanent; unlinking it could create two lock domains.
pub struct ServerLock {
    _file: File,
}
impl ServerLock {
    pub fn acquire(socket: &Path) -> io::Result<Self> {
        let (dir, name) = sidecar(socket, ".lock", true)?;
        let file = open_sidecar(&dir, &name, libc::O_RDWR | libc::O_CREAT)?;
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { _file: file })
    }
}

/// Read or create the private capability. No error includes capability bytes.
pub fn owner_token(socket: &Path, create: bool) -> io::Result<String> {
    let (dir, name) = sidecar(socket, ".owner", create)?;
    // Serialize creation/read so a concurrent reader never sees a partially
    // written capability. The short-lived writer lock is independent of the
    // server instance lock (which remains held for the server's entire life).
    let (_, guard_name) = sidecar(socket, ".owner.lock", false)?;
    let guard = open_sidecar(
        &dir,
        &guard_name,
        libc::O_RDWR | if create { libc::O_CREAT } else { 0 },
    )?;
    let mode = if create { libc::LOCK_EX } else { libc::LOCK_SH };
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        if unsafe { libc::flock(guard.as_raw_fd(), mode | libc::LOCK_NB) } == 0 {
            break;
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::WouldBlock {
            return Err(error);
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "capability lock timed out",
            ));
        }
        thread::sleep(Duration::from_millis(5));
    }
    let mut file = match open_sidecar(&dir, &name, libc::O_RDONLY) {
        Ok(file) => file,
        Err(e) if create && e.kind() == io::ErrorKind::NotFound => {
            let mut random = [0u8; 32];
            File::open("/dev/urandom")?.read_exact(&mut random)?;
            let token: String = random.iter().map(|b| format!("{b:02x}")).collect();
            let mut file =
                open_sidecar(&dir, &name, libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL)?;
            file.write_all(token.as_bytes())?;
            file.sync_all()?;
            return Ok(token);
        }
        Err(e) => return Err(e),
    };
    if file.metadata()?.len() != 64 {
        return Err(io::Error::other("invalid owner capability file"));
    }
    let mut bytes = [0u8; 64];
    file.read_exact(&mut bytes)?;
    if !bytes.iter().all(|b| b.is_ascii_hexdigit()) {
        return Err(io::Error::other("invalid owner capability file"));
    }
    String::from_utf8(bytes.to_vec()).map_err(|_| io::Error::other("invalid owner capability file"))
}

pub fn default_socket() -> io::Result<PathBuf> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("/run/user/{}", uid())));
    private_directory(&runtime, false)?;
    let directory = runtime.join("clanky");
    private_directory(&directory, true)?;
    Ok(directory.join("server.sock"))
}

/// Create private input/observation files without following an existing link.
pub fn create_private_file(path: &Path) -> io::Result<File> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
}

/// Replace this process while preserving its PID and ancestry.
pub fn exec_program(program: &Path, args: &[String]) -> io::Error {
    HelperCommand::new(program).args(args).exec()
}

pub fn spawn_server(exe: &Path, socket: &Path) -> io::Result<Child> {
    let mut command = HelperCommand::new(exe);
    command
        .arg("--server")
        .arg("--socket")
        .arg(socket)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    command.spawn()
}
pub fn spawn_pipe(program: &OsStr, args: &[OsString]) -> io::Result<Child> {
    HelperCommand::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
}
pub fn spawn_ssh(host: &str) -> io::Result<Child> {
    if host.is_empty()
        || host.starts_with('-')
        || host.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid SSH host",
        ));
    }
    let args: Vec<OsString> = ["-T", "-oBatchMode=yes", "--", host, "clanky", "--stdio"]
        .iter()
        .map(OsString::from)
        .collect();
    spawn_pipe(OsStr::new("ssh"), &args)
}

/// Validate the process lifetime on both sides of the cwd read.
fn identity_cwd(identity: ProcessIdentity) -> Option<String> {
    if process_identity(identity.pid).ok() != Some(identity) {
        return None;
    }
    let path = fs::read_link(format!("/proc/{}/cwd", identity.pid)).ok()?;
    let path = path.to_str()?;
    // proc appends this suffix for deleted directories: it is not a usable path.
    if !Path::new(path).is_absolute() || path.ends_with(" (deleted)")
        || path.contains('\0') || process_identity(identity.pid).ok() != Some(identity)
    {
        return None;
    }
    Some(path.to_owned())
}

/// Decode only local file URLs. No DNS, filesystem lookup or guessed directory.
pub(crate) fn local_osc7_cwd(bytes: &[u8]) -> Option<String> {
    let url = std::str::from_utf8(bytes).ok()?.strip_prefix("file://")?;
    let (host, path) = url.split_once('/')?;
    if !host.is_empty() && !host.eq_ignore_ascii_case("localhost") {
        static HOST: OnceLock<Option<String>> = OnceLock::new();
        let local = HOST.get_or_init(|| {
            let mut bytes = [0u8; 256];
            if unsafe { libc::gethostname(bytes.as_mut_ptr().cast(), bytes.len()) } != 0 {
                return None;
            }
            let end = bytes.iter().position(|b| *b == 0)?;
            std::str::from_utf8(&bytes[..end]).ok().map(str::to_owned)
        });
        if !local.as_ref().is_some_and(|local| host.eq_ignore_ascii_case(local)) {
            return None;
        }
    }
    // Reject credentials, ports, query/fragment components and raw controls.
    if host.contains(['@', ':', '%']) || path.contains(['?', '#'])
        || bytes.iter().any(|b| b.is_ascii_control())
    {
        return None;
    }
    let mut decoded = vec![b'/'];
    let mut bytes = path.as_bytes().iter().copied();
    while let Some(byte) = bytes.next() {
        decoded.push(if byte == b'%' {
            let hex = |b: u8| (b as char).to_digit(16).map(|n| n as u8);
            (hex(bytes.next()?)? << 4) | hex(bytes.next()?)?
        } else {
            byte
        });
    }
    let path = String::from_utf8(decoded).ok()?;
    if path.contains('\0') || !Path::new(&path).is_absolute() {
        return None;
    }
    Some(path)
}

/// Stop and reap a server child by its owned PID, allowing normal PTY cleanup.
pub fn stop_server(child: &mut Child) -> io::Result<()> {
    if child.try_wait()?.is_some() {
        return Ok(());
    }
    if unsafe { libc::kill(child.id() as i32, libc::SIGTERM) } < 0 {
        return Err(io::Error::last_os_error());
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(20));
    }
    child.kill()?;
    child.wait()?;
    Ok(())
}
