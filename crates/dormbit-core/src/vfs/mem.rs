use std::collections::BTreeMap;

use super::{DirEntry, EntryKind, Limited, Metadata, ReadVfs, RelPath, VfsError};

#[derive(Debug, Clone)]
enum Node {
    File {
        data: Vec<u8>,
        mode: u32,
        mtime: i64,
    },
    Dir,
    Symlink {
        target: String,
    },
    Special,
}

/// An in-memory project folder for tests, the CLI demo, and the web demo. Paths are validated
/// like every other path, and nothing is ever resolved through a symlink.
#[derive(Debug, Clone, Default)]
pub struct MemVfs {
    nodes: BTreeMap<String, Node>,
    /// Names that stand in for entries whose real name is not valid UTF-8: parent -> names.
    bad_names: BTreeMap<String, Vec<String>>,
    /// Paths that fail every read, to exercise the Unreadable group.
    unreadable: Vec<String>,
}

impl MemVfs {
    pub fn new() -> Self {
        Self::default()
    }

    fn ensure_parents(&mut self, path: &str) {
        let mut acc = String::new();
        let comps: Vec<&str> = path.split('/').collect();
        for c in &comps[..comps.len().saturating_sub(1)] {
            if !acc.is_empty() {
                acc.push('/');
            }
            acc.push_str(c);
            self.nodes.entry(acc.clone()).or_insert(Node::Dir);
        }
    }

    /// Adds a file (and its parent folders). Panics are avoided: an invalid path is ignored,
    /// so builders in tests must use valid paths (they are checked by `RelPath::parse`).
    pub fn add_file(&mut self, path: &str, data: impl Into<Vec<u8>>) -> &mut Self {
        self.add_file_with(path, data, 0o644, 0)
    }

    pub fn add_file_with(
        &mut self,
        path: &str,
        data: impl Into<Vec<u8>>,
        mode: u32,
        mtime: i64,
    ) -> &mut Self {
        if RelPath::parse(path).is_ok() {
            self.ensure_parents(path);
            self.nodes.insert(
                path.to_owned(),
                Node::File {
                    data: data.into(),
                    mode,
                    mtime,
                },
            );
        }
        self
    }

    pub fn add_dir(&mut self, path: &str) -> &mut Self {
        if RelPath::parse(path).is_ok() {
            self.ensure_parents(path);
            self.nodes.insert(path.to_owned(), Node::Dir);
        }
        self
    }

    pub fn add_symlink(&mut self, path: &str, target: &str) -> &mut Self {
        if RelPath::parse(path).is_ok() {
            self.ensure_parents(path);
            self.nodes.insert(
                path.to_owned(),
                Node::Symlink {
                    target: target.to_owned(),
                },
            );
        }
        self
    }

    pub fn add_special(&mut self, path: &str) -> &mut Self {
        if RelPath::parse(path).is_ok() {
            self.ensure_parents(path);
            self.nodes.insert(path.to_owned(), Node::Special);
        }
        self
    }

    /// Lists an entry in `parent` whose name is not valid UTF-8 (shown as `lossy`).
    pub fn add_bad_name(&mut self, parent: &str, lossy: &str) -> &mut Self {
        self.bad_names
            .entry(parent.to_owned())
            .or_default()
            .push(lossy.to_owned());
        self
    }

    /// Makes reads and stats of `path` fail with an I/O error.
    pub fn add_unreadable(&mut self, path: &str) -> &mut Self {
        self.unreadable.push(path.to_owned());
        self
    }

    /// Replaces a file's bytes (to simulate a change during capture).
    pub fn set_file_bytes(&mut self, path: &str, data: impl Into<Vec<u8>>) {
        if let Some(Node::File { data: d, .. }) = self.nodes.get_mut(path) {
            *d = data.into();
        }
    }

    fn get(&self, path: &RelPath) -> Result<&Node, VfsError> {
        if self.unreadable.iter().any(|u| u == path.as_str()) {
            return Err(VfsError::Io("simulated read failure".into()));
        }
        self.nodes.get(path.as_str()).ok_or(VfsError::NotFound)
    }
}

fn kind_of(n: &Node) -> EntryKind {
    match n {
        Node::File { .. } => EntryKind::File,
        Node::Dir => EntryKind::Dir,
        Node::Symlink { .. } => EntryKind::Symlink,
        Node::Special => EntryKind::Special,
    }
}

impl ReadVfs for MemVfs {
    fn read_dir(&self, dir: &RelPath) -> Result<Vec<DirEntry>, VfsError> {
        if !dir.is_root() && !matches!(self.get(dir)?, Node::Dir) {
            return Err(VfsError::Io("not a directory".into()));
        }
        let prefix = if dir.is_root() {
            String::new()
        } else {
            format!("{}/", dir.as_str())
        };
        let mut out = Vec::new();
        for (path, node) in self.nodes.range(prefix.clone()..) {
            let Some(rest) = path.strip_prefix(&prefix) else {
                break;
            };
            if rest.contains('/') {
                continue;
            }
            out.push(DirEntry {
                name: rest.to_owned(),
                name_ok: true,
                kind: kind_of(node),
            });
        }
        for lossy in self.bad_names.get(dir.as_str()).into_iter().flatten() {
            out.push(DirEntry {
                name: lossy.clone(),
                name_ok: false,
                kind: EntryKind::File,
            });
        }
        Ok(out)
    }

    fn symlink_metadata(&self, path: &RelPath) -> Result<Metadata, VfsError> {
        let n = self.get(path)?;
        Ok(match n {
            Node::File { data, mode, mtime } => Metadata {
                kind: EntryKind::File,
                len: data.len() as u64,
                mtime: *mtime,
                mode: *mode,
            },
            Node::Dir => Metadata {
                kind: EntryKind::Dir,
                len: 0,
                mtime: 0,
                mode: 0o755,
            },
            Node::Symlink { target } => Metadata {
                kind: EntryKind::Symlink,
                len: target.len() as u64,
                mtime: 0,
                mode: 0o777,
            },
            Node::Special => Metadata {
                kind: EntryKind::Special,
                len: 0,
                mtime: 0,
                mode: 0o644,
            },
        })
    }

    fn read_limited(&self, path: &RelPath, max_bytes: u64) -> Result<Limited, VfsError> {
        match self.get(path)? {
            Node::File { data, .. } => {
                let len = data.len() as u64;
                if len > max_bytes {
                    Ok(Limited::TooLarge { len })
                } else {
                    Ok(Limited::Complete(data.clone()))
                }
            }
            _ => Err(VfsError::Io("not a regular file".into())),
        }
    }

    fn read_link(&self, path: &RelPath) -> Result<String, VfsError> {
        match self.get(path)? {
            Node::Symlink { target } => Ok(target.clone()),
            _ => Err(VfsError::Io("not a symlink".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> RelPath {
        RelPath::parse(s).unwrap()
    }

    #[test]
    fn lists_direct_children_only_and_creates_parents() {
        let mut v = MemVfs::new();
        v.add_file("a/b/c.txt", "x")
            .add_file("a/d.txt", "y")
            .add_file("top.txt", "z");
        let mut root: Vec<_> = v
            .read_dir(&RelPath::root())
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect();
        root.sort();
        assert_eq!(root, ["a", "top.txt"]);
        let mut a: Vec<_> = v
            .read_dir(&p("a"))
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect();
        a.sort();
        assert_eq!(a, ["b", "d.txt"]);
    }

    #[test]
    fn symlinks_are_reported_not_followed() {
        let mut v = MemVfs::new();
        v.add_file("real.txt", "secret")
            .add_symlink("link", "real.txt");
        assert_eq!(
            v.symlink_metadata(&p("link")).unwrap().kind,
            EntryKind::Symlink
        );
        assert_eq!(v.read_link(&p("link")).unwrap(), "real.txt");
        assert!(
            v.read_limited(&p("link"), 100).is_err(),
            "reading through a link must fail"
        );
    }

    #[test]
    fn read_limited_reports_too_large() {
        let mut v = MemVfs::new();
        v.add_file("big", vec![0u8; 10]);
        assert_eq!(
            v.read_limited(&p("big"), 10).unwrap(),
            Limited::Complete(vec![0u8; 10])
        );
        assert_eq!(
            v.read_limited(&p("big"), 9).unwrap(),
            Limited::TooLarge { len: 10 }
        );
    }

    #[test]
    fn unreadable_and_missing_paths_fail() {
        let mut v = MemVfs::new();
        v.add_file("x", "1").add_unreadable("x");
        assert!(matches!(v.symlink_metadata(&p("x")), Err(VfsError::Io(_))));
        assert_eq!(v.symlink_metadata(&p("nope")), Err(VfsError::NotFound));
    }

    #[test]
    fn invalid_paths_are_never_stored() {
        let mut v = MemVfs::new();
        v.add_file("../escape", "x").add_file("/abs", "x");
        assert!(v.read_dir(&RelPath::root()).unwrap().is_empty());
    }
}
