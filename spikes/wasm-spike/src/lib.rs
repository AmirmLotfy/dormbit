//! Task 3 spike (b): prove the capsule core's libraries compile for
//! wasm32-unknown-unknown and produce byte-identical output natively and in
//! WebAssembly. Capsule golden tests (PROP-10) depend on this.
//!
//! The spike builds a tiny deterministic "capsule-like" tar in memory:
//! each file is gzipped separately and stored under objects/<sha256>, with
//! fixed tar headers. It reports the SHA-256 of the tar bytes plus results
//! from the gitignore matcher, NFC normalization, and uuid formatting.

use std::io::Write;

use flate2::{Compression, GzBuilder};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

const FILES: &[(&str, &[u8])] = &[
    ("README.md", b"# Dockline Invoices\n\nPaused mid-PDF-export.\n"),
    ("src/pdf/renderInvoice.ts", b"// TODO: finish PDF layout\nexport function render() {}\n"),
    ("package.json", b"{\n  \"name\": \"dockline-invoices\"\n}\n"),
];

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn sha256_hex(data: &[u8]) -> String {
    hex(&Sha256::digest(data))
}

fn gzip(data: &[u8]) -> Vec<u8> {
    let mut enc = GzBuilder::new().mtime(0).write(Vec::new(), Compression::new(6));
    enc.write_all(data).expect("gzip write");
    enc.finish().expect("gzip finish")
}

fn append(builder: &mut tar::Builder<Vec<u8>>, path: &str, data: &[u8]) {
    let mut header = tar::Header::new_ustar();
    header.set_path(path).expect("path");
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_mtime(1_735_689_600); // 2025-01-01T00:00:00Z, fixed
    header.set_uid(0);
    header.set_gid(0);
    header.set_entry_type(tar::EntryType::Regular);
    header.set_cksum();
    builder.append(&header, data).expect("append");
}

/// Deterministic capsule-like tar bytes.
pub fn build_tar() -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    append(&mut builder, "DORMBIT", b"dormbit-capsule/1\n");
    let mut manifest = String::from("{\"entries\":[");
    for (i, (path, data)) in FILES.iter().enumerate() {
        if i > 0 {
            manifest.push(',');
        }
        manifest.push_str(&format!("{{\"path\":\"{path}\",\"sha256\":\"{}\"}}", sha256_hex(data)));
    }
    manifest.push_str("]}");
    append(&mut builder, "manifest.json", manifest.as_bytes());
    for (_, data) in FILES {
        let object = gzip(data);
        append(&mut builder, &format!("objects/{}", sha256_hex(data)), &object);
    }
    append(&mut builder, "END", b"end\n");
    builder.into_inner().expect("finish tar")
}

/// One-line report used to compare the native and wasm builds.
pub fn report() -> String {
    let tar_bytes = build_tar();

    let mut gi = ignore::gitignore::GitignoreBuilder::new("/");
    gi.add_line(None, "node_modules/").expect("rule");
    gi.add_line(None, "*.log").expect("rule");
    gi.add_line(None, "!keep.log").expect("rule");
    let gi = gi.build().expect("gitignore");
    let ignored = |p: &str, is_dir: bool| gi.matched_path_or_any_parents(p, is_dir).is_ignore();

    // "é" as e + combining acute (NFD) must normalize to the single NFC code point.
    let nfc: String = "Cafe\u{0301}".nfc().collect();

    let id = uuid::Uuid::from_bytes([
        0x6f, 0x1d, 0x2a, 0x33, 0x80, 0x11, 0x4c, 0x7e, 0x9a, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd,
    ]);

    format!(
        "tar_len={} tar_sha256={} ignore[node_modules/a.js]={} ignore[debug.log]={} ignore[keep.log]={} ignore[src/a.ts]={} nfc_eq={} nfc_len={} uuid={}",
        tar_bytes.len(),
        sha256_hex(&tar_bytes),
        ignored("node_modules/a.js", false),
        ignored("debug.log", false),
        ignored("keep.log", false),
        ignored("src/a.ts", false),
        nfc == "Caf\u{e9}",
        nfc.len(),
        id
    )
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub fn spike_report() -> String {
        super::report()
    }

    #[wasm_bindgen]
    pub fn spike_tar() -> Vec<u8> {
        super::build_tar()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn print_native_report() {
        let r = super::report();
        println!("NATIVE {r}");
        assert!(r.contains("ignore[node_modules/a.js]=true"));
        assert!(r.contains("ignore[debug.log]=true"));
        assert!(r.contains("ignore[keep.log]=false"));
        assert!(r.contains("ignore[src/a.ts]=false"));
        assert!(r.contains("nfc_eq=true"));
    }
}
