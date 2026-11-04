//! DORMBIT native filesystem layer (filled by scope-and-discovery,
//! capsule-and-verification, and transactional-restore).
//!
//! The project's single `unsafe` call (`renameatx_np`) will live here, in
//! `restore/commit.rs`, behind a scoped `#[allow(unsafe_code)]` (ADR-006).

#![deny(unsafe_code)]

/// Re-exported so downstream crates can confirm they link the same core.
pub const CORE_VERSION: &str = dormbit_core::VERSION;
