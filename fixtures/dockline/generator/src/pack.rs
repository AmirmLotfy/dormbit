//! Deterministic archive writers (foundation Requirement 19.2): a ustar tar of the work folder
//! and a stored (uncompressed) zip with fixed DOS timestamps.

use std::fs;
use std::path::Path;

use tar::{EntryType, Header};

use crate::time::utc;
use crate::{Ctx, R};

/// Every tar entry's mtime: 2025-05-23T00:00:00Z.
pub fn tar_mtime() -> u64 {
    utc(2025, 5, 23, 0, 0, 0) as u64
}

/// Paths under `root`, relative and `/`-separated, directories ending in `/`, sorted by bytes.
pub fn walk(root: &Path) -> R<Vec<String>> {
    fn go(root: &Path, dir: &Path, out: &mut Vec<String>) -> R<()> {
        for entry in fs::read_dir(dir).ctx("read dir")? {
            let path = entry.ctx("dir entry")?.path();
            let rel = path
                .strip_prefix(root)
                .ctx("strip prefix")?
                .to_str()
                .ok_or("non-UTF-8 path")?
                .replace('\\', "/");
            let meta = fs::symlink_metadata(&path).ctx(&rel)?;
            if meta.is_dir() {
                out.push(format!("{rel}/"));
                go(root, &path, out)?;
            } else if meta.is_file() {
                out.push(rel);
            } else {
                return Err(format!(
                    "{rel}: only files and folders are allowed in the fixture"
                ));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    go(root, root, &mut out)?;
    out.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    Ok(out)
}

pub fn tar(root: &Path) -> R<Vec<u8>> {
    let mut b = tar::Builder::new(Vec::new());
    for rel in walk(root)? {
        let mut h = Header::new_ustar();
        h.set_path(&rel).ctx(&rel)?;
        h.set_uid(0);
        h.set_gid(0);
        h.set_username("").ctx("username")?;
        h.set_groupname("").ctx("groupname")?;
        h.set_mtime(tar_mtime());
        if rel.ends_with('/') {
            h.set_entry_type(EntryType::Directory);
            h.set_mode(0o755);
            h.set_size(0);
            h.set_cksum();
            b.append(&h, std::io::empty()).ctx(&rel)?;
        } else {
            let data = fs::read(root.join(&rel)).ctx(&rel)?;
            h.set_entry_type(EntryType::Regular);
            h.set_mode(0o644);
            h.set_size(data.len() as u64);
            h.set_cksum();
            b.append(&h, data.as_slice()).ctx(&rel)?;
        }
    }
    b.into_inner().ctx("finish tar")
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// A zip with stored entries. Every entry is dated 2025-05-20 15:40:00 (DOS time).
pub fn stored_zip(files: &[(&str, Vec<u8>)]) -> Vec<u8> {
    const DOS_TIME: u16 = (15 << 11) | (40 << 5);
    const DOS_DATE: u16 = ((2025 - 1980) << 9) | (5 << 5) | 20;
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let offset = out.len() as u32;
        let crc = crc32(data);
        let size = data.len() as u32;
        let name_len = name.len() as u16;
        // Local file header.
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        for v in [20u16, 0, 0, DOS_TIME, DOS_DATE] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for v in [crc, size, size] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&name_len.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);
        // Central directory record.
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        for v in [20u16, 20, 0, 0, DOS_TIME, DOS_DATE] {
            central.extend_from_slice(&v.to_le_bytes());
        }
        for v in [crc, size, size] {
            central.extend_from_slice(&v.to_le_bytes());
        }
        for v in [name_len, 0, 0, 0, 0] {
            central.extend_from_slice(&v.to_le_bytes());
        }
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let cd_offset = out.len() as u32;
    let cd_size = central.len() as u32;
    let count = files.len() as u16;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    for v in [0u16, 0, count, count] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::crc32;

    #[test]
    fn crc32_matches_the_standard_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }
}
