//! Integers that survive a JavaScript round trip.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use ts_rs::TS;

/// The largest integer JavaScript represents exactly: 2^53 - 1.
const MAX: u64 = (1 << 53) - 1;

/// A `u64` limited to `0 ..= 2^53 - 1`, so sizes and counts never lose precision in TypeScript.
///
/// Serializes as a plain JSON number and is exported to TypeScript as `number`.
/// Deserializing a larger value fails, so untrusted input can't smuggle one in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, TS)]
#[ts(type = "number")]
pub struct SafeU64(u64);

/// The value was larger than 2^53 - 1. Callers map this to their own error code
/// (for example `CAPSULE_MALFORMED` in the verifier).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("value {0} exceeds 2^53 - 1")]
pub struct SafeU64Overflow(pub u64);

impl SafeU64 {
    /// 2^53 - 1.
    pub const MAX: u64 = MAX;

    /// Returns `Err` when `v` is larger than [`SafeU64::MAX`].
    pub const fn new(v: u64) -> Result<Self, SafeU64Overflow> {
        if v > MAX {
            Err(SafeU64Overflow(v))
        } else {
            Ok(Self(v))
        }
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for SafeU64 {
    type Error = SafeU64Overflow;
    fn try_from(v: u64) -> Result<Self, Self::Error> {
        Self::new(v)
    }
}

impl Serialize for SafeU64 {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(self.0)
    }
}

impl<'de> Deserialize<'de> for SafeU64 {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = u64::deserialize(d)?;
        Self::new(v).map_err(serde::de::Error::custom)
    }
}

impl From<SafeU64> for u64 {
    fn from(v: SafeU64) -> Self {
        v.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_largest_safe_integer() {
        assert_eq!(
            SafeU64::new((1 << 53) - 1).map(SafeU64::get),
            Ok((1 << 53) - 1)
        );
    }

    #[test]
    fn rejects_one_past_the_largest_safe_integer() {
        assert_eq!(SafeU64::new(1 << 53), Err(SafeU64Overflow(1 << 53)));
    }

    #[test]
    fn serializes_as_a_plain_number() {
        let v = SafeU64::new(42).unwrap();
        assert_eq!(serde_json::to_string(&v).unwrap(), "42");
    }

    #[test]
    fn deserializing_an_unsafe_value_fails() {
        assert!(serde_json::from_str::<SafeU64>("9007199254740992").is_err());
        assert!(serde_json::from_str::<SafeU64>("9007199254740991").is_ok());
    }
}
