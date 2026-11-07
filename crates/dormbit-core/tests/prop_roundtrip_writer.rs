//! Property 1 (PROP-2, writer side): captured content equals the source.
//! Until the verifier lands (Task 3.6) the capsule is read back with the `tar` crate.

#[path = "support/capture_kit.rs"]
mod kit;

use std::collections::BTreeMap;

use dormbit_core::capsule::manifest::Entry;
use dormbit_core::capsule::verify::RegistryState;
use dormbit_core::capsule::{BytesSource, CapsuleLimits, MemRegistry, ReportStatus, verify_all};
use dormbit_core::model::GitTreatment;
use dormbit_core::util::sha256_hex;
use dormbit_core::vfs::MemVfs;
use kit::*;
use proptest::prelude::*;

const NAMES: [&str; 8] = ["a", "b.txt", "c.rs", "d.md", "e", "f.json", "g.sh", "h"];
const DIRS: [&str; 4] = ["", "src/", "src/deep/", "docs/"];

#[derive(Debug, Clone)]
enum Node {
    File { data: Vec<u8>, mode: u32 },
    Link { target: String },
}

fn node() -> impl Strategy<Value = Node> {
    prop_oneof![
        4 => (
            prop_oneof![
                Just(Vec::new()),
                Just(b"same content\n".to_vec()),
                prop::collection::vec(any::<u8>(), 0..2048),
            ],
            prop::sample::select(vec![0o644u32, 0o755, 0o600, 0o4755, 0o700, 0o444]),
        )
            .prop_map(|(data, mode)| Node::File { data, mode }),
        1 => "[a-z./]{1,12}".prop_map(|target| Node::Link { target }),
    ]
}

fn tree() -> impl Strategy<Value = BTreeMap<String, Node>> {
    prop::collection::btree_map(
        (0..DIRS.len(), 0..NAMES.len()).prop_map(|(d, n)| format!("{}{}", DIRS[d], NAMES[n])),
        node(),
        1..20,
    )
}

fn build(t: &BTreeMap<String, Node>) -> MemVfs {
    let mut v = MemVfs::new();
    for (path, n) in t {
        match n {
            Node::File { data, mode } => {
                v.add_file_with(path, data.clone(), *mode, 1_700_000_000);
            }
            Node::Link { target } => {
                v.add_symlink(path, target);
            }
        }
    }
    v
}

proptest! {
    #[test]
    fn decompressed_objects_modes_and_link_targets_equal_the_source(t in tree()) {
        // A file and a folder with the same name can't both exist; skip those trees.
        let v = build(&t);
        prop_assume!(t.keys().all(|k| !t.keys().any(|o| o.starts_with(&format!("{k}/")))));
        let (plan, ok) = approved(&v, GitTreatment::Full, &[]);
        let (out, bytes) = capture(&v, plan, ok);
        let entries = read_tar(&bytes);
        prop_assert_eq!(&out.record.file_sha256, &sha256_hex(&bytes));
        // Task 3.10: the verifier agrees, and the capsule matches its registry record.
        let mut reg = MemRegistry::default();
        reg.records.insert(out.record.capsule_id.as_str().to_owned(), out.record.clone());
        let report = verify_all(BytesSource::new(bytes.clone()), reg, CapsuleLimits::default())
            .unwrap_or_else(|e| panic!("{e}"));
        prop_assert_eq!(report.status, ReportStatus::Verified);
        prop_assert_eq!(report.registry_match, RegistryState::Match);
        for e in &out.manifest.entries {
            match (e, t.get(e.path())) {
                (Entry::File { sha256, mode, size, .. }, Some(Node::File { data, mode: m })) => {
                    let obj = entries.iter().find(|x| x.name == format!("objects/{sha256}"));
                    prop_assert!(obj.is_some(), "object for {}", e.path());
                    prop_assert_eq!(&gunzip(&obj.map(|o| o.data.clone()).unwrap_or_default()), data);
                    prop_assert_eq!(*mode, m & 0o777);
                    prop_assert_eq!(*size, data.len() as u64);
                }
                (Entry::Symlink { target, restored, .. }, Some(Node::Link { target: t2 })) => {
                    prop_assert_eq!(target, t2);
                    prop_assert!(!restored);
                }
                (e, other) => prop_assert!(false, "{} mismatched with {:?}", e.path(), other),
            }
        }
        // Every source file and link is either captured or listed as excluded.
        for path in t.keys() {
            let captured = out.manifest.entries.iter().any(|e| e.path() == path);
            let excluded = out.manifest.excluded.iter().any(|e| &e.path == path);
            prop_assert!(captured ^ excluded, "{path}");
        }
    }
}
