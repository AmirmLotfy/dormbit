//! The one ustar header form a capsule uses (design: Tar layout, header fields).
//!
//! Written by hand rather than with the `tar` crate: core can't depend on a crate that carries
//! filesystem code, and the verifier compares every header byte against this same table.
//! Tests cross-check the output with the `tar` crate as an independent reader.

use crate::model::{DormbitError, ErrorCode};

pub const BLOCK: usize = 512;
/// The largest size or mtime 11 octal digits hold.
pub const MAX_OCTAL_11: u64 = (1 << 33) - 1;

pub const MODE_FILE: u32 = 0o644;
pub const MODE_EXEC: u32 = 0o755;

fn octal(field: &mut [u8], value: u64) {
    // `field.len() - 1` zero-padded octal digits, then NUL.
    let digits = field.len() - 1;
    let s = format!("{value:0digits$o}");
    field[..digits].copy_from_slice(s.as_bytes());
    field[digits] = 0;
}

/// One 512-byte header. `name` must be ASCII and at most 100 bytes; `size` and `mtime` must fit
/// 11 octal digits. Every other field is fixed.
pub fn header(name: &str, size: u64, mode: u32, mtime: u64) -> Result<[u8; BLOCK], DormbitError> {
    if !name.is_ascii() || name.len() > 100 || name.is_empty() {
        return Err(DormbitError::new(
            ErrorCode::CapsuleMalformed,
            "A capsule entry name is not valid.",
        ));
    }
    if size > MAX_OCTAL_11 || mtime > MAX_OCTAL_11 {
        return Err(DormbitError::new(
            ErrorCode::CapsuleLimitExceeded,
            "A capsule entry is too large for the capsule format.",
        ));
    }
    let mut h = [0u8; BLOCK];
    h[..name.len()].copy_from_slice(name.as_bytes());
    octal(&mut h[100..108], u64::from(mode & 0o7777));
    octal(&mut h[108..116], 0);
    octal(&mut h[116..124], 0);
    octal(&mut h[124..136], size);
    octal(&mut h[136..148], mtime);
    h[156] = b'0';
    h[257..263].copy_from_slice(b"ustar\0");
    h[263..265].copy_from_slice(b"00");
    // Checksum: the sum of all header bytes with the checksum field read as eight spaces.
    h[148..156].copy_from_slice(b"        ");
    let sum: u64 = h.iter().map(|&b| u64::from(b)).sum();
    let s = format!("{sum:06o}");
    h[148..154].copy_from_slice(s.as_bytes());
    h[154] = 0;
    h[155] = b' ';
    Ok(h)
}

/// Zero bytes that pad `len` up to a whole block.
pub fn padding(len: u64) -> usize {
    let rem = (len % BLOCK as u64) as usize;
    if rem == 0 { 0 } else { BLOCK - rem }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_fields_match_the_format_table() {
        let h = header("DORMBIT", 18, MODE_FILE, 1_800_000_000).unwrap();
        assert_eq!(&h[0..8], b"DORMBIT\0");
        assert_eq!(&h[100..108], b"0000644\0");
        assert_eq!(&h[108..116], b"0000000\0");
        assert_eq!(&h[116..124], b"0000000\0");
        assert_eq!(&h[124..136], b"00000000022\0");
        assert_eq!(&h[136..148], b"15322351000\0");
        assert_eq!(h[154], 0);
        assert_eq!(h[155], b' ');
        assert_eq!(h[156], b'0');
        assert!(h[157..257].iter().all(|&b| b == 0));
        assert_eq!(&h[257..265], b"ustar\x0000");
        assert!(h[265..].iter().all(|&b| b == 0));
    }

    #[test]
    fn checksum_counts_the_field_as_spaces() {
        let h = header("manifest.json", 1234, MODE_EXEC, 0).unwrap();
        let mut copy = h;
        copy[148..156].copy_from_slice(b"        ");
        let sum: u64 = copy.iter().map(|&b| u64::from(b)).sum();
        let field = std::str::from_utf8(&h[148..154]).unwrap();
        assert_eq!(u64::from_str_radix(field, 8).unwrap(), sum);
    }

    #[test]
    fn refuses_long_or_non_ascii_names_and_oversized_values() {
        assert!(header(&"a".repeat(101), 0, MODE_FILE, 0).is_err());
        assert!(header("é", 0, MODE_FILE, 0).is_err());
        assert!(header("", 0, MODE_FILE, 0).is_err());
        assert!(header("x", MAX_OCTAL_11 + 1, MODE_FILE, 0).is_err());
    }

    #[test]
    fn padding_rounds_up_to_a_block() {
        assert_eq!(padding(0), 0);
        assert_eq!(padding(1), 511);
        assert_eq!(padding(512), 0);
        assert_eq!(padding(513), 511);
    }
}
