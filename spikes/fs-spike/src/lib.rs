//! Task 3 spike (a1): prove the two filesystem primitives the restore engine
//! depends on, on this machine:
//!   1. cap-std confines access to a directory handle (no `..`, no absolute
//!      paths, no symlink escape).
//!   2. `renameatx_np(RENAME_EXCL)` commits a staged directory atomically and
//!      refuses to replace an existing target.

use std::ffi::CString;
use std::io;
use std::os::fd::AsRawFd;

use cap_std::fs::Dir;

/// Rename `from` to `to`, both relative to `dir`, failing with `EEXIST` if
/// `to` already exists. Never overwrites.
pub fn rename_exclusive(dir: &Dir, from: &str, to: &str) -> io::Result<()> {
    let from_c = CString::new(from).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    let to_c = CString::new(to).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    let fd = dir.as_raw_fd();
    // SAFETY: `fd` is a valid open directory descriptor borrowed from `dir`
    // for the duration of the call, and both paths are NUL-terminated C
    // strings that outlive the call.
    let rc = unsafe { libc::renameatx_np(fd, from_c.as_ptr(), fd, to_c.as_ptr(), libc::RENAME_EXCL) };
    if rc == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cap_std::ambient_authority;
    use std::os::unix::fs::symlink;

    fn open(path: &std::path::Path) -> Dir {
        Dir::open_ambient_dir(path, ambient_authority()).expect("open dir")
    }

    #[test]
    fn exclusive_rename_commits_when_target_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = open(tmp.path());
        dir.create_dir("stage").unwrap();
        dir.write("stage/file.txt", b"hello").unwrap();

        rename_exclusive(&dir, "stage", "final").unwrap();

        assert!(!dir.exists("stage"));
        assert_eq!(dir.read("final/file.txt").unwrap(), b"hello");
    }

    #[test]
    fn exclusive_rename_refuses_existing_target_and_leaves_it_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = open(tmp.path());
        dir.create_dir("final").unwrap();
        dir.write("final/keep.txt", b"original").unwrap();
        dir.create_dir("stage").unwrap();
        dir.write("stage/new.txt", b"new").unwrap();

        let err = rename_exclusive(&dir, "stage", "final").unwrap_err();

        assert_eq!(err.raw_os_error(), Some(libc::EEXIST));
        assert_eq!(dir.read("final/keep.txt").unwrap(), b"original");
        assert!(!dir.exists("final/new.txt"));
        assert!(dir.exists("stage/new.txt"));
    }

    #[test]
    fn exclusive_rename_refuses_existing_file_target() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = open(tmp.path());
        dir.write("final", b"a file, not a dir").unwrap();
        dir.create_dir("stage").unwrap();

        let err = rename_exclusive(&dir, "stage", "final").unwrap_err();
        assert_eq!(err.raw_os_error(), Some(libc::EEXIST));
    }

    #[test]
    fn cap_std_rejects_parent_traversal() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("root")).unwrap();
        std::fs::write(tmp.path().join("outside.txt"), b"secret").unwrap();
        let root = open(&tmp.path().join("root"));

        assert!(root.read("../outside.txt").is_err());
        assert!(root.write("../escaped.txt", b"x").is_err());
        assert!(!tmp.path().join("escaped.txt").exists());
    }

    #[test]
    fn cap_std_rejects_absolute_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tmp.path().join("outside.txt");
        std::fs::write(&outside, b"secret").unwrap();
        std::fs::create_dir(tmp.path().join("root")).unwrap();
        let root = open(&tmp.path().join("root"));

        assert!(root.read(&outside).is_err());
    }

    #[test]
    fn cap_std_rejects_symlink_escape() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("root")).unwrap();
        std::fs::create_dir(tmp.path().join("elsewhere")).unwrap();
        std::fs::write(tmp.path().join("elsewhere/secret.txt"), b"secret").unwrap();
        symlink(tmp.path().join("elsewhere"), tmp.path().join("root/link")).unwrap();
        symlink("../elsewhere/secret.txt", tmp.path().join("root/rel-link")).unwrap();
        let root = open(&tmp.path().join("root"));

        assert!(root.read("link/secret.txt").is_err(), "absolute symlink escaped");
        assert!(root.read("rel-link").is_err(), "relative symlink escaped");
        assert!(root.write("link/planted.txt", b"x").is_err());
        assert!(!tmp.path().join("elsewhere/planted.txt").exists());
    }

    #[test]
    fn cap_std_allows_symlink_inside_root() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("root/sub")).unwrap();
        std::fs::write(tmp.path().join("root/sub/a.txt"), b"inside").unwrap();
        symlink("sub/a.txt", tmp.path().join("root/inner-link")).unwrap();
        let root = open(&tmp.path().join("root"));

        assert_eq!(root.read("inner-link").unwrap(), b"inside");
    }
}
