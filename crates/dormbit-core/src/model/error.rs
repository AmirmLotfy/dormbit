//! The one error shape that crosses the IPC boundary.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Stable error codes (master plan §8). The set is closed: a new code needs a matching
/// Zod string, a row in the foundation design table, and the plan owner's approval.
/// `IPC_SCHEMA_MISMATCH` is deliberately absent: only TypeScript produces it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum ErrorCode {
    ScopeDenied,
    PathUnsafe,
    PlanDigestMismatch,
    FileChangedDuringCapture,
    CapsuleMalformed,
    CapsuleTruncated,
    CapsuleHashMismatch,
    CapsuleVersionUnsupported,
    RestoreDestExists,
    RestoreCollision,
    RestoreVolumeUnsupported,
    RestoreInsufficientSpace,
    Cancelled,
}

impl ErrorCode {
    /// Every code, in declaration order. Used by tests and the TS/Rust parity check.
    pub const ALL: [ErrorCode; 13] = [
        ErrorCode::ScopeDenied,
        ErrorCode::PathUnsafe,
        ErrorCode::PlanDigestMismatch,
        ErrorCode::FileChangedDuringCapture,
        ErrorCode::CapsuleMalformed,
        ErrorCode::CapsuleTruncated,
        ErrorCode::CapsuleHashMismatch,
        ErrorCode::CapsuleVersionUnsupported,
        ErrorCode::RestoreDestExists,
        ErrorCode::RestoreCollision,
        ErrorCode::RestoreVolumeUnsupported,
        ErrorCode::RestoreInsufficientSpace,
        ErrorCode::Cancelled,
    ];

    /// Whether trying again can reasonably succeed. Deliberately conservative: only two codes
    /// are retryable, so the UI never auto-retries a cancel or a security refusal.
    pub const fn retryable(self) -> bool {
        matches!(
            self,
            ErrorCode::FileChangedDuringCapture | ErrorCode::RestoreInsufficientSpace
        )
    }
}

/// `{code, message, retryable}`. `message` is already redacted by the host: paths use `~`,
/// and file contents never appear.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
#[error("{code:?}: {message}")]
#[ts(export)]
pub struct DormbitError {
    pub code: ErrorCode,
    pub message: String,
    pub retryable: bool,
}

impl DormbitError {
    /// Builds an error whose `retryable` follows the fixed table in [`ErrorCode::retryable`].
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retryable: code.retryable(),
        }
    }

    /// A Channel send failed because the WebView went away. Surfaces as `CANCELLED`.
    pub fn window_closed() -> Self {
        Self::new(ErrorCode::Cancelled, "window closed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_two_codes_are_retryable() {
        let retryable: Vec<ErrorCode> = ErrorCode::ALL
            .iter()
            .copied()
            .filter(|c| c.retryable())
            .collect();
        assert_eq!(
            retryable,
            vec![
                ErrorCode::FileChangedDuringCapture,
                ErrorCode::RestoreInsufficientSpace
            ]
        );
    }

    #[test]
    fn codes_serialize_as_screaming_snake_case() {
        let json = serde_json::to_string(&ErrorCode::FileChangedDuringCapture).unwrap();
        assert_eq!(json, "\"FILE_CHANGED_DURING_CAPTURE\"");
        let json = serde_json::to_string(&ErrorCode::RestoreVolumeUnsupported).unwrap();
        assert_eq!(json, "\"RESTORE_VOLUME_UNSUPPORTED\"");
    }

    #[test]
    fn error_serializes_with_exactly_three_fields() {
        let e = DormbitError::new(ErrorCode::ScopeDenied, "no access");
        let v: serde_json::Value = serde_json::to_value(&e).unwrap();
        let obj = v.as_object().unwrap();
        assert_eq!(obj.len(), 3);
        assert_eq!(obj["code"], "SCOPE_DENIED");
        assert_eq!(obj["message"], "no access");
        assert_eq!(obj["retryable"], false);
    }

    #[test]
    fn constructor_takes_retryable_from_the_table() {
        assert!(DormbitError::new(ErrorCode::RestoreInsufficientSpace, "x").retryable);
        assert!(!DormbitError::new(ErrorCode::Cancelled, "x").retryable);
    }

    #[test]
    fn all_lists_every_variant_once() {
        let mut seen = std::collections::HashSet::new();
        for c in ErrorCode::ALL {
            assert!(seen.insert(c), "duplicate {c:?}");
        }
        assert_eq!(seen.len(), 13);
    }

    #[test]
    fn window_closed_is_cancelled_and_not_retryable() {
        let e = DormbitError::window_closed();
        assert_eq!(e.code, ErrorCode::Cancelled);
        assert!(!e.retryable);
    }
}
