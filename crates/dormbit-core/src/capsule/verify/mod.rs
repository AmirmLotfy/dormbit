//! The capsule verifier (design: Capsule verifier, checks V01-V14).
//!
//! One sequential pass over a fresh read-only handle. No `tar` crate: every header is parsed
//! with strict octal and must be byte-identical to the one form the writer produces
//! ([`ustar::header`]). Every limit is checked before the bytes it protects are read, and
//! decompression stops at `size + 1` bytes, so hostile input can't make the work unbounded.

pub mod report;

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use super::limits::CapsuleLimits;
use super::manifest::{Encoding, Entry, MARKER, Manifest, SCHEMA_VERSION, Trailer};
use super::ustar::{self, BLOCK, MODE_EXEC, MODE_FILE};
use super::writer::{Budget, CapsuleRecord, Job, JobProgress, Phase, Registry, Step};
use crate::model::{CapsuleId, DormbitError, ErrorCode};
use crate::util::{hex, sha256_hex, time::parse_rfc3339};
use crate::vfs::RelPath;
pub use report::{
    CheckId, CheckResult, CheckState, FailReason, FailedFile, REPORT_LIMITATIONS, RegistryState,
    Report, ReportStatus,
};

const TRAILER_MAX: u64 = 256;
/// What `GzBuilder::new().mtime(0)` writes at level 6: magic, deflate, no flags, mtime 0,
/// XFL 0, OS 255.
pub(crate) const GZIP_HEADER: [u8; 10] = [0x1f, 0x8b, 0x08, 0x00, 0, 0, 0, 0, 0x00, 0xff];
const MARKER_MAX: u64 = 64;
const CHUNK: usize = 64 * 1024;

/// The capsule file being verified. `open` returns a fresh read-only handle every time, so
/// the verifier never reuses the writer's handle (Requirement 13.1).
pub trait CapsuleSource {
    fn byte_len(&self) -> Result<u64, DormbitError>;
    fn open(&self) -> Result<Box<dyn Read + Send>, DormbitError>;
}

pub trait RegistryRead {
    fn get(&self, id: &CapsuleId) -> Result<Option<CapsuleRecord>, DormbitError>;
}

impl<T: Registry> RegistryRead for T {
    fn get(&self, id: &CapsuleId) -> Result<Option<CapsuleRecord>, DormbitError> {
        Registry::get(self, id)
    }
}

/// An in-memory capsule (tests and the web demo).
#[derive(Debug, Clone)]
pub struct BytesSource(pub std::sync::Arc<Vec<u8>>);

impl BytesSource {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(std::sync::Arc::new(bytes))
    }
}

struct ArcCursor {
    data: std::sync::Arc<Vec<u8>>,
    pos: usize,
}

impl Read for ArcCursor {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let rest = self.data.get(self.pos..).unwrap_or(&[]);
        let n = rest.len().min(buf.len());
        buf[..n].copy_from_slice(&rest[..n]);
        self.pos += n;
        Ok(n)
    }
}

impl CapsuleSource for BytesSource {
    fn byte_len(&self) -> Result<u64, DormbitError> {
        Ok(self.0.len() as u64)
    }
    fn open(&self) -> Result<Box<dyn Read + Send>, DormbitError> {
        Ok(Box::new(ArcCursor {
            data: self.0.clone(),
            pos: 0,
        }))
    }
}

/// A registry that knows no capsules (verifying a file picked from outside the library).
pub struct NoRegistry;

impl RegistryRead for NoRegistry {
    fn get(&self, _: &CapsuleId) -> Result<Option<CapsuleRecord>, DormbitError> {
        Ok(None)
    }
}

// ---- reading ---------------------------------------------------------------------------------

/// Why parsing stopped.
#[derive(Debug, Clone)]
pub(crate) enum Stop {
    Fail(CheckId, ErrorCode, String),
    Unsupported(CheckId, ErrorCode, String),
    Truncated(u64),
    Io(DormbitError),
}

struct Reader {
    inner: Box<dyn Read + Send>,
    pos: u64,
    hash: Sha256,
}

impl Reader {
    fn exact(&mut self, buf: &mut [u8]) -> Result<(), Stop> {
        let mut filled = 0;
        while filled < buf.len() {
            match self.inner.read(&mut buf[filled..]) {
                Ok(0) => return Err(Stop::Truncated(self.pos + filled as u64)),
                Ok(n) => filled += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => {
                    return Err(Stop::Io(DormbitError::new(
                        ErrorCode::IoFailed,
                        "DORMBIT could not read the capsule file.",
                    )));
                }
            }
        }
        self.hash.update(&buf[..]);
        self.pos += buf.len() as u64;
        Ok(())
    }

    fn vec(&mut self, n: u64) -> Result<Vec<u8>, Stop> {
        let mut out = Vec::with_capacity(usize::try_from(n.min(1 << 24)).unwrap_or(0));
        let mut left = n;
        let mut chunk = vec![0u8; CHUNK];
        while left > 0 {
            let k = usize::try_from(left.min(CHUNK as u64)).unwrap_or(CHUNK);
            self.exact(&mut chunk[..k])?;
            out.extend_from_slice(&chunk[..k]);
            left -= k as u64;
        }
        Ok(out)
    }

    fn padding(&mut self, len: u64) -> Result<(), Stop> {
        let pad = ustar::padding(len);
        if pad > 0 {
            let mut p = [0u8; BLOCK];
            self.exact(&mut p[..pad])?;
            if p[..pad].iter().any(|&b| b != 0) {
                return Err(Stop::Fail(
                    CheckId::V05Padding,
                    ErrorCode::CapsuleMalformed,
                    "Padding after an entry is not all zero.".into(),
                ));
            }
        }
        Ok(())
    }

    fn at_eof(&mut self) -> Result<bool, Stop> {
        let mut b = [0u8; 1];
        loop {
            match self.inner.read(&mut b) {
                Ok(0) => return Ok(true),
                Ok(_) => return Ok(false),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => {
                    return Err(Stop::Io(DormbitError::new(
                        ErrorCode::IoFailed,
                        "DORMBIT could not read the capsule file.",
                    )));
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Hdr {
    pub(crate) name: String,
    pub(crate) size: u64,
    pub(crate) mode: u32,
    pub(crate) mtime: u64,
}

fn strict_octal(field: &[u8]) -> Option<u64> {
    let (digits, last) = field.split_at(field.len() - 1);
    if last != [0] || digits.is_empty() || !digits.iter().all(|b| (b'0'..=b'7').contains(b)) {
        return None;
    }
    digits.iter().try_fold(0u64, |acc, &b| {
        acc.checked_mul(8)?.checked_add(u64::from(b - b'0'))
    })
}

fn malformed(id: CheckId, msg: &str) -> Stop {
    Stop::Fail(id, ErrorCode::CapsuleMalformed, msg.to_owned())
}

fn allowed_name(name: &str) -> bool {
    matches!(name, "DORMBIT" | "manifest.json" | "END") || object_hash(name).is_some()
}

pub(crate) fn object_hash(name: &str) -> Option<&str> {
    let h = name.strip_prefix("objects/")?;
    is_hex64(h).then_some(h)
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// `None` is an all-zero block (end of archive).
pub(crate) fn parse_header(h: &[u8; BLOCK]) -> Result<Option<Hdr>, Stop> {
    if h.iter().all(|&b| b == 0) {
        return Ok(None);
    }
    if h[156] != b'0' {
        return Err(malformed(
            CheckId::V03EntryTypeName,
            "An entry is not a regular file.",
        ));
    }
    let end = h[..100].iter().position(|&b| b == 0).unwrap_or(100);
    let name = std::str::from_utf8(&h[..end])
        .ok()
        .filter(|n| allowed_name(n))
        .ok_or_else(|| {
            malformed(
                CheckId::V03EntryTypeName,
                "An entry has a name a capsule never uses.",
            )
        })?
        .to_owned();
    let bad = || {
        malformed(
            CheckId::V02Header,
            "An archive header is not in the capsule format.",
        )
    };
    let mode = strict_octal(&h[100..108]).ok_or_else(bad)?;
    let size = strict_octal(&h[124..136]).ok_or_else(bad)?;
    let mtime = strict_octal(&h[136..148]).ok_or_else(bad)?;
    let mode = u32::try_from(mode).map_err(|_| bad())?;
    // The writer gives 0755 only to objects that hold an executable file; the marker,
    // manifest, and trailer are always 0644.
    let mode_ok = if object_hash(&name).is_some() {
        mode == MODE_FILE || mode == MODE_EXEC
    } else {
        mode == MODE_FILE
    };
    if !mode_ok {
        return Err(bad());
    }
    let expected = ustar::header(&name, size, mode, mtime).map_err(|_| bad())?;
    if &expected != h {
        return Err(bad());
    }
    Ok(Some(Hdr {
        name,
        size,
        mode,
        mtime,
    }))
}

// ---- the job ---------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VPhase {
    Start,
    Objects,
    Registry,
    Done,
}

/// A check passes only once it has seen everything it covers (`complete`). After a stop,
/// an incomplete check without a failure is `NotRun`, never `Pass`.
#[derive(Debug, Clone, Default)]
struct Slot {
    complete: bool,
    fail: Option<String>,
    unknown: Option<String>,
}

#[derive(Debug, Clone)]
struct Expected {
    size: u64,
    stored_size: u64,
    stored_sha: String,
    encoding: Encoding,
    exec: bool,
    paths: Vec<String>,
}

pub struct VerifyJob<S: CapsuleSource, R: RegistryRead> {
    source: S,
    registry: R,
    limits: CapsuleLimits,
    phase: VPhase,
    cancelled: bool,
    reader: Option<Reader>,
    slots: Vec<Slot>,
    first_error: Option<ErrorCode>,
    stop: Option<Stop>,
    file_len: u64,
    manifest: Option<Manifest>,
    manifest_parsed: bool,
    manifest_sha: Option<String>,
    file_sha: Option<String>,
    created: u64,
    expected: BTreeMap<String, Expected>,
    seen: BTreeSet<String>,
    verified: BTreeSet<String>,
    last_object: Option<String>,
    objects_read: u64,
    failed: BTreeMap<String, FailReason>,
    registry_state: RegistryState,
    bytes_done: u64,
    report: Option<Report>,
}

impl<S: CapsuleSource, R: RegistryRead> VerifyJob<S, R> {
    pub fn new(source: S, registry: R, limits: CapsuleLimits) -> Self {
        Self {
            source,
            registry,
            limits,
            phase: VPhase::Start,
            cancelled: false,
            reader: None,
            slots: vec![Slot::default(); CheckId::ALL.len()],
            first_error: None,
            stop: None,
            file_len: 0,
            manifest: None,
            manifest_parsed: false,
            manifest_sha: None,
            file_sha: None,
            created: 0,
            expected: BTreeMap::new(),
            seen: BTreeSet::new(),
            verified: BTreeSet::new(),
            last_object: None,
            objects_read: 0,
            failed: BTreeMap::new(),
            registry_state: RegistryState::NotChecked,
            bytes_done: 0,
            report: None,
        }
    }

    fn complete(&mut self, ids: &[CheckId]) {
        for id in ids {
            self.slots[id.index()].complete = true;
        }
    }

    /// Records a failure that doesn't stop parsing.
    fn fail(&mut self, id: CheckId, code: ErrorCode, detail: String) {
        let s = &mut self.slots[id.index()];
        if s.fail.is_none() {
            s.fail = Some(detail);
        }
        if self.first_error.is_none() {
            self.first_error = Some(code);
        }
    }

    fn mark_file(&mut self, sha: &str, reason: FailReason) {
        let paths = self
            .expected
            .get(sha)
            .map(|e| e.paths.clone())
            .unwrap_or_default();
        for p in paths {
            self.failed.entry(p).or_insert(reason);
        }
    }

    fn reader(&mut self) -> Result<&mut Reader, Stop> {
        self.reader.as_mut().ok_or_else(|| {
            Stop::Io(DormbitError::new(
                ErrorCode::IoFailed,
                "The capsule file is not open.",
            ))
        })
    }

    fn header(&mut self) -> Result<Option<Hdr>, Stop> {
        let mut h = [0u8; BLOCK];
        self.reader()?.exact(&mut h)?;
        parse_header(&h)
    }

    // ---- marker and manifest -----------------------------------------------------------------

    fn start(&mut self) -> Result<(), Stop> {
        self.file_len = self.source.byte_len().map_err(Stop::Io)?;
        if self.file_len > self.limits.max_capsule_size {
            return Err(Stop::Unsupported(
                CheckId::V06Limits,
                ErrorCode::CapsuleLimitExceeded,
                "The capsule file is larger than the capsule size limit.".into(),
            ));
        }
        let inner = self.source.open().map_err(Stop::Io)?;
        self.reader = Some(Reader {
            inner,
            pos: 0,
            hash: Sha256::new(),
        });

        // V01: the marker.
        let marker_name_wrong = || {
            malformed(
                CheckId::V01Marker,
                "The first entry is not the DORMBIT marker.",
            )
        };
        let h1 = match self.header() {
            Ok(Some(h)) if h.name == "DORMBIT" => h,
            Ok(_) => return Err(marker_name_wrong()),
            Err(Stop::Fail(CheckId::V03EntryTypeName, ..)) => return Err(marker_name_wrong()),
            Err(e) => return Err(e),
        };
        if h1.size > MARKER_MAX {
            return Err(malformed(
                CheckId::V01Marker,
                "The capsule marker is not valid.",
            ));
        }
        let marker = self.reader()?.vec(h1.size)?;
        self.reader()?.padding(h1.size)?;
        if marker != MARKER {
            let text = std::str::from_utf8(&marker).unwrap_or("");
            let version = text
                .strip_prefix("dormbit-capsule/")
                .and_then(|v| v.strip_suffix('\n'))
                .filter(|v| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()));
            return Err(match version {
                Some(_) => Stop::Unsupported(
                    CheckId::V01Marker,
                    ErrorCode::CapsuleVersionUnsupported,
                    "This capsule uses a format version DORMBIT doesn't support.".into(),
                ),
                None => malformed(CheckId::V01Marker, "The capsule marker is not valid."),
            });
        }
        self.complete(&[CheckId::V01Marker]);

        // V07: the manifest.
        let h2 = match self.header()? {
            Some(h) if h.name == "manifest.json" => h,
            _ => {
                return Err(malformed(
                    CheckId::V04Order,
                    "The manifest is not the second entry.",
                ));
            }
        };
        if h2.size > self.limits.max_manifest_size {
            return Err(Stop::Unsupported(
                CheckId::V06Limits,
                ErrorCode::CapsuleLimitExceeded,
                "The manifest is larger than the manifest size limit.".into(),
            ));
        }
        let mbytes = self.reader()?.vec(h2.size)?;
        self.reader()?.padding(h2.size)?;
        self.manifest_sha = Some(sha256_hex(&mbytes));
        let bad_manifest = || malformed(CheckId::V07Manifest, "The manifest is not valid.");
        let value: serde_json::Value =
            serde_json::from_slice(&mbytes).map_err(|_| bad_manifest())?;
        match value
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
        {
            Some(v) if v == u64::from(SCHEMA_VERSION) => {}
            Some(_) => {
                return Err(Stop::Unsupported(
                    CheckId::V07Manifest,
                    ErrorCode::CapsuleVersionUnsupported,
                    "This capsule's manifest uses a version DORMBIT doesn't support.".into(),
                ));
            }
            None => return Err(bad_manifest()),
        }
        let m: Manifest = serde_json::from_value(value).map_err(|_| bad_manifest())?;
        // From here on the file list is known, so any later stop can name the files it
        // leaves unverified (Requirement 14.10).
        self.index_objects(&m);
        self.manifest_parsed = true;
        let created = parse_rfc3339(&m.created_at)
            .and_then(|t| u64::try_from(t).ok())
            .ok_or_else(bad_manifest)?;
        if !m
            .capsule_id
            .bytes()
            .all(|b| b.is_ascii_hexdigit() || b == b'-')
            || m.capsule_id.is_empty()
            || m.capsule_id.len() > 64
            || !is_hex64(&m.plan_digest)
        {
            return Err(bad_manifest());
        }
        self.created = created;
        if h1.mtime != created || h2.mtime != created {
            return Err(malformed(
                CheckId::V02Header,
                "An archive header's time doesn't match the manifest.",
            ));
        }
        self.check_manifest(&m)?;
        self.manifest = Some(m);
        self.complete(&[CheckId::V07Manifest, CheckId::V08Paths, CheckId::V12Totals]);
        Ok(())
    }

    /// V06 entry limits, V08 paths, V09 object references, V12 totals.
    fn check_manifest(&mut self, m: &Manifest) -> Result<(), Stop> {
        let l = &self.limits;
        let over = |msg: &str| {
            Stop::Unsupported(
                CheckId::V06Limits,
                ErrorCode::CapsuleLimitExceeded,
                msg.to_owned(),
            )
        };
        if m.entries.len() as u64 > l.max_entries {
            return Err(over("The capsule has more entries than the limit allows."));
        }
        if m.excluded.len() as u64 > l.max_excluded || m.warnings.len() as u64 > l.max_warnings {
            return Err(over(
                "The capsule's lists are longer than the limits allow.",
            ));
        }
        for e in &m.entries {
            if let Entry::File {
                size,
                object,
                sha256,
                ..
            } = e
            {
                if !is_hex64(sha256) || !is_hex64(&object.stored_sha256) {
                    return Err(malformed(
                        CheckId::V07Manifest,
                        "The manifest is not valid.",
                    ));
                }
                if *size > l.max_file_size {
                    return Err(over(
                        "A file in the capsule is larger than the per-file limit.",
                    ));
                }
                let allowed = l
                    .max_expansion_ratio
                    .saturating_mul(object.stored_size.max(1))
                    .max(l.expansion_floor);
                if *size > allowed {
                    return Err(over(
                        "A stored file would expand far beyond its stored size.",
                    ));
                }
            }
        }

        // V08: Safe paths, no exact or NFC duplicates, no path that is also a folder.
        let mut exact = BTreeSet::new();
        let mut nfc = BTreeSet::new();
        for e in &m.entries {
            let p = e.path();
            if RelPath::parse(p).is_err() || p.is_empty() {
                self.fail(
                    CheckId::V08Paths,
                    ErrorCode::PathUnsafe,
                    format!("{} is not a safe path.", printable(p)),
                );
                continue;
            }
            if !exact.insert(p.to_owned()) || !nfc.insert(p.nfc().collect::<String>()) {
                self.fail(
                    CheckId::V08Paths,
                    ErrorCode::CapsuleMalformed,
                    format!("{} appears twice.", printable(p)),
                );
            }
            if let Entry::Symlink {
                target, restored, ..
            } = e
                && (*restored || target.is_empty() || target.contains('\0'))
            {
                self.fail(
                    CheckId::V08Paths,
                    ErrorCode::CapsuleMalformed,
                    format!("The link {} is not recorded correctly.", printable(p)),
                );
            }
        }
        for p in &exact {
            let mut prefix = p.clone();
            prefix.push('/');
            if exact
                .range(prefix.clone()..)
                .next()
                .is_some_and(|q| q.starts_with(&prefix))
            {
                self.fail(
                    CheckId::V08Paths,
                    ErrorCode::CapsuleMalformed,
                    format!("{} is both a file and a folder.", printable(p)),
                );
            }
        }

        self.check_totals(m);
        Ok(())
    }

    /// V09: groups file entries by object; every entry that shares an object must agree on it.
    fn index_objects(&mut self, m: &Manifest) {
        for e in &m.entries {
            if let Entry::File {
                path,
                size,
                mode,
                sha256,
                object,
                ..
            } = e
            {
                let exec = mode & 0o111 != 0;
                let mut disagree = false;
                match self.expected.get_mut(sha256) {
                    Some(x) => {
                        disagree = x.size != *size
                            || x.stored_size != object.stored_size
                            || x.stored_sha != object.stored_sha256
                            || x.encoding != object.encoding;
                        x.exec |= exec;
                        x.paths.push(path.clone());
                    }
                    None => {
                        self.expected.insert(
                            sha256.clone(),
                            Expected {
                                size: *size,
                                stored_size: object.stored_size,
                                stored_sha: object.stored_sha256.clone(),
                                encoding: object.encoding,
                                exec,
                                paths: vec![path.clone()],
                            },
                        );
                    }
                }
                if disagree {
                    self.fail(
                        CheckId::V09ObjectSet,
                        ErrorCode::CapsuleMalformed,
                        "Two entries describe the same stored file differently.".into(),
                    );
                }
            }
        }
    }

    /// V12: totals.
    fn check_totals(&mut self, m: &Manifest) {
        let t = &m.totals;
        let files = m
            .entries
            .iter()
            .filter(|e| matches!(e, Entry::File { .. }))
            .count() as u64;
        let symlinks = m.entries.len() as u64 - files;
        let bytes = m
            .entries
            .iter()
            .map(|e| match e {
                Entry::File { size, .. } => *size,
                Entry::Symlink { .. } => 0,
            })
            .fold(0u64, u64::saturating_add);
        let stored = self
            .expected
            .values()
            .map(|x| x.stored_size)
            .fold(0u64, u64::saturating_add);
        if t.files != files
            || t.symlinks != symlinks
            || t.objects != self.expected.len() as u64
            || t.bytes != bytes
            || t.stored_bytes != stored
            || t.excluded != m.excluded.len() as u64
        {
            self.fail(
                CheckId::V12Totals,
                ErrorCode::CapsuleMalformed,
                "The manifest totals don't match its entries.".into(),
            );
        }
    }

    // ---- objects -----------------------------------------------------------------------------

    /// Reads one entry after the manifest. Returns `true` once `END` and the end of the
    /// archive have been read.
    fn next_entry(&mut self) -> Result<bool, Stop> {
        let h = match self.header()? {
            Some(h) => h,
            None => {
                return Err(Stop::Fail(
                    CheckId::V13Trailer,
                    ErrorCode::CapsuleTruncated,
                    "The capsule ends without its END trailer.".into(),
                ));
            }
        };
        if h.mtime != self.created {
            return Err(malformed(
                CheckId::V02Header,
                "An archive header's time doesn't match the manifest.",
            ));
        }
        if h.name == "END" {
            self.trailer(&h)?;
            return Ok(true);
        }
        let Some(sha) = object_hash(&h.name).map(str::to_owned) else {
            return Err(malformed(CheckId::V04Order, "An entry is out of order."));
        };
        if self.last_object.as_ref().is_some_and(|last| *last >= sha) {
            return Err(malformed(
                CheckId::V04Order,
                "Stored files are out of order or repeated.",
            ));
        }
        self.last_object = Some(sha.clone());
        self.objects_read += 1;
        self.seen.insert(sha.clone());

        // A size longer than what's left of the file means the file was cut short, not that
        // a limit was exceeded.
        let remaining = self.file_len.saturating_sub(self.reader()?.pos);
        if h.size > remaining {
            return Err(Stop::Truncated(self.file_len));
        }
        let bound = self
            .limits
            .max_file_size
            .saturating_add(self.limits.max_file_size / 64)
            .saturating_add(1 << 20);
        if h.size > bound {
            return Err(Stop::Unsupported(
                CheckId::V06Limits,
                ErrorCode::CapsuleLimitExceeded,
                "A stored file is larger than the limits allow.".into(),
            ));
        }

        let Some(exp) = self.expected.get(&sha).cloned() else {
            self.fail(
                CheckId::V09ObjectSet,
                ErrorCode::CapsuleMalformed,
                "The capsule holds a stored file the manifest doesn't list.".into(),
            );
            let _ = self.reader()?.vec(h.size)?;
            self.reader()?.padding(h.size)?;
            return Ok(false);
        };
        let want_mode = if exp.exec { MODE_EXEC } else { MODE_FILE };
        if h.mode != want_mode {
            return Err(malformed(
                CheckId::V02Header,
                "A stored file's archive mode doesn't match the manifest.",
            ));
        }

        let data = self.reader()?.vec(h.size)?;
        self.reader()?.padding(h.size)?;
        self.bytes_done = self.bytes_done.saturating_add(h.size);

        // V10: the stored bytes.
        if h.size != exp.stored_size || sha256_hex(&data) != exp.stored_sha {
            self.fail(
                CheckId::V10StoredHash,
                ErrorCode::CapsuleHashMismatch,
                "Stored bytes don't match their recorded SHA-256.".into(),
            );
            self.mark_file(&sha, FailReason::StoredHashMismatch);
            return Ok(false);
        }

        // V11: the content, decompressed at most `size + 1` bytes.
        match content_check(&data, exp.encoding, exp.size) {
            Ok(content_sha) if content_sha == sha => {
                self.verified.insert(sha);
            }
            Ok(_) | Err(FailReason::ContentHashMismatch) => {
                self.fail(
                    CheckId::V11ContentHash,
                    ErrorCode::CapsuleHashMismatch,
                    "A file's contents don't match their recorded SHA-256.".into(),
                );
                self.mark_file(&sha, FailReason::ContentHashMismatch);
            }
            Err(reason) => {
                self.fail(
                    CheckId::V11ContentHash,
                    ErrorCode::CapsuleHashMismatch,
                    "A file's size doesn't match the manifest.".into(),
                );
                self.mark_file(&sha, reason);
            }
        }
        Ok(false)
    }

    fn trailer(&mut self, h: &Hdr) -> Result<(), Stop> {
        if h.size > TRAILER_MAX {
            return Err(malformed(
                CheckId::V13Trailer,
                "The END trailer is not valid.",
            ));
        }
        let t = self.reader()?.vec(h.size)?;
        self.reader()?.padding(h.size)?;
        let trailer: Trailer = serde_json::from_slice(&t)
            .map_err(|_| malformed(CheckId::V13Trailer, "The END trailer is not valid."))?;
        if Some(&trailer.manifest_sha256) != self.manifest_sha.as_ref() {
            self.fail(
                CheckId::V13Trailer,
                ErrorCode::CapsuleHashMismatch,
                "The manifest doesn't match the hash in the END trailer.".into(),
            );
        }
        if trailer.object_count != self.objects_read {
            self.fail(
                CheckId::V13Trailer,
                ErrorCode::CapsuleHashMismatch,
                "The END trailer counts a different number of stored files.".into(),
            );
        }
        let missing: Vec<String> = self
            .expected
            .keys()
            .filter(|k| !self.seen.contains(*k))
            .cloned()
            .collect();
        if !missing.is_empty() {
            self.fail(
                CheckId::V09ObjectSet,
                ErrorCode::CapsuleMalformed,
                format!(
                    "{} stored file(s) listed in the manifest are missing.",
                    missing.len()
                ),
            );
            for sha in missing {
                self.mark_file(&sha, FailReason::Unverifiable);
            }
        }
        // Exactly two zero blocks, then the end of the file.
        let mut z = [0u8; BLOCK * 2];
        self.reader()?.exact(&mut z)?;
        if z.iter().any(|&b| b != 0) {
            return Err(malformed(
                CheckId::V05Padding,
                "The end of the archive is not two zero blocks.",
            ));
        }
        if !self.reader()?.at_eof()? {
            return Err(malformed(
                CheckId::V05Padding,
                "There is data after the end of the archive.",
            ));
        }
        let hash = self.reader()?.hash.clone();
        self.file_sha = Some(hex(&hash.finalize()));
        // The whole file has been read: every per-entry check has seen everything.
        self.complete(&[
            CheckId::V02Header,
            CheckId::V03EntryTypeName,
            CheckId::V04Order,
            CheckId::V05Padding,
            CheckId::V06Limits,
            CheckId::V09ObjectSet,
            CheckId::V10StoredHash,
            CheckId::V11ContentHash,
            CheckId::V13Trailer,
        ]);
        Ok(())
    }

    fn check_registry(&mut self) -> Result<(), DormbitError> {
        self.complete(&[CheckId::V14Registry]);
        let Some(m) = &self.manifest else {
            return Ok(());
        };
        let id = CapsuleId(m.capsule_id.clone());
        match self.registry.get(&id)? {
            None => {
                self.registry_state = RegistryState::NotInRegistry;
                self.slots[CheckId::V14Registry.index()].unknown = Some(
                    "This capsule isn't in DORMBIT's list, so there's no record to compare.".into(),
                );
            }
            Some(rec) => {
                let same = Some(&rec.file_sha256) == self.file_sha.as_ref()
                    && Some(&rec.manifest_sha256) == self.manifest_sha.as_ref();
                if same {
                    self.registry_state = RegistryState::Match;
                } else {
                    self.registry_state = RegistryState::Mismatch;
                    self.fail(
                        CheckId::V14Registry,
                        ErrorCode::CapsuleHashMismatch,
                        "The file doesn't match the SHA-256 DORMBIT recorded when it was made."
                            .into(),
                    );
                }
            }
        }
        Ok(())
    }

    fn apply_stop(&mut self, stop: Stop) -> Result<(), DormbitError> {
        let (id, code, mut detail) = match &stop {
            Stop::Io(e) => return Err(e.clone()),
            Stop::Fail(id, code, d) | Stop::Unsupported(id, code, d) => (*id, *code, d.clone()),
            Stop::Truncated(at) => (
                CheckId::V13Trailer,
                ErrorCode::CapsuleTruncated,
                format!("The capsule file ends early, at byte {at}."),
            ),
        };
        if !self.manifest_parsed {
            detail.push_str(" The manifest wasn't read, so no file list is available.");
        }
        let s = &mut self.slots[id.index()];
        // An Unsupported stop isn't a failure of the capsule: the check is Unknown and the
        // report status is Unsupported (Requirements 16.1-16.3).
        if matches!(stop, Stop::Unsupported(..)) {
            s.unknown = Some(detail);
        } else {
            s.fail = Some(detail);
        }
        if self.first_error.is_none() {
            self.first_error = Some(code);
        }
        // Every file whose stored bytes weren't verified can't be trusted (Requirement 14.10).
        let unverified: Vec<String> = self
            .expected
            .keys()
            .filter(|k| !self.verified.contains(*k))
            .cloned()
            .collect();
        for sha in unverified {
            self.mark_file(&sha, FailReason::Unverifiable);
        }
        self.stop = Some(stop);
        Ok(())
    }

    fn build_report(&self) -> Report {
        let checks: Vec<CheckResult> = CheckId::ALL
            .iter()
            .map(|&id| {
                let s = &self.slots[id.index()];
                let (status, detail) = if let Some(d) = &s.fail {
                    (CheckState::Fail, d.clone())
                } else if let Some(d) = &s.unknown {
                    (CheckState::Unknown, d.clone())
                } else if s.complete {
                    (CheckState::Pass, "Passed.".to_owned())
                } else {
                    (
                        CheckState::NotRun,
                        "Not run: verification stopped earlier.".to_owned(),
                    )
                };
                CheckResult { id, status, detail }
            })
            .collect();
        let any_fail = checks.iter().any(|c| c.status == CheckState::Fail);
        let unsupported = matches!(self.stop, Some(Stop::Unsupported(..)));
        let status = if any_fail {
            ReportStatus::Failed
        } else if unsupported {
            ReportStatus::Unsupported
        } else {
            ReportStatus::Verified
        };
        let mut failed_files: Vec<FailedFile> = self
            .failed
            .iter()
            .map(|(path, reason)| FailedFile {
                path: path.clone(),
                reason: *reason,
            })
            .collect();
        failed_files.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
        Report {
            capsule_id: self.manifest.as_ref().map(|m| m.capsule_id.clone()),
            file_sha256: self.file_sha.clone(),
            manifest_sha256: self.manifest_sha.clone(),
            checks,
            failed_files,
            registry_match: self.registry_state,
            status,
            error_code: self.first_error,
            limitations: REPORT_LIMITATIONS.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    fn progress(&self, phase: Phase) -> JobProgress {
        JobProgress {
            phase,
            files_done: self.objects_read,
            files_total: self.expected.len() as u64,
            bytes_done: self.reader.as_ref().map_or(0, |r| r.pos),
            bytes_total: self.file_len,
        }
    }

    fn finish(&mut self) -> Step<Report> {
        self.phase = VPhase::Done;
        let r = self.build_report();
        self.report = Some(r.clone());
        Step::Done(r)
    }
}

/// Pretty-prints a manifest path for a message without letting control characters through.
fn printable(p: &str) -> String {
    p.chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .take(200)
        .collect()
}

/// Decompresses (or passes through) `stored`, stopping one byte past `size`. Returns the
/// content SHA-256.
fn content_check(stored: &[u8], encoding: Encoding, size: u64) -> Result<String, FailReason> {
    let mut hasher = Sha256::new();
    let mut total = 0u64;
    match encoding {
        Encoding::Identity => {
            total = stored.len() as u64;
            hasher.update(stored);
        }
        Encoding::Gzip => {
            // Only the writer's form: the fixed 10-byte header (no name, no comment, mtime 0,
            // level 6, OS unknown), exactly one member, and nothing after it.
            if stored.get(..10) != Some(&GZIP_HEADER[..]) {
                return Err(FailReason::ContentHashMismatch);
            }
            let mut rest: &[u8] = stored;
            {
                let mut d = flate2::bufread::GzDecoder::new(&mut rest).take(size.saturating_add(1));
                let mut buf = vec![0u8; CHUNK];
                loop {
                    match d.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            total += n as u64;
                            hasher.update(&buf[..n]);
                        }
                        Err(_) => return Err(FailReason::ContentHashMismatch),
                    }
                }
            }
            if total == size && !rest.is_empty() {
                return Err(FailReason::ContentHashMismatch);
            }
        }
    }
    if total != size {
        return Err(FailReason::SizeMismatch);
    }
    Ok(hex(&hasher.finalize()))
}

impl<S: CapsuleSource, R: RegistryRead> Job for VerifyJob<S, R> {
    type Output = Report;

    fn step(&mut self, budget: Budget) -> Result<Step<Report>, DormbitError> {
        if let Some(r) = &self.report {
            return Ok(Step::Done(r.clone()));
        }
        if self.cancelled {
            return Err(DormbitError::new(
                ErrorCode::Cancelled,
                "Verification cancelled. No result was recorded.",
            ));
        }
        match self.phase {
            VPhase::Start => {
                match self.start() {
                    Ok(()) => self.phase = VPhase::Objects,
                    Err(stop) => {
                        self.apply_stop(stop)?;
                        return Ok(self.finish());
                    }
                }
                Ok(Step::Progress(self.progress(Phase::Preparing)))
            }
            VPhase::Objects => {
                let (mut ops, start_bytes) = (0u32, self.bytes_done);
                loop {
                    match self.next_entry() {
                        Ok(true) => {
                            self.phase = VPhase::Registry;
                            break;
                        }
                        Ok(false) => {}
                        Err(stop) => {
                            self.apply_stop(stop)?;
                            return Ok(self.finish());
                        }
                    }
                    ops = ops.saturating_add(1);
                    if ops >= budget.max_ops || self.bytes_done - start_bytes >= budget.max_bytes {
                        break;
                    }
                }
                Ok(Step::Progress(self.progress(Phase::Hashing)))
            }
            VPhase::Registry => {
                self.check_registry()?;
                Ok(self.finish())
            }
            VPhase::Done => Err(DormbitError::new(
                ErrorCode::Cancelled,
                "This verification has already finished.",
            )),
        }
    }

    fn cancel(&mut self) {
        self.cancelled = true;
    }
}

/// Runs a verification to completion with an unbounded budget (tests and the CLI).
pub fn verify_all<S: CapsuleSource, R: RegistryRead>(
    source: S,
    registry: R,
    limits: CapsuleLimits,
) -> Result<Report, DormbitError> {
    let mut job = VerifyJob::new(source, registry, limits);
    super::writer::run_to_end(&mut job)
}
