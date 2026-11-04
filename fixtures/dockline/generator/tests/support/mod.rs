//! Shared helpers for the Fixture_Check and the determinism property test.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use sha2::{Digest, Sha256};

/// Holds file stat data (times, inodes), so it differs on every run; excluded by design.
pub const INDEX: &str = "dockline-invoices/.git/index";
pub const TAR: &str = "dockline-invoices.tar";
pub const SIDECAR: &str = "dockline-invoices.sidecar.json";

#[derive(PartialEq, Eq, Debug)]
pub struct Entry {
    kind: u8,
    mode: u32,
    size: u64,
    sha256: String,
}

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn entries(tar_path: &Path) -> BTreeMap<String, Entry> {
    let file = std::fs::File::open(tar_path)
        .unwrap_or_else(|e| panic!("open {}: {e}", tar_path.display()));
    let mut archive = tar::Archive::new(file);
    let mut out = BTreeMap::new();
    for entry in archive.entries().expect("tar entries") {
        let mut entry = entry.expect("tar entry");
        let path = entry
            .path()
            .expect("entry path")
            .to_string_lossy()
            .into_owned();
        let header = entry.header();
        let (kind, mode, size) = (
            header.entry_type().as_byte(),
            header.mode().expect("mode"),
            header.size().expect("size"),
        );
        let mut data = Vec::new();
        entry.read_to_end(&mut data).expect("entry data");
        out.insert(
            path,
            Entry {
                kind,
                mode,
                size,
                sha256: hex(&data),
            },
        );
    }
    out
}

/// A fresh, unique folder under the system temp folder.
pub fn temp_dir(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!("dockline-{label}-{}-{nanos}", std::process::id()))
}

/// Runs the generator with the given `TZ` and umask. Values are passed as argv, never spliced.
pub fn run(out: &Path, tz: &str, umask: &str) {
    let status = Command::new("sh")
        .arg("-c")
        .arg(r#"umask "$1" && exec "$2" --out "$3""#)
        .arg("sh")
        .arg(umask)
        .arg(env!("CARGO_BIN_EXE_dockline-gen"))
        .arg(out)
        .env("TZ", tz)
        .status()
        .expect("run dockline-gen");
    assert!(
        status.success(),
        "dockline-gen failed for TZ={tz} umask={umask}"
    );
}

fn sidecar(dir: &Path) -> Value {
    let text = std::fs::read_to_string(dir.join(SIDECAR)).expect("read sidecar");
    let mut v: Value = serde_json::from_str(&text).expect("parse sidecar");
    let obj = v.as_object_mut().expect("sidecar object");
    obj.remove("fixture_sha256");
    if let Some(Value::Object(h)) = obj.get_mut("expected_hashes") {
        h.remove(INDEX);
    }
    v
}

/// Every difference between two fixture folders, ignoring only the `.git/index` entry, the
/// Sidecar's `fixture_sha256`, and the Sidecar's hash of `.git/index` (Requirement 19.5).
pub fn differences(a: &Path, b: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let (ea, eb) = (entries(&a.join(TAR)), entries(&b.join(TAR)));
    for path in ea
        .keys()
        .chain(eb.keys())
        .collect::<std::collections::BTreeSet<_>>()
    {
        match (ea.get(path), eb.get(path)) {
            (Some(x), Some(y)) if path == INDEX => {
                if (x.kind, x.mode) != (y.kind, y.mode) {
                    out.push(format!("{path}: type or mode differs"));
                }
            }
            (Some(x), Some(y)) if x != y => out.push(format!("{path}: {x:?} vs {y:?}")),
            (Some(_), None) => out.push(format!("{path}: only in {}", a.display())),
            (None, Some(_)) => out.push(format!("{path}: only in {}", b.display())),
            _ => {}
        }
    }
    let (sa, sb) = (sidecar(a), sidecar(b));
    if sa != sb {
        let (oa, ob) = (
            sa.as_object().expect("object"),
            sb.as_object().expect("object"),
        );
        for key in oa
            .keys()
            .chain(ob.keys())
            .collect::<std::collections::BTreeSet<_>>()
        {
            if oa.get(key) != ob.get(key) {
                out.push(format!("sidecar field `{key}` differs"));
            }
        }
    }
    out
}
