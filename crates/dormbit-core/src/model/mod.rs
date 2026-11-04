//! Shared types that cross the IPC boundary. Each type is exported to
//! TypeScript with ts-rs (`packages/schema/generated`) and mirrored by a Zod
//! schema in `packages/schema/src` (PROP-11).

pub mod app;
pub mod error;
pub mod id;
pub mod ipc;
pub mod num;

pub use app::{AppInfo, AppInfoEvent, BuildProfile};
pub use error::{DormbitError, ErrorCode};
pub use id::{AdapterId, CapsuleId, JobId, OpId, PlanId, ProjectId, ScopeId};
pub use ipc::*;
pub use num::{SafeU64, SafeU64Overflow};
