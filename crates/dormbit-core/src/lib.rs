//! DORMBIT core.
//!
//! Pure logic only. This crate never touches the disk, the clock, or the
//! network directly: hosts inject a `Vfs`, a `Clock`, and an `IdGen`
//! (ADR-003). That keeps it deterministic and buildable for
//! `wasm32-unknown-unknown` (ADR-001).

#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

pub mod model;
pub mod vfs;

/// The crate version, reported by `app_info` and written into capsule manifests.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::VERSION;

    #[test]
    fn version_is_semver_like() {
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert_eq!(parts.len(), 3, "expected MAJOR.MINOR.PATCH, got {VERSION}");
        assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()));
    }
}
