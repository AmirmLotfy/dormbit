//! DORMBIT native filesystem layer: cap-std handles for granted folders, the capsule library
//! sink and registry, and the exclusive rename (filled further by scope-and-discovery and
//! transactional-restore).
//!
//! The project's single `unsafe` call (`renameatx_np`) lives in [`rename::rename_excl`],
//! behind a scoped `#[allow(unsafe_code)]` (ADR-006).
#![deny(unsafe_code)]

pub mod capsule_sink;
pub mod capsule_source;
pub mod registry;
pub mod rename;
pub mod vfs;

pub use capsule_sink::{FsCapsuleSink, FsWorkspace};
pub use capsule_source::FsCapsuleSource;
pub use registry::FsCapsuleRegistry;
pub use rename::{RenameError, rename_excl};
pub use vfs::FsVfs;

/// Re-exported so downstream crates can confirm they link the same core.
pub const CORE_VERSION: &str = dormbit_core::VERSION;
