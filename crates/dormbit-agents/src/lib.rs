//! DORMBIT agent-context readers (filled by intelligence-and-brief Tasks 28 and 31-33).
//!
//! `inrepo` parses agent files that live inside a project (`.kiro/specs`,
//! `AGENTS.md`, `CLAUDE.md`, `.cursor/rules`, `.specstory/` metadata).
//! `adapters` (P1) reads external agent stores, metadata only (ADR-008).

#![forbid(unsafe_code)]

/// Re-exported so downstream crates can confirm they link the same core.
pub const CORE_VERSION: &str = dormbit_core::VERSION;
