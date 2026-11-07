//! Reads Git's HEAD and refs straight from `.git` over a `ReadVfs` (design: Git ref reader).
//! Never runs git, never reads config, never follows symlinks. Loose refs win over packed.

use std::collections::BTreeMap;

use crate::capsule::manifest::{HeadRef, RefEntry};
use crate::vfs::{EntryKind, Limited, ReadVfs, RelPath};

const SMALL: u64 = 4096;
const PACKED_MAX: u64 = 16 << 20;
const MAX_REFS: usize = 50_000;
const MAX_DEPTH: u32 = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRefs {
    pub head: Option<HeadRef>,
    pub refs: Vec<RefEntry>,
}

fn is_oid(s: &str) -> bool {
    (s.len() == 40 || s.len() == 64) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn read_small<V: ReadVfs>(vfs: &V, path: &str, max: u64) -> Option<String> {
    let p = RelPath::parse(path).ok()?;
    match vfs.read_limited(&p, max).ok()? {
        Limited::Complete(b) => String::from_utf8(b).ok(),
        Limited::TooLarge { .. } => None,
    }
}

fn walk_loose<V: ReadVfs>(
    vfs: &V,
    dir: &RelPath,
    name: &str,
    depth: u32,
    out: &mut BTreeMap<String, String>,
) {
    if depth > MAX_DEPTH || out.len() >= MAX_REFS {
        return;
    }
    let Ok(mut entries) = vfs.read_dir(dir) else {
        return;
    };
    entries.sort_by(|a, b| a.name.as_bytes().cmp(b.name.as_bytes()));
    for e in entries {
        if !e.name_ok || out.len() >= MAX_REFS {
            continue;
        }
        let Ok(p) = dir.join(&e.name) else { continue };
        let ref_name = format!("{name}/{}", e.name);
        match e.kind {
            EntryKind::Dir => walk_loose(vfs, &p, &ref_name, depth + 1, out),
            EntryKind::File => {
                if let Some(text) = read_small(vfs, p.as_str(), SMALL) {
                    let t = text.trim();
                    if is_oid(t) {
                        out.insert(ref_name, t.to_ascii_lowercase());
                    }
                }
            }
            EntryKind::Symlink | EntryKind::Special => {}
        }
    }
}

/// Reads refs from `.git` at the project root. Returns `head: None` if HEAD is missing or
/// unreadable; an unborn branch gives `commit: None`.
pub fn read_refs<V: ReadVfs>(vfs: &V) -> GitRefs {
    let mut refs: BTreeMap<String, String> = BTreeMap::new();
    if let Some(packed) = read_small(vfs, ".git/packed-refs", PACKED_MAX) {
        for line in packed.lines() {
            if line.starts_with('#') || line.starts_with('^') || refs.len() >= MAX_REFS {
                continue;
            }
            if let Some((oid, name)) = line.split_once(' ')
                && is_oid(oid)
                && name.starts_with("refs/")
            {
                refs.insert(name.to_owned(), oid.to_ascii_lowercase());
            }
        }
    }
    if let Ok(dir) = RelPath::parse(".git/refs") {
        walk_loose(vfs, &dir, "refs", 0, &mut refs);
    }

    let head = read_small(vfs, ".git/HEAD", SMALL).map(|text| {
        let t = text.trim();
        if let Some(target) = t.strip_prefix("ref: ") {
            let target = target.trim();
            HeadRef {
                branch: Some(
                    target
                        .strip_prefix("refs/heads/")
                        .unwrap_or(target)
                        .to_owned(),
                ),
                detached: false,
                commit: refs.get(target).cloned(),
            }
        } else if is_oid(t) {
            HeadRef {
                branch: None,
                detached: true,
                commit: Some(t.to_ascii_lowercase()),
            }
        } else {
            HeadRef {
                branch: None,
                detached: false,
                commit: None,
            }
        }
    });

    GitRefs {
        head,
        refs: refs
            .into_iter()
            .map(|(name, target)| RefEntry { name, target })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vfs::MemVfs;

    const A: &str = "1111111111111111111111111111111111111111";
    const B: &str = "2222222222222222222222222222222222222222";

    #[test]
    fn reads_head_loose_and_packed_refs_with_loose_winning() {
        let mut v = MemVfs::new();
        v.add_file(".git/HEAD", "ref: refs/heads/main\n")
            .add_file(".git/refs/heads/main", format!("{A}\n"))
            .add_file(
                ".git/packed-refs",
                format!("# pack-refs\n{B} refs/heads/main\n{B} refs/tags/v1\n^{A}\n"),
            );
        let r = read_refs(&v);
        let head = r.head.unwrap();
        assert_eq!(head.branch.as_deref(), Some("main"));
        assert!(!head.detached);
        assert_eq!(head.commit.as_deref(), Some(A));
        let names: Vec<_> = r
            .refs
            .iter()
            .map(|e| (e.name.as_str(), e.target.as_str()))
            .collect();
        assert_eq!(names, vec![("refs/heads/main", A), ("refs/tags/v1", B)]);
    }

    #[test]
    fn detached_and_unborn_heads() {
        let mut v = MemVfs::new();
        v.add_file(".git/HEAD", format!("{A}\n"));
        let h = read_refs(&v).head.unwrap();
        assert!(h.detached);
        assert_eq!(h.commit.as_deref(), Some(A));

        let mut v = MemVfs::new();
        v.add_file(".git/HEAD", "ref: refs/heads/new\n");
        let h = read_refs(&v).head.unwrap();
        assert_eq!(h.branch.as_deref(), Some("new"));
        assert_eq!(h.commit, None);
    }

    #[test]
    fn no_head_and_no_symlink_following() {
        let mut v = MemVfs::new();
        v.add_symlink(".git/refs/heads/evil", "../../../../etc/passwd");
        let r = read_refs(&v);
        assert_eq!(r.head, None);
        assert!(r.refs.is_empty());
    }
}
