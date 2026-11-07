//! A capsule file to verify, opened again read-only (no symlink following) on every `open()`,
//! so the verifier never reuses the writer's handle (Requirement 13.1).

use std::io::Read;

use cap_std::fs::{Dir, MetadataExt as _, OpenOptions, OpenOptionsExt as _};
use dormbit_core::capsule::CapsuleSource;
use dormbit_core::model::{DormbitError, ErrorCode};

pub struct FsCapsuleSource {
    dir: Dir,
    name: String,
}

fn unreadable() -> DormbitError {
    DormbitError::new(
        ErrorCode::IoFailed,
        "DORMBIT could not open the capsule file.",
    )
}

impl FsCapsuleSource {
    /// `name` is one file name inside `dir` (for example `<capsule_id>.dormbit`).
    pub fn new(dir: Dir, name: &str) -> Result<Self, DormbitError> {
        if name.is_empty() || name.contains('/') || name == "." || name == ".." {
            return Err(DormbitError::new(
                ErrorCode::PathUnsafe,
                "The capsule name is not valid.",
            ));
        }
        Ok(Self {
            dir,
            name: name.to_owned(),
        })
    }
}

impl CapsuleSource for FsCapsuleSource {
    fn byte_len(&self) -> Result<u64, DormbitError> {
        let m = self
            .dir
            .symlink_metadata(&self.name)
            .map_err(|_| unreadable())?;
        if !m.is_file() {
            return Err(DormbitError::new(
                ErrorCode::PathUnsafe,
                "The capsule is not a regular file.",
            ));
        }
        Ok(m.len())
    }

    fn open(&self) -> Result<Box<dyn Read + Send>, DormbitError> {
        let before = self
            .dir
            .symlink_metadata(&self.name)
            .map_err(|_| unreadable())?;
        if !before.is_file() {
            return Err(DormbitError::new(
                ErrorCode::PathUnsafe,
                "The capsule is not a regular file.",
            ));
        }
        let mut o = OpenOptions::new();
        o.read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        let f = self
            .dir
            .open_with(&self.name, &o)
            .map_err(|_| unreadable())?;
        let after = f.metadata().map_err(|_| unreadable())?;
        if !after.is_file() || after.dev() != before.dev() || after.ino() != before.ino() {
            return Err(DormbitError::new(
                ErrorCode::PathUnsafe,
                "The capsule file changed while DORMBIT was opening it.",
            ));
        }
        Ok(Box::new(f.into_std()))
    }
}
