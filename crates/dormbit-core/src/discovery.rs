//! Discovery (scope-and-discovery, DSC-1..DSC-3): finds project folders inside a granted
//! folder over a `ReadVfs`. Never follows symlinks, never descends into rebuildable folders
//! or `.git`, and stops at a project root (projects aren't nested). Activity and size come
//! from bounded `lstat`s only; no file is read except the manifest that names a project.
//! Discovery never marks a project dormant: that is the user's manual mark (DSC-4,
//! Requirement 12.1). `idle_days` is a number of days since the latest file change, never a
//! label.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::plan::REBUILDABLE_DIRS;
use crate::util::{sha256_hex, time::rfc3339};
use crate::vfs::{EntryKind, Limited, ReadVfs, RelPath};

/// Files whose presence makes a folder a project root.
pub const ROOT_MARKERS: [&str; 11] = [
    "package.json",
    "Cargo.toml",
    "pyproject.toml",
    "requirements.txt",
    "go.mod",
    "Gemfile",
    "pom.xml",
    "build.gradle",
    "composer.json",
    "Package.swift",
    ".git",
];

const DOC_FILES: [&str; 4] = ["README.md", "README", "CHANGELOG.md", "CONTRIBUTING.md"];
const AGENT_ARTIFACTS: [&str; 7] = [
    ".kiro",
    ".specstory",
    ".cursor",
    "AGENTS.md",
    "CLAUDE.md",
    "GEMINI.md",
    ".cursorrules",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryLimits {
    /// Folder depth searched for project roots below the granted folder.
    pub max_depth: u32,
    /// `lstat`s per project for activity and size.
    pub max_entries_per_project: u64,
    /// Entries visited while looking for roots.
    pub max_walk_entries: u64,
}

impl Default for DiscoveryLimits {
    fn default() -> Self {
        Self {
            max_depth: 6,
            max_entries_per_project: 200_000,
            max_walk_entries: 500_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredProject {
    pub id: String,
    /// Path relative to the granted folder (`""` if the grant itself is the project).
    pub rel_root: String,
    pub name: String,
    pub name_source: String,
    pub manifests: Vec<String>,
    pub docs: Vec<String>,
    pub agent_artifacts: Vec<String>,
    pub has_git: bool,
    pub latest_mtime: Option<String>,
    pub latest_mtime_unix: Option<i64>,
    pub files_scanned: u64,
    pub total_bytes: u64,
    pub rebuildable_bytes: u64,
    /// The scan hit a limit, so activity and size are lower bounds.
    pub partial: bool,
    pub idle_days: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveryOutput {
    pub projects: Vec<DiscoveredProject>,
    pub entries_scanned: u64,
    pub partial: bool,
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_owned()
    } else {
        format!("{dir}/{name}")
    }
}

/// A stable project ID from the grant and the relative root.
pub fn project_id(scope_id: &str, rel_root: &str) -> String {
    let h = sha256_hex(format!("{scope_id}\0{rel_root}").as_bytes());
    format!("p-{}", &h[..16])
}

struct Stats {
    /// Entries this whole discovery may still visit (shared by every project).
    budget: u64,
    latest: Option<i64>,
    files: u64,
    total: u64,
    rebuildable: u64,
    partial: bool,
}

fn measure<V: ReadVfs>(
    vfs: &V,
    dir: &RelPath,
    in_rebuildable: bool,
    st: &mut Stats,
    limits: &DiscoveryLimits,
    depth: u32,
) {
    if depth > 64 {
        st.partial = true;
        return;
    }
    let Ok(entries) = vfs.read_dir(dir) else {
        return;
    };
    for e in entries {
        if st.files >= limits.max_entries_per_project || st.budget == 0 {
            st.partial = true;
            return;
        }
        st.budget -= 1;
        let Ok(p) = dir.join(&e.name) else { continue };
        match e.kind {
            EntryKind::Dir => {
                if dir.is_root() && e.name == ".git" {
                    continue;
                }
                let rebuildable = in_rebuildable || REBUILDABLE_DIRS.contains(&e.name.as_str());
                measure(vfs, &p, rebuildable, st, limits, depth + 1);
            }
            EntryKind::File => {
                st.files += 1;
                if let Ok(m) = vfs.symlink_metadata(&p) {
                    st.total = st.total.saturating_add(m.len);
                    if in_rebuildable {
                        st.rebuildable = st.rebuildable.saturating_add(m.len);
                    } else {
                        st.latest = Some(st.latest.map_or(m.mtime, |l| l.max(m.mtime)));
                    }
                }
            }
            EntryKind::Symlink | EntryKind::Special => {}
        }
    }
}

/// A view of one subfolder of the granted folder as its own root.
struct Sub<'a, V: ReadVfs> {
    vfs: &'a V,
    prefix: String,
}

impl<V: ReadVfs> Sub<'_, V> {
    fn map(&self, p: &RelPath) -> Result<RelPath, crate::vfs::VfsError> {
        if self.prefix.is_empty() {
            return Ok(p.clone());
        }
        let full = if p.is_root() {
            self.prefix.clone()
        } else {
            join(&self.prefix, p.as_str())
        };
        RelPath::parse(&full).map_err(|_| crate::vfs::VfsError::Unsafe)
    }
}

impl<V: ReadVfs> ReadVfs for Sub<'_, V> {
    fn read_dir(&self, d: &RelPath) -> Result<Vec<crate::vfs::DirEntry>, crate::vfs::VfsError> {
        self.vfs.read_dir(&self.map(d)?)
    }
    fn symlink_metadata(&self, p: &RelPath) -> Result<crate::vfs::Metadata, crate::vfs::VfsError> {
        self.vfs.symlink_metadata(&self.map(p)?)
    }
    fn read_limited(&self, p: &RelPath, m: u64) -> Result<Limited, crate::vfs::VfsError> {
        self.vfs.read_limited(&self.map(p)?, m)
    }
    fn read_prefix(&self, p: &RelPath, n: u64) -> Result<Vec<u8>, crate::vfs::VfsError> {
        self.vfs.read_prefix(&self.map(p)?, n)
    }
    fn read_link(&self, p: &RelPath) -> Result<String, crate::vfs::VfsError> {
        self.vfs.read_link(&self.map(p)?)
    }
}

/// A `ReadVfs` rooted at one discovered project (for planning, evidence, and capture).
pub fn project_view<'a, V: ReadVfs>(vfs: &'a V, rel_root: &str) -> impl ReadVfs + 'a {
    Sub {
        vfs,
        prefix: rel_root.to_owned(),
    }
}

fn describe<V: ReadVfs>(
    vfs: &V,
    scope_id: &str,
    rel_root: &str,
    names: &[String],
    now: i64,
    limits: &DiscoveryLimits,
    budget: &mut u64,
) -> DiscoveredProject {
    let view = Sub {
        vfs,
        prefix: rel_root.to_owned(),
    };
    let has = |n: &str| names.iter().any(|x| x == n);
    let manifests: Vec<String> = ROOT_MARKERS
        .iter()
        .filter(|m| **m != ".git" && has(m))
        .map(|m| (*m).to_owned())
        .collect();
    let folder = rel_root
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("project")
        .to_owned();
    let (name, name_source) = match RelPath::parse("package.json")
        .ok()
        .and_then(|p| view.read_limited(&p, 1 << 20).ok())
    {
        Some(Limited::Complete(b)) => match serde_json::from_slice::<serde_json::Value>(&b)
            .ok()
            .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(str::to_owned))
        {
            // Untrusted: capped and stripped of control characters.
            Some(n) if !n.trim().is_empty() => (
                n.chars().filter(|c| !c.is_control()).take(200).collect(),
                "package.json".to_owned(),
            ),
            _ => (folder.clone(), "folder".to_owned()),
        },
        _ => (folder.clone(), "folder".to_owned()),
    };
    let mut st = Stats {
        budget: *budget,
        latest: None,
        files: 0,
        total: 0,
        rebuildable: 0,
        partial: false,
    };
    measure(&view, &RelPath::root(), false, &mut st, limits, 0);
    *budget = st.budget;
    // A file dated in the future counts as changed today.
    let idle_days = st.latest.map(|l| (now - l).div_euclid(86_400).max(0));
    DiscoveredProject {
        id: project_id(scope_id, rel_root),
        rel_root: rel_root.to_owned(),
        name,
        name_source,
        manifests,
        docs: DOC_FILES
            .iter()
            .filter(|d| has(d))
            .map(|d| (*d).to_owned())
            .collect(),
        agent_artifacts: AGENT_ARTIFACTS
            .iter()
            .filter(|a| has(a))
            .map(|a| (*a).to_owned())
            .collect(),
        has_git: has(".git"),
        latest_mtime: st.latest.map(rfc3339),
        latest_mtime_unix: st.latest,
        files_scanned: st.files,
        total_bytes: st.total,
        rebuildable_bytes: st.rebuildable,
        partial: st.partial,
        idle_days,
    }
}

/// Finds projects under the granted folder. `now` comes from the host clock.
pub fn discover<V: ReadVfs>(
    vfs: &V,
    scope_id: &str,
    now: i64,
    limits: &DiscoveryLimits,
) -> DiscoveryOutput {
    let mut found: BTreeMap<String, DiscoveredProject> = BTreeMap::new();
    let mut scanned = 0u64;
    let mut partial = false;
    // One budget for the whole call: walking and measuring both spend it.
    let mut budget = limits.max_walk_entries;
    let mut stack: Vec<(String, u32)> = vec![(String::new(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rel) = (if dir.is_empty() {
            Ok(RelPath::root())
        } else {
            RelPath::parse(&dir)
        }) else {
            continue;
        };
        let Ok(entries) = vfs.read_dir(&rel) else {
            continue;
        };
        scanned += entries.len() as u64;
        if entries.len() as u64 > budget {
            partial = true;
            break;
        }
        budget -= entries.len() as u64;
        let names: Vec<String> = entries
            .iter()
            .filter(|e| e.name_ok)
            .map(|e| e.name.clone())
            .collect();
        if names.iter().any(|n| ROOT_MARKERS.contains(&n.as_str())) {
            let p = describe(vfs, scope_id, &dir, &names, now, limits, &mut budget);
            partial |= p.partial;
            found.insert(dir.clone(), p);
            continue;
        }
        let mut subdirs: Vec<String> = entries
            .iter()
            .filter(|e| e.kind == EntryKind::Dir && e.name_ok)
            .filter(|e| !e.name.starts_with('.') && !REBUILDABLE_DIRS.contains(&e.name.as_str()))
            .map(|e| join(&dir, &e.name))
            .collect();
        if depth >= limits.max_depth {
            // Only partial if there was something deeper to look at.
            partial |= !subdirs.is_empty();
            continue;
        }
        subdirs.sort();
        subdirs.reverse();
        stack.extend(subdirs.into_iter().map(|d| (d, depth + 1)));
    }
    DiscoveryOutput {
        projects: found.into_values().collect(),
        entries_scanned: scanned,
        partial,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vfs::MemVfs;

    #[test]
    fn one_budget_bounds_the_whole_discovery_and_no_dormant_label_exists() {
        let mut v = MemVfs::new();
        for i in 0..50 {
            v.add_file(&format!("p{i}/package.json"), "{}");
            for j in 0..20 {
                v.add_dir(&format!("p{i}/empty{j}"));
            }
        }
        let limits = DiscoveryLimits {
            max_walk_entries: 300,
            ..DiscoveryLimits::default()
        };
        let out = discover(&v, "g", 0, &limits);
        assert!(out.partial);
        assert!(out.projects.len() < 50);
        let json = serde_json::to_string(&out).unwrap();
        assert!(!json.contains("dormant"));
        for word in ["abandoned", "stale", "inactive"] {
            assert!(!json.contains(word));
        }
    }

    #[test]
    fn finds_roots_without_nesting_and_measures_activity() {
        let mut v = MemVfs::new();
        v.add_file_with(
            "work/old/package.json",
            r#"{"name":"old-app"}"#,
            0o644,
            1_000_000,
        )
        .add_file_with("work/old/src/a.ts", "x", 0o644, 2_000_000)
        .add_file_with("work/old/node_modules/x/i.js", "yyyy", 0o644, 9_999_999_999)
        .add_file_with("work/old/packages/inner/package.json", "{}", 0o644, 1)
        .add_file_with("work/new/Cargo.toml", "[package]", 0o644, 99_000_000)
        .add_file("notes/readme.txt", "not a project")
        .add_symlink("work/link", "/elsewhere");
        let now = 100_000_000;
        let out = discover(&v, "g1", now, &DiscoveryLimits::default());
        let names: Vec<(&str, &str, Option<i64>)> = out
            .projects
            .iter()
            .map(|p| (p.rel_root.as_str(), p.name.as_str(), p.idle_days))
            .collect();
        assert_eq!(
            names,
            vec![
                ("work/new", "new", Some(11)),
                ("work/old", "old-app", Some(1134))
            ]
        );
        let old = &out.projects[1];
        assert_eq!(
            old.latest_mtime_unix,
            Some(2_000_000),
            "rebuildable files don't count as activity"
        );
        assert_eq!(old.rebuildable_bytes, 4);
        assert_eq!(old.name_source, "package.json");
        assert_eq!(old.id, project_id("g1", "work/old"));
    }
}
