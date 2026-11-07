//! The fictional Dockline project as a `MemVfs`. The fixture tar is embedded at compile time,
//! so tests do no I/O (core's clippy denies `std::fs`).

use std::io::Read as _;

use dormbit_core::vfs::MemVfs;

const PREFIX: &str = "dockline-invoices/";

const FIXTURE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/dockline/dockline-invoices.tar"
));

pub fn dockline() -> MemVfs {
    let mut archive = tar::Archive::new(FIXTURE);
    let mut vfs = MemVfs::new();
    for entry in archive.entries().unwrap_or_else(|e| panic!("{e}")) {
        let mut entry = entry.unwrap_or_else(|e| panic!("{e}"));
        let name = entry
            .path()
            .unwrap_or_else(|e| panic!("{e}"))
            .to_string_lossy()
            .into_owned();
        let Some(rel) = name.strip_prefix(PREFIX) else {
            continue;
        };
        let rel = rel.trim_end_matches('/');
        if rel.is_empty() {
            continue;
        }
        let kind = entry.header().entry_type();
        if kind.is_dir() {
            vfs.add_dir(rel);
        } else if kind.is_symlink() {
            let target = entry
                .link_name()
                .ok()
                .flatten()
                .map(|t| t.to_string_lossy().into_owned())
                .unwrap_or_default();
            vfs.add_symlink(rel, &target);
        } else if kind.is_file() {
            let mode = entry.header().mode().unwrap_or(0o644);
            let mtime = i64::try_from(entry.header().mtime().unwrap_or(0)).unwrap_or(0);
            let mut data = Vec::new();
            entry
                .read_to_end(&mut data)
                .unwrap_or_else(|e| panic!("{e}"));
            vfs.add_file_with(rel, data, mode, mtime);
        }
    }
    vfs
}
