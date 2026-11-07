//! Builds capsules byte by byte for malicious-input tests: start from a real capsule, change
//! one thing, and rebuild with a correct END trailer, so only the defect under test differs.
#![allow(dead_code)]

use dormbit_core::capsule::manifest::{Manifest, Trailer, to_canonical};
use dormbit_core::capsule::ustar::{self, BLOCK};
use dormbit_core::util::sha256_hex;

#[derive(Debug, Clone)]
pub struct RawEntry {
    pub name: String,
    pub data: Vec<u8>,
    pub mode: u32,
    /// Overrides the typeflag byte (with a recomputed checksum).
    pub typeflag: Option<u8>,
}

#[derive(Debug, Clone)]
pub struct Raw {
    pub marker: RawEntry,
    pub manifest: Manifest,
    /// Raw manifest bytes; when set, used instead of serializing `manifest`.
    pub manifest_bytes: Option<Vec<u8>>,
    pub objects: Vec<RawEntry>,
    pub mtime: u64,
    pub with_trailer: bool,
}

impl Raw {
    /// Splits a real capsule into its parts.
    pub fn from_capsule(bytes: &[u8]) -> Raw {
        let entries = super::kit::read_tar(bytes);
        let manifest: Manifest = serde_json::from_slice(&entries[1].data).unwrap();
        let mtime = entries[0].mtime;
        let objects = entries[2..entries.len() - 1]
            .iter()
            .map(|e| RawEntry {
                name: e.name.clone(),
                data: e.data.clone(),
                mode: e.mode,
                typeflag: None,
            })
            .collect();
        Raw {
            marker: RawEntry {
                name: "DORMBIT".into(),
                data: entries[0].data.clone(),
                mode: 0o644,
                typeflag: None,
            },
            manifest,
            manifest_bytes: None,
            objects,
            mtime,
            with_trailer: true,
        }
    }

    pub fn manifest_bytes(&self) -> Vec<u8> {
        self.manifest_bytes
            .clone()
            .unwrap_or_else(|| to_canonical(&self.manifest).unwrap())
    }

    fn put(out: &mut Vec<u8>, e: &RawEntry, mtime: u64) {
        let mut h = ustar::header(&e.name, e.data.len() as u64, e.mode, mtime).unwrap();
        if let Some(t) = e.typeflag {
            h[156] = t;
            h[148..156].copy_from_slice(b"        ");
            let sum: u64 = h.iter().map(|&b| u64::from(b)).sum();
            h[148..154].copy_from_slice(format!("{sum:06o}").as_bytes());
            h[154] = 0;
            h[155] = b' ';
        }
        out.extend_from_slice(&h);
        out.extend_from_slice(&e.data);
        out.extend(std::iter::repeat_n(
            0u8,
            ustar::padding(e.data.len() as u64),
        ));
    }

    pub fn build(&self) -> Vec<u8> {
        let mut out = Vec::new();
        Self::put(&mut out, &self.marker, self.mtime);
        let m = self.manifest_bytes();
        Self::put(
            &mut out,
            &RawEntry {
                name: "manifest.json".into(),
                data: m.clone(),
                mode: 0o644,
                typeflag: None,
            },
            self.mtime,
        );
        for o in &self.objects {
            Self::put(&mut out, o, self.mtime);
        }
        if self.with_trailer {
            let t = to_canonical(&Trailer {
                manifest_sha256: sha256_hex(&m),
                object_count: self.objects.len() as u64,
            })
            .unwrap();
            Self::put(
                &mut out,
                &RawEntry {
                    name: "END".into(),
                    data: t,
                    mode: 0o644,
                    typeflag: None,
                },
                self.mtime,
            );
        }
        out.extend_from_slice(&[0u8; BLOCK * 2]);
        out
    }
}
