//! DORMBIT browser demo core (filled by judge-demo Task 38).
//!
//! `dormbit-core` plus an in-memory filesystem, step jobs, and wasm-bindgen
//! bindings. Any new dependency must pass
//! `cargo build -p dormbit-wasm --target wasm32-unknown-unknown` (ADR-001).

#![forbid(unsafe_code)]

/// Re-exported so downstream crates can confirm they link the same core.
pub const CORE_VERSION: &str = dormbit_core::VERSION;
