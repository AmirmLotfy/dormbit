//! The capsule: manifest, writer, and verifier (capsule-format.md, ADR-002).

pub mod limits;
pub mod manifest;
pub mod mem;
pub mod ustar;
pub mod verify;
pub mod writer;

pub use verify::{
    BytesSource, CapsuleSource, NoRegistry, RegistryRead, Report, ReportStatus, VerifyJob,
    verify_all,
};

pub use limits::CapsuleLimits;
pub use mem::{MemRegistry, MemWorkspace};
pub use writer::{
    Budget, CapsuleRecord, CaptureContext, CaptureJob, CaptureOutput, CaptureWorkspace, Job,
    JobProgress, Phase, Registry, Step, run_to_end,
};
