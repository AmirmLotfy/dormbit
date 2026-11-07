//! Property 8: verification work is bounded on hostile input. A counting source shows the
//! verifier never reads more than the file (one pass), and it never panics on random or
//! mutated input.

#[path = "support/capture_kit.rs"]
mod kit;

use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use dormbit_core::capsule::{CapsuleLimits, CapsuleSource, NoRegistry, ReportStatus, verify_all};
use dormbit_core::model::{DormbitError, GitTreatment};
use dormbit_core::vfs::MemVfs;
use kit::*;
use proptest::prelude::*;

struct Counting {
    data: Arc<Vec<u8>>,
    read: Arc<AtomicU64>,
    opens: Arc<AtomicU64>,
}

struct CountingReader {
    data: Arc<Vec<u8>>,
    pos: usize,
    read: Arc<AtomicU64>,
}

impl Read for CountingReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let rest = self.data.get(self.pos..).unwrap_or(&[]);
        let n = rest.len().min(buf.len());
        buf[..n].copy_from_slice(&rest[..n]);
        self.pos += n;
        self.read.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}

impl CapsuleSource for Counting {
    fn byte_len(&self) -> Result<u64, DormbitError> {
        Ok(self.data.len() as u64)
    }
    fn open(&self) -> Result<Box<dyn Read + Send>, DormbitError> {
        self.opens.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(CountingReader {
            data: self.data.clone(),
            pos: 0,
            read: self.read.clone(),
        }))
    }
}

fn check(bytes: Vec<u8>) -> (ReportStatus, u64, u64, u64) {
    let len = bytes.len() as u64;
    let read = Arc::new(AtomicU64::new(0));
    let opens = Arc::new(AtomicU64::new(0));
    let src = Counting {
        data: Arc::new(bytes),
        read: read.clone(),
        opens: opens.clone(),
    };
    let r = verify_all(src, NoRegistry, CapsuleLimits::default()).unwrap();
    (
        r.status,
        read.load(Ordering::Relaxed),
        opens.load(Ordering::Relaxed),
        len,
    )
}

fn sample() -> Vec<u8> {
    let mut v = MemVfs::new();
    v.add_file("a.txt", "alpha\n")
        .add_file("b/c.rs", "fn c() {}\n");
    let (plan, ok) = approved(&v, GitTreatment::Full, &[]);
    capture(&v, plan, ok).1
}

proptest! {
    #[test]
    fn random_bytes_are_read_at_most_once(bytes in prop::collection::vec(any::<u8>(), 0..8192)) {
        let (status, read, opens, len) = check(bytes);
        prop_assert_ne!(status, ReportStatus::Verified);
        prop_assert!(read <= len);
        prop_assert!(opens <= 1);
    }

    #[test]
    fn mutated_capsules_are_read_at_most_once(
        edits in prop::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 1..8),
    ) {
        let mut b = sample();
        for (at, v) in edits {
            let i = at.index(b.len());
            b[i] = v;
        }
        let (_, read, opens, len) = check(b);
        prop_assert!(read <= len);
        prop_assert!(opens <= 1);
    }
}

#[test]
fn an_oversized_file_is_refused_before_any_read() {
    let limits = CapsuleLimits {
        max_capsule_size: 1024,
        ..CapsuleLimits::default()
    };
    let read = Arc::new(AtomicU64::new(0));
    let opens = Arc::new(AtomicU64::new(0));
    let src = Counting {
        data: Arc::new(sample()),
        read: read.clone(),
        opens: opens.clone(),
    };
    let r = verify_all(src, NoRegistry, limits).unwrap();
    assert_eq!(r.status, ReportStatus::Unsupported);
    assert_eq!(read.load(Ordering::Relaxed), 0);
    assert_eq!(opens.load(Ordering::Relaxed), 0);
}
