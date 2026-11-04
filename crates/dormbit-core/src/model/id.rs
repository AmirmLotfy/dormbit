//! String newtypes for IDs, so a scope ID can't be passed where a capsule ID is expected.
//! Hosts generate the values (ADR-001): UUIDs, except `AdapterId`, which is a fixed adapter
//! name such as `claude-code`. Each one serializes as a plain JSON string.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

macro_rules! id_types {
    ($($(#[$doc:meta])* $name:ident),* $(,)?) => {$(
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
        #[ts(export)]
        pub struct $name(pub String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_owned())
            }
        }
    )*};
}

id_types!(
    /// A folder grant (`ScopeGrant.id`).
    ScopeId,
    /// A discovered project (`ProjectRecord.id`).
    ProjectId,
    /// A capture plan (`CapturePlan.plan_id`).
    PlanId,
    /// A capsule (`CapsuleRef.id`, the manifest's `capsule_id`).
    CapsuleId,
    /// A running job (discovery, capture, verify, restore).
    JobId,
    /// A restore operation (`RestoreJournal.op_id`).
    OpId,
    /// An agent adapter, for example `claude-code`.
    AdapterId,
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_serialize_as_plain_strings() {
        let id = ScopeId::from("4b7e9a1c-0000-4000-8000-000000000001");
        assert_eq!(
            serde_json::to_string(&id).unwrap(),
            "\"4b7e9a1c-0000-4000-8000-000000000001\""
        );
        let back: ScopeId = serde_json::from_str("\"x\"").unwrap();
        assert_eq!(back.as_str(), "x");
    }
}
