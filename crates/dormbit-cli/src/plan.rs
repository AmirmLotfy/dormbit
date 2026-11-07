//! `dormbit-cli plan`: prints the capture plan's groups, reason codes, warnings, and digest.
//!
//! The project is read from a tar archive into a `MemVfs` (`--tar <file> [--prefix <dir>]`).
//! Reading a live folder waits for the cap-std `ReadVfs` in `dormbit-fs`.

use std::fmt::Write as _;
use std::io::Read;

use dormbit_core::capsule::CapsuleLimits;
use dormbit_core::model::{CapturePlan, Decision, GitTreatment};
use dormbit_core::plan::build_plan;
use dormbit_core::vfs::{CountingIds, MemVfs};

pub struct PlanArgs {
    pub tar: String,
    pub prefix: Option<String>,
    pub treatment: GitTreatment,
}

pub fn parse(args: &[String]) -> Result<PlanArgs, String> {
    let mut tar = None;
    let mut prefix = None;
    let mut treatment = GitTreatment::Full;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--tar" => tar = it.next().cloned(),
            "--prefix" => prefix = it.next().cloned(),
            "--working-tree-only" => treatment = GitTreatment::WorkingTreeOnly,
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    let tar =
        tar.ok_or("usage: dormbit-cli plan --tar <file> [--prefix <dir>] [--working-tree-only]")?;
    Ok(PlanArgs {
        tar,
        prefix,
        treatment,
    })
}

/// Loads the entries under `prefix/` (or the whole archive) into a `MemVfs`.
pub fn load_tar<R: Read>(reader: R, prefix: Option<&str>) -> Result<MemVfs, String> {
    let mut archive = tar::Archive::new(reader);
    let mut vfs = MemVfs::new();
    let want = prefix.map(|p| format!("{}/", p.trim_end_matches('/')));
    for entry in archive.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        let name = entry
            .path()
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .into_owned();
        let rel = match &want {
            Some(p) => match name.strip_prefix(p.as_str()) {
                Some(r) => r.to_owned(),
                None => continue,
            },
            None => name,
        };
        let rel = rel.trim_end_matches('/');
        if rel.is_empty() {
            continue;
        }
        let kind = entry.header().entry_type();
        if kind.is_dir() {
            vfs.add_dir(rel);
        } else if kind.is_symlink() {
            let target = entry
                .link_name()
                .map_err(|e| e.to_string())?
                .map(|t| t.to_string_lossy().into_owned())
                .unwrap_or_default();
            vfs.add_symlink(rel, &target);
        } else if kind.is_file() {
            let mode = entry.header().mode().unwrap_or(0o644);
            let mtime = i64::try_from(entry.header().mtime().unwrap_or(0)).unwrap_or(0);
            let mut data = Vec::new();
            entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
            vfs.add_file_with(rel, data, mode, mtime);
        }
    }
    Ok(vfs)
}

pub fn plan_from(vfs: &MemVfs, treatment: GitTreatment) -> Result<CapturePlan, String> {
    build_plan(
        vfs,
        treatment,
        &CapsuleLimits::default(),
        &CountingIds::default(),
    )
    .map_err(|e| format!("{:?}: {}", e.code, e.message))
}

fn decision(d: Decision) -> &'static str {
    match d {
        Decision::Include => "include",
        Decision::Exclude => "exclude",
        Decision::Undecided => "undecided",
    }
}

/// The human-readable report. Included `project_file` and `git_internal` items without a
/// warning are summarized by count, so the interesting rows stay visible.
pub fn render(plan: &CapturePlan) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "groups:");
    for g in &plan.groups {
        let _ = writeln!(
            out,
            "  {:<24} {:>6} items {:>12} bytes",
            g.name,
            g.file_count.get(),
            g.bytes.get()
        );
    }
    let _ = writeln!(out, "items needing attention:");
    let mut quiet = 0u64;
    for it in &plan.items {
        let routine = it.decision == Decision::Include
            && it.warning.is_none()
            && (it.reason == "project_file" || it.reason == "git_internal");
        if routine {
            quiet += 1;
            continue;
        }
        let lock = if it.editable { "" } else { " (locked)" };
        let warn = it
            .warning
            .as_deref()
            .map(|w| format!("  warning={w}"))
            .unwrap_or_default();
        let _ = writeln!(
            out,
            "  {:<9} {:<26} {}{lock}{warn}",
            decision(it.decision),
            it.reason,
            it.path
        );
    }
    let t = &plan.totals;
    let _ = writeln!(out, "  … and {quiet} routine included items");
    let _ = writeln!(
        out,
        "totals: {} items, {} included ({} bytes), {} not secret-scanned",
        t.files.get(),
        t.included_files.get(),
        t.included_bytes.get(),
        t.secret_scan_skipped.get()
    );
    let _ = writeln!(out, "digest: {}", plan.digest);
    out
}

pub fn run(args: &[String]) -> Result<String, String> {
    let a = parse(args)?;
    let file = std::fs::File::open(&a.tar).map_err(|e| format!("cannot open {}: {e}", a.tar))?;
    let vfs = load_tar(file, a.prefix.as_deref())?;
    let plan = plan_from(&vfs, a.treatment)?;
    Ok(render(&plan))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dockline() -> String {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/dockline/dockline-invoices.tar"
        );
        let args: Vec<String> = ["--tar", path, "--prefix", "dockline-invoices"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        run(&args).unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn prints_groups_reasons_and_the_digest_for_dockline() {
        let out = dockline();
        assert!(out.contains("Excluded: sensitive"));
        assert!(out.contains("sensitive_env"));
        assert!(out.contains(".env (locked)"));
        assert!(out.contains("undecided data_file"));
        assert!(out.contains("rebuildable"));
        let digest = out
            .lines()
            .find_map(|l| l.strip_prefix("digest: "))
            .unwrap_or_default();
        assert_eq!(digest.len(), 64);
    }

    #[test]
    fn output_is_deterministic() {
        assert_eq!(dockline(), dockline());
    }

    #[test]
    fn refuses_unknown_arguments_and_a_missing_tar() {
        assert!(parse(&["--nope".to_owned()]).is_err());
        assert!(parse(&[]).is_err());
    }
}
