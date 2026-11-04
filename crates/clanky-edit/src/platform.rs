//! Linux filesystem details used by the editor's disk storage.

use std::fs::{File, Metadata, OpenOptions};
use std::io;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

/// Identity and change-sensitive metadata; access time is deliberately excluded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Stamp {
    device: u64,
    inode: u64,
    mode: u32,
    links: u64,
    uid: u32,
    gid: u32,
    size: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}

/// Capture Linux identity, permissions and modification metadata.
pub(crate) fn stamp(meta: &Metadata) -> Stamp {
    Stamp {
        device: meta.dev(),
        inode: meta.ino(),
        mode: meta.mode(),
        links: meta.nlink(),
        uid: meta.uid(),
        gid: meta.gid(),
        size: meta.size(),
        modified: (meta.mtime(), meta.mtime_nsec()),
        changed: (meta.ctime(), meta.ctime_nsec()),
    }
}

/// Compare inode identity without requiring unchanged contents or timestamps.
pub(crate) fn same_file(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev() && a.ino() == b.ino()
}

/// Open a pinned target without following a newly substituted symlink or blocking
/// on a substituted FIFO. The caller must still reject nonregular files.
pub(crate) fn open_target(path: &Path) -> io::Result<File> {
    // Linux O_NOFOLLOW and O_NONBLOCK. No platform flags escape this module.
    OpenOptions::new()
        .read(true)
        .custom_flags(0x20000 | 0x800)
        .open(path)
}

/// Exclusively create a private temporary sibling.
pub(crate) fn create_temp(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

/// Restore all Unix permission bits after writing, including special bits.
pub(crate) fn preserve_permissions(file: &File, meta: &Metadata) -> io::Result<()> {
    file.set_permissions(std::fs::Permissions::from_mode(meta.mode() & 0o7777))
}

/// Persist a completed rename; errors must be reported even after replacement.
pub(crate) fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}
