//! Hand-written sample generators for every exported IPC type (PROP-11).
//!
//! Deterministic: a SplitMix64 stream seeded per type name, so a failing sample is
//! reproducible. Strings mix tricky constants (empty, `~`, quotes, NFC and NFD pairs, emoji,
//! control characters) with random text from several scripts. SafeU64 spans 0 to 2^53 - 1.
//! `Option` is `None` about one case in four, and lists hold 0 to 3 items so files stay small.

use dormbit_core::model::*;

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u64) as usize]
    }
}

pub trait Sample: Sized {
    fn sample(r: &mut Rng) -> Self;
}

const TRICKY: &[&str] = &[
    "",
    "~",
    "~/dev/dockline",
    "\"quoted\"",
    "back\\slash",
    "caf\u{e9}",
    "cafe\u{301}",
    "\u{c5}",
    "A\u{30a}",
    "\u{65e5}\u{672c}\u{8a9e}",
    "\u{1f642}",
    "a\u{200d}b",
    "tab\tnew\nline",
    "</script>",
    "nul\u{0}byte",
];

/// Code point ranges for random text: ASCII, Latin-1, combining marks, CJK, emoji.
const RANGES: &[(u32, u32)] = &[
    (0x20, 0x7e),
    (0xa0, 0xff),
    (0x300, 0x36f),
    (0x4e00, 0x4e3f),
    (0x1f600, 0x1f64f),
];

impl Sample for String {
    fn sample(r: &mut Rng) -> Self {
        if r.below(3) == 0 {
            return (*r.pick(TRICKY)).to_owned();
        }
        let len = r.below(13);
        (0..len)
            .filter_map(|_| {
                let (lo, hi) = *r.pick(RANGES);
                char::from_u32(lo + r.below(u64::from(hi - lo + 1)) as u32)
            })
            .collect()
    }
}

impl Sample for bool {
    fn sample(r: &mut Rng) -> Self {
        r.below(2) == 0
    }
}

impl Sample for SafeU64 {
    fn sample(r: &mut Rng) -> Self {
        let v = match r.below(4) {
            0 => 0,
            1 => SafeU64::MAX,
            2 => r.below(1000),
            _ => r.below(SafeU64::MAX + 1),
        };
        SafeU64::new(v).expect("generated within range")
    }
}

impl<T: Sample> Sample for Option<T> {
    fn sample(r: &mut Rng) -> Self {
        if r.below(4) == 0 {
            None
        } else {
            Some(T::sample(r))
        }
    }
}

impl<T: Sample> Sample for Vec<T> {
    fn sample(r: &mut Rng) -> Self {
        let n = r.below(4);
        (0..n).map(|_| T::sample(r)).collect()
    }
}

impl Sample for ErrorCode {
    fn sample(r: &mut Rng) -> Self {
        *r.pick(&ErrorCode::ALL)
    }
}

macro_rules! structs {
    ($($t:ident { $($f:ident),* $(,)? })*) => {$(
        impl Sample for $t {
            fn sample(r: &mut Rng) -> Self {
                $t { $($f: Sample::sample(r)),* }
            }
        }
    )*};
}

macro_rules! unit_enums {
    ($($t:ident [$($v:ident),* $(,)?])*) => {$(
        impl Sample for $t {
            fn sample(r: &mut Rng) -> Self {
                *r.pick(&[$($t::$v),*])
            }
        }
    )*};
}

macro_rules! newtypes {
    ($($t:ident),* $(,)?) => {$(
        impl Sample for $t {
            fn sample(r: &mut Rng) -> Self {
                $t(String::sample(r))
            }
        }
    )*};
}

newtypes!(
    ScopeId, ProjectId, PlanId, CapsuleId, JobId, OpId, AdapterId
);

unit_enums! {
    BuildProfile [Debug, Release]
    ScopeKind [Read, CapsuleLibrary, RestoreTarget]
    ScopeVia [NativePicker]
    FindingStatus [Confirmed, Inferred, Unknown]
    Decision [Include, Exclude, Undecided]
    PlanItemKind [File, Dir, Symlink, Special]
    GitTreatment [Full, WorkingTreeOnly]
    CheckStatus [Pass, Fail, Skipped]
    RegistryMatch [Match, Mismatch, NotInRegistry]
    RestoreState [Staged, Written, Verified, Committed, Cleaned]
    RecoverAction [Discard]
    Outcome [Preserved, Partial, NotCaptured]
    AdapterSupport [Supported, Unsupported]
}

structs! {
    AppInfo { name, version, core_version, profile, min_macos }
    DormbitError { code, message, retryable }
    ScopeGrant { id, kind, root_display, granted_at, via }
    EvidenceRef { path, detail }
    Finding { id, rule, statement, status, evidence }
    NamedValue { value, source }
    GitHead { branch, detached, commit, time, summary }
    GitUpstream { ahead, behind, as_of_last_fetch }
    GitStatus { modified, staged, untracked, deleted, conflicted }
    GitSnapshot { head, upstream, status, stash_count, worktree_external, submodules, lfs_detected, risky_config, remotes_redacted }
    Activity { latest_mtime, scanned, partial }
    SizeInfo { total, rebuildable, partial }
    Marks { dormant }
    Preservation { last_capsule, last_verified_at, last_verified_restore }
    ProjectRecord { id, scope_id, rel_root, name, stacks, git, activity, size, docs, manifests, agent_artifacts, marks, preservation }
    DiscoveryResult { projects_found, entries_scanned }
    PlanItem { path, kind, size, decision, reason, editable, warning }
    PlanGroup { name, file_count, bytes }
    PlanTotals { files, bytes, included_files, included_bytes, secret_scan_skipped }
    CapturePlan { plan_id, items, groups, git_treatment, totals, digest }
    PlanApproveInput { digest, acks, overrides }
    ApprovedPlan { digest, approved_at, acks, overrides }
    CapsuleRef { id, project_name, created_at, size, file_count }
    CaptureResult { capsule }
    VerificationCheck { id, status, detail }
    VerificationReport { capsule_id, file_sha256, manifest_sha256, checks, failed_files, registry_match, status }
    RestoreJournal { op_id, staging, owner_token, state }
    RestoreInput { capsule_id, target_scope_id, dest_name }
    PrecheckReport { ok, issues, required_bytes, available_bytes }
    RestoreReport { op_id, capsule_id, destination_display, files_restored, bytes_restored, verified }
    RecoverRequest { op_id, action }
    RecoverResult { op_id, state }
    BriefSection { title, body }
    BriefOutcomes { files, context, session, build }
    Brief { sections, outcomes, evidence_index }
    ExportResult { file_name, bytes }
    Settings { reduce_motion, capsule_max_mb }
    SettingsPatch { reduce_motion, capsule_max_mb }
    ConsentRecord { adapter_id, granted, updated_at }
    AdapterInfo { id, name, support, granted }
}

impl Sample for AppInfoEvent {
    fn sample(r: &mut Rng) -> Self {
        if r.below(2) == 0 {
            AppInfoEvent::Info {
                info: AppInfo::sample(r),
            }
        } else {
            AppInfoEvent::Done
        }
    }
}

impl<R: Sample> Sample for JobEvent<R> {
    fn sample(r: &mut Rng) -> Self {
        match r.below(5) {
            0 => JobEvent::State {
                state: String::sample(r),
            },
            1 => JobEvent::Progress {
                files_done: SafeU64::sample(r),
                files_total: SafeU64::sample(r),
                bytes_done: SafeU64::sample(r),
                bytes_total: SafeU64::sample(r),
            },
            2 => JobEvent::Done {
                result: R::sample(r),
            },
            3 => JobEvent::Failed {
                error: DormbitError::sample(r),
            },
            _ => JobEvent::Cancelled,
        }
    }
}
