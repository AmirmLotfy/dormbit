//! The capsule library: a staging folder per capture and the exclusive commit (design:
//! dormbit-fs responsibilities).
//!
//! `begin` creates `.dormbit-capture-<op_id>/` in the library with an `owner-token` file.
//! Every file is created with create-new semantics. `commit` syncs `capsule.tmp`, moves it to
//! `<capsule_id>.dormbit` with [`rename_excl`] (never replacing anything), and syncs the
//! library folder. Cleanup removes only DORMBIT's own staging files, and only after the
//! owner token matches; that is the only deletion DORMBIT performs.

use std::io::{Read as _, Write as _};

use cap_std::fs::{Dir, File, OpenOptions, OpenOptionsExt as _};
use dormbit_core::capsule::CaptureWorkspace;
use dormbit_core::model::{DormbitError, ErrorCode};

use crate::rename::{RenameError, rename_excl};

const TOKEN_FILE: &str = "owner-token";
const OBJECTS: &str = "objects";
const OUT: &str = "capsule.tmp";
pub const STAGING_PREFIX: &str = ".dormbit-capture-";

fn io_err(what: &str) -> impl Fn(std::io::Error) -> DormbitError + '_ {
    move |_| DormbitError::new(ErrorCode::IoFailed, format!("DORMBIT could not {what}."))
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn is_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
}

fn create_new(dir: &Dir, name: &str) -> std::io::Result<File> {
    let mut o = OpenOptions::new();
    o.write(true)
        .create_new(true)
        .custom_flags(libc::O_NOFOLLOW);
    dir.open_with(name, &o)
}

/// A library folder the user granted for capsules.
pub struct FsCapsuleSink {
    library: Dir,
}

impl FsCapsuleSink {
    pub fn new(library: Dir) -> Self {
        Self { library }
    }

    /// Starts a capture's staging folder. Fails if a folder with that op ID already exists.
    pub fn begin(&self, op_id: &str, owner_token: &str) -> Result<FsWorkspace, DormbitError> {
        if !is_id(op_id) || !is_id(owner_token) {
            return Err(DormbitError::new(
                ErrorCode::PathUnsafe,
                "The capture ID is not valid.",
            ));
        }
        let staging_name = format!("{STAGING_PREFIX}{op_id}");
        self.library
            .create_dir(&staging_name)
            .map_err(io_err("create a staging folder in the library"))?;
        let staging = self
            .library
            .open_dir(&staging_name)
            .map_err(io_err("open the staging folder"))?;
        let token = format!("{op_id}:{owner_token}");
        let mut t = create_new(&staging, TOKEN_FILE).map_err(io_err("write the owner token"))?;
        t.write_all(token.as_bytes())
            .and_then(|()| t.sync_all())
            .map_err(io_err("write the owner token"))?;
        staging
            .create_dir(OBJECTS)
            .map_err(io_err("create the objects folder"))?;
        let objects = staging
            .open_dir(OBJECTS)
            .map_err(io_err("open the objects folder"))?;
        let out = create_new(&staging, OUT).map_err(io_err("create the capsule file"))?;
        let library = self
            .library
            .try_clone()
            .map_err(io_err("open the library"))?;
        Ok(FsWorkspace {
            library,
            staging_name,
            staging,
            objects,
            out: Some(out),
            token,
            tmp_counter: 0,
            done: false,
        })
    }
}

pub struct FsWorkspace {
    library: Dir,
    staging_name: String,
    staging: Dir,
    objects: Dir,
    out: Option<File>,
    token: String,
    tmp_counter: u64,
    done: bool,
}

impl FsWorkspace {
    pub fn staging_name(&self) -> &str {
        &self.staging_name
    }

    fn token_matches(&self) -> bool {
        let Ok(mut f) = self
            .staging
            .open_with(TOKEN_FILE, OpenOptions::new().read(true))
        else {
            return false;
        };
        let mut s = String::new();
        f.read_to_string(&mut s).is_ok() && s == self.token
    }

    /// Removes the staging folder if, and only if, its owner token is ours. Removes only the
    /// files DORMBIT creates there; anything else makes the folder stay and be reported.
    fn cleanup(&mut self) -> Result<(), DormbitError> {
        if self.done {
            return Ok(());
        }
        if !self.token_matches() {
            return Err(DormbitError::new(
                ErrorCode::IoFailed,
                "A staging folder in the library isn't DORMBIT's, so it was left in place.",
            ));
        }
        self.out = None;
        let fail = io_err("clean up its staging folder");
        let entries = self.objects.entries().map_err(&fail)?;
        for e in entries {
            let e = e.map_err(&fail)?;
            let name = e.file_name();
            let name = name.to_str().unwrap_or("");
            let ours = is_hex64(name) || name.starts_with("tmp-");
            let is_file = e.file_type().map(|t| t.is_file()).unwrap_or(false);
            if ours && is_file {
                self.objects.remove_file(name).map_err(&fail)?;
            }
        }
        if self.staging.symlink_metadata(OUT).is_ok() {
            self.staging.remove_file(OUT).map_err(&fail)?;
        }
        self.staging.remove_dir(OBJECTS).map_err(&fail)?;
        self.staging.remove_file(TOKEN_FILE).map_err(&fail)?;
        self.library.remove_dir(&self.staging_name).map_err(&fail)?;
        self.done = true;
        Ok(())
    }
}

/// A workspace dropped without `commit` or `abort` (for example a job that failed to start)
/// still cleans up its own staging folder, with the same owner-token check.
impl Drop for FsWorkspace {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

impl FsCapsuleSink {
    /// Staging folders left in the library by an earlier run (a crash or a failed cleanup).
    /// They are reported, never removed here: the owner token belongs to that earlier run.
    pub fn leftover_staging(&self) -> Vec<String> {
        let Ok(entries) = self.library.entries() else {
            return Vec::new();
        };
        let mut out: Vec<String> = entries
            .filter_map(Result::ok)
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.starts_with(STAGING_PREFIX))
            .collect();
        out.sort();
        out
    }
}

impl CaptureWorkspace for FsWorkspace {
    fn put_object(&mut self, content_sha256: &str, stored: &[u8]) -> Result<bool, DormbitError> {
        if !is_hex64(content_sha256) {
            return Err(DormbitError::new(
                ErrorCode::CapsuleMalformed,
                "An object name is not a SHA-256.",
            ));
        }
        if self.objects.symlink_metadata(content_sha256).is_ok() {
            return Ok(true);
        }
        self.tmp_counter += 1;
        let tmp = format!("tmp-{}", self.tmp_counter);
        let mut f = create_new(&self.objects, &tmp).map_err(io_err("stage a file"))?;
        f.write_all(stored)
            .and_then(|()| f.sync_all())
            .map_err(io_err("stage a file"))?;
        drop(f);
        match rename_excl(&self.objects, &tmp, &self.objects, content_sha256) {
            Ok(()) => Ok(false),
            Err(RenameError::Exists) => {
                self.objects
                    .remove_file(&tmp)
                    .map_err(io_err("stage a file"))?;
                Ok(true)
            }
            Err(_) => Err(DormbitError::new(
                ErrorCode::IoFailed,
                "DORMBIT could not stage a file.",
            )),
        }
    }

    fn read_object(&self, content_sha256: &str) -> Result<Vec<u8>, DormbitError> {
        if !is_hex64(content_sha256) {
            return Err(DormbitError::new(
                ErrorCode::CapsuleMalformed,
                "An object name is not a SHA-256.",
            ));
        }
        let mut o = OpenOptions::new();
        o.read(true).custom_flags(libc::O_NOFOLLOW);
        let mut f = self.objects.open_with(content_sha256, &o).map_err(|_| {
            DormbitError::new(
                ErrorCode::CapsuleHashMismatch,
                "A stored file is missing from the staging folder.",
            )
        })?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)
            .map_err(io_err("read a staged file"))?;
        Ok(buf)
    }

    fn write_out(&mut self, bytes: &[u8]) -> Result<(), DormbitError> {
        let f = self
            .out
            .as_mut()
            .ok_or_else(|| DormbitError::new(ErrorCode::IoFailed, "The capsule file is closed."))?;
        f.write_all(bytes).map_err(io_err("write the capsule"))
    }

    fn commit(&mut self, final_name: &str) -> Result<(), DormbitError> {
        let ok_name = final_name.strip_suffix(".dormbit").is_some_and(is_id);
        if !ok_name {
            return Err(DormbitError::new(
                ErrorCode::PathUnsafe,
                "The capsule name is not valid.",
            ));
        }
        if let Some(f) = self.out.take() {
            // On macOS, std's sync_all issues F_FULLFSYNC.
            f.sync_all().map_err(io_err("save the capsule to disk"))?;
        }
        match rename_excl(&self.staging, OUT, &self.library, final_name) {
            Ok(()) => {}
            Err(RenameError::Exists) => {
                return Err(DormbitError::new(
                    ErrorCode::CapsuleExists,
                    "A capsule with that name is already in the library. Nothing was replaced.",
                ));
            }
            Err(_) => {
                return Err(DormbitError::new(
                    ErrorCode::IoFailed,
                    "DORMBIT could not move the capsule into the library.",
                ));
            }
        }
        if let Ok(d) = self.library.open(".") {
            let _ = d.sync_all();
        }
        // The capsule is in place, so a cleanup problem doesn't undo the commit. If the
        // staging folder can't be removed, it stays (token-checked) for `leftover_staging` to
        // report on the next launch.
        let _ = self.cleanup();
        Ok(())
    }

    fn abort(&mut self) -> Result<(), DormbitError> {
        self.cleanup()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cap_std::ambient_authority;

    const SHA: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const NAME: &str = "00000000-0000-4000-8000-000000000001.dormbit";

    fn lib() -> (tempfile::TempDir, FsCapsuleSink) {
        let t = tempfile::tempdir().unwrap();
        let d = Dir::open_ambient_dir(t.path(), ambient_authority()).unwrap();
        (t, FsCapsuleSink::new(d))
    }

    fn names(p: &std::path::Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(p)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn commit_moves_the_capsule_in_and_removes_staging() {
        let (t, s) = lib();
        let mut w = s.begin("01", "abc").unwrap();
        assert!(!w.put_object(SHA, b"gz").unwrap());
        assert!(w.put_object(SHA, b"gz").unwrap(), "second put deduplicates");
        assert_eq!(w.read_object(SHA).unwrap(), b"gz");
        w.write_out(b"capsule bytes").unwrap();
        w.commit(NAME).unwrap();
        assert_eq!(names(t.path()), vec![NAME.to_owned()]);
        assert_eq!(
            std::fs::read(t.path().join(NAME)).unwrap(),
            b"capsule bytes"
        );
    }

    #[test]
    fn an_existing_capsule_is_refused_and_left_byte_identical() {
        let (t, s) = lib();
        std::fs::write(t.path().join(NAME), b"original").unwrap();
        let mut w = s.begin("02", "abc").unwrap();
        w.write_out(b"new").unwrap();
        let e = w.commit(NAME).unwrap_err();
        assert_eq!(e.code, ErrorCode::CapsuleExists);
        assert_eq!(std::fs::read(t.path().join(NAME)).unwrap(), b"original");
        w.abort().unwrap();
        assert_eq!(names(t.path()), vec![NAME.to_owned()]);
    }

    #[test]
    fn abort_removes_the_staging_folder() {
        let (t, s) = lib();
        let mut w = s.begin("03", "abc").unwrap();
        w.put_object(SHA, b"x").unwrap();
        w.write_out(b"partial").unwrap();
        w.abort().unwrap();
        assert!(names(t.path()).is_empty());
    }

    #[test]
    fn a_staging_folder_with_a_wrong_token_is_left_in_place() {
        let (t, s) = lib();
        let mut w = s.begin("04", "abc").unwrap();
        let staging = t.path().join(w.staging_name());
        std::fs::write(staging.join(TOKEN_FILE), b"someone-else").unwrap();
        assert!(w.abort().is_err());
        assert!(staging.exists());
    }

    #[test]
    fn unknown_files_in_staging_are_never_deleted() {
        let (t, s) = lib();
        let mut w = s.begin("05", "abc").unwrap();
        let staging = t.path().join(w.staging_name());
        std::fs::write(staging.join("objects/user-file"), b"keep").unwrap();
        assert!(
            w.abort().is_err(),
            "objects/ isn't empty, so it can't be removed"
        );
        assert_eq!(
            std::fs::read(staging.join("objects/user-file")).unwrap(),
            b"keep"
        );
    }

    #[test]
    fn begin_refuses_an_existing_staging_folder_and_bad_ids() {
        let (_t, s) = lib();
        let _w = s.begin("06", "abc").unwrap();
        assert!(s.begin("06", "abc").is_err());
        assert!(s.begin("../x", "tok").is_err());
        assert!(s.begin("07", "a/b").is_err());
    }

    #[test]
    fn commit_refuses_names_that_are_not_capsule_ids() {
        let (_t, s) = lib();
        let mut w = s.begin("08", "abc").unwrap();
        for bad in ["../x.dormbit", "x", ".dormbit", "a/b.dormbit"] {
            assert_eq!(w.commit(bad).unwrap_err().code, ErrorCode::PathUnsafe);
        }
    }
}
