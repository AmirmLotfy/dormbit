//! Exclusive rename: `renameatx_np(RENAME_EXCL)` on directory handles (ADR-006).
//!
//! This is the project's only `unsafe` call. It moves `from` (relative to `from_dir`) to `to`
//! (relative to `to_dir`) and fails with `EEXIST` if anything exists at `to`, so DORMBIT can
//! never replace a file or folder. Both names are resolved from the handles, never from the
//! process working directory.

use std::ffi::CString;
use std::io;
use std::os::fd::AsRawFd;

use cap_std::fs::Dir;

#[derive(Debug, thiserror::Error)]
pub enum RenameError {
    #[error("the target already exists")]
    Exists,
    #[error("the volume does not support exclusive rename")]
    Unsupported,
    #[error("invalid name")]
    InvalidName,
    #[error("rename failed: {0}")]
    Io(io::Error),
}

/// A single path component: no `/`, no NUL, not `.` or `..`, not empty.
fn single_component(name: &str) -> Result<CString, RenameError> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') {
        return Err(RenameError::InvalidName);
    }
    CString::new(name).map_err(|_| RenameError::InvalidName)
}

/// Moves `from_dir/from` to `to_dir/to`, refusing if `to` exists. Names must be single path
/// components, so the call can't reach outside either handle.
#[allow(unsafe_code)]
pub fn rename_excl(from_dir: &Dir, from: &str, to_dir: &Dir, to: &str) -> Result<(), RenameError> {
    let from_c = single_component(from)?;
    let to_c = single_component(to)?;
    let from_fd = from_dir.as_raw_fd();
    let to_fd = to_dir.as_raw_fd();
    // SAFETY: `from_fd` and `to_fd` are open directory descriptors borrowed from `from_dir`
    // and `to_dir`, which outlive this call. `from_c` and `to_c` are NUL-terminated C strings
    // owned by this function and alive for the whole call. `renameatx_np` doesn't retain any
    // pointer after it returns, and `RENAME_EXCL` makes it fail rather than replace a target.
    let rc = unsafe {
        libc::renameatx_np(
            from_fd,
            from_c.as_ptr(),
            to_fd,
            to_c.as_ptr(),
            libc::RENAME_EXCL,
        )
    };
    if rc == 0 {
        return Ok(());
    }
    let err = io::Error::last_os_error();
    Err(match err.raw_os_error() {
        Some(libc::EEXIST) => RenameError::Exists,
        Some(libc::ENOTSUP) | Some(libc::EINVAL) => RenameError::Unsupported,
        _ => RenameError::Io(err),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cap_std::ambient_authority;

    fn tmp() -> (tempfile::TempDir, Dir) {
        let t = tempfile::tempdir().unwrap();
        let d = Dir::open_ambient_dir(t.path(), ambient_authority()).unwrap();
        (t, d)
    }

    #[test]
    fn moves_a_folder_when_the_target_is_absent() {
        let (_t, d) = tmp();
        d.create_dir("stage").unwrap();
        d.write("stage/f", b"x").unwrap();
        rename_excl(&d, "stage", &d, "final").unwrap();
        assert!(!d.exists("stage"));
        assert_eq!(d.read("final/f").unwrap(), b"x");
    }

    #[test]
    fn refuses_an_existing_folder_or_file_and_changes_nothing() {
        let (_t, d) = tmp();
        d.create_dir("final").unwrap();
        d.write("final/keep", b"original").unwrap();
        d.create_dir("stage").unwrap();
        assert!(matches!(
            rename_excl(&d, "stage", &d, "final"),
            Err(RenameError::Exists)
        ));
        assert_eq!(d.read("final/keep").unwrap(), b"original");
        assert!(d.exists("stage"));

        d.write("file", b"keep").unwrap();
        d.write("new", b"new").unwrap();
        assert!(matches!(
            rename_excl(&d, "new", &d, "file"),
            Err(RenameError::Exists)
        ));
        assert_eq!(d.read("file").unwrap(), b"keep");
    }

    #[test]
    fn works_across_two_handles_on_one_volume() {
        let (_t, d) = tmp();
        d.create_dir("a").unwrap();
        d.create_dir("b").unwrap();
        let a = d.open_dir("a").unwrap();
        let b = d.open_dir("b").unwrap();
        a.write("x", b"1").unwrap();
        rename_excl(&a, "x", &b, "y").unwrap();
        assert_eq!(b.read("y").unwrap(), b"1");
    }

    #[test]
    fn refuses_names_that_are_not_single_components() {
        let (_t, d) = tmp();
        for bad in ["", ".", "..", "a/b", "../x", "x\0y"] {
            assert!(matches!(
                rename_excl(&d, bad, &d, "ok"),
                Err(RenameError::InvalidName)
            ));
            assert!(matches!(
                rename_excl(&d, "ok", &d, bad),
                Err(RenameError::InvalidName)
            ));
        }
    }
}
