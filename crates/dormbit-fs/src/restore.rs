//! Native restore support (transactional-restore Tasks 23-25): the cap-std `RestoreFs`,
//! the read-only Volume_Probe, and the durable Journal_Store.
//!
//! No new `unsafe`: the commit uses [`crate::rename::rename_excl`], and `fstatfs` goes through
//! `rustix`'s safe wrapper.

use std::collections::BTreeMap;
use std::io::{Read as _, Write as _};

use cap_std::fs::{
    Dir, MetadataExt as _, OpenOptions, OpenOptionsExt as _, Permissions, PermissionsExt as _,
};
use dormbit_core::model::{DormbitError, ErrorCode, RestoreState};
use dormbit_core::restore::fs::{EntryMeta, FsFault, MetaKind, OwnerTokenProof, RestoreFs};
use dormbit_core::restore::journal::{Journal, JournalStore, WriteRecord};
use dormbit_core::restore::keys::SafeName;
use dormbit_core::restore::{CaseSensitivity, RestoreReport, VolumeFacts};

use crate::rename::{RenameError, rename_excl};

fn fault(e: &std::io::Error) -> FsFault {
    match e.raw_os_error() {
        Some(libc::EEXIST) => FsFault::Exists,
        Some(libc::ENOENT) => FsFault::NotFound,
        Some(libc::ENOSPC) | Some(libc::EDQUOT) => FsFault::NoSpace,
        Some(libc::ENOTSUP) => FsFault::NotSupported,
        Some(libc::ELOOP) => FsFault::Unsafe,
        _ => match e.kind() {
            std::io::ErrorKind::AlreadyExists => FsFault::Exists,
            std::io::ErrorKind::NotFound => FsFault::NotFound,
            _ => FsFault::Io(e.kind().to_string()),
        },
    }
}

fn meta_of(m: &cap_std::fs::Metadata) -> EntryMeta {
    let ft = m.file_type();
    EntryMeta {
        kind: if ft.is_symlink() {
            MetaKind::Symlink
        } else if ft.is_dir() {
            MetaKind::Dir
        } else if ft.is_file() {
            MetaKind::File
        } else {
            MetaKind::Other
        },
        size: m.len(),
        mode: m.mode(),
        dev: m.dev(),
        ino: m.ino(),
    }
}

/// Opens `name` under `parent` as a folder, refusing a link or a swap (same dev and inode
/// before and after).
fn descend(parent: &Dir, name: &str) -> Result<Dir, FsFault> {
    let before = parent.symlink_metadata(name).map_err(|e| fault(&e))?;
    if before.file_type().is_symlink() {
        return Err(FsFault::Unsafe);
    }
    if !before.is_dir() {
        return Err(FsFault::Io("not a folder".into()));
    }
    let d = parent.open_dir(name).map_err(|e| fault(&e))?;
    let after = d.dir_metadata().map_err(|e| fault(&e))?;
    if after.dev() != before.dev() || after.ino() != before.ino() {
        return Err(FsFault::Unsafe);
    }
    Ok(d)
}

/// `RestoreFs` over cap-std handles rooted at the destination parent.
#[derive(Default)]
pub struct CapStdRestoreFs;

impl RestoreFs for CapStdRestoreFs {
    type Dir = Dir;

    fn create_dir_excl(&mut self, parent: &Dir, name: &SafeName) -> Result<Dir, FsFault> {
        parent.create_dir(name.as_str()).map_err(|e| fault(&e))?;
        descend(parent, name.as_str())
    }

    fn write_new_file(
        &mut self,
        parent: &Dir,
        name: &SafeName,
        data: &[u8],
        mode: u32,
    ) -> Result<(), FsFault> {
        let mut o = OpenOptions::new();
        o.write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW);
        let mut f = parent.open_with(name.as_str(), &o).map_err(|e| fault(&e))?;
        f.write_all(data).map_err(|e| fault(&e))?;
        // fchmod on the open handle: setuid, setgid, and sticky are never set (& 0o777).
        f.set_permissions(Permissions::from_mode(mode & 0o777))
            .map_err(|e| fault(&e))?;
        f.sync_all().map_err(|e| fault(&e))
    }

    fn sync_dir(&mut self, d: &Dir) -> Result<(), FsFault> {
        let f = d.open(".").map_err(|e| fault(&e))?;
        f.sync_all().map_err(|e| fault(&e))
    }

    fn open_dir_nofollow(&self, parent: &Dir, name: &SafeName) -> Result<Dir, FsFault> {
        descend(parent, name.as_str())
    }

    fn read_file_nofollow(
        &self,
        parent: &Dir,
        name: &SafeName,
        max: u64,
    ) -> Result<Vec<u8>, FsFault> {
        let before = parent
            .symlink_metadata(name.as_str())
            .map_err(|e| fault(&e))?;
        if before.file_type().is_symlink() {
            return Err(FsFault::Unsafe);
        }
        if !before.is_file() {
            return Err(FsFault::Io("not a file".into()));
        }
        if before.len() > max {
            return Err(FsFault::Io("file too large".into()));
        }
        let mut o = OpenOptions::new();
        o.read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        let f = parent.open_with(name.as_str(), &o).map_err(|e| fault(&e))?;
        let after = f.metadata().map_err(|e| fault(&e))?;
        if !after.is_file() || after.dev() != before.dev() || after.ino() != before.ino() {
            return Err(FsFault::Unsafe);
        }
        let mut buf = Vec::new();
        f.take(max.saturating_add(1))
            .read_to_end(&mut buf)
            .map_err(|e| fault(&e))?;
        if buf.len() as u64 > max {
            return Err(FsFault::Io("file too large".into()));
        }
        Ok(buf)
    }

    fn lookup_nofollow(&self, parent: &Dir, name: &str) -> Result<Option<EntryMeta>, FsFault> {
        if name.is_empty() || name.contains('/') || name == "." || name == ".." {
            return Err(FsFault::Unsafe);
        }
        match parent.symlink_metadata(name) {
            Ok(m) => Ok(Some(meta_of(&m))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(fault(&e)),
        }
    }

    fn list(&self, d: &Dir) -> Result<Vec<(String, EntryMeta)>, FsFault> {
        let mut out = Vec::new();
        for e in d.entries().map_err(|e| fault(&e))? {
            let e = e.map_err(|e| fault(&e))?;
            let Ok(name) = e.file_name().into_string() else {
                return Err(FsFault::Io("a name is not UTF-8".into()));
            };
            let m = d.symlink_metadata(&name).map_err(|e| fault(&e))?;
            out.push((name, meta_of(&m)));
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(out)
    }

    fn rename_exclusive(
        &mut self,
        from_dir: &Dir,
        from: &SafeName,
        to_dir: &Dir,
        to: &SafeName,
    ) -> Result<(), FsFault> {
        rename_excl(from_dir, from.as_str(), to_dir, to.as_str()).map_err(|e| match e {
            RenameError::Exists => FsFault::Exists,
            RenameError::Unsupported => FsFault::NotSupported,
            RenameError::InvalidName => FsFault::Unsafe,
            RenameError::Io(io) => fault(&io),
        })
    }

    fn remove_staging(&mut self, parent: &Dir, proof: OwnerTokenProof) -> Result<(), FsFault> {
        // cap-std's remove_dir_all removes links without following them. The proof shows the
        // owner token in this exact folder matched this restore.
        parent
            .remove_dir_all(proof.staging().as_str())
            .map_err(|e| fault(&e))
    }
}

// ---- Volume_Probe ----------------------------------------------------------------------------

/// Filesystem type and free space from `fstatfs`, and case sensitivity from a read-only
/// lookup of a case-swapped existing name (Requirements 4.6-4.9). Writes nothing.
pub fn probe_volume(parent: &Dir) -> Result<VolumeFacts, DormbitError> {
    let fail = |_| {
        DormbitError::new(
            ErrorCode::IoFailed,
            "DORMBIT could not read the destination volume.",
        )
    };
    let st = rustix::fs::fstatfs(parent).map_err(fail)?;
    let name: Vec<u8> = st
        .f_fstypename
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    let fs_type = String::from_utf8_lossy(&name).into_owned();
    let block_size = u64::from(st.f_bsize).max(512);
    let free_bytes = st.f_bavail.saturating_mul(block_size);
    Ok(VolumeFacts {
        fs_type,
        block_size,
        free_bytes,
        case: case_sensitivity(parent),
    })
}

fn case_sensitivity(parent: &Dir) -> CaseSensitivity {
    let Ok(entries) = parent.entries() else {
        return CaseSensitivity::Unknown;
    };
    for e in entries.flatten() {
        let Ok(name) = e.file_name().into_string() else {
            continue;
        };
        if !name.bytes().any(|b| b.is_ascii_alphabetic()) {
            continue;
        }
        let swapped: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_lowercase() {
                    c.to_ascii_uppercase()
                } else {
                    c.to_ascii_lowercase()
                }
            })
            .collect();
        let (Ok(a), b) = (
            parent.symlink_metadata(&name),
            parent.symlink_metadata(&swapped),
        ) else {
            continue;
        };
        return match b {
            Ok(b) if b.dev() == a.dev() && b.ino() == a.ino() => CaseSensitivity::Insensitive,
            Ok(_) => continue,
            Err(_) => CaseSensitivity::Sensitive,
        };
    }
    CaseSensitivity::Unknown
}

/// The names in the destination parent (for the target-variant check).
pub fn list_names(parent: &Dir) -> Result<Vec<String>, DormbitError> {
    let fail = |_| {
        DormbitError::new(
            ErrorCode::IoFailed,
            "DORMBIT could not list the destination folder.",
        )
    };
    let mut out = Vec::new();
    for e in parent.entries().map_err(fail)? {
        let e = e.map_err(fail)?;
        out.push(e.file_name().to_string_lossy().into_owned());
    }
    Ok(out)
}

// ---- Journal_Store ---------------------------------------------------------------------------

/// `restores/<op_id>/{journal.json, writes.jsonl, report.json}` in DORMBIT's own state
/// folder, mode 0600, with atomic replace for `journal.json` and `report.json`.
pub struct FileJournalStore {
    root: Dir,
    counter: u64,
}

fn jfail(_: std::io::Error) -> DormbitError {
    DormbitError::new(
        ErrorCode::IoFailed,
        "DORMBIT could not write its restore journal.",
    )
}

fn op_ok(op: &str) -> Result<(), DormbitError> {
    if !op.is_empty() && op.len() <= 64 && op.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
        Ok(())
    } else {
        Err(DormbitError::new(
            ErrorCode::PathUnsafe,
            "The restore ID is not valid.",
        ))
    }
}

impl FileJournalStore {
    pub fn new(root: Dir) -> Self {
        Self { root, counter: 0 }
    }

    fn op_dir(&self, op: &str) -> Result<Dir, DormbitError> {
        op_ok(op)?;
        descend(&self.root, op)
            .map_err(|_| jfail(std::io::Error::from(std::io::ErrorKind::NotFound)))
    }

    fn write_atomic(&mut self, d: &Dir, name: &str, bytes: &[u8]) -> Result<(), DormbitError> {
        self.counter += 1;
        let tmp = format!("{name}.tmp-{}-{}", std::process::id(), self.counter);
        let mut o = OpenOptions::new();
        o.write(true).create_new(true).mode(0o600);
        let mut f = d.open_with(&tmp, &o).map_err(jfail)?;
        let r = f.write_all(bytes).and_then(|()| f.sync_all());
        drop(f);
        if let Err(e) = r.and_then(|()| d.rename(&tmp, d, name)) {
            let _ = d.remove_file(&tmp);
            return Err(jfail(e));
        }
        if let Ok(h) = d.open(".") {
            let _ = h.sync_all();
        }
        Ok(())
    }

    fn read_json<T: serde::de::DeserializeOwned>(d: &Dir, name: &str) -> Option<T> {
        let mut f = d.open(name).ok()?;
        let mut buf = Vec::new();
        (&mut f).take(16 << 20).read_to_end(&mut buf).ok()?;
        serde_json::from_slice(&buf).ok()
    }
}

impl JournalStore for FileJournalStore {
    fn create(&mut self, j: &Journal) -> Result<(), DormbitError> {
        op_ok(&j.op_id)?;
        self.root.create_dir(&j.op_id).map_err(jfail)?;
        let d = self.op_dir(&j.op_id)?;
        let bytes =
            serde_json::to_vec_pretty(j).map_err(|_| jfail(std::io::Error::other("json")))?;
        self.write_atomic(&d, "journal.json", &bytes)
    }

    fn set_state(&mut self, j: &Journal) -> Result<(), DormbitError> {
        let d = self.op_dir(&j.op_id)?;
        // The write log is synced before the state that depends on it (Requirement 11.8).
        if let Ok(f) = d.open("writes.jsonl") {
            f.sync_all().map_err(jfail)?;
        }
        let bytes =
            serde_json::to_vec_pretty(j).map_err(|_| jfail(std::io::Error::other("json")))?;
        self.write_atomic(&d, "journal.json", &bytes)
    }

    fn append_write(&mut self, op_id: &str, rec: &WriteRecord) -> Result<(), DormbitError> {
        let d = self.op_dir(op_id)?;
        let mut o = OpenOptions::new();
        o.append(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW);
        let mut f = d.open_with("writes.jsonl", &o).map_err(jfail)?;
        let mut line = serde_json::to_vec(rec).map_err(|_| jfail(std::io::Error::other("json")))?;
        line.push(b'\n');
        f.write_all(&line).map_err(jfail)
    }

    fn put_report(&mut self, op_id: &str, r: &RestoreReport) -> Result<(), DormbitError> {
        let d = self.op_dir(op_id)?;
        let bytes =
            serde_json::to_vec_pretty(r).map_err(|_| jfail(std::io::Error::other("json")))?;
        self.write_atomic(&d, "report.json", &bytes)
    }

    fn load(&self, op_id: &str) -> Result<Option<Journal>, DormbitError> {
        op_ok(op_id)?;
        let Ok(d) = descend(&self.root, op_id) else {
            return Ok(None);
        };
        Ok(Self::read_json(&d, "journal.json"))
    }

    fn report(&self, op_id: &str) -> Result<Option<RestoreReport>, DormbitError> {
        op_ok(op_id)?;
        let Ok(d) = descend(&self.root, op_id) else {
            return Ok(None);
        };
        Ok(Self::read_json(&d, "report.json"))
    }

    fn writes(&self, op_id: &str) -> Result<Vec<WriteRecord>, DormbitError> {
        let d = self.op_dir(op_id)?;
        let Ok(mut f) = d.open("writes.jsonl") else {
            return Ok(Vec::new());
        };
        let mut s = String::new();
        (&mut f)
            .take(64 << 20)
            .read_to_string(&mut s)
            .map_err(jfail)?;
        Ok(s.lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect())
    }

    fn pending(&self) -> Result<Vec<Journal>, DormbitError> {
        let mut out = BTreeMap::new();
        for e in self.root.entries().map_err(jfail)?.flatten() {
            let Ok(name) = e.file_name().into_string() else {
                continue;
            };
            if op_ok(&name).is_err() {
                continue;
            }
            let Ok(d) = descend(&self.root, &name) else {
                continue;
            };
            let Some(j) = Self::read_json::<Journal>(&d, "journal.json") else {
                continue;
            };
            let has_report = d.symlink_metadata("report.json").is_ok();
            if j.state != RestoreState::Cleaned || !has_report {
                out.insert(name, j);
            }
        }
        Ok(out.into_values().collect())
    }
}
