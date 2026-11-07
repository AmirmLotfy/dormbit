//! In-memory workspace, registry, and capsule source. Used by core tests and the web demo
//! (`dormbit-wasm`). `MemWorkspace::commit` refuses an existing name like the native sink.

use std::collections::BTreeMap;

use super::writer::{CapsuleRecord, CaptureWorkspace, Registry};
use crate::model::{CapsuleId, DormbitError, ErrorCode};

#[derive(Debug, Default, Clone)]
pub struct MemWorkspace {
    /// The library folder: committed capsules by file name.
    pub library: BTreeMap<String, Vec<u8>>,
    objects: BTreeMap<String, Vec<u8>>,
    out: Vec<u8>,
    pub aborted: bool,
    pub committed: Option<String>,
}

impl MemWorkspace {
    pub fn new() -> Self {
        Self::default()
    }

    /// A library that already holds `name` (to test `CAPSULE_EXISTS`).
    pub fn with_existing(name: &str, bytes: Vec<u8>) -> Self {
        let mut ws = Self::default();
        ws.library.insert(name.to_owned(), bytes);
        ws
    }

    /// Test hook: replaces a staged object's bytes (simulates staging corruption).
    pub fn tamper_object(&mut self, content_sha256: &str, bytes: Vec<u8>) {
        if let Some(o) = self.objects.get_mut(content_sha256) {
            *o = bytes;
        }
    }

    pub fn staged_objects(&self) -> usize {
        self.objects.len()
    }
}

impl CaptureWorkspace for MemWorkspace {
    fn put_object(&mut self, content_sha256: &str, stored: &[u8]) -> Result<bool, DormbitError> {
        if self.objects.contains_key(content_sha256) {
            return Ok(true);
        }
        self.objects
            .insert(content_sha256.to_owned(), stored.to_vec());
        Ok(false)
    }

    fn read_object(&self, content_sha256: &str) -> Result<Vec<u8>, DormbitError> {
        self.objects.get(content_sha256).cloned().ok_or_else(|| {
            DormbitError::new(
                ErrorCode::CapsuleHashMismatch,
                "A stored file is missing from the staging folder.",
            )
        })
    }

    fn write_out(&mut self, bytes: &[u8]) -> Result<(), DormbitError> {
        self.out.extend_from_slice(bytes);
        Ok(())
    }

    fn commit(&mut self, final_name: &str) -> Result<(), DormbitError> {
        if self.library.contains_key(final_name) {
            return Err(DormbitError::new(
                ErrorCode::CapsuleExists,
                "A capsule with that name is already in the library. Nothing was replaced.",
            ));
        }
        self.library
            .insert(final_name.to_owned(), std::mem::take(&mut self.out));
        self.objects.clear();
        self.committed = Some(final_name.to_owned());
        Ok(())
    }

    fn abort(&mut self) -> Result<(), DormbitError> {
        self.objects.clear();
        self.out.clear();
        self.aborted = true;
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct MemRegistry {
    pub records: BTreeMap<String, CapsuleRecord>,
    /// Test hook: makes `put` fail.
    pub fail_put: bool,
}

impl Registry for MemRegistry {
    fn get(&self, id: &CapsuleId) -> Result<Option<CapsuleRecord>, DormbitError> {
        Ok(self.records.get(id.as_str()).cloned())
    }

    fn put(&mut self, record: CapsuleRecord) -> Result<(), DormbitError> {
        if self.fail_put {
            return Err(DormbitError::new(
                ErrorCode::IoFailed,
                "registry write failed",
            ));
        }
        self.records
            .insert(record.capsule_id.as_str().to_owned(), record);
        Ok(())
    }
}
