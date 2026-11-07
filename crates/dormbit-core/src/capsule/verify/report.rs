//! The verification report (design: VerificationReport).

use serde::{Deserialize, Serialize};

use crate::model::ErrorCode;

/// Always part of every report (Requirement 16.5).
pub const REPORT_LIMITATIONS: [&str; 2] = [
    "SHA-256 shows integrity, not authorship.",
    "The operating system's cache may serve the bytes that were read.",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CheckId {
    #[serde(rename = "V01_MARKER")]
    V01Marker,
    #[serde(rename = "V02_HEADER")]
    V02Header,
    #[serde(rename = "V03_ENTRY_TYPE_NAME")]
    V03EntryTypeName,
    #[serde(rename = "V04_ORDER")]
    V04Order,
    #[serde(rename = "V05_PADDING")]
    V05Padding,
    #[serde(rename = "V06_LIMITS")]
    V06Limits,
    #[serde(rename = "V07_MANIFEST")]
    V07Manifest,
    #[serde(rename = "V08_PATHS")]
    V08Paths,
    #[serde(rename = "V09_OBJECT_SET")]
    V09ObjectSet,
    #[serde(rename = "V10_STORED_HASH")]
    V10StoredHash,
    #[serde(rename = "V11_CONTENT_HASH")]
    V11ContentHash,
    #[serde(rename = "V12_TOTALS")]
    V12Totals,
    #[serde(rename = "V13_TRAILER")]
    V13Trailer,
    #[serde(rename = "V14_REGISTRY")]
    V14Registry,
}

impl CheckId {
    pub const ALL: [CheckId; 14] = [
        CheckId::V01Marker,
        CheckId::V02Header,
        CheckId::V03EntryTypeName,
        CheckId::V04Order,
        CheckId::V05Padding,
        CheckId::V06Limits,
        CheckId::V07Manifest,
        CheckId::V08Paths,
        CheckId::V09ObjectSet,
        CheckId::V10StoredHash,
        CheckId::V11ContentHash,
        CheckId::V12Totals,
        CheckId::V13Trailer,
        CheckId::V14Registry,
    ];

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|c| *c == self).unwrap_or(0)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            CheckId::V01Marker => "V01_MARKER",
            CheckId::V02Header => "V02_HEADER",
            CheckId::V03EntryTypeName => "V03_ENTRY_TYPE_NAME",
            CheckId::V04Order => "V04_ORDER",
            CheckId::V05Padding => "V05_PADDING",
            CheckId::V06Limits => "V06_LIMITS",
            CheckId::V07Manifest => "V07_MANIFEST",
            CheckId::V08Paths => "V08_PATHS",
            CheckId::V09ObjectSet => "V09_OBJECT_SET",
            CheckId::V10StoredHash => "V10_STORED_HASH",
            CheckId::V11ContentHash => "V11_CONTENT_HASH",
            CheckId::V12Totals => "V12_TOTALS",
            CheckId::V13Trailer => "V13_TRAILER",
            CheckId::V14Registry => "V14_REGISTRY",
        }
    }

    /// What the check covers, in plain words for the UI.
    pub fn label(self) -> &'static str {
        match self {
            CheckId::V01Marker => "Capsule marker and format version",
            CheckId::V02Header => "Archive headers",
            CheckId::V03EntryTypeName => "Entry types and names",
            CheckId::V04Order => "Entry order",
            CheckId::V05Padding => "Padding and end of archive",
            CheckId::V06Limits => "Size limits",
            CheckId::V07Manifest => "Manifest",
            CheckId::V08Paths => "Safe paths",
            CheckId::V09ObjectSet => "Stored files match the manifest",
            CheckId::V10StoredHash => "Stored bytes (SHA-256)",
            CheckId::V11ContentHash => "File contents (SHA-256)",
            CheckId::V12Totals => "Totals",
            CheckId::V13Trailer => "Trailer and manifest hash",
            CheckId::V14Registry => "Matches DORMBIT's record",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckState {
    Pass,
    Fail,
    Unknown,
    NotRun,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckResult {
    pub id: CheckId,
    pub status: CheckState,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailReason {
    StoredHashMismatch,
    ContentHashMismatch,
    SizeMismatch,
    Unverifiable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailedFile {
    pub path: String,
    pub reason: FailReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegistryState {
    Match,
    Mismatch,
    NotInRegistry,
    NotChecked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportStatus {
    Verified,
    Failed,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub capsule_id: Option<String>,
    pub file_sha256: Option<String>,
    pub manifest_sha256: Option<String>,
    pub checks: Vec<CheckResult>,
    pub failed_files: Vec<FailedFile>,
    pub registry_match: RegistryState,
    pub status: ReportStatus,
    pub error_code: Option<ErrorCode>,
    pub limitations: Vec<String>,
}

impl Report {
    pub fn check(&self, id: CheckId) -> Option<&CheckResult> {
        self.checks.iter().find(|c| c.id == id)
    }
}
