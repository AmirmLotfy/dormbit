//! The filesystem the restore writes through (design: RestoreFs trait).
//!
//! It has no "open existing for write", no truncate, no recursive create, and no general
//! delete. The only removal takes an [`OwnerTokenProof`], which only [`check_owner_token`]
//! can build. Native: `CapStdRestoreFs` in `dormbit-fs`. Tests and the web demo:
//! [`MemRestoreFs`].

use std::collections::BTreeMap;

use super::keys::{SafeName, fold_key, internal_name};
use crate::model::{DormbitError, ErrorCode};

pub const OWNER_FILE: &str = ".dormbit-owner";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsFault {
    Exists,
    NotFound,
    NoSpace,
    NotSupported,
    Unsafe,
    Io(String),
}

impl FsFault {
    pub fn to_error(&self, what: &str) -> DormbitError {
        match self {
            FsFault::Exists => DormbitError::new(
                ErrorCode::RestoreDestExists,
                format!("{what} already exists. Nothing was replaced."),
            ),
            FsFault::NoSpace => DormbitError::new(
                ErrorCode::RestoreInsufficientSpace,
                "The destination ran out of space. Nothing was restored.",
            ),
            FsFault::NotSupported => DormbitError::new(
                ErrorCode::RestoreVolumeUnsupported,
                "The destination volume doesn't support safe exclusive rename.",
            ),
            FsFault::Unsafe => {
                DormbitError::new(ErrorCode::PathUnsafe, format!("{what} is not a safe path."))
            }
            FsFault::NotFound | FsFault::Io(_) => DormbitError::new(
                ErrorCode::IoFailed,
                format!("DORMBIT could not write {what}."),
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetaKind {
    File,
    Dir,
    Symlink,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryMeta {
    pub kind: MetaKind,
    pub size: u64,
    pub mode: u32,
    pub dev: u64,
    pub ino: u64,
}

/// Proof that the owner token in one staging folder matched. Only [`check_owner_token`]
/// builds it, and `remove_staging` requires it.
#[derive(Debug)]
pub struct OwnerTokenProof {
    staging: SafeName,
}

impl OwnerTokenProof {
    pub fn staging(&self) -> &SafeName {
        &self.staging
    }
}

pub trait RestoreFs {
    type Dir;

    /// Creates a folder; fails with `Exists` if any entry has that name (or a variant the
    /// volume treats as the same).
    fn create_dir_excl(
        &mut self,
        parent: &Self::Dir,
        name: &SafeName,
    ) -> Result<Self::Dir, FsFault>;
    /// Creates a new file (never opens an existing one), writes `data`, sets `mode` on the
    /// open handle, and syncs it.
    fn write_new_file(
        &mut self,
        parent: &Self::Dir,
        name: &SafeName,
        data: &[u8],
        mode: u32,
    ) -> Result<(), FsFault>;
    fn sync_dir(&mut self, d: &Self::Dir) -> Result<(), FsFault>;
    fn open_dir_nofollow(&self, parent: &Self::Dir, name: &SafeName) -> Result<Self::Dir, FsFault>;
    fn read_file_nofollow(
        &self,
        parent: &Self::Dir,
        name: &SafeName,
        max: u64,
    ) -> Result<Vec<u8>, FsFault>;
    fn lookup_nofollow(&self, parent: &Self::Dir, name: &str)
    -> Result<Option<EntryMeta>, FsFault>;
    fn list(&self, d: &Self::Dir) -> Result<Vec<(String, EntryMeta)>, FsFault>;
    /// Moves `from` to `to` only if nothing named `to` exists. Never replaces anything.
    fn rename_exclusive(
        &mut self,
        from_dir: &Self::Dir,
        from: &SafeName,
        to_dir: &Self::Dir,
        to: &SafeName,
    ) -> Result<(), FsFault>;
    /// Removes one staging folder and everything in it, without following links.
    fn remove_staging(&mut self, parent: &Self::Dir, proof: OwnerTokenProof)
    -> Result<(), FsFault>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnerCheckFailure {
    /// The name isn't `.dormbit-staging-<op_id>`.
    NotStaging,
    StagingMissing,
    TokenMissing,
    TokenMismatch,
}

/// Reads `<staging>/.dormbit-owner` and compares it with `expected` (Requirements 12.5,
/// 12.6). A link at either name, or any difference, fails the check.
pub fn check_owner_token<F: RestoreFs>(
    fs: &F,
    parent: &F::Dir,
    staging: &SafeName,
    expected: &str,
) -> Result<OwnerTokenProof, OwnerCheckFailure> {
    // A proof can only ever name a DORMBIT staging folder, whatever the journal says.
    if !super::keys::is_staging_name(staging.as_str()) {
        return Err(OwnerCheckFailure::NotStaging);
    }
    match fs.lookup_nofollow(parent, staging.as_str()) {
        Ok(Some(m)) if m.kind == MetaKind::Dir => {}
        _ => return Err(OwnerCheckFailure::StagingMissing),
    }
    let dir = fs
        .open_dir_nofollow(parent, staging)
        .map_err(|_| OwnerCheckFailure::StagingMissing)?;
    let bytes = fs
        .read_file_nofollow(&dir, &internal_name(OWNER_FILE), 4096)
        .map_err(|_| OwnerCheckFailure::TokenMissing)?;
    if bytes != expected.as_bytes() {
        return Err(OwnerCheckFailure::TokenMismatch);
    }
    Ok(OwnerTokenProof {
        staging: staging.clone(),
    })
}

// ---- in memory -------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemNode {
    Dir,
    File { data: Vec<u8>, mode: u32 },
    Symlink { target: String },
}

/// An in-memory destination parent. Paths are `/`-joined keys; `""` is the parent itself.
/// `case_insensitive` makes name lookups fold case and normalization, like default APFS.
#[derive(Debug, Clone)]
pub struct MemRestoreFs {
    pub nodes: BTreeMap<String, (MemNode, u64)>,
    pub case_insensitive: bool,
    next_ino: u64,
    /// Test hook: called with each operation name before it runs; returning `Some` fails it.
    pub ops: u64,
    pub fail_at: Option<(u64, FsFault)>,
}

impl Default for MemRestoreFs {
    fn default() -> Self {
        let mut nodes = BTreeMap::new();
        nodes.insert(String::new(), (MemNode::Dir, 1));
        Self {
            nodes,
            case_insensitive: true,
            next_ino: 2,
            ops: 0,
            fail_at: None,
        }
    }
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_owned()
    } else {
        format!("{dir}/{name}")
    }
}

impl MemRestoreFs {
    pub fn new(case_insensitive: bool) -> Self {
        Self {
            case_insensitive,
            ..Self::default()
        }
    }

    /// The root handle (the destination parent).
    pub fn root(&self) -> String {
        String::new()
    }

    fn tick(&mut self) -> Result<(), FsFault> {
        self.ops += 1;
        if let Some((at, f)) = &self.fail_at
            && *at == self.ops
        {
            return Err(f.clone());
        }
        Ok(())
    }

    fn ino(&mut self) -> u64 {
        self.next_ino += 1;
        self.next_ino
    }

    /// Finds the actual key of `name` in `dir`, honoring case folding.
    fn find(&self, dir: &str, name: &str) -> Option<String> {
        let exact = join(dir, name);
        if self.nodes.contains_key(&exact) {
            return Some(exact);
        }
        if !self.case_insensitive {
            return None;
        }
        let want = fold_key(name);
        self.children(dir)
            .into_iter()
            .find(|c| fold_key(c) == want)
            .map(|c| join(dir, &c))
    }

    fn children(&self, dir: &str) -> Vec<String> {
        let prefix = if dir.is_empty() {
            String::new()
        } else {
            format!("{dir}/")
        };
        self.nodes
            .keys()
            .filter(|k| !k.is_empty() && k.starts_with(&prefix))
            .filter_map(|k| {
                let rest = &k[prefix.len()..];
                (!rest.contains('/')).then(|| rest.to_owned())
            })
            .collect()
    }

    fn is_dir(&self, path: &str) -> bool {
        matches!(self.nodes.get(path), Some((MemNode::Dir, _)))
    }

    fn meta(&self, path: &str) -> Option<EntryMeta> {
        self.nodes.get(path).map(|(n, ino)| match n {
            MemNode::Dir => EntryMeta {
                kind: MetaKind::Dir,
                size: 0,
                mode: 0o755,
                dev: 1,
                ino: *ino,
            },
            MemNode::File { data, mode } => EntryMeta {
                kind: MetaKind::File,
                size: data.len() as u64,
                mode: *mode,
                dev: 1,
                ino: *ino,
            },
            MemNode::Symlink { .. } => EntryMeta {
                kind: MetaKind::Symlink,
                size: 0,
                mode: 0o777,
                dev: 1,
                ino: *ino,
            },
        })
    }

    /// Test setup: an existing entry in the destination parent.
    pub fn add(&mut self, path: &str, node: MemNode) {
        let ino = self.ino();
        self.nodes.insert(path.to_owned(), (node, ino));
    }

    /// Everything outside `except` (by top-level name), for before/after comparisons.
    pub fn snapshot_except(&self, except: &[&str]) -> BTreeMap<String, (MemNode, u64)> {
        self.nodes
            .iter()
            .filter(|(k, _)| {
                let top = k.split('/').next().unwrap_or("");
                !except.contains(&top)
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

impl RestoreFs for MemRestoreFs {
    type Dir = String;

    fn create_dir_excl(&mut self, parent: &String, name: &SafeName) -> Result<String, FsFault> {
        self.tick()?;
        if !self.is_dir(parent) {
            return Err(FsFault::NotFound);
        }
        if self.find(parent, name.as_str()).is_some() {
            return Err(FsFault::Exists);
        }
        let p = join(parent, name.as_str());
        let ino = self.ino();
        self.nodes.insert(p.clone(), (MemNode::Dir, ino));
        Ok(p)
    }

    fn write_new_file(
        &mut self,
        parent: &String,
        name: &SafeName,
        data: &[u8],
        mode: u32,
    ) -> Result<(), FsFault> {
        self.tick()?;
        if !self.is_dir(parent) {
            return Err(FsFault::NotFound);
        }
        if self.find(parent, name.as_str()).is_some() {
            return Err(FsFault::Exists);
        }
        let ino = self.ino();
        self.nodes.insert(
            join(parent, name.as_str()),
            (
                MemNode::File {
                    data: data.to_vec(),
                    mode: mode & 0o777,
                },
                ino,
            ),
        );
        Ok(())
    }

    fn sync_dir(&mut self, d: &String) -> Result<(), FsFault> {
        self.tick()?;
        if self.is_dir(d) {
            Ok(())
        } else {
            Err(FsFault::NotFound)
        }
    }

    fn open_dir_nofollow(&self, parent: &String, name: &SafeName) -> Result<String, FsFault> {
        let p = self.find(parent, name.as_str()).ok_or(FsFault::NotFound)?;
        match self.nodes.get(&p) {
            Some((MemNode::Dir, _)) => Ok(p),
            Some((MemNode::Symlink { .. }, _)) => Err(FsFault::Unsafe),
            _ => Err(FsFault::Io("not a folder".into())),
        }
    }

    fn read_file_nofollow(
        &self,
        parent: &String,
        name: &SafeName,
        max: u64,
    ) -> Result<Vec<u8>, FsFault> {
        let p = self.find(parent, name.as_str()).ok_or(FsFault::NotFound)?;
        match self.nodes.get(&p) {
            Some((MemNode::File { data, .. }, _)) if data.len() as u64 <= max => Ok(data.clone()),
            Some((MemNode::File { .. }, _)) => Err(FsFault::Io("file too large".into())),
            Some((MemNode::Symlink { .. }, _)) => Err(FsFault::Unsafe),
            _ => Err(FsFault::Io("not a file".into())),
        }
    }

    fn lookup_nofollow(&self, parent: &String, name: &str) -> Result<Option<EntryMeta>, FsFault> {
        Ok(self.find(parent, name).and_then(|p| self.meta(&p)))
    }

    fn list(&self, d: &String) -> Result<Vec<(String, EntryMeta)>, FsFault> {
        if !self.is_dir(d) {
            return Err(FsFault::NotFound);
        }
        Ok(self
            .children(d)
            .into_iter()
            .filter_map(|c| self.meta(&join(d, &c)).map(|m| (c, m)))
            .collect())
    }

    fn rename_exclusive(
        &mut self,
        from_dir: &String,
        from: &SafeName,
        to_dir: &String,
        to: &SafeName,
    ) -> Result<(), FsFault> {
        self.tick()?;
        let src = self
            .find(from_dir, from.as_str())
            .ok_or(FsFault::NotFound)?;
        if self.find(to_dir, to.as_str()).is_some() {
            return Err(FsFault::Exists);
        }
        let dst = join(to_dir, to.as_str());
        let moved: Vec<(String, (MemNode, u64))> = self
            .nodes
            .iter()
            .filter(|(k, _)| **k == src || k.starts_with(&format!("{src}/")))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (k, v) in moved {
            self.nodes.remove(&k);
            let rest = &k[src.len()..];
            self.nodes.insert(format!("{dst}{rest}"), v);
        }
        Ok(())
    }

    fn remove_staging(&mut self, parent: &String, proof: OwnerTokenProof) -> Result<(), FsFault> {
        self.tick()?;
        let p = self
            .find(parent, proof.staging.as_str())
            .ok_or(FsFault::NotFound)?;
        let prefix = format!("{p}/");
        self.nodes.retain(|k, _| *k != p && !k.starts_with(&prefix));
        Ok(())
    }
}
