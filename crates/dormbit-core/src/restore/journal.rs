//! The restore journal (design: RestoreJournal, Journal_Store). The host keeps it in
//! DORMBIT's own state folder; the state is written before the step it describes, so a
//! crash at any point leaves enough evidence for recovery.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::report::RestoreReport;
use crate::model::{DormbitError, ErrorCode, RestoreState};

pub const JOURNAL_SCHEMA: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevIno {
    pub dev: u64,
    pub ino: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateStamp {
    pub state: RestoreState,
    pub at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Journal {
    pub schema_version: u32,
    pub op_id: String,
    pub staging: String,
    pub owner_token: String,
    pub state: RestoreState,
    pub capsule_id: String,
    pub manifest_sha256: String,
    pub target_grant_id: String,
    pub target_name: String,
    pub tree_id: Option<DevIno>,
    pub files: u64,
    pub bytes: u64,
    pub history: Vec<StateStamp>,
    pub created_at: String,
    pub updated_at: String,
}

impl Journal {
    /// The exact contents of the staging folder's owner file.
    pub fn owner_line(&self) -> String {
        format!("{}:{}", self.op_id, self.owner_token)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WriteOp {
    Mkdir,
    Create,
    Rename,
    RemoveStaging,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WriteRoot {
    Staging,
    Target,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteRecord {
    pub seq: u64,
    pub op: WriteOp,
    pub root: WriteRoot,
    pub path: String,
    pub bytes: Option<u64>,
}

pub trait JournalStore {
    fn create(&mut self, j: &Journal) -> Result<(), DormbitError>;
    /// Persists a state change (and everything in the write log before it).
    fn set_state(&mut self, j: &Journal) -> Result<(), DormbitError>;
    fn append_write(&mut self, op_id: &str, rec: &WriteRecord) -> Result<(), DormbitError>;
    fn put_report(&mut self, op_id: &str, r: &RestoreReport) -> Result<(), DormbitError>;
    fn load(&self, op_id: &str) -> Result<Option<Journal>, DormbitError>;
    fn report(&self, op_id: &str) -> Result<Option<RestoreReport>, DormbitError>;
    fn writes(&self, op_id: &str) -> Result<Vec<WriteRecord>, DormbitError>;
    /// Every journal whose state isn't `Cleaned` or that has no report yet.
    fn pending(&self) -> Result<Vec<Journal>, DormbitError>;
}

#[derive(Debug, Default, Clone)]
pub struct MemJournal {
    pub journals: BTreeMap<String, Journal>,
    pub writes: BTreeMap<String, Vec<WriteRecord>>,
    pub reports: BTreeMap<String, RestoreReport>,
    /// Test hook: fail the n-th `set_state` call.
    pub fail_set_state_at: Option<u32>,
    set_state_calls: u32,
}

impl JournalStore for MemJournal {
    fn create(&mut self, j: &Journal) -> Result<(), DormbitError> {
        if self.journals.contains_key(&j.op_id) {
            return Err(DormbitError::new(
                ErrorCode::IoFailed,
                "A restore with that ID already exists.",
            ));
        }
        self.journals.insert(j.op_id.clone(), j.clone());
        Ok(())
    }

    fn set_state(&mut self, j: &Journal) -> Result<(), DormbitError> {
        self.set_state_calls += 1;
        if self.fail_set_state_at == Some(self.set_state_calls) {
            return Err(DormbitError::new(
                ErrorCode::IoFailed,
                "journal write failed",
            ));
        }
        self.journals.insert(j.op_id.clone(), j.clone());
        Ok(())
    }

    fn append_write(&mut self, op_id: &str, rec: &WriteRecord) -> Result<(), DormbitError> {
        self.writes
            .entry(op_id.to_owned())
            .or_default()
            .push(rec.clone());
        Ok(())
    }

    fn put_report(&mut self, op_id: &str, r: &RestoreReport) -> Result<(), DormbitError> {
        self.reports.insert(op_id.to_owned(), r.clone());
        Ok(())
    }

    fn load(&self, op_id: &str) -> Result<Option<Journal>, DormbitError> {
        Ok(self.journals.get(op_id).cloned())
    }

    fn report(&self, op_id: &str) -> Result<Option<RestoreReport>, DormbitError> {
        Ok(self.reports.get(op_id).cloned())
    }

    fn writes(&self, op_id: &str) -> Result<Vec<WriteRecord>, DormbitError> {
        Ok(self.writes.get(op_id).cloned().unwrap_or_default())
    }

    fn pending(&self) -> Result<Vec<Journal>, DormbitError> {
        Ok(self
            .journals
            .values()
            .filter(|j| j.state != RestoreState::Cleaned || !self.reports.contains_key(&j.op_id))
            .cloned()
            .collect())
    }
}
