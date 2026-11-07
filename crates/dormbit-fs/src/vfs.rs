//! `ReadVfs` over a cap-std directory handle for one granted folder.
//!
//! Nothing here follows a symlink, at any depth. cap-std resolves in-folder links on its own
//! (even with `O_NOFOLLOW`), so every path is walked one folder at a time from the root:
//! each component is `lstat`ed and must be a real folder, then opened, and the opened handle
//! must have the same device and inode. The last component is handled relative to its parent
//! handle: metadata is `lstat`-style, files get the same before/after identity check, and
//! links are read as text only. A link or a swap anywhere along the path is refused.

use cap_std::fs::{Dir, MetadataExt as _, OpenOptions, OpenOptionsExt as _};
use dormbit_core::vfs::{DirEntry, EntryKind, Limited, Metadata, ReadVfs, RelPath, VfsError};
use std::io::Read as _;

pub struct FsVfs {
    root: Dir,
}

impl FsVfs {
    pub fn new(root: Dir) -> Self {
        Self { root }
    }
}

fn io(e: std::io::Error) -> VfsError {
    match e.kind() {
        std::io::ErrorKind::NotFound => VfsError::NotFound,
        std::io::ErrorKind::PermissionDenied => VfsError::Io("permission denied".into()),
        _ if e.raw_os_error() == Some(libc::ELOOP) => VfsError::Unsafe,
        _ => VfsError::Io(e.kind().to_string()),
    }
}

fn kind_of(ft: cap_std::fs::FileType) -> EntryKind {
    if ft.is_symlink() {
        EntryKind::Symlink
    } else if ft.is_dir() {
        EntryKind::Dir
    } else if ft.is_file() {
        EntryKind::File
    } else {
        EntryKind::Special
    }
}

/// Opens `name` (one component) under `parent` as a folder, refusing a link or a swap.
fn descend(parent: &Dir, name: &str) -> Result<Dir, VfsError> {
    let before = parent.symlink_metadata(name).map_err(io)?;
    if before.file_type().is_symlink() || !before.is_dir() {
        return Err(VfsError::Unsafe);
    }
    let d = parent.open_dir(name).map_err(io)?;
    let after = d.dir_metadata().map_err(io)?;
    if after.dev() != before.dev() || after.ino() != before.ino() {
        return Err(VfsError::Unsafe);
    }
    Ok(d)
}

impl FsVfs {
    /// The handle of `path`'s parent folder and `path`'s last component. `None` for the root.
    fn parent_of<'p>(&self, path: &'p RelPath) -> Result<(Dir, Option<&'p str>), VfsError> {
        let mut d = self.root.try_clone().map_err(io)?;
        if path.is_root() {
            return Ok((d, None));
        }
        let mut parts = path.as_str().split('/').peekable();
        while let Some(part) = parts.next() {
            if parts.peek().is_none() {
                return Ok((d, Some(part)));
            }
            d = descend(&d, part)?;
        }
        Err(VfsError::Unsafe)
    }

    fn dir_at(&self, path: &RelPath) -> Result<Dir, VfsError> {
        match self.parent_of(path)? {
            (d, None) => Ok(d),
            (parent, Some(name)) => descend(&parent, name),
        }
    }

    /// Opens a regular file: `lstat` before must show a regular file, `fstat` after must show
    /// the same device and inode. `O_NONBLOCK` keeps a FIFO swapped in from blocking.
    fn open_file(&self, path: &RelPath) -> Result<cap_std::fs::File, VfsError> {
        let (parent, Some(name)) = self.parent_of(path)? else {
            return Err(VfsError::Io("not a regular file".into()));
        };
        let before = parent.symlink_metadata(name).map_err(io)?;
        if before.file_type().is_symlink() {
            return Err(VfsError::Unsafe);
        }
        if !before.is_file() {
            return Err(VfsError::Io("not a regular file".into()));
        }
        let mut opts = OpenOptions::new();
        opts.read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        let f = parent.open_with(name, &opts).map_err(io)?;
        let after = f.metadata().map_err(io)?;
        if !after.is_file() || after.dev() != before.dev() || after.ino() != before.ino() {
            return Err(VfsError::Unsafe);
        }
        Ok(f)
    }
}

impl ReadVfs for FsVfs {
    fn read_dir(&self, dir: &RelPath) -> Result<Vec<DirEntry>, VfsError> {
        let d = self.dir_at(dir)?;
        let mut out = Vec::new();
        for e in d.entries().map_err(io)? {
            let e = e.map_err(io)?;
            let raw = e.file_name();
            let (name, name_ok) = match raw.to_str() {
                Some(s) => (s.to_owned(), true),
                None => (raw.to_string_lossy().into_owned(), false),
            };
            let kind = e.file_type().map(kind_of).unwrap_or(EntryKind::Special);
            out.push(DirEntry {
                name,
                name_ok,
                kind,
            });
        }
        Ok(out)
    }

    fn symlink_metadata(&self, path: &RelPath) -> Result<Metadata, VfsError> {
        let m = match self.parent_of(path)? {
            (d, None) => d.dir_metadata().map_err(io)?,
            (parent, Some(name)) => parent.symlink_metadata(name).map_err(io)?,
        };
        Ok(Metadata {
            kind: kind_of(m.file_type()),
            len: m.len(),
            mtime: m.mtime(),
            mode: m.mode(),
        })
    }

    fn read_limited(&self, path: &RelPath, max_bytes: u64) -> Result<Limited, VfsError> {
        let f = self.open_file(path)?;
        let len = f.metadata().map_err(io)?.len();
        if len > max_bytes {
            return Ok(Limited::TooLarge { len });
        }
        let mut buf = Vec::with_capacity(usize::try_from(len).unwrap_or(0));
        // Read one byte past the limit so a file that grew after `stat` is noticed.
        f.take(max_bytes.saturating_add(1))
            .read_to_end(&mut buf)
            .map_err(io)?;
        if buf.len() as u64 > max_bytes {
            return Ok(Limited::TooLarge {
                len: buf.len() as u64,
            });
        }
        Ok(Limited::Complete(buf))
    }

    fn read_prefix(&self, path: &RelPath, max_bytes: u64) -> Result<Vec<u8>, VfsError> {
        let f = self.open_file(path)?;
        let mut buf = Vec::new();
        f.take(max_bytes).read_to_end(&mut buf).map_err(io)?;
        Ok(buf)
    }

    fn read_link(&self, path: &RelPath) -> Result<String, VfsError> {
        let (parent, Some(name)) = self.parent_of(path)? else {
            return Err(VfsError::Io("not a symlink".into()));
        };
        // `read_link_contents` returns the link text as stored; it never resolves it.
        let target = parent.read_link_contents(name).map_err(io)?;
        target
            .to_str()
            .map(str::to_owned)
            .ok_or_else(|| VfsError::Io("link text is not UTF-8".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cap_std::ambient_authority;
    use std::os::unix::fs::{PermissionsExt as _, symlink};

    fn setup() -> (tempfile::TempDir, tempfile::TempDir, FsVfs) {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let p = root.path();
        std::fs::create_dir_all(p.join("src")).unwrap();
        std::fs::write(p.join("src/a.rs"), b"fn a() {}\n").unwrap();
        std::fs::write(p.join("run.sh"), b"#!/bin/sh\n").unwrap();
        std::fs::set_permissions(p.join("run.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(outside.path().join("secret"), b"outside").unwrap();
        symlink(outside.path().join("secret"), p.join("abs-link")).unwrap();
        symlink("src/a.rs", p.join("rel-link")).unwrap();
        symlink(outside.path(), p.join("dir-link")).unwrap();
        let dir = Dir::open_ambient_dir(p, ambient_authority()).unwrap();
        (root, outside, FsVfs::new(dir))
    }

    fn rp(s: &str) -> RelPath {
        RelPath::parse(s).unwrap()
    }

    #[test]
    fn lists_entries_without_following_links() {
        let (_r, _o, v) = setup();
        let mut e = v.read_dir(&RelPath::root()).unwrap();
        e.sort_by(|a, b| a.name.cmp(&b.name));
        let kinds: Vec<(&str, EntryKind)> = e.iter().map(|x| (x.name.as_str(), x.kind)).collect();
        assert!(kinds.contains(&("abs-link", EntryKind::Symlink)));
        assert!(kinds.contains(&("dir-link", EntryKind::Symlink)));
        assert!(kinds.contains(&("src", EntryKind::Dir)));
        assert!(kinds.contains(&("run.sh", EntryKind::File)));
    }

    #[test]
    fn metadata_reports_links_as_links_with_mode() {
        let (_r, _o, v) = setup();
        assert_eq!(
            v.symlink_metadata(&rp("abs-link")).unwrap().kind,
            EntryKind::Symlink
        );
        assert_eq!(
            v.symlink_metadata(&rp("run.sh")).unwrap().mode & 0o777,
            0o755
        );
    }

    #[test]
    fn reading_through_any_symlink_is_refused() {
        let (_r, _o, v) = setup();
        assert!(v.read_limited(&rp("abs-link"), 1 << 20).is_err());
        assert!(v.read_limited(&rp("rel-link"), 1 << 20).is_err());
        assert!(v.read_prefix(&rp("abs-link"), 16).is_err());
        assert!(v.read_dir(&rp("dir-link")).is_err());
        assert!(v.read_limited(&rp("dir-link/secret"), 1 << 20).is_err());
    }

    #[test]
    fn a_link_to_a_folder_inside_the_grant_is_never_walked_through() {
        let (r, _o, v) = setup();
        symlink("src", r.path().join("src-link")).unwrap();
        assert_eq!(
            v.symlink_metadata(&rp("src-link")).unwrap().kind,
            EntryKind::Symlink
        );
        assert!(matches!(v.read_dir(&rp("src-link")), Err(VfsError::Unsafe)));
        assert!(matches!(
            v.read_limited(&rp("src-link/a.rs"), 1 << 20),
            Err(VfsError::Unsafe)
        ));
        assert!(matches!(
            v.read_prefix(&rp("src-link/a.rs"), 4),
            Err(VfsError::Unsafe)
        ));
        assert!(matches!(
            v.symlink_metadata(&rp("src-link/a.rs")),
            Err(VfsError::Unsafe)
        ));
        // The real path still works.
        assert!(v.read_limited(&rp("src/a.rs"), 1 << 20).is_ok());
    }

    #[test]
    fn link_text_is_returned_unresolved() {
        let (_r, _o, v) = setup();
        assert_eq!(v.read_link(&rp("rel-link")).unwrap(), "src/a.rs");
    }

    #[test]
    fn limited_reads_and_prefixes() {
        let (_r, _o, v) = setup();
        assert_eq!(
            v.read_limited(&rp("src/a.rs"), 1 << 20).unwrap(),
            Limited::Complete(b"fn a() {}\n".to_vec())
        );
        assert!(matches!(
            v.read_limited(&rp("src/a.rs"), 3).unwrap(),
            Limited::TooLarge { len: 10 }
        ));
        assert_eq!(v.read_prefix(&rp("src/a.rs"), 4).unwrap(), b"fn a");
    }

    #[test]
    fn fifos_are_special_and_never_read() {
        let (r, _o, v) = setup();
        // Test setup only: the system `mkfifo` tool, so the crate keeps exactly one `unsafe`.
        let ok = std::process::Command::new("mkfifo")
            .arg(r.path().join("pipe"))
            .status()
            .unwrap();
        assert!(ok.success());
        assert_eq!(
            v.symlink_metadata(&rp("pipe")).unwrap().kind,
            EntryKind::Special
        );
        assert!(v.read_limited(&rp("pipe"), 16).is_err());
    }
}
