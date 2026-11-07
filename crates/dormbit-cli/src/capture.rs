//! `dormbit-cli capture`: builds the default plan (undecided items excluded, warnings
//! acknowledged), captures it in memory, and writes `<capsule_id>.dormbit` into `--out`.
//! Refuses to overwrite an existing file. Dev harness only; the app uses `dormbit-fs`.

use std::io::Write as _;

use dormbit_core::capsule::manifest::ManifestProject;
use dormbit_core::capsule::{
    CapsuleLimits, CaptureContext, CaptureJob, MemRegistry, MemWorkspace, run_to_end,
};
use dormbit_core::model::{Decision, ProjectId, ScopeId};
use dormbit_core::plan::{ack_id, apply_edit, approve};
use dormbit_core::vfs::{CountingIds, FixedClock};

use crate::plan::{PlanArgs, load, parse, plan_from};

/// The fixed demo clock, so repeated runs give the same bytes.
const DEMO_NOW: i64 = 1_800_000_000;

pub fn run(args: &[String]) -> Result<String, String> {
    let (out_dir, rest) = split_out(args)?;
    let a: PlanArgs = parse(&rest)?;
    let vfs = load(&a)?;
    let mut plan = plan_from(&vfs, a.treatment)?;
    let undecided: Vec<String> = plan
        .items
        .iter()
        .filter(|i| i.decision == Decision::Undecided)
        .map(|i| i.path.clone())
        .collect();
    for p in undecided {
        apply_edit(&mut plan, &p, Decision::Exclude).map_err(|e| e.message)?;
    }
    let acks: Vec<String> = plan
        .items
        .iter()
        .filter_map(|i| i.warning.as_deref().map(|w| ack_id(&i.path, w)))
        .collect();
    let limits = CapsuleLimits::default();
    let clock = FixedClock(DEMO_NOW);
    let ok = approve(&plan, &plan.digest, &acks, &clock, &limits).map_err(|e| e.message)?;
    let name = a.prefix.clone().unwrap_or_else(|| "project".to_owned());
    let ctx = CaptureContext {
        app_version: dormbit_core::VERSION.to_owned(),
        project_id: ProjectId::from("cli"),
        project: ManifestProject {
            name,
            name_source: "folder".to_owned(),
        },
        library_grant: ScopeId::from("cli"),
        limits,
    };
    let mut job = CaptureJob::new(
        &vfs,
        plan,
        ok,
        MemWorkspace::new(),
        MemRegistry::default(),
        &clock,
        &CountingIds::default(),
        ctx,
    )
    .map_err(|e| e.message)?;
    let out = run_to_end(&mut job).map_err(|e| format!("{:?}: {}", e.code, e.message))?;
    let (ws, _) = job.into_parts();
    let bytes = ws
        .library
        .get(&out.record.file_name)
        .ok_or("capsule missing after commit")?;
    let path = std::path::Path::new(&out_dir).join(&out.record.file_name);
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| format!("cannot create {}: {e}", path.display()))?;
    f.write_all(bytes).map_err(|e| e.to_string())?;
    f.sync_all().map_err(|e| e.to_string())?;
    Ok(format!(
        "wrote {}\nsize: {} bytes\nfiles: {}\nobjects: {}\nexcluded: {}\nsha256: {}\n",
        out.record.file_name,
        out.record.size,
        out.manifest.totals.files,
        out.manifest.totals.objects,
        out.manifest.totals.excluded,
        out.record.file_sha256
    ))
}

fn split_out(args: &[String]) -> Result<(String, Vec<String>), String> {
    let mut out = None;
    let mut rest = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--out" {
            out = it.next().cloned();
        } else {
            rest.push(a.clone());
        }
    }
    Ok((
        out.ok_or("usage: dormbit-cli capture --tar <file> [--prefix <dir>] --out <dir>")?,
        rest,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_capsule_the_system_tar_lists_in_order_and_refuses_to_overwrite() {
        let dir = std::env::temp_dir().join(format!("dormbit-cli-capture-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let fixture = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/dockline/dockline-invoices.tar"
        );
        let args: Vec<String> = [
            "--tar",
            fixture,
            "--prefix",
            "dockline-invoices",
            "--out",
            dir.to_str().unwrap(),
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        let report = run(&args).unwrap();
        let name = report
            .lines()
            .next()
            .unwrap()
            .trim_start_matches("wrote ")
            .to_owned();
        let capsule = dir.join(&name);

        // The system tar (bsdtar on macOS) is an independent reader. This lists an archive;
        // it never runs project code.
        let listing = std::process::Command::new("tar")
            .arg("-tf")
            .arg(&capsule)
            .output()
            .unwrap();
        assert!(listing.status.success());
        let names: Vec<String> = String::from_utf8(listing.stdout)
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect();
        assert_eq!(names[0], "DORMBIT");
        assert_eq!(names[1], "manifest.json");
        assert_eq!(names.last().map(String::as_str), Some("END"));
        let objs = &names[2..names.len() - 1];
        assert!(objs.windows(2).all(|w| w[0] < w[1]));

        assert!(run(&args).is_err(), "second run must not overwrite");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
