//! Owned desktop helpers and home-directory discovery at the Linux boundary.
use std::ffi::{CStr, OsStr};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Shared validation for both hit detection and the defensive process boundary.
pub fn valid_http_url(url: &str) -> bool {
    if url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return false;
    }
    let Some((scheme, rest)) = url.split_once("://") else { return false; };
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return false;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host_port = authority.rsplit('@').next().unwrap_or("");
    if let Some(bracketed) = host_port.strip_prefix('[') {
        let Some((host, suffix)) = bracketed.split_once(']') else { return false; };
        return host.parse::<std::net::Ipv6Addr>().is_ok()
            && (suffix.is_empty() || valid_port(suffix));
    }
    let (host, port) = host_port.split_once(':').unwrap_or((host_port, ""));
    !host.is_empty()
        && !host.chars().any(|c| matches!(c, '\\' | '[' | ']' | '<' | '>' | '"' | '\'' | '`'))
        && (!host_port.contains(':') || valid_port(&format!(":{port}")))
}

fn valid_port(suffix: &str) -> bool {
    suffix.strip_prefix(':').is_some_and(|p| {
        !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) && p.parse::<u16>().is_ok()
    })
}

fn existing_home(path: PathBuf) -> Option<PathBuf> {
    (path.is_absolute() && path.is_dir()).then_some(path)
}

pub fn home_dir() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("HOME").and_then(|p| existing_home(p.into())) {
        return Some(home);
    }
    // getpwuid_r owns no static passwd storage; copy the result before the buffer drops.
    let mut size = 1024;
    while size <= 1024 * 1024 {
        let mut buffer = vec![0u8; size];
        let mut entry = std::mem::MaybeUninit::<libc::passwd>::uninit();
        let mut result = std::ptr::null_mut();
        let status = unsafe {
            libc::getpwuid_r(libc::getuid(), entry.as_mut_ptr(), buffer.as_mut_ptr().cast(), buffer.len(), &mut result)
        };
        if status == libc::ERANGE {
            size *= 2;
            continue;
        }
        if status != 0 || result.is_null() { return None; }
        let entry = unsafe { entry.assume_init() };
        if entry.pw_dir.is_null() { return None; }
        let bytes = unsafe { CStr::from_ptr(entry.pw_dir) }.to_bytes();
        return existing_home(Path::new(OsStr::from_bytes(bytes)).to_path_buf());
    }
    None
}

const MAX_HELPERS: usize = 8;
const HELPER_DEADLINE: Duration = Duration::from_secs(10);

#[derive(Default)]
pub struct UrlOpener {
    helpers: Vec<(Child, Instant)>,
}

impl UrlOpener {
    pub fn open(&mut self, url: &str) -> io::Result<()> {
        if !valid_http_url(url) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid HTTP URL"));
        }
        self.poll()?;
        if self.helpers.len() >= MAX_HELPERS {
            return Err(io::Error::new(io::ErrorKind::WouldBlock, "Too many URL helpers"));
        }
        let child = Command::new("xdg-open").arg(url)
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()?;
        self.helpers.push((child, Instant::now()));
        Ok(())
    }

    pub fn poll(&mut self) -> io::Result<()> {
        let mut error = None;
        let mut i = 0;
        while i < self.helpers.len() {
            let (child, started) = &mut self.helpers[i];
            let remove = match child.try_wait() {
                Ok(Some(status)) => {
                    if !status.success() {
                        error.get_or_insert_with(|| io::Error::other("URL helper failed"));
                    }
                    true
                }
                Ok(None) if started.elapsed() >= HELPER_DEADLINE => {
                    // Only the saved direct child, never its browser descendants or a PID pattern.
                    let cleanup = child.kill().and_then(|_| child.wait().map(|_| ()));
                    match cleanup {
                        Ok(()) => { error.get_or_insert_with(|| io::Error::new(io::ErrorKind::TimedOut, "URL helper timed out")); true }
                        Err(e) => { error.get_or_insert(e); false }
                    }
                }
                Ok(None) => false,
                Err(e) => { error.get_or_insert(e); false }
            };
            if remove { self.helpers.swap_remove(i); } else { i += 1; }
        }
        error.map_or(Ok(()), Err)
    }
}

impl Drop for UrlOpener {
    fn drop(&mut self) {
        for (child, _) in &mut self.helpers {
            if !matches!(child.try_wait(), Ok(Some(_))) {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}
