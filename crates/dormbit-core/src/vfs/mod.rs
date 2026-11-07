//! The read-only view of a project folder that all core logic works through (ADR-003).
//!
//! Core never touches the disk. Hosts implement [`ReadVfs`] (the desktop app over `cap-std`
//! handles, tests and the web demo over [`MemVfs`]). Nothing here follows a symlink: a link is
//! reported as a link and its text is read with [`ReadVfs::read_link`], never resolved.

mod mem;
mod path;

pub use mem::MemVfs;
pub use path::RelPath;

use crate::model::{DormbitError, ErrorCode};

/// What kind of thing a directory entry is. Symlinks are never followed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Dir,
    Symlink,
    /// FIFO, socket, block or character device.
    Special,
}

/// `lstat`-style facts about one entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Metadata {
    pub kind: EntryKind,
    pub len: u64,
    /// Seconds since the Unix epoch, UTC.
    pub mtime: i64,
    /// Unix permission bits (only the low 12 are meaningful).
    pub mode: u32,
}

/// One child of a directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    /// The file name. If the real name is not valid UTF-8 this is a lossy copy and `name_ok`
    /// is false, so the planner can put the entry in Unsupported without ever using the name
    /// as a path.
    pub name: String,
    pub name_ok: bool,
    pub kind: EntryKind,
}

/// The result of a size-limited read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Limited {
    Complete(Vec<u8>),
    /// The file is longer than the limit; nothing was returned.
    TooLarge {
        len: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VfsError {
    #[error("not found")]
    NotFound,
    /// The path was refused by the host's safety checks (traversal, symlink in the middle).
    #[error("unsafe path")]
    Unsafe,
    #[error("I/O error: {0}")]
    Io(String),
}

impl VfsError {
    /// The IPC error for this failure while naming `path`.
    pub fn to_error(&self, path: &str) -> DormbitError {
        match self {
            VfsError::Unsafe => {
                DormbitError::new(ErrorCode::PathUnsafe, format!("{path} is not a safe path."))
            }
            VfsError::NotFound => DormbitError::new(
                ErrorCode::FileChangedDuringCapture,
                format!("{path} disappeared while DORMBIT was reading it."),
            ),
            VfsError::Io(_) => DormbitError::new(
                ErrorCode::IoFailed,
                format!("DORMBIT could not read {path}."),
            ),
        }
    }
}

/// Read-only access to one project folder. No method follows a symlink.
pub trait ReadVfs {
    fn read_dir(&self, dir: &RelPath) -> Result<Vec<DirEntry>, VfsError>;
    fn symlink_metadata(&self, path: &RelPath) -> Result<Metadata, VfsError>;
    fn read_limited(&self, path: &RelPath, max_bytes: u64) -> Result<Limited, VfsError>;
    /// The link text of a symlink. It is never resolved.
    fn read_link(&self, path: &RelPath) -> Result<String, VfsError>;
}

/// The current time, supplied by the host. Core never reads a clock.
pub trait Clock {
    fn now_unix(&self) -> i64;
}

/// Random identifiers, supplied by the host. Core has no randomness.
pub trait IdGen {
    fn new_uuid(&self) -> String;
}

/// A clock that always returns the same time (tests and deterministic captures).
#[derive(Debug, Clone, Copy)]
pub struct FixedClock(pub i64);

impl Clock for FixedClock {
    fn now_unix(&self) -> i64 {
        self.0
    }
}

/// A counter-based id source: `00000000-0000-4000-8000-000000000001`, then `…02`, and so on.
#[derive(Debug, Default)]
pub struct CountingIds(std::cell::Cell<u64>);

impl IdGen for CountingIds {
    fn new_uuid(&self) -> String {
        let n = self.0.get() + 1;
        self.0.set(n);
        format!("00000000-0000-4000-8000-{n:012x}")
    }
}
