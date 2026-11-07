//! Restore properties over `MemRestoreFs`:
//! - PROP-2 full round trip: capture → verify → restore gives the approved files byte for
//!   byte with mode & 0o777, nothing else, and nothing outside the target.
//! - PROP-8 collisions: two paths equal under the applicable key are refused.
//! - PROP-4 no overwrite: any existing variant of the target name is refused and unchanged.

#[path = "support/capture_kit.rs"]
mod kit;

use std::collections::BTreeMap;
use std::io::Cursor;

use dormbit_core::capsule::{
    BytesSource, CapsuleLimits, MemRegistry, MemWorkspace, run_to_end, verify_all,
};
use dormbit_core::model::{ErrorCode, GitTreatment};
use dormbit_core::restore::fs::{MemNode, MemRestoreFs, RestoreFs};
use dormbit_core::restore::{
    CaseSensitivity, MemJournal, OpenedCapsule, PlanInput, RestoreContext, RestoreJob,
    RestoreStatus, VolumeFacts, plan_restore,
};
use dormbit_core::vfs::{FixedClock, MemVfs};
use kit::*;
use proptest::prelude::*;

fn tree() -> impl Strategy<Value = BTreeMap<String, (Vec<u8>, u32)>> {
    prop::collection::btree_map(
        "(src/|docs/|a/b/|Ünï/)?[a-z]{1,6}\\.(rs|md|txt)",
        (
            prop::collection::vec(any::<u8>(), 0..512),
            prop::sample::select(vec![0o644u32, 0o755, 0o600, 0o700]),
        ),
        1..12,
    )
}

fn vol(case: CaseSensitivity) -> VolumeFacts {
    VolumeFacts {
        fs_type: "mem".into(),
        block_size: 4096,
        free_bytes: 1 << 32,
        case,
    }
}

fn capsule(t: &BTreeMap<String, (Vec<u8>, u32)>) -> (Vec<u8>, dormbit_core::capsule::Report) {
    let mut v = MemVfs::new();
    for (p, (d, m)) in t {
        v.add_file_with(p, d.clone(), *m, 1_700_000_000);
    }
    let (plan, ok) = approved(&v, GitTreatment::Full, &[]);
    let mut j = job(&v, plan, ok, MemWorkspace::new(), MemRegistry::default());
    let out = run_to_end(&mut j).unwrap();
    let (ws, reg) = j.into_parts();
    let bytes = ws.library[&out.record.file_name].clone();
    let rep = verify_all(
        BytesSource::new(bytes.clone()),
        reg,
        CapsuleLimits::default(),
    )
    .unwrap();
    (bytes, rep)
}

proptest! {
    #[test]
    fn a_round_trip_restores_exactly_the_captured_files(t in tree(), insensitive in any::<bool>()) {
        // Skip trees that collide on a case-insensitive volume or mix a file and a folder.
        let (bytes, rep) = capsule(&t);
        let opened = OpenedCapsule::open(Cursor::new(bytes.clone()), 1 << 26).unwrap();
        let m = opened.manifest.clone();
        let ms = opened.manifest_sha256.clone();
        let fs = MemRestoreFs::new(insensitive);
        let case = if insensitive { CaseSensitivity::Insensitive } else { CaseSensitivity::Sensitive };
        let pre = plan_restore(&PlanInput {
            manifest: &m, manifest_sha256_now: &ms, verification: Some(&rep), target_name: "out",
            target_display: "~/out".into(), parent_entries: &[], volume: vol(case),
            inside_source: Some(false), inside_library: false,
        });
        let Some(plan) = pre.plan else {
            prop_assert_eq!(pre.report.first_error(), Some(ErrorCode::RestoreCollision));
            return Ok(());
        };
        let root = fs.root();
        let mut j = RestoreJob::new(plan, RestoreContext {
            op_id: "0001".into(), owner_token: "7a".repeat(16), target_grant_id: "g".into(),
            target_display: "~/out".into(), manifest_git: None,
        }, fs, MemJournal::default(), opened, FixedClock(5), root).unwrap();
        let r = run_to_end(&mut j).unwrap();
        prop_assert_eq!(r.status, RestoreStatus::Verified);
        let (fs, _) = j.into_parts();
        let restored: BTreeMap<String, (Vec<u8>, u32)> = fs.nodes.iter()
            .filter_map(|(k, (n, _))| match n {
                MemNode::File { data, mode } => k.strip_prefix("out/").map(|p| (p.to_owned(), (data.clone(), *mode))),
                _ => None,
            })
            .collect();
        let want: BTreeMap<String, (Vec<u8>, u32)> = t.iter().map(|(p, (d, m))| (p.clone(), (d.clone(), m & 0o777))).collect();
        prop_assert_eq!(restored, want);
        let tops: Vec<String> = fs.list(&fs.root()).unwrap().into_iter().map(|(n, _)| n).collect();
        prop_assert_eq!(tops, vec!["out".to_owned()]);
    }

    #[test]
    fn any_existing_variant_of_the_target_is_refused(variant in prop::sample::select(vec!["out", "Out", "OUT", "oUt"]), kind in 0u8..3) {
        let mut t = BTreeMap::new();
        t.insert("a.txt".to_owned(), (b"x".to_vec(), 0o644));
        let (bytes, rep) = capsule(&t);
        let opened = OpenedCapsule::open(Cursor::new(bytes), 1 << 26).unwrap();
        let mut fs = MemRestoreFs::new(true);
        let node = match kind { 0 => MemNode::Dir, 1 => MemNode::File { data: b"keep".to_vec(), mode: 0o644 }, _ => MemNode::Symlink { target: "gone".into() } };
        fs.add(variant, node.clone());
        let names: Vec<String> = fs.list(&fs.root()).unwrap().into_iter().map(|(n, _)| n).collect();
        let pre = plan_restore(&PlanInput {
            manifest: &opened.manifest, manifest_sha256_now: &opened.manifest_sha256, verification: Some(&rep),
            target_name: "out", target_display: "~/out".into(), parent_entries: &names,
            volume: vol(CaseSensitivity::Insensitive), inside_source: Some(false), inside_library: false,
        });
        prop_assert!(pre.plan.is_none());
        prop_assert_eq!(pre.report.first_error(), Some(ErrorCode::RestoreDestExists));
        prop_assert_eq!(fs.nodes.get(variant).map(|(n, _)| n.clone()), Some(node));
    }
}
