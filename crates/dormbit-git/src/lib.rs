//! DORMBIT Git inspection (filled by scope-and-discovery Task 17).
//!
//! Reads repositories with `git2` (vendored libgit2, network features off)
//! and never spawns the git CLI (ADR-004).

#![forbid(unsafe_code)]

/// Re-exported so downstream crates can confirm they link the same core.
pub const CORE_VERSION: &str = dormbit_core::VERSION;
