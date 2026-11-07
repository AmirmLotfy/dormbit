//! Verifier behavior and the malicious-capsule set (capsule-and-verification Task 3).
//!
//! The malicious capsules are built in memory from a real capsule with one defect each
//! (STATUS D-28), so every hash and the trailer are valid except the defect under test.

#[path = "support/dockline.rs"]
mod fixture;
#[path = "support/capture_kit.rs"]
mod kit;
#[path = "support/raw.rs"]
mod raw;

use std::io::Write as _;

use dormbit_core::capsule::manifest::{Encoding, Entry};
use dormbit_core::capsule::verify::{CheckId, CheckState, FailReason, RegistryState};
use dormbit_core::capsule::{
    Budget, BytesSource, CapsuleLimits, Job, MemRegistry, MemWorkspace, NoRegistry, Report,
    ReportStatus, Step, VerifyJob, verify_all,
};
use dormbit_core::model::{ErrorCode, GitTreatment};
use dormbit_core::util::sha256_hex;
use dormbit_core::vfs::MemVfs;
use kit::*;
use raw::{Raw, RawEntry};

fn small() -> MemVfs {
    let mut v = MemVfs::new();
    v.add_file("README.md", "# demo\n")
        .add_file_with("run.sh", "#!/bin/sh\n", 0o755, 1_700_000_000)
        .add_file("src/a.rs", "same\n")
        .add_file("src/b.rs", "same\n")
        .add_symlink("ln", "README.md");
    v
}

/// A capsule plus the registry that recorded it.
fn capsule_with_registry(v: &MemVfs) -> (Vec<u8>, MemRegistry) {
    let (plan, ok) = approved(v, GitTreatment::Full, &[]);
    let mut j = job(v, plan, ok, MemWorkspace::new(), MemRegistry::default());
    let out = run_with(&mut j, Budget::UNBOUNDED).unwrap();
    let (ws, reg) = j.into_parts();
    (ws.library[&out.record.file_name].clone(), reg)
}

fn verify(bytes: Vec<u8>) -> Report {
    verify_all(
        BytesSource::new(bytes),
        NoRegistry,
        CapsuleLimits::default(),
    )
    .unwrap()
}

fn state(r: &Report, id: CheckId) -> CheckState {
    r.check(id).map(|c| c.status).unwrap_or(CheckState::NotRun)
}

fn base() -> Raw {
    let (bytes, _) = capsule_with_registry(&small());
    Raw::from_capsule(&bytes)
}

#[test]
fn a_fresh_capsule_verifies_and_matches_its_record() {
    let (bytes, reg) = capsule_with_registry(&small());
    let r = verify_all(
        BytesSource::new(bytes.clone()),
        reg,
        CapsuleLimits::default(),
    )
    .unwrap();
    assert_eq!(r.status, ReportStatus::Verified, "{r:#?}");
    assert_eq!(r.registry_match, RegistryState::Match);
    assert!(r.checks.iter().all(|c| c.status == CheckState::Pass));
    assert_eq!(r.checks.len(), 14);
    assert_eq!(r.file_sha256.as_deref(), Some(sha256_hex(&bytes).as_str()));
    assert!(r.failed_files.is_empty());
    assert_eq!(r.limitations.len(), 2);
}

#[test]
fn without_a_record_the_registry_check_is_unknown_and_the_capsule_still_verifies() {
    let (bytes, _) = capsule_with_registry(&small());
    let r = verify(bytes);
    assert_eq!(r.status, ReportStatus::Verified);
    assert_eq!(r.registry_match, RegistryState::NotInRegistry);
    assert_eq!(state(&r, CheckId::V14Registry), CheckState::Unknown);
}

#[test]
fn a_record_with_a_different_hash_is_a_mismatch() {
    let (bytes, mut reg) = capsule_with_registry(&small());
    for r in reg.records.values_mut() {
        r.file_sha256 = "0".repeat(64);
    }
    let r = verify_all(BytesSource::new(bytes), reg, CapsuleLimits::default()).unwrap();
    assert_eq!(r.status, ReportStatus::Failed);
    assert_eq!(r.registry_match, RegistryState::Mismatch);
}

#[test]
fn the_dockline_capsule_verifies() {
    let v = fixture::dockline();
    let (plan, ok) = approved(&v, GitTreatment::Full, &[]);
    let (_, bytes) = capture(&v, plan, ok);
    assert_eq!(verify(bytes).status, ReportStatus::Verified);
}

#[test]
fn every_single_byte_flip_fails_verification() {
    let (bytes, _) = capsule_with_registry(&small());
    for i in 0..bytes.len() {
        let mut b = bytes.clone();
        b[i] ^= 0x01;
        let r = verify(b);
        assert_ne!(r.status, ReportStatus::Verified, "flip at byte {i} passed");
    }
}

#[test]
fn every_truncation_fails_with_truncated() {
    let (bytes, _) = capsule_with_registry(&small());
    for n in 0..bytes.len() {
        let r = verify(bytes[..n].to_vec());
        assert_eq!(r.status, ReportStatus::Failed, "truncated to {n}");
        assert_eq!(
            r.error_code,
            Some(ErrorCode::CapsuleTruncated),
            "truncated to {n}"
        );
    }
}

#[test]
fn data_after_the_end_of_the_archive_fails() {
    let (mut bytes, _) = capsule_with_registry(&small());
    bytes.push(0);
    let r = verify(bytes);
    assert_eq!(r.status, ReportStatus::Failed);
    assert_eq!(state(&r, CheckId::V05Padding), CheckState::Fail);
}

#[test]
fn budgets_and_cancel() {
    let (bytes, _) = capsule_with_registry(&small());
    let mut j = VerifyJob::new(
        BytesSource::new(bytes.clone()),
        NoRegistry,
        CapsuleLimits::default(),
    );
    let one = Budget {
        max_bytes: 1,
        max_ops: 1,
    };
    let mut steps = 0;
    let r = loop {
        steps += 1;
        if let Step::Done(r) = j.step(one).unwrap() {
            break r;
        }
    };
    assert!(steps > 3);
    assert_eq!(r, verify(bytes.clone()));

    let mut j = VerifyJob::new(
        BytesSource::new(bytes),
        NoRegistry,
        CapsuleLimits::default(),
    );
    assert!(matches!(j.step(one), Ok(Step::Progress(_))));
    j.cancel();
    assert_eq!(
        j.step(one).err().map(|e| e.code),
        Some(ErrorCode::Cancelled)
    );
}

// ---- malicious capsules ----------------------------------------------------------------------

struct Case {
    name: &'static str,
    bytes: Vec<u8>,
    status: ReportStatus,
    code: ErrorCode,
    check: CheckId,
}

fn set_path(raw: &mut Raw, from: &str, to: &str) {
    for e in &mut raw.manifest.entries {
        match e {
            Entry::File { path, .. } | Entry::Symlink { path, .. } if path == from => {
                *path = to.to_owned();
            }
            _ => {}
        }
    }
}

fn gzip(data: &[u8]) -> Vec<u8> {
    let mut e = flate2::GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), flate2::Compression::new(6));
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

fn cases() -> Vec<Case> {
    let full = base().build();
    let mut out = vec![Case {
        name: "truncation",
        bytes: full[..full.len() - 700].to_vec(),
        status: ReportStatus::Failed,
        code: ErrorCode::CapsuleTruncated,
        check: CheckId::V13Trailer,
    }];
    let mut add = |name, raw: Raw, status, code, check| {
        out.push(Case {
            name,
            bytes: raw.build(),
            status,
            code,
            check,
        })
    };
    let f = ReportStatus::Failed;
    let u = ReportStatus::Unsupported;

    let mut r = base();
    set_path(&mut r, "README.md", "../outside.md");
    add("traversal", r, f, ErrorCode::PathUnsafe, CheckId::V08Paths);

    let mut r = base();
    set_path(&mut r, "README.md", "/etc/passwd");
    add(
        "absolute-path",
        r,
        f,
        ErrorCode::PathUnsafe,
        CheckId::V08Paths,
    );

    let mut r = base();
    r.objects[0].typeflag = Some(b'2');
    add(
        "symlink-entry",
        r,
        f,
        ErrorCode::CapsuleMalformed,
        CheckId::V03EntryTypeName,
    );

    let mut r = base();
    let dup = r.objects[0].clone();
    r.objects.insert(1, dup);
    add(
        "duplicate-object",
        r,
        f,
        ErrorCode::CapsuleMalformed,
        CheckId::V04Order,
    );

    let mut r = base();
    set_path(&mut r, "src/b.rs", "src/a.rs");
    add(
        "duplicate-path",
        r,
        f,
        ErrorCode::CapsuleMalformed,
        CheckId::V08Paths,
    );

    let mut r = base();
    r.with_trailer = false;
    add(
        "missing-trailer",
        r,
        f,
        ErrorCode::CapsuleTruncated,
        CheckId::V13Trailer,
    );

    // Ratio variant: the manifest claims a huge size for a tiny stored object.
    let mut r = base();
    if let Some(Entry::File { size, .. }) = r.manifest.entries.first_mut() {
        *size = 900 << 20;
    }
    add(
        "bomb-ratio",
        r,
        u,
        ErrorCode::CapsuleLimitExceeded,
        CheckId::V06Limits,
    );

    // Overflow variant: a valid stored object that expands far past its declared size.
    let mut r = base();
    let bomb = gzip(&vec![0u8; 8 << 20]);
    let bomb_sha = sha256_hex(&bomb);
    let name = "f".repeat(64);
    r.manifest.entries.push(Entry::File {
        path: "zz-bomb".into(),
        size: 16,
        mode: 0o644,
        mtime: 0,
        sha256: name.clone(),
        object: dormbit_core::capsule::manifest::ObjectInfo {
            encoding: Encoding::Gzip,
            stored_size: bomb.len() as u64,
            stored_sha256: bomb_sha,
        },
    });
    r.manifest.totals.files += 1;
    r.manifest.totals.objects += 1;
    r.manifest.totals.bytes += 16;
    r.manifest.totals.stored_bytes += bomb.len() as u64;
    r.objects.push(RawEntry {
        name: format!("objects/{name}"),
        data: bomb,
        mode: 0o644,
        typeflag: None,
    });
    add(
        "bomb-overflow",
        r,
        f,
        ErrorCode::CapsuleHashMismatch,
        CheckId::V11ContentHash,
    );

    let mut r = base();
    r.marker.data = b"dormbit-capsule/1\r\n".to_vec();
    add(
        "bad-marker-content",
        r,
        f,
        ErrorCode::CapsuleMalformed,
        CheckId::V01Marker,
    );

    let mut r = base();
    r.marker.name = "DORMBIT2".into();
    add(
        "bad-marker-name",
        r,
        f,
        ErrorCode::CapsuleMalformed,
        CheckId::V01Marker,
    );

    let mut r = base();
    r.marker.data = b"dormbit-capsule/2\n".to_vec();
    add(
        "unknown-version-marker",
        r,
        u,
        ErrorCode::CapsuleVersionUnsupported,
        CheckId::V01Marker,
    );

    let mut r = base();
    let mut v: serde_json::Value = serde_json::to_value(&r.manifest).unwrap();
    v["schema_version"] = 2.into();
    r.manifest_bytes = Some(serde_json::to_vec(&v).unwrap());
    add(
        "unknown-version-schema",
        r,
        u,
        ErrorCode::CapsuleVersionUnsupported,
        CheckId::V07Manifest,
    );

    let mut r = base();
    r.objects.remove(0);
    add(
        "missing-object",
        r,
        f,
        ErrorCode::CapsuleMalformed,
        CheckId::V09ObjectSet,
    );

    let mut r = base();
    let extra = gzip(b"not listed\n");
    r.objects.push(RawEntry {
        name: format!("objects/{}", "e".repeat(64)),
        data: extra,
        mode: 0o644,
        typeflag: None,
    });
    r.objects.sort_by(|a, b| a.name.cmp(&b.name));
    add(
        "extra-object",
        r,
        f,
        ErrorCode::CapsuleMalformed,
        CheckId::V09ObjectSet,
    );

    let mut r = base();
    let last = r.objects[0].data.len() - 9;
    r.objects[0].data[last] ^= 0xff;
    add(
        "corrupt-object",
        r,
        f,
        ErrorCode::CapsuleHashMismatch,
        CheckId::V10StoredHash,
    );

    out
}

#[test]
fn every_malicious_capsule_gets_its_expected_status_code_and_check() {
    for c in cases() {
        let r = verify(c.bytes);
        assert_eq!(r.status, c.status, "{}: {r:#?}", c.name);
        assert_eq!(r.error_code, Some(c.code), "{}", c.name);
        let want = if c.status == ReportStatus::Unsupported {
            CheckState::Unknown
        } else {
            CheckState::Fail
        };
        assert_eq!(
            state(&r, c.check),
            want,
            "{}: {:?}",
            c.name,
            r.check(c.check)
        );
    }
}

#[test]
fn a_corrupt_object_names_every_file_that_uses_it() {
    let mut r = base();
    let i = r
        .objects
        .iter()
        .position(|o| o.name.ends_with(&sha256_hex(b"same\n")))
        .unwrap();
    let mid = r.objects[i].data.len() / 2;
    r.objects[i].data[mid] ^= 0x55;
    let report = verify(r.build());
    let named: Vec<(&str, FailReason)> = report
        .failed_files
        .iter()
        .map(|f| (f.path.as_str(), f.reason))
        .collect();
    assert_eq!(
        named,
        vec![
            ("src/a.rs", FailReason::StoredHashMismatch),
            ("src/b.rs", FailReason::StoredHashMismatch)
        ]
    );
    // Other objects were still checked (Requirement 14.9).
    assert_eq!(state(&report, CheckId::V11ContentHash), CheckState::Pass);
}

#[test]
fn after_a_stop_later_checks_are_not_run_and_files_are_unverifiable() {
    let mut r = base();
    r.objects[0].typeflag = Some(b'2');
    let report = verify(r.build());
    assert_eq!(state(&report, CheckId::V10StoredHash), CheckState::NotRun);
    assert_eq!(state(&report, CheckId::V13Trailer), CheckState::NotRun);
    assert_eq!(report.registry_match, RegistryState::NotChecked);
    assert!(!report.failed_files.is_empty());
    assert!(
        report
            .failed_files
            .iter()
            .all(|f| f.reason == FailReason::Unverifiable)
    );
}

#[test]
fn random_bytes_never_panic_and_never_verify() {
    let mut x: u64 = 0x1234_5678;
    for len in [0usize, 1, 511, 512, 1024, 4096] {
        for _ in 0..20 {
            let bytes: Vec<u8> = (0..len)
                .map(|_| {
                    x ^= x << 13;
                    x ^= x >> 7;
                    x ^= x << 17;
                    x as u8
                })
                .collect();
            assert_ne!(verify(bytes).status, ReportStatus::Verified);
        }
    }
}

/// Rewrites one octal header field (with a valid checksum), as an attacker could.
fn patch_field(bytes: &mut [u8], header_at: usize, field: std::ops::Range<usize>, value: u64) {
    let h = &mut bytes[header_at..header_at + 512];
    let digits = field.len() - 1;
    h[field.start..field.start + digits].copy_from_slice(format!("{value:0digits$o}").as_bytes());
    h[148..156].copy_from_slice(b"        ");
    let sum: u64 = h.iter().map(|&b| u64::from(b)).sum();
    h[148..154].copy_from_slice(format!("{sum:06o}").as_bytes());
    h[154] = 0;
    h[155] = b' ';
}

#[test]
fn marker_manifest_and_trailer_headers_must_be_mode_0644() {
    let (bytes, _) = capsule_with_registry(&small());
    let end_at = bytes.len() - 2048;
    for at in [0usize, 1024, end_at] {
        let mut b = bytes.clone();
        patch_field(&mut b, at, 100..108, 0o755);
        let r = verify(b);
        assert_eq!(r.status, ReportStatus::Failed, "header at {at}");
        assert_eq!(
            state(&r, CheckId::V02Header),
            CheckState::Fail,
            "header at {at}"
        );
    }
}

#[test]
fn cutting_into_a_large_first_object_is_truncation_not_a_limit() {
    let mut v = MemVfs::new();
    let mut x: u32 = 7;
    let big: Vec<u8> = (0..200_000)
        .map(|_| {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            (x >> 16) as u8
        })
        .collect();
    v.add_file("big.bin", big);
    let (plan, ok) = approved(&v, GitTreatment::Full, &[]);
    let (_, bytes) = capture(&v, plan, ok);
    for cut in [bytes.len() / 4, bytes.len() / 2, bytes.len() - 5000] {
        let r = verify(bytes[..cut].to_vec());
        assert_eq!(r.status, ReportStatus::Failed, "cut at {cut}");
        assert_eq!(
            r.error_code,
            Some(ErrorCode::CapsuleTruncated),
            "cut at {cut}"
        );
    }
}

#[test]
fn a_stop_after_the_manifest_lists_every_file_as_unverifiable() {
    let (mut bytes, _) = capsule_with_registry(&small());
    patch_field(&mut bytes, 0, 136..148, 12_345);
    let r = verify(bytes);
    assert_eq!(state(&r, CheckId::V02Header), CheckState::Fail);
    let paths: Vec<&str> = r.failed_files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, vec!["README.md", "run.sh", "src/a.rs", "src/b.rs"]);
    assert!(
        r.failed_files
            .iter()
            .all(|f| f.reason == FailReason::Unverifiable)
    );
}

#[test]
fn a_stop_before_the_manifest_says_no_file_list_is_available() {
    let mut r = base();
    r.marker.data = b"something else\n".to_vec();
    let report = verify(r.build());
    assert!(report.failed_files.is_empty());
    let detail = &report.check(CheckId::V01Marker).unwrap().detail;
    assert!(detail.contains("no file list is available"), "{detail}");
}

#[test]
fn a_gzip_object_with_bytes_after_its_member_fails() {
    let mut r = base();
    let i = 0;
    r.objects[i].data.extend_from_slice(b"trailing junk");
    let sha = r.objects[i].name.trim_start_matches("objects/").to_owned();
    let stored_sha = sha256_hex(&r.objects[i].data);
    let stored_len = r.objects[i].data.len() as u64;
    let mut delta = 0u64;
    for e in &mut r.manifest.entries {
        if let Entry::File { sha256, object, .. } = e
            && *sha256 == sha
        {
            delta = stored_len - object.stored_size;
            object.stored_size = stored_len;
            object.stored_sha256 = stored_sha.clone();
        }
    }
    r.manifest.totals.stored_bytes += delta;
    let report = verify(r.build());
    assert_eq!(report.status, ReportStatus::Failed, "{report:#?}");
    assert_eq!(state(&report, CheckId::V11ContentHash), CheckState::Fail);
    assert_eq!(state(&report, CheckId::V10StoredHash), CheckState::Pass);
}
