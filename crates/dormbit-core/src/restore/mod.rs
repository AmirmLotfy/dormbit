//! Transactional restore (transactional-restore spec, ADR-006): pre-checks, a staged write
//! committed by one exclusive rename, post-verification, a journal, and recovery.

pub mod executor;
pub mod fs;
pub mod journal;
pub mod keys;
pub mod objects;
pub mod planner;
pub mod recovery;
pub mod report;

pub use executor::{CancelResult, RestoreContext, RestoreJob};
pub use fs::{FsFault, MemNode, MemRestoreFs, RestoreFs};
pub use journal::{Journal, JournalStore, MemJournal};
pub use objects::{CapsuleObjects, OpenedCapsule};
pub use planner::{
    CaseSensitivity, PlanInput, PrecheckOutcome, PrecheckReport, PrecheckState, RestorePlan,
    VolumeFacts, plan_restore,
};
pub use recovery::{RecoveryAction, RecoveryOutcome, recover};
pub use report::{RestoreReport, RestoreStatus};
