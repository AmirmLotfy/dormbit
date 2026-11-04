//! PROP-11 schema round trip, Rust side (foundation Requirement 15). `scripts/schema-parity.sh`
//! runs the two ignored tests around the TypeScript step:
//!
//! 1. `write_samples` writes `<dir>/rust/<Type>.jsonl`, one JSON value per line.
//! 2. `packages/schema/test/parity.test.ts` checks those with Zod and writes `<dir>/ts/`.
//! 3. `read_samples` deserializes every TypeScript sample and checks it re-serializes unchanged.
//!
//! `<dir>` is `SCHEMA_PARITY_DIR`, default `target/schema-parity`. `PROPTEST_CASES` sets the
//! number of samples per type (default 256).
#![allow(clippy::disallowed_methods, clippy::disallowed_types)] // test-only file I/O; core never does this

#[path = "support/arb.rs"]
mod arb;

use std::path::PathBuf;

use arb::{Rng, Sample};
use dormbit_core::model::*;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Every type exported to `packages/schema/generated`, except `JobEvent`, which is generic and
/// is checked as `JobEvent<DiscoveryResult>`. The TypeScript side fails if this list misses one.
macro_rules! all_types {
    ($m:ident) => {
        $m!(
            Activity AdapterId AdapterInfo AdapterSupport AppInfo AppInfoEvent ApprovedPlan Brief
            BriefOutcomes BriefSection BuildProfile CapsuleId CapsuleRef CapturePlan CaptureResult
            CheckStatus ConsentRecord Decision DiscoveryResult DormbitError ErrorCode EvidenceRef
            ExportResult Finding FindingStatus GitHead GitSnapshot GitStatus GitTreatment
            GitUpstream JobId Marks NamedValue OpId Outcome PlanApproveInput PlanGroup PlanId
            PlanItem PlanItemKind PlanTotals PrecheckReport Preservation ProjectId ProjectRecord
            RecoverAction RecoverRequest RecoverResult RegistryMatch RestoreInput RestoreJournal
            RestoreReport RestoreState SafeU64 ScopeGrant ScopeId ScopeKind ScopeVia Settings
            SettingsPatch SizeInfo VerificationCheck VerificationReport
        )
    };
}

fn dir() -> PathBuf {
    std::env::var_os("SCHEMA_PARITY_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/schema-parity")
        })
}

fn cases() -> usize {
    std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(256)
}

fn seed(name: &str) -> u64 {
    name.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn write<T: Sample + Serialize>(name: &str, count: usize) {
    let mut r = Rng::new(seed(name));
    let mut out = String::new();
    for _ in 0..count {
        out.push_str(&serde_json::to_string(&T::sample(&mut r)).expect("serialize"));
        out.push('\n');
    }
    let d = dir().join("rust");
    std::fs::create_dir_all(&d).expect("create the samples folder");
    std::fs::write(d.join(format!("{name}.jsonl")), out).expect("write samples");
}

/// Deserializes each TypeScript sample into `T` and checks that it re-serializes to equal JSON.
fn check<T: Serialize + DeserializeOwned>(name: &str, failures: &mut Vec<String>) {
    let path = dir().join("ts").join(format!("{name}.jsonl"));
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            failures.push(format!("{name}: cannot read {}: {e}", path.display()));
            return;
        }
    };
    let mut seen = 0;
    for line in text.lines().filter(|l| !l.is_empty()) {
        seen += 1;
        let original: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                failures.push(format!("{name}: not JSON ({e})\n  sample: {line}"));
                continue;
            }
        };
        match serde_json::from_str::<T>(line) {
            Ok(v) => {
                let again = serde_json::to_value(&v).expect("serialize");
                if again != original {
                    failures.push(format!(
                        "{name}: round trip changed the value\n  sample: {line}\n  after:  {again}"
                    ));
                }
            }
            Err(e) => failures.push(format!(
                "{name}: Rust rejected a TypeScript sample ({e})\n  sample: {line}"
            )),
        }
    }
    if seen == 0 {
        failures.push(format!("{name}: no TypeScript samples"));
    }
}

/// Rust-only round trip on a few samples, so plain `cargo test` exercises the generators.
fn round_trip<T: Sample + Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(name: &str) {
    let mut r = Rng::new(seed(name));
    for _ in 0..16 {
        let v = T::sample(&mut r);
        let json = serde_json::to_string(&v).expect("serialize");
        let back: T =
            serde_json::from_str(&json).unwrap_or_else(|e| panic!("{name}: {e}\n  {json}"));
        assert_eq!(back, v, "{name} changed in a Rust round trip: {json}");
    }
}

#[test]
fn samples_round_trip_in_rust() {
    macro_rules! go { ($($t:ident)*) => { $( round_trip::<$t>(stringify!($t)); )* } }
    all_types!(go);
    round_trip::<JobEvent<DiscoveryResult>>("JobEvent");
}

#[test]
#[ignore = "run by scripts/schema-parity.sh"]
fn write_samples() {
    let n = cases();
    macro_rules! go { ($($t:ident)*) => { $( write::<$t>(stringify!($t), n); )* } }
    all_types!(go);
    write::<JobEvent<DiscoveryResult>>("JobEvent", n);
}

#[test]
#[ignore = "run by scripts/schema-parity.sh"]
fn read_samples() {
    let mut failures = Vec::new();
    macro_rules! go { ($($t:ident)*) => { $( check::<$t>(stringify!($t), &mut failures); )* } }
    all_types!(go);
    check::<JobEvent<DiscoveryResult>>("JobEvent", &mut failures);
    assert!(
        failures.is_empty(),
        "{} TypeScript sample(s) failed:\n\n{}",
        failures.len(),
        failures
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n\n")
    );
}
