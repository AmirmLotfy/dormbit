//! Property 4 (PROP-10): capsule bytes are deterministic. Shuffled directory listing order and
//! random budget sequences give identical bytes.

#[path = "support/capture_kit.rs"]
mod kit;

use dormbit_core::capsule::{Budget, Job, MemRegistry, MemWorkspace, Step};
use dormbit_core::model::GitTreatment;
use dormbit_core::vfs::{DirEntry, Limited, MemVfs, Metadata, ReadVfs, RelPath, VfsError};
use kit::*;
use proptest::prelude::*;

/// Lists directory entries in a seed-dependent order.
struct Shuffled<'a> {
    inner: &'a MemVfs,
    seed: u64,
}

fn mix(seed: u64, s: &str) -> u64 {
    s.bytes().fold(seed ^ 0x9e37_79b9_7f4a_7c15, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

impl ReadVfs for Shuffled<'_> {
    fn read_dir(&self, d: &RelPath) -> Result<Vec<DirEntry>, VfsError> {
        let mut e = self.inner.read_dir(d)?;
        e.sort_by_key(|x| mix(self.seed, &x.name));
        Ok(e)
    }
    fn symlink_metadata(&self, p: &RelPath) -> Result<Metadata, VfsError> {
        self.inner.symlink_metadata(p)
    }
    fn read_limited(&self, p: &RelPath, m: u64) -> Result<Limited, VfsError> {
        self.inner.read_limited(p, m)
    }
    fn read_prefix(&self, p: &RelPath, n: u64) -> Result<Vec<u8>, VfsError> {
        self.inner.read_prefix(p, n)
    }
    fn read_link(&self, p: &RelPath) -> Result<String, VfsError> {
        self.inner.read_link(p)
    }
}

fn tree() -> MemVfs {
    let mut v = MemVfs::new();
    for (i, p) in [
        "z.txt",
        "a.txt",
        "src/m.rs",
        "src/b.rs",
        "src/k/x.rs",
        "docs/r.md",
        "dup1",
        "dup2",
    ]
    .iter()
    .enumerate()
    {
        let body = if p.starts_with("dup") {
            "dup\n".to_owned()
        } else {
            format!("file {i}\n")
        };
        v.add_file_with(
            p,
            body,
            if i % 2 == 0 { 0o644 } else { 0o755 },
            1_700_000_000 + i as i64,
        );
    }
    v.add_symlink("ln", "a.txt")
        .add_file("node_modules/q/i.js", "x\n")
        .add_file("node_modules/q/.env", "S=1\n");
    v
}

proptest! {
    #[test]
    fn listing_order_and_budgets_do_not_change_the_bytes(
        seed in any::<u64>(),
        budgets in prop::collection::vec((1u64..4096, 1u32..5), 1..16),
    ) {
        let base = tree();
        let (plan, ok) = approved(&base, GitTreatment::Full, &["node_modules"]);
        let (_, reference) = capture(&base, plan.clone(), ok.clone());

        let shuffled = Shuffled { inner: &base, seed };
        let got = {
            let mut j = job(&shuffled, plan.clone(), ok.clone(), MemWorkspace::new(), MemRegistry::default());
            let mut k = 0usize;
            let name = loop {
                let (mb, mo) = budgets[k % budgets.len()];
                k += 1;
                match j.step(Budget { max_bytes: mb, max_ops: mo }) {
                    Ok(Step::Done(o)) => break o.record.file_name,
                    Ok(Step::Progress(_)) => {}
                    Err(e) => panic!("{e}"),
                }
            };
            let (ws, _) = j.into_parts();
            ws.library.get(&name).cloned().unwrap_or_default()
        };
        prop_assert_eq!(got, reference);
    }
}
