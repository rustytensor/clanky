//! Optimistic disk conflict checks and atomic, same-directory saves.

use crate::{Error, Result, platform};
use std::fs::{self, File, Metadata};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

/// Storage pinned to the canonical target resolved at open, not its input link.
pub(crate) struct Disk {
    path: PathBuf,
    baseline: platform::Stamp,
    bytes: Vec<u8>,
}

impl Disk {
    /// Open an existing regular UTF-8 file, following symlinks only here.
    pub(crate) fn open(path: &Path) -> Result<(Self, String)> {
        let path = fs::canonicalize(path)?;
        let (meta, bytes) = snapshot(&path)?;
        let text = String::from_utf8(bytes.clone()).map_err(|_| Error::InvalidUtf8)?;
        Ok((
            Self {
                path,
                baseline: platform::stamp(&meta),
                bytes,
            },
            text,
        ))
    }

    /// Compare identity, metadata and exact bytes with the open/last-save state.
    /// IO errors fail closed; callers latch both errors and mismatches.
    pub(crate) fn matches(&self) -> Result<bool> {
        let (meta, bytes) = snapshot(&self.path)?;
        Ok(platform::stamp(&meta) == self.baseline && bytes == self.bytes)
    }

    /// Write and sync a private sibling, recheck the baseline, then rename it
    /// over the pinned target and sync its parent. Never truncates the old inode.
    /// This is optimistic: another writer can race the final check and rename.
    pub(crate) fn save(&mut self, text: &str) -> Result<()> {
        if !self.matches()? {
            return Err(Error::Conflict);
        }
        let parent = self.path.parent().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "file has no parent directory")
        })?;
        let meta = fs::symlink_metadata(&self.path)?;
        if platform::stamp(&meta) != self.baseline {
            return Err(Error::Conflict);
        }
        let (mut temp, mut file) = Temp::create(parent)?;
        file.write_all(text.as_bytes())?;
        platform::preserve_permissions(&file, &meta)?;
        file.sync_all()?;
        if !self.matches()? {
            return Err(Error::Conflict);
        }
        fs::rename(&temp.path, &self.path)?;
        temp.owned = false;
        // A failure here is still a save error even though rename took effect.
        platform::sync_directory(parent)?;
        let (saved_meta, bytes) = snapshot(&self.path)?;
        if !platform::same_file(&file.metadata()?, &saved_meta) || bytes != text.as_bytes() {
            return Err(Error::Conflict);
        }
        self.baseline = platform::stamp(&saved_meta);
        self.bytes = bytes;
        Ok(())
    }
}

/// Read one stable regular inode and ensure the pathname still names it.
fn snapshot(path: &Path) -> Result<(Metadata, Vec<u8>)> {
    let mut file = platform::open_target(path)?;
    let before = file.metadata()?;
    if !before.is_file() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a regular file").into());
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    let named = fs::symlink_metadata(path)?;
    if platform::stamp(&before) != platform::stamp(&after)
        || platform::stamp(&after) != platform::stamp(&named)
        || !named.is_file()
        || after.len() != bytes.len() as u64
    {
        return Err(Error::Conflict);
    }
    Ok((after, bytes))
}

/// Own only the unique file we created, never a pre-existing name collision.
struct Temp {
    path: PathBuf,
    owned: bool,
}

impl Temp {
    fn create(parent: &Path) -> io::Result<(Self, File)> {
        for _ in 0..128 {
            let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(".clanky-edit-{}-{id}.tmp", std::process::id()));
            match platform::create_temp(&path) {
                Ok(file) => {
                    // Establish cleanup ownership immediately after create_new;
                    // no fallible metadata call may leave an unguarded sibling.
                    return Ok((Self { path, owned: true }, file));
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "temporary file names exhausted",
        ))
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        if self.owned {
            let _ = fs::remove_file(&self.path);
        }
    }
}
