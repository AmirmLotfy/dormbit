//! Reads objects out of a capsule that already passed verification.
//!
//! The index is built with the verifier's strict header parser and never uses tar names as
//! output paths. Every object is re-checked when it's read (stored hash, gzip form, size
//! cap, content hash), so a capsule swapped after verification can't slip anything in.

use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};

use sha2::{Digest, Sha256};

use crate::capsule::manifest::{Encoding, MARKER, Manifest};
use crate::capsule::ustar::{self, BLOCK};
use crate::capsule::verify::{GZIP_HEADER, object_hash, parse_header};
use crate::model::{DormbitError, ErrorCode};
use crate::util::{hex, sha256_hex};

fn damaged() -> DormbitError {
    DormbitError::new(
        ErrorCode::CapsuleHashMismatch,
        "The capsule changed since it was verified. Nothing was restored; verify it again.",
    )
}

/// Supplies decoded file contents by content hash.
pub trait CapsuleObjects {
    fn content(
        &mut self,
        content_sha256: &str,
        stored_sha256: &str,
        stored_size: u64,
        encoding: Encoding,
        size: u64,
    ) -> Result<Vec<u8>, DormbitError>;
}

pub struct OpenedCapsule<R: Read + Seek> {
    inner: R,
    pub manifest: Manifest,
    pub manifest_sha256: String,
    index: BTreeMap<String, (u64, u64)>,
}

fn exact<R: Read>(r: &mut R, buf: &mut [u8]) -> Result<(), DormbitError> {
    r.read_exact(buf).map_err(|_| damaged())
}

impl<R: Read + Seek> OpenedCapsule<R> {
    /// Indexes a capsule: marker, manifest, then object offsets up to `END`.
    pub fn open(mut inner: R, max_manifest: u64) -> Result<Self, DormbitError> {
        let mut h = [0u8; BLOCK];
        let mut header = |r: &mut R| -> Result<_, DormbitError> {
            exact(r, &mut h)?;
            parse_header(&h).map_err(|_| damaged())?.ok_or_else(damaged)
        };
        let m1 = header(&mut inner)?;
        if m1.name != "DORMBIT" || m1.size != MARKER.len() as u64 {
            return Err(damaged());
        }
        let mut mk = vec![0u8; MARKER.len()];
        exact(&mut inner, &mut mk)?;
        if mk != MARKER {
            return Err(damaged());
        }
        inner
            .seek(SeekFrom::Current(ustar::padding(m1.size) as i64))
            .map_err(|_| damaged())?;
        let m2 = header(&mut inner)?;
        if m2.name != "manifest.json" || m2.size > max_manifest {
            return Err(damaged());
        }
        let mut mbytes = vec![0u8; usize::try_from(m2.size).map_err(|_| damaged())?];
        exact(&mut inner, &mut mbytes)?;
        let manifest: Manifest = serde_json::from_slice(&mbytes).map_err(|_| damaged())?;
        inner
            .seek(SeekFrom::Current(ustar::padding(m2.size) as i64))
            .map_err(|_| damaged())?;
        let mut index = BTreeMap::new();
        loop {
            let e = header(&mut inner)?;
            if e.name == "END" {
                break;
            }
            let sha = object_hash(&e.name).ok_or_else(damaged)?.to_owned();
            let at = inner.stream_position().map_err(|_| damaged())?;
            index.insert(sha, (at, e.size));
            let skip = e.size + ustar::padding(e.size) as u64;
            inner
                .seek(SeekFrom::Current(
                    i64::try_from(skip).map_err(|_| damaged())?,
                ))
                .map_err(|_| damaged())?;
        }
        Ok(Self {
            inner,
            manifest,
            manifest_sha256: sha256_hex(&mbytes),
            index,
        })
    }
}

impl<R: Read + Seek> CapsuleObjects for OpenedCapsule<R> {
    fn content(
        &mut self,
        content_sha256: &str,
        stored_sha256: &str,
        stored_size: u64,
        encoding: Encoding,
        size: u64,
    ) -> Result<Vec<u8>, DormbitError> {
        let &(at, len) = self.index.get(content_sha256).ok_or_else(damaged)?;
        if len != stored_size {
            return Err(damaged());
        }
        self.inner
            .seek(SeekFrom::Start(at))
            .map_err(|_| damaged())?;
        let mut stored = vec![0u8; usize::try_from(len).map_err(|_| damaged())?];
        exact(&mut self.inner, &mut stored)?;
        if sha256_hex(&stored) != stored_sha256 {
            return Err(damaged());
        }
        let content = decode(&stored, encoding, size).ok_or_else(damaged)?;
        let mut h = Sha256::new();
        h.update(&content);
        if hex(&h.finalize()) != content_sha256 {
            return Err(damaged());
        }
        Ok(content)
    }
}

/// Decodes one object in the writer's form, producing at most `size` bytes.
fn decode(stored: &[u8], encoding: Encoding, size: u64) -> Option<Vec<u8>> {
    match encoding {
        Encoding::Identity => (stored.len() as u64 == size).then(|| stored.to_vec()),
        Encoding::Gzip => {
            if stored.get(..10) != Some(&GZIP_HEADER[..]) {
                return None;
            }
            let mut rest: &[u8] = stored;
            let mut out = Vec::with_capacity(usize::try_from(size.min(1 << 24)).unwrap_or(0));
            flate2::bufread::GzDecoder::new(&mut rest)
                .take(size.saturating_add(1))
                .read_to_end(&mut out)
                .ok()?;
            (out.len() as u64 == size && rest.is_empty()).then_some(out)
        }
    }
}
