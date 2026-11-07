//! Property 2 (PROP-3): any single-byte flip or truncation fails verification, over generated
//! capsules. `verify.rs` also sweeps every offset of one fixed capsule.

#[path = "support/capture_kit.rs"]
mod kit;

use dormbit_core::capsule::{BytesSource, CapsuleLimits, NoRegistry, ReportStatus, verify_all};
use dormbit_core::model::{ErrorCode, GitTreatment};
use dormbit_core::vfs::MemVfs;
use kit::*;
use proptest::prelude::*;

fn tree() -> impl Strategy<Value = Vec<(String, Vec<u8>, bool)>> {
    prop::collection::vec(
        (
            "[a-e]{1,3}(/[a-e]{1,3})?\\.(rs|md|txt)",
            prop::collection::vec(any::<u8>(), 0..600),
            any::<bool>(),
        ),
        1..5,
    )
}

fn build(t: &[(String, Vec<u8>, bool)]) -> Option<Vec<u8>> {
    let mut v = MemVfs::new();
    for (p, data, exec) in t {
        // Skip trees where a file path is also a folder of another file.
        if t.iter().any(|(q, _, _)| q.starts_with(&format!("{p}/"))) {
            return None;
        }
        v.add_file_with(
            p,
            data.clone(),
            if *exec { 0o755 } else { 0o644 },
            1_700_000_000,
        );
    }
    let (plan, ok) = approved(&v, GitTreatment::Full, &[]);
    Some(capture(&v, plan, ok).1)
}

fn verify(b: Vec<u8>) -> dormbit_core::capsule::Report {
    verify_all(BytesSource::new(b), NoRegistry, CapsuleLimits::default()).unwrap()
}

proptest! {
    #[test]
    fn a_flip_anywhere_fails(t in tree(), at in any::<prop::sample::Index>(), mask in 1u8..=255) {
        let Some(bytes) = build(&t) else { return Ok(()) };
        prop_assert_eq!(verify(bytes.clone()).status, ReportStatus::Verified);
        let mut b = bytes;
        let i = at.index(b.len());
        b[i] ^= mask;
        prop_assert_ne!(verify(b).status, ReportStatus::Verified, "flip at {} passed", i);
    }

    #[test]
    fn any_truncation_fails_as_truncated(t in tree(), at in any::<prop::sample::Index>()) {
        let Some(bytes) = build(&t) else { return Ok(()) };
        let n = at.index(bytes.len());
        let r = verify(bytes[..n].to_vec());
        prop_assert_eq!(r.status, ReportStatus::Failed);
        prop_assert_eq!(r.error_code, Some(ErrorCode::CapsuleTruncated));
    }
}
