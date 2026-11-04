use clanky_vt::{
    event::{OnResize, WindowSize},
    tty::{self, ChildEvent, EventedPty, EventedReadWrite},
};
use std::{
    fs,
    io::{self, Read, Seek, Write},
    net::Shutdown,
    os::{
        fd::AsRawFd,
        unix::{
            fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
            net::{UnixListener, UnixStream},
            process::ExitStatusExt,
        },
    },
    path::{Path, PathBuf},
    process::{Child, Command as HelperCommand, Stdio},
    sync::{
        Arc, Mutex,
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

    /// Copies at most 64 KiB without writing into a potentially blocked pipe.
    /// Success means the helper was spawned, not that the display accepted it.
    pub fn copy(&mut self, bytes: &[u8]) -> io::Result<()> {
        if bytes.len() > 64 * 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "clipboard input exceeds 64 KiB",
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

        // Unlink before writing any clipboard bytes. create_new rejects existing
        // files and symlinks; mode 0600 also protects the brief named lifetime.
        static NEXT_FILE: AtomicUsize = AtomicUsize::new(0);
        let mut input = None;
        for _ in 0..32 {
            let path = std::env::temp_dir().join(format!(
                ".clanky-clipboard-{}-{}",
                std::process::id(),
                NEXT_FILE.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
            {
                Ok(file) => {
                    fs::remove_file(&path)?;
                    input = Some(file);
                    break;
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        let mut input = input.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "could not create private clipboard input",
            )
        })?;
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
                        format!("clipboard helper unavailable: {error}"),
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
pub struct LocalStream(UnixStream);
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
        if cred.uid != uid() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "peer UID differs",
            ));
        }
        Ok(LocalStream(stream))
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
    pub fn try_clone(&self) -> io::Result<Self> {
        self.0.try_clone().map(Self)
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
    queued: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    join: Mutex<Option<thread::JoinHandle<()>>>,
}
impl Process {
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
    let mut pty = tty::new(&options, size(rows, cols), 0)?;
    let pid = pty.child().id() as i32;
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
        queued,
        stop,
        join: Mutex::new(Some(join)),
    })
}
