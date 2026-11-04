//! UTF-8 editing and explicit review, independent of rendering and transports.
//!
//! Byte ranges are half-open UTF-8 boundaries. Files retain their original line
//! endings. Edits and accepted proposals remain in memory until explicit save.
//! See `docs/design-m7-core.md` for persistence guarantees and limitations.

mod disk;
#[cfg(target_os = "linux")]
mod platform;
#[cfg(not(target_os = "linux"))]
compile_error!("clanky-edit disk storage currently requires Linux");

use std::{
    fmt,
    ops::Range,
    path::{Path, PathBuf},
};

/// A checked editor operation.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors never imply that a pending proposal was accepted.
#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    InvalidUtf8,
    InvalidRange,
    InvalidPosition,
    InvalidLocation,
    Conflict,
    UnknownHunk,
    NotPending,
    IdExhausted,
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "file operation failed: {error}"),
            Self::InvalidUtf8 => f.write_str("file is not valid UTF-8"),
            Self::InvalidRange => f.write_str("range is not ordered UTF-8 boundaries"),
            Self::InvalidPosition => f.write_str("line or column is out of range"),
            Self::InvalidLocation => {
                f.write_str("expected path:line[:column], with positive positions")
            }
            Self::Conflict => {
                f.write_str("file changed on disk; reopen before saving or accepting")
            }
            Self::UnknownHunk => f.write_str("unknown proposal ID"),
            Self::NotPending => f.write_str("proposal is no longer pending"),
            Self::IdExhausted => f.write_str("proposal IDs exhausted"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

/// Direction-preserving selection; both endpoints are UTF-8 byte boundaries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub anchor: usize,
    pub cursor: usize,
}

/// Review decisions are not reversed by text undo/redo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HunkState {
    Pending,
    Accepted,
    Rejected,
    Stale,
}

/// A document-local proposal. Only pending ranges track subsequent local edits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunk {
    pub id: u64,
    pub range: Range<usize>,
    pub replacement: String,
    pub state: HunkState,
}

struct Change {
    start: usize,
    removed: String,
    inserted: String,
    before: Selection,
    after: Selection,
}

/// An open document pinned to its resolved target path, including through links.
///
/// Selection and history belong to this document, not a renderer.
pub struct Document {
    disk: disk::Disk,
    text: String,
    selection: Selection,
    undo: Vec<Change>,
    redo: Vec<Change>,
    hunks: Vec<Hunk>,
    next_id: u64,
    conflict: bool,
}

impl Document {
    /// Open an existing regular UTF-8 file, following and resolving symlinks.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let (disk, text) = disk::Disk::open(path.as_ref())?;
        Ok(Self {
            disk,
            text,
            selection: Selection::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            hunks: Vec::new(),
            next_id: 1,
            conflict: false,
        })
    }

    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn selection(&self) -> Selection {
        self.selection
    }
    pub fn hunks(&self) -> &[Hunk] {
        &self.hunks
    }

    /// Change selection without adding a text undo step. Backwards is allowed.
    pub fn select(&mut self, selection: Selection) -> Result<()> {
        if !self.text.is_char_boundary(selection.anchor)
            || !self.text.is_char_boundary(selection.cursor)
        {
            return Err(Error::InvalidRange);
        }
        self.selection = selection;
        Ok(())
    }

    fn check_range(&self, range: &Range<usize>) -> Result<()> {
        if range.start > range.end
            || !self.text.is_char_boundary(range.start)
            || !self.text.is_char_boundary(range.end)
        {
            return Err(Error::InvalidRange);
        }
        Ok(())
    }

    /// Replace a span as one undo step. A no-op leaves caret and history alone.
    pub fn edit(&mut self, range: Range<usize>, replacement: &str) -> Result<()> {
        self.check_range(&range)?;
        if &self.text[range.clone()] == replacement {
            return Ok(());
        }
        let caret = range
            .start
            .checked_add(replacement.len())
            .ok_or(Error::InvalidRange)?;
        let change = Change {
            start: range.start,
            removed: self.text[range.clone()].to_owned(),
            inserted: replacement.to_owned(),
            before: self.selection,
            after: Selection {
                anchor: caret,
                cursor: caret,
            },
        };
        self.replace(range, replacement);
        self.selection = change.after;
        self.undo.push(change);
        self.redo.clear();
        Ok(())
    }

    // All callers validate spans or use internally recorded inverse spans.
    fn replace(&mut self, range: Range<usize>, replacement: &str) {
        for hunk in &mut self.hunks {
            if hunk.state != HunkState::Pending {
                continue;
            }
            let intersects = if range.is_empty() {
                hunk.range.start <= range.start && range.start <= hunk.range.end
            } else if hunk.range.is_empty() {
                range.start <= hunk.range.start && hunk.range.start <= range.end
            } else {
                range.start < hunk.range.end && hunk.range.start < range.end
            };
            if intersects {
                hunk.state = HunkState::Stale;
            } else if hunk.range.start >= range.end {
                // Subtract before adding: no signed conversion or underflow.
                hunk.range.start = hunk.range.start - range.len() + replacement.len();
                hunk.range.end = hunk.range.end - range.len() + replacement.len();
            }
        }
        self.text.replace_range(range, replacement);
    }

    /// Restore text and selection; pending hunks are shifted or made stale.
    pub fn undo(&mut self) -> Result<bool> {
        let Some(change) = self.undo.pop() else {
            return Ok(false);
        };
        self.replace(
            change.start..change.start + change.inserted.len(),
            &change.removed,
        );
        self.selection = change.before;
        self.redo.push(change);
        Ok(true)
    }

    /// Reapply one undone text edit. Does not resurrect old review decisions.
    pub fn redo(&mut self) -> Result<bool> {
        let Some(change) = self.redo.pop() else {
            return Ok(false);
        };
        self.replace(
            change.start..change.start + change.removed.len(),
            &change.inserted,
        );
        self.selection = change.after;
        self.undo.push(change);
        Ok(true)
    }

    fn mark_conflict(&mut self) {
        self.conflict = true;
        for hunk in &mut self.hunks {
            if hunk.state == HunkState::Pending {
                hunk.state = HunkState::Stale;
            }
        }
    }

    /// True means unchanged. False means a latched disk conflict; reopen to clear.
    /// IO errors also latch a conflict and stale all pending hunks.
    pub fn refresh(&mut self) -> Result<bool> {
        if self.conflict {
            return Ok(false);
        }
        match self.disk.matches() {
            Ok(true) => Ok(true),
            Ok(false) => {
                self.mark_conflict();
                Ok(false)
            }
            Err(error) => {
                self.mark_conflict();
                Err(error)
            }
        }
    }

    fn require_current_disk(&mut self) -> Result<()> {
        if self.refresh()? {
            Ok(())
        } else {
            Err(Error::Conflict)
        }
    }

    /// Atomically replace the resolved target, never the original symlink.
    /// Undo history survives save. A failed save conservatively latches conflict;
    /// a post-rename sync error can mean bytes were written but durability failed.
    pub fn save(&mut self) -> Result<()> {
        self.require_current_disk()?;
        if let Err(error) = self.disk.save(&self.text) {
            self.mark_conflict();
            return Err(error);
        }
        Ok(())
    }

    /// Propose against current text, without changing text, selection or disk.
    pub fn propose(&mut self, range: Range<usize>, replacement: &str) -> Result<u64> {
        self.check_range(&range)?;
        self.require_current_disk()?;
        let id = self.next_id;
        let next = id.checked_add(1).ok_or(Error::IdExhausted)?;
        self.hunks.push(Hunk {
            id,
            range,
            replacement: replacement.to_owned(),
            state: HunkState::Pending,
        });
        self.next_id = next;
        Ok(id)
    }

    fn pending_index(&self, id: u64) -> Result<usize> {
        let index = self
            .hunks
            .iter()
            .position(|h| h.id == id)
            .ok_or(Error::UnknownHunk)?;
        if self.hunks[index].state != HunkState::Pending {
            return Err(Error::NotPending);
        }
        Ok(index)
    }

    /// Accept one pending hunk as an ordinary undoable in-memory edit.
    pub fn accept(&mut self, id: u64) -> Result<()> {
        self.pending_index(id)?;
        self.require_current_disk()?;
        let index = self.pending_index(id)?;
        let range = self.hunks[index].range.clone();
        let replacement = self.hunks[index].replacement.clone();
        self.check_range(&range)?;
        // Exclude this hunk from the edit's overlap invalidation.
        self.hunks[index].state = HunkState::Accepted;
        if let Err(error) = self.edit(range, &replacement) {
            self.hunks[index].state = HunkState::Pending;
            return Err(error);
        }
        Ok(())
    }

    /// Reject only the specified pending proposal; no text or IO is changed.
    pub fn reject(&mut self, id: u64) -> Result<()> {
        let index = self.pending_index(id)?;
        self.hunks[index].state = HunkState::Rejected;
        Ok(())
    }

    /// One-based line/scalar column to byte offset. CRLF and LF are terminators;
    /// standalone CR is content. The column just past content is allowed.
    pub fn offset(&self, line: usize, column: usize) -> Result<usize> {
        if line == 0 || column == 0 {
            return Err(Error::InvalidPosition);
        }
        let mut start = 0;
        let mut lines = self.text.split('\n').peekable();
        for number in 1..=line {
            let raw = lines.next().ok_or(Error::InvalidPosition)?;
            if number == line {
                let content = if lines.peek().is_some() {
                    raw.strip_suffix('\r').unwrap_or(raw)
                } else {
                    raw
                };
                return content
                    .char_indices()
                    .map(|(i, _)| i)
                    .chain(std::iter::once(content.len()))
                    .nth(column - 1)
                    .map(|i| start + i)
                    .ok_or(Error::InvalidPosition);
            }
            start += raw.len() + 1;
        }
        Err(Error::InvalidPosition)
    }
}

/// A clicked source location. Resolve relative paths against an explicit cwd.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    pub path: PathBuf,
    pub line: usize,
    pub column: usize,
}

impl Location {
    /// Parse numeric suffixes from the right; earlier colons remain in the path.
    /// Numeric path suffixes are inherently ambiguous and treated as line numbers.
    pub fn parse(value: &str) -> Result<Self> {
        fn position(s: &str) -> Result<usize> {
            if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
                return Err(Error::InvalidLocation);
            }
            let n = s.parse::<usize>().map_err(|_| Error::InvalidLocation)?;
            if n == 0 {
                Err(Error::InvalidLocation)
            } else {
                Ok(n)
            }
        }
        let (prefix, last) = value.rsplit_once(':').ok_or(Error::InvalidLocation)?;
        let last = position(last)?;
        let (path, line, column) = match prefix.rsplit_once(':') {
            Some((path, numeric))
                if !numeric.is_empty() && numeric.bytes().all(|b| b.is_ascii_digit()) =>
            {
                (path, position(numeric)?, last)
            }
            _ => (prefix, last, 1),
        };
        if path.is_empty() || path.contains('\0') {
            return Err(Error::InvalidLocation);
        }
        Ok(Self {
            path: PathBuf::from(path),
            line,
            column,
        })
    }
}
