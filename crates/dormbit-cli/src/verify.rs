//! `dormbit-cli verify <file>`: prints the status, each check, damaged files, the registry
//! match, and the stated limitations. Dev harness only (no registry, so the registry check
//! is Unknown).

use std::fmt::Write as _;
use std::io::Read;

use dormbit_core::capsule::verify::{CheckState, FailReason, RegistryState};
use dormbit_core::capsule::{
    CapsuleLimits, CapsuleSource, NoRegistry, Report, ReportStatus, verify_all,
};
use dormbit_core::model::{DormbitError, ErrorCode};

struct FileSource(std::path::PathBuf);

impl CapsuleSource for FileSource {
    fn byte_len(&self) -> Result<u64, DormbitError> {
        std::fs::symlink_metadata(&self.0)
            .map(|m| m.len())
            .map_err(|_| DormbitError::new(ErrorCode::IoFailed, "cannot read the capsule"))
    }
    fn open(&self) -> Result<Box<dyn Read + Send>, DormbitError> {
        std::fs::File::open(&self.0)
            .map(|f| Box::new(f) as Box<dyn Read + Send>)
            .map_err(|_| DormbitError::new(ErrorCode::IoFailed, "cannot open the capsule"))
    }
}

pub fn render(r: &Report) -> String {
    let mut out = String::new();
    let status = match r.status {
        ReportStatus::Verified => "Verified",
        ReportStatus::Failed => "Failed",
        ReportStatus::Unsupported => "Unsupported",
    };
    let _ = writeln!(out, "status: {status}");
    if let Some(code) = r.error_code {
        let _ = writeln!(out, "error: {code:?}");
    }
    let _ = writeln!(out, "checks:");
    for c in &r.checks {
        let mark = match c.status {
            CheckState::Pass => "pass",
            CheckState::Fail => "FAIL",
            CheckState::Unknown => "unknown",
            CheckState::NotRun => "not run",
        };
        let _ = writeln!(
            out,
            "  [{mark:>7}] {} {}: {}",
            c.id.as_str(),
            c.id.label(),
            c.detail
        );
    }
    if !r.failed_files.is_empty() {
        let _ = writeln!(out, "damaged files:");
        for f in &r.failed_files {
            let why = match f.reason {
                FailReason::StoredHashMismatch => "stored bytes changed",
                FailReason::ContentHashMismatch => "contents changed",
                FailReason::SizeMismatch => "size differs",
                FailReason::Unverifiable => "could not be verified",
            };
            let _ = writeln!(out, "  {} ({why})", f.path);
        }
    }
    let reg = match r.registry_match {
        RegistryState::Match => "matches DORMBIT's record",
        RegistryState::Mismatch => "DOES NOT match DORMBIT's record",
        RegistryState::NotInRegistry => "not in DORMBIT's list (unknown)",
        RegistryState::NotChecked => "not checked",
    };
    let _ = writeln!(out, "registry: {reg}");
    if let Some(s) = &r.file_sha256 {
        let _ = writeln!(out, "sha256: {s}");
    }
    let _ = writeln!(out, "limitations:");
    for l in &r.limitations {
        let _ = writeln!(out, "  {l}");
    }
    out
}

pub fn run(args: &[String]) -> Result<String, String> {
    let path = args
        .first()
        .ok_or("usage: dormbit-cli verify <capsule file>")?;
    let r = verify_all(
        FileSource(path.into()),
        NoRegistry,
        CapsuleLimits::default(),
    )
    .map_err(|e| e.message)?;
    Ok(render(&r))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dockline_capsule(dir: &std::path::Path) -> std::path::PathBuf {
        let fixture = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/dockline/dockline-invoices.tar"
        );
        let args: Vec<String> = ["--tar", fixture, "--prefix", "dockline-invoices", "--out"]
            .iter()
            .map(|s| (*s).to_owned())
            .chain(std::iter::once(dir.to_string_lossy().into_owned()))
            .collect();
        let report = crate::capture::run(&args).unwrap();
        let name = report
            .lines()
            .next()
            .unwrap()
            .trim_start_matches("wrote ")
            .to_owned();
        dir.join(name)
    }

    #[test]
    fn verifies_the_dockline_capsule_and_names_a_damaged_file() {
        let dir = std::env::temp_dir().join(format!("dormbit-cli-verify-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let capsule = dockline_capsule(&dir);
        let out = run(&[capsule.to_string_lossy().into_owned()]).unwrap();
        assert!(out.starts_with("status: Verified"), "{out}");
        assert!(out.contains("SHA-256 shows integrity, not authorship."));

        // Corrupt one byte inside the first stored object (after marker and manifest).
        let mut bytes = std::fs::read(&capsule).unwrap();
        let manifest_size = {
            let field = std::str::from_utf8(&bytes[1024 + 124..1024 + 135]).unwrap();
            u64::from_str_radix(field, 8).unwrap() as usize
        };
        let first_obj_data = 1024 + 512 + manifest_size.div_ceil(512) * 512 + 512;
        bytes[first_obj_data + 20] ^= 0xff;
        let bad = dir.join("damaged.dormbit");
        std::fs::write(&bad, &bytes).unwrap();
        let out = run(&[bad.to_string_lossy().into_owned()]).unwrap();
        assert!(out.starts_with("status: Failed"), "{out}");
        assert!(out.contains("damaged files:"), "{out}");
        assert!(out.contains("(stored bytes changed)"), "{out}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
