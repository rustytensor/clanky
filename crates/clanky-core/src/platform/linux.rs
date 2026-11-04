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

static NOTIFICATION_WORKERS: AtomicUsize = AtomicUsize::new(0);

struct NotificationSlot;
impl Drop for NotificationSlot {
    fn drop(&mut self) {
        NOTIFICATION_WORKERS.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Best-effort desktop notification. Never includes command text or output.
/// At most eight helpers run concurrently; excess notifications are dropped.
/// Spawning and waiting happen off the caller's thread. Helpers get two seconds
/// before being killed and reaped; missing helpers/session buses are nonfatal.
pub fn send_command_finished_notification(exit_code: Option<i32>) {
    if NOTIFICATION_WORKERS
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
            (n < 8).then_some(n + 1)
        })
        .is_err()
    {
        return;
    }
    let slot = NotificationSlot;
    let _ = thread::Builder::new()
        .name("clanky-notification".into())
        .spawn(move || {
            let _slot = slot;
            let code = exit_code.map_or_else(|| "unknown".into(), |code| code.to_string());
            let Ok(mut child) = HelperCommand::new("notify-send")
                .arg("--app-name=Clanky")
                .arg("Clanky")
                .arg(format!("Command finished (exit {code})"))
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            else {
                return;
            };
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => return,
                    Ok(None) if Instant::now() < deadline => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    _ => {
                        let _ = child.kill();
                        let _ = child.wait();
                        return;
                    }
                }
            }
        });
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
    pub tx: SyncSender<Control>,
    pid: u32,
    identity: Option<ProcessIdentity>,
    master: File,
    slave_device: Option<u64>,
    queued: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    join: Mutex<Option<thread::JoinHandle<()>>>,
}
impl Process {
    pub(crate) fn pid(&self) -> u32 {
        self.pid
    }

    /// Evidence only: the caller also applies its output-quiet interval.
    /// A sleeping read must target this PTY's slave, in its foreground group
    /// and original session. Missing procfs access is not input evidence.
    pub(crate) fn waiting_for_input(&self) -> bool {
        let Some(root) = self.identity else {
            return false;
        };
        let Some(device) = self.slave_device else {
            return false;
        };
        if self.stop.load(Ordering::Acquire) {
            return false;
        }
        let foreground = unsafe { libc::tcgetpgrp(self.master.as_raw_fd()) };
        if foreground < 1 {
            return false;
        }
        let Ok(entries) = fs::read_dir("/proc") else {
            return false;
        };
        for entry in entries.take(65_536).flatten() {
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|s| s.parse::<i32>().ok())
            else {
                continue;
            };
            let Ok(identity) = process_identity(pid) else {
                continue;
            };
            if identity.session != root.pid || identity.group != foreground {
                continue;
            }
            let Ok(tasks) = fs::read_dir(format!("/proc/{pid}/task")) else {
                continue;
            };
            for task in tasks.take(4096).flatten() {
                let path = task.path();
                if blocked_slave_read(&path, device)
                    && process_identity(pid).ok() == Some(identity)
                    && unsafe { libc::tcgetpgrp(self.master.as_raw_fd()) } == foreground
                {
                    return true;
                }
            }
        }
        false
    }

    /// Read live kernel state, never the directory supplied at spawn time.
    pub fn cwd(&self) -> Option<String> {
        if self.stop.load(Ordering::Acquire) {
            return None;
        }
        let shell = self.identity?;
        {
            let tty = &self.master;
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

    /// Current shell directory, not the foreground program's directory.
    /// Fail closed if spawn-time identity was unavailable or the PID changed.
    pub fn current_cwd(&self) -> io::Result<PathBuf> {
        let shell = self.identity
            .ok_or_else(|| io::Error::other("shell identity unavailable"))?;
        let verify = || -> io::Result<()> {
            let current = process_identity(shell.pid)?;
            if current.start != shell.start {
                return Err(io::Error::other("shell process identity changed"));
            }
            Ok(())
        };
        verify()?;
        let cwd = fs::read_link(format!("/proc/{}/cwd", shell.pid))?;
        verify()?;
        Ok(cwd)
    }

    pub fn current_exe(&self) -> io::Result<PathBuf> {
        let shell = self.identity.ok_or_else(|| io::Error::other("shell identity unavailable"))?;
        let verify = || -> io::Result<()> {
            if process_identity(shell.pid)?.start != shell.start { return Err(io::Error::other("shell process identity changed")); }
            Ok(())
        };
        verify()?;
        let exe = fs::read_link(format!("/proc/{}/exe", shell.pid))?;
        verify()?;
        Ok(exe)
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
    let identity = process_identity(pid).ok();
    let master = pty.file().try_clone()?;
    let mut slave_number: libc::c_uint = 0;
    let slave_device =
        if unsafe { libc::ioctl(master.as_raw_fd(), libc::TIOCGPTN, &mut slave_number) } == 0 {
            fs::metadata(format!("/dev/pts/{slave_number}"))
                .ok()
                .map(|m| m.rdev())
        } else {
            None
        };
    // tty::from_fd completes setsid before spawn returns, so PGID and SID
    // are known even if a very short-lived child already exited.
    roots.push(PtyRoot {
        pid,
        start: identity.map(|p| p.start),
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
        tx,
        pid: pid as u32,
        identity,
        master,
        slave_device,
        queued,
        stop,
        join: Mutex::new(Some(join)),
    })
}

/// Require a kernel sleeping state and a read syscall on the actual slave
/// device. In particular, nanosleep, poll, pipe reads and silence do not count.
fn blocked_slave_read(task: &Path, device: u64) -> bool {
    let Ok(stat) = fs::read_to_string(task.join("stat")) else {
        return false;
    };
    if stat
        .rsplit_once(')')
        .and_then(|(_, tail)| tail.split_whitespace().next())
        != Some("S")
    {
        return false;
    }
    let Ok(syscall) = fs::read_to_string(task.join("syscall")) else {
        return false;
    };
    let fields: Vec<_> = syscall.split_whitespace().collect();
    let number = fields.first().and_then(|s| s.parse::<libc::c_long>().ok());
    if number != Some(libc::SYS_read) && number != Some(libc::SYS_readv) {
        return false;
    }
    let hex = |index: usize| {
        fields
            .get(index)
            .and_then(|s| u64::from_str_radix(s.strip_prefix("0x").unwrap_or(s), 16).ok())
    };
    let Some(fd) = hex(1).filter(|fd| *fd <= i32::MAX as u64) else {
        return false;
    };
    if hex(3).is_none_or(|count| count == 0) {
        return false;
    }
    use std::os::unix::fs::FileTypeExt;
    let Ok(metadata) = fs::metadata(task.join(format!("fd/{fd}"))) else {
        return false;
    };
    if !metadata.file_type().is_char_device() || metadata.rdev() != device {
        return false;
    }
    // Recheck the syscall after resolving its descriptor to reject moving
    // observations (e.g. a task waking and replacing the descriptor).
    fs::read_to_string(task.join("syscall")).ok().as_deref() == Some(syscall.as_str())
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

/// Server-observed provenance for audit; never taken from request arguments.
#[derive(Clone, serde::Serialize)]
pub struct AgentPeer {
    pub pid: u32,
    pub ancestry: Vec<u32>,
    pub terminal_roots: Vec<u32>,
}

pub fn agent_peer(stream: &LocalStream) -> io::Result<AgentPeer> {
    let invalid = || io::Error::other("peer ancestry unavailable or changed");
    let peer = stream.1.ok_or_else(invalid)?;
    if process_identity(peer.pid).ok() != Some(peer) {
        return Err(invalid());
    }
    let roots = pty_roots().lock().map_err(|_| invalid())?;
    let mut chain: Vec<ProcessIdentity> = Vec::new();
    let mut terminal_roots = Vec::new();
    let mut pid = peer.pid;
    for _ in 0..256 {
        let current = process_identity(pid)?;
        if chain.iter().any(|p| p.pid == pid)
            || chain
                .last()
                .is_some_and(|child| current.start > child.start)
        {
            return Err(invalid());
        }
        for root in roots.iter() {
            if (current.group == root.pid
                || current.session == root.pid
                || (current.pid == root.pid && root.start.is_none_or(|s| s == current.start)))
                && !terminal_roots.contains(&(root.pid as u32))
            {
                terminal_roots.push(root.pid as u32);
            }
        }
        chain.push(current);
        if current.pid == 1 && current.parent == 0 {
            if chain
                .iter()
                .any(|p| process_identity(p.pid).ok() != Some(*p))
            {
                return Err(invalid());
            }
            return Ok(AgentPeer {
                pid: peer.pid as u32,
                ancestry: chain.iter().map(|p| p.pid as u32).collect(),
                terminal_roots,
            });
        }
        if current.parent < 1 {
            return Err(invalid());
        }
        pid = current.parent;
    }
    Err(invalid())
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

// Existing configuration directories may be 0755. Require ownership, no
// group/other write permission, and no symlink; the state file itself is 0600.
fn config_directory(path: &Path, create: bool) -> io::Result<File> {
    if create {
        match fs::DirBuilder::new().mode(0o700).create(path) {
            Ok(()) => (),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e),
        }
    }
    let dir = fs::OpenOptions::new().read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC).open(path)?;
    let meta = dir.metadata()?;
    if !meta.is_dir() || meta.uid() != uid() || meta.mode() & 0o022 != 0 {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "unsafe configuration directory"));
    }
    Ok(dir)
}

/// Open an existing owner-only regular state file without following symlinks.
pub fn read_private_file(path: &Path) -> io::Result<File> {
    let parent = path.parent().ok_or_else(|| io::Error::other("missing configuration directory"))?;
    let name = path.file_name().ok_or_else(|| io::Error::other("invalid state file"))?;
    let name = CString::new(name.as_bytes()).map_err(|_| io::Error::other("invalid state file"))?;
    open_sidecar(&config_directory(parent, false)?, &name, libc::O_RDONLY)
}

/// Create or validate a safe configuration directory without following symlinks.
pub fn create_private_directory(path: &Path) -> io::Result<()> {
    config_directory(path, true).map(|_| ())
}

/// Serialize new-server startup and restoration separately from server lifetime.
/// Keep this guard until startup/restore is complete. Never unlink its file.
pub struct StartupLock {
    _file: File,
}
impl StartupLock {
    pub fn acquire(socket: &Path) -> io::Result<Self> {
        let (directory, name) = sidecar(socket, ".startup.lock", true)?;
        let file = open_sidecar(&directory, &name, libc::O_RDWR | libc::O_CREAT)?;
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
                return Ok(Self { _file: file });
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::WouldBlock {
                return Err(error);
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "startup lock timed out"));
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
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
    socket_under(&runtime)
}

/// The runtime directory must be ours and writable by nobody else, so no other
/// user can replace `clanky/` inside it. Group search (mode 0710, used on some
/// systems) is allowed; Clanky's own directory must still be fully private.
fn socket_under(runtime: &Path) -> io::Result<PathBuf> {
    let meta = fs::metadata(runtime)?;
    if !meta.is_dir() || meta.uid() != uid() || meta.mode() & 0o022 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "runtime directory must be owned and not writable by others",
        ));
    }
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

#[cfg(test)]
mod runtime_directory_tests {
    use super::*;

    fn scratch(mode: u32) -> PathBuf {
        let base = std::env::temp_dir().join(format!("clanky-runtime-{}-{mode:o}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir(&base).unwrap();
        fs::set_permissions(&base, fs::Permissions::from_mode(mode)).unwrap();
        base
    }

    #[test]
    fn group_search_runtime_is_accepted_and_clanky_dir_is_private() {
        // Fedora with a 0710 /run/user/UID: seen on the developer's machine.
        let base = scratch(0o710);
        let socket = socket_under(&base).unwrap();
        assert_eq!(socket, base.join("clanky/server.sock"));
        assert_eq!(fs::metadata(base.join("clanky")).unwrap().mode() & 0o777, 0o700);
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn group_writable_runtime_is_refused() {
        let base = scratch(0o770);
        assert_eq!(socket_under(&base).unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        assert!(!base.join("clanky").exists());
        fs::remove_dir_all(&base).unwrap();
    }
}
