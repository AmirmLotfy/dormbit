//! The judge-demo journey through the engine (natively; the browser runs the same code as
//! WebAssembly): discover → evidence → plan → approve → capture → verify → refused restore →
//! restore → independent re-hash → brief, plus corruption controls and Reset.

use dormbit_core::util::sha256_hex;
use dormbit_wasm::engine::Engine;
use serde_json::{Value, json};

const TAR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/dockline/dockline-invoices.tar"
));
const SIDECAR: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/dockline/dockline-invoices.sidecar.json"
));

fn engine() -> Engine {
    Engine::new(TAR, SIDECAR).unwrap()
}

fn call(e: &mut Engine, cmd: &str, args: Value) -> Value {
    e.call(cmd, &args)
        .unwrap_or_else(|err| panic!("{cmd}: {err:?}"))
}

/// Steps 1-5: discover, evidence, plan, decide, approve. Returns the approved digest.
fn through_approval(e: &mut Engine) -> String {
    let d = call(e, "discover", json!({}));
    let projects = d["result"]["projects"].as_array().unwrap();
    assert_eq!(projects.len(), 1, "notes/ isn't a project");
    assert_eq!(projects[0]["name"], "dockline-invoices");

    let ev = call(e, "evidence", json!({}));
    let text = ev.to_string();
    assert!(text.contains("feature/pdf-export"));
    assert!(text.contains("tasks complete"));

    let plan = call(e, "plan_create", json!({ "git_treatment": "full" }));
    let undecided: Vec<String> = plan["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["decision"] == "undecided")
        .map(|i| i["path"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(undecided, vec!["data/dev.sqlite".to_owned()]);
    // Approve is refused while an item is undecided.
    let refused = e.call(
        "plan_approve",
        &json!({ "digest": plan["digest"], "acks": [] }),
    );
    assert_eq!(
        refused.err().map(|x| x.code),
        Some(dormbit_core::model::ErrorCode::PlanIncomplete)
    );
    let plan = call(
        e,
        "plan_set_decision",
        json!({ "path": "data/dev.sqlite", "decision": "exclude" }),
    );
    let ok = call(
        e,
        "plan_approve",
        json!({ "digest": plan["digest"], "acks": [] }),
    );
    ok["digest"].as_str().unwrap().to_owned()
}

#[test]
fn the_full_journey_runs_on_the_real_core() {
    let mut e = engine();
    let digest = through_approval(&mut e);

    let cap = call(&mut e, "capture", json!({}));
    let phases: Vec<&str> = cap["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["phase"].as_str().unwrap())
        .collect();
    for want in [
        "hashing",
        "rechecking",
        "assembling",
        "committing",
        "recording",
    ] {
        assert!(phases.contains(&want), "{want} in {phases:?}");
    }
    assert_eq!(cap["manifest"]["plan_digest"], digest.as_str());
    let bytes = e.capsule_bytes().unwrap();
    assert_eq!(cap["record"]["file_sha256"], sha256_hex(&bytes).as_str());

    let v = call(&mut e, "verify", json!({}));
    assert_eq!(v["status"], "verified");
    assert_eq!(v["registry_match"], "Match");

    // Step 8: the original name is taken in ~/Restores, so the restore is refused.
    let refused = call(
        &mut e,
        "restore",
        json!({ "target_name": "Dockline-Invoices" }),
    );
    assert_eq!(refused["refused"], true);
    assert_eq!(refused["code"], "RESTORE_DEST_EXISTS");

    let r = call(
        &mut e,
        "restore",
        json!({ "target_name": "dockline-restored" }),
    );
    assert_eq!(r["refused"], false);
    assert_eq!(r["report"]["status"], "verified", "{}", r["report"]);
    assert_eq!(r["report"]["git_check"]["refs"]["kind"], "match");

    // Step 9: an independent re-hash of every restored file matches the manifest.
    let files = call(&mut e, "restored_files", json!({}));
    let list = files["files"].as_array().unwrap();
    assert!(list.len() > 100);
    for f in list {
        let b = e.restored_file_bytes(f["path"].as_str().unwrap()).unwrap();
        assert_eq!(sha256_hex(&b), f["expected_sha256"].as_str().unwrap());
    }

    // Step 10: the brief with the four outcomes.
    let b = call(&mut e, "brief", json!({}));
    let md = b["markdown"].as_str().unwrap();
    assert!(md.contains("- Files restored and verified. "));
    assert!(md.contains("- AI session resumed: not performed."));
    assert!(md.contains("- Builds/runs: unknown (never executed). [F-"));
    dormbit_core::brief::check_citations(
        md,
        &serde_json::from_value::<Vec<dormbit_core::evidence::Finding>>(
            b["brief"]["evidence_index"].clone(),
        )
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn every_corruption_control_is_caught() {
    // Damage a stored file → verification fails and names it.
    let mut e = engine();
    through_approval(&mut e);
    call(&mut e, "capture", json!({}));
    let c = call(
        &mut e,
        "corrupt",
        json!({ "control": "stored_file", "seed": 7, "xor": 0x40 }),
    );
    assert!(
        c["region"]
            .as_str()
            .unwrap()
            .starts_with("object:src/pdf/renderInvoice.ts")
    );
    let v = call(&mut e, "verify", json!({}));
    assert_eq!(v["status"], "failed");
    assert!(
        v["failed_files"]
            .to_string()
            .contains("src/pdf/renderInvoice.ts")
    );

    // Damage capsule bytes (outside object data) at many offsets → never verified.
    for seed in (0..4000).step_by(97) {
        let mut e = engine();
        through_approval(&mut e);
        call(&mut e, "capture", json!({}));
        call(
            &mut e,
            "corrupt",
            json!({ "control": "capsule_bytes", "seed": seed, "xor": 1 }),
        );
        let v = call(&mut e, "verify", json!({}));
        assert_ne!(v["status"], "verified", "seed {seed}");
    }

    // Damage after verification → the restore rolls back and the folder never appears.
    let mut e = engine();
    through_approval(&mut e);
    call(&mut e, "capture", json!({}));
    assert_eq!(call(&mut e, "verify", json!({}))["status"], "verified");
    call(
        &mut e,
        "corrupt",
        json!({ "control": "after_verification", "seed": 3, "xor": 0x10 }),
    );
    let r = call(
        &mut e,
        "restore",
        json!({ "target_name": "dockline-restored" }),
    );
    assert_eq!(r["report"]["status"], "rolled_back", "{}", r["report"]);
    assert_eq!(r["report"]["error"]["code"], "CAPSULE_HASH_MISMATCH");
    let dest = call(&mut e, "destination", json!({}));
    let names: Vec<&str> = dest["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["dockline-invoices"], "no target, no staging");
}

#[test]
fn reset_returns_to_the_pristine_state_and_is_idempotent() {
    let mut e = engine();
    let pristine = e.state_digest();
    through_approval(&mut e);
    call(&mut e, "capture", json!({}));
    call(&mut e, "verify", json!({}));
    call(&mut e, "restore", json!({ "target_name": "x" }));
    assert_ne!(e.state_digest(), pristine);
    e.reset();
    assert_eq!(e.state_digest(), pristine);
    e.reset();
    assert_eq!(e.state_digest(), pristine);
    // And the journey works again after a reset, with the same capsule bytes.
    through_approval(&mut e);
    call(&mut e, "capture", json!({}));
    let a = e.capsule_bytes().unwrap();
    let mut f = engine();
    through_approval(&mut f);
    call(&mut f, "capture", json!({}));
    assert_eq!(a, f.capsule_bytes().unwrap(), "deterministic");
}

#[test]
fn a_damaged_dataset_is_refused() {
    let mut bad = TAR.to_vec();
    bad[700] ^= 1;
    assert!(Engine::new(&bad, SIDECAR).is_err());
}

/// A tar with the given (name, bytes) files, and a Sidecar whose hashes match unless told.
fn dataset(files: &[(&str, &[u8])], hash_override: Option<&str>) -> (Vec<u8>, String) {
    let mut b = tar::Builder::new(Vec::new());
    let mut hashes = serde_json::Map::new();
    for (name, data) in files {
        let mut h = tar::Header::new_ustar();
        h.set_size(data.len() as u64);
        h.set_mode(0o644);
        h.set_entry_type(tar::EntryType::Regular);
        // Bypass the crate's path checks so hostile names reach the loader.
        h.as_old_mut().name[..name.len()].copy_from_slice(name.as_bytes());
        h.set_cksum();
        b.append(&h, *data).unwrap();
        hashes.insert((*name).to_owned(), Value::String(sha256_hex(data)));
    }
    let tar = b.into_inner().unwrap();
    let mut side: Value = serde_json::from_str(SIDECAR).unwrap();
    side["expected_hashes"] = Value::Object(hashes);
    side["fixture_sha256"] =
        Value::String(hash_override.map_or_else(|| sha256_hex(&tar), str::to_owned));
    (tar, side.to_string())
}

#[test]
fn the_loader_refuses_missing_hashes_duplicates_and_unsafe_paths() {
    let ok: &[(&str, &[u8])] = &[("dockline-invoices/package.json", b"{}")];
    let (tar, side) = dataset(ok, None);
    assert!(Engine::new(&tar, &side).is_ok());

    let (tar, side) = dataset(ok, Some(""));
    assert!(Engine::new(&tar, &side).is_err(), "missing dataset hash");

    let dup: &[(&str, &[u8])] = &[
        ("dockline-invoices/a.txt", b"1"),
        ("dockline-invoices/a.txt", b"1"),
    ];
    let (tar, side) = dataset(dup, None);
    assert!(Engine::new(&tar, &side).is_err(), "duplicate entry");

    let evil: &[(&str, &[u8])] = &[("dockline-invoices/../escape.txt", b"x")];
    let (tar, side) = dataset(evil, None);
    assert!(Engine::new(&tar, &side).is_err(), "unsafe path");
}

#[test]
fn damage_after_verification_stays_on_the_virtual_disk_and_every_file_is_listed() {
    let mut e = engine();
    through_approval(&mut e);
    let cap = call(&mut e, "capture", json!({}));
    let manifest_files = cap["manifest"]["totals"]["files"].as_u64().unwrap();
    call(&mut e, "verify", json!({}));
    call(
        &mut e,
        "corrupt",
        json!({ "control": "after_verification", "seed": 3, "xor": 0x10 }),
    );
    let r = call(
        &mut e,
        "restore",
        json!({ "target_name": "dockline-restored" }),
    );
    assert_eq!(r["report"]["status"], "rolled_back");
    assert_eq!(
        call(&mut e, "verify", json!({}))["status"],
        "failed",
        "the damage persists"
    );

    let mut e = engine();
    through_approval(&mut e);
    call(&mut e, "capture", json!({}));
    call(&mut e, "verify", json!({}));
    call(
        &mut e,
        "restore",
        json!({ "target_name": "dockline-restored" }),
    );
    let files = call(&mut e, "restored_files", json!({}));
    let list = files["files"].as_array().unwrap();
    assert_eq!(
        list.len() as u64,
        manifest_files,
        "every manifest file is listed"
    );
    assert!(list.iter().all(|f| f["present"] == true));
}
