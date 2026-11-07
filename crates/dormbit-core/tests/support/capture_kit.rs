//! Shared helpers for capture tests: build an approved plan over a `MemVfs`, run a capture,
//! and read the result back with the `tar` crate (an independent reader).
#![allow(dead_code)]

use std::io::Read as _;

use dormbit_core::capsule::manifest::{Manifest, ManifestProject};
use dormbit_core::capsule::{
    Budget, CapsuleLimits, CaptureContext, CaptureJob, CaptureOutput, Job, MemRegistry,
    MemWorkspace, Step,
};
use dormbit_core::model::{
    ApprovedPlan, CapturePlan, Decision, DormbitError, GitTreatment, ProjectId, ScopeId,
};
use dormbit_core::plan::{ack_id, apply_edit, approve, build_plan};
use dormbit_core::vfs::{CountingIds, FixedClock, ReadVfs};

pub const NOW: i64 = 1_800_000_000;

pub fn ctx() -> CaptureContext {
    CaptureContext {
        app_version: "0.1.0".into(),
        project_id: ProjectId::from("p-1"),
        project: ManifestProject {
            name: "demo".into(),
            name_source: "folder".into(),
        },
        library_grant: ScopeId::from("lib-1"),
        limits: CapsuleLimits::default(),
    }
}

/// Builds a plan, excludes every undecided item, applies `includes`, and approves with every
/// warning acknowledged.
pub fn approved<V: ReadVfs>(
    v: &V,
    treatment: GitTreatment,
    includes: &[&str],
) -> (CapturePlan, ApprovedPlan) {
    let mut plan = build_plan(
        v,
        treatment,
        &CapsuleLimits::default(),
        &CountingIds::default(),
    )
    .unwrap_or_else(|e| panic!("plan: {e}"));
    let undecided: Vec<String> = plan
        .items
        .iter()
        .filter(|i| i.decision == Decision::Undecided)
        .map(|i| i.path.clone())
        .collect();
    for p in undecided {
        apply_edit(&mut plan, &p, Decision::Exclude).unwrap_or_else(|e| panic!("{e}"));
    }
    for p in includes {
        apply_edit(&mut plan, p, Decision::Include).unwrap_or_else(|e| panic!("{e}"));
    }
    let acks: Vec<String> = plan
        .items
        .iter()
        .filter_map(|i| i.warning.as_deref().map(|w| ack_id(&i.path, w)))
        .collect();
    let ok = approve(
        &plan,
        &plan.digest,
        &acks,
        &FixedClock(NOW),
        &CapsuleLimits::default(),
    )
    .unwrap_or_else(|e| panic!("approve: {e}"));
    (plan, ok)
}

pub fn job<'a, V: ReadVfs>(
    v: &'a V,
    plan: CapturePlan,
    ok: ApprovedPlan,
    ws: MemWorkspace,
    reg: MemRegistry,
) -> CaptureJob<'a, V, MemWorkspace, MemRegistry> {
    CaptureJob::new(
        v,
        plan,
        ok,
        ws,
        reg,
        &FixedClock(NOW),
        &CountingIds::default(),
        ctx(),
    )
    .unwrap_or_else(|e| panic!("job: {e}"))
}

pub fn run_with<J: Job>(j: &mut J, budget: Budget) -> Result<J::Output, DormbitError> {
    loop {
        if let Step::Done(o) = j.step(budget)? {
            return Ok(o);
        }
    }
}

/// Captures with an unbounded budget and returns the output and the capsule bytes.
pub fn capture<V: ReadVfs>(v: &V, plan: CapturePlan, ok: ApprovedPlan) -> (CaptureOutput, Vec<u8>) {
    let mut j = job(v, plan, ok, MemWorkspace::new(), MemRegistry::default());
    let out = run_with(&mut j, Budget::UNBOUNDED).unwrap_or_else(|e| panic!("capture: {e}"));
    let (ws, _) = j.into_parts();
    let bytes = ws
        .library
        .get(&out.record.file_name)
        .cloned()
        .unwrap_or_else(|| panic!("capsule not in library"));
    (out, bytes)
}

pub struct TarEntry {
    pub name: String,
    pub mode: u32,
    pub mtime: u64,
    pub data: Vec<u8>,
}

pub fn read_tar(bytes: &[u8]) -> Vec<TarEntry> {
    let mut a = tar::Archive::new(bytes);
    let mut out = Vec::new();
    for e in a.entries().unwrap_or_else(|e| panic!("{e}")) {
        let mut e = e.unwrap_or_else(|e| panic!("{e}"));
        let name = e
            .path()
            .unwrap_or_else(|e| panic!("{e}"))
            .to_string_lossy()
            .into_owned();
        let mode = e.header().mode().unwrap_or(0);
        let mtime = e.header().mtime().unwrap_or(0);
        let mut data = Vec::new();
        e.read_to_end(&mut data).unwrap_or_else(|e| panic!("{e}"));
        out.push(TarEntry {
            name,
            mode,
            mtime,
            data,
        });
    }
    out
}

pub fn gunzip(data: &[u8]) -> Vec<u8> {
    let mut d = flate2::read::GzDecoder::new(data);
    let mut out = Vec::new();
    d.read_to_end(&mut out).unwrap_or_else(|e| panic!("{e}"));
    out
}

pub fn manifest_of(entries: &[TarEntry]) -> Manifest {
    serde_json::from_slice(&entries[1].data).unwrap_or_else(|e| panic!("{e}"))
}
