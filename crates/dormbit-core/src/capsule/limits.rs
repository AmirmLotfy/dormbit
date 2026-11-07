use crate::model::{DormbitError, ErrorCode};

const GIB: u64 = 1 << 30;
const MIB: u64 = 1 << 20;
const MAX_SAFE: u64 = (1 << 53) - 1;

/// Capsule limits (capsule-format §8, LIM-1 to LIM-8). Defaults are conservative; each field can
/// be raised up to its hard ceiling and never beyond.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapsuleLimits {
    /// LIM-1: manifest entries (files + symlinks).
    pub max_entries: u64,
    /// LIM-2: size of one file.
    pub max_file_size: u64,
    /// LIM-3: `totals.bytes`.
    pub max_total_bytes: u64,
    /// LIM-4: the capsule file.
    pub max_capsule_size: u64,
    /// LIM-5: `manifest.json`.
    pub max_manifest_size: u64,
    /// LIM-6: a gzip object may expand to at most `max(ratio × stored_size, floor)`.
    pub max_expansion_ratio: u64,
    pub expansion_floor: u64,
    /// LIM-7: `excluded` items.
    pub max_excluded: u64,
    /// LIM-8: `warnings` items.
    pub max_warnings: u64,
}

impl Default for CapsuleLimits {
    fn default() -> Self {
        Self {
            max_entries: 100_000,
            max_file_size: GIB,
            max_total_bytes: 4 * GIB,
            max_capsule_size: 5 * GIB,
            max_manifest_size: 64 * MIB,
            max_expansion_ratio: 200,
            expansion_floor: MIB,
            max_excluded: 100_000,
            max_warnings: 10_000,
        }
    }
}

impl CapsuleLimits {
    /// Refuses a value above its hard ceiling or a zero where zero makes no sense.
    pub fn validate(&self) -> Result<(), DormbitError> {
        let ceilings: [(&str, u64, u64); 8] = [
            ("max_entries", self.max_entries, 1_000_000),
            ("max_file_size", self.max_file_size, 8_589_934_591),
            ("max_total_bytes", self.max_total_bytes, MAX_SAFE),
            ("max_capsule_size", self.max_capsule_size, MAX_SAFE),
            ("max_manifest_size", self.max_manifest_size, 512 * MIB),
            ("max_expansion_ratio", self.max_expansion_ratio, 1_032),
            ("max_excluded", self.max_excluded, 1_000_000),
            ("max_warnings", self.max_warnings, 100_000),
        ];
        for (name, value, ceiling) in ceilings {
            if value == 0 || value > ceiling {
                return Err(DormbitError::new(
                    ErrorCode::CapsuleLimitExceeded,
                    format!("The limit {name} must be between 1 and {ceiling}."),
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        assert_eq!(CapsuleLimits::default().validate(), Ok(()));
    }

    #[test]
    fn a_value_above_its_hard_ceiling_is_refused() {
        let l = CapsuleLimits {
            max_file_size: 8_589_934_592,
            ..CapsuleLimits::default()
        };
        assert_eq!(
            l.validate().unwrap_err().code,
            ErrorCode::CapsuleLimitExceeded
        );
        let l = CapsuleLimits {
            max_entries: 0,
            ..CapsuleLimits::default()
        };
        assert!(l.validate().is_err());
        let l = CapsuleLimits {
            max_expansion_ratio: 1_033,
            ..CapsuleLimits::default()
        };
        assert!(l.validate().is_err());
    }
}
