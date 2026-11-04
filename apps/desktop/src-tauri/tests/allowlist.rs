//! Allowlist_Guard: the desktop shell may expose only the commands in the master plan, with
//! exactly the expected capability permissions, and none of the powerful Tauri plugins.
//!
//! Later specs add their commands to `EXPECTED_PERMISSIONS`, `build.rs`, and
//! `generate_handler!` in the same commit.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Master plan §6, in order.
const PLAN_COMMANDS: [&str; 28] = [
    "app_info",
    "scope_pick",
    "scope_list",
    "scope_revoke",
    "discovery_start",
    "discovery_cancel",
    "projects_list",
    "project_get",
    "project_set_dormant",
    "evidence_get",
    "plan_create",
    "plan_set_decision",
    "plan_approve",
    "capture_start",
    "job_cancel",
    "capsule_pick",
    "capsules_list",
    "verify_start",
    "restore_target_pick",
    "restore_precheck",
    "restore_start",
    "restore_recover",
    "brief_get",
    "brief_export",
    "settings_get",
    "settings_set",
    "consent_set",
    "adapters_list",
];

/// Grows with each spec's commands. No `core:*` entry is allowed unless added here by name.
const EXPECTED_PERMISSIONS: &[&str] = &["allow-app-info"];

const BANNED_PREFIXES: &[&str] = &[
    "fs:",
    "shell:",
    "dialog:",
    "http:",
    "opener:",
    "process:",
    "core:path:",
];

const BANNED_PLUGINS: &[&str] = &[
    "tauri-plugin-fs",
    "tauri-plugin-shell",
    "tauri-plugin-dialog",
    "tauri-plugin-http",
    "tauri-plugin-opener",
    "tauri-plugin-process",
    "@tauri-apps/plugin-",
];

/// One parsed capability file: its identifier and permission strings.
struct Capability {
    name: String,
    permissions: Vec<String>,
}

/// Returns a description of every rule broken. Empty means the allowlist holds.
fn check(
    caps: &[Capability],
    handlers: &[String],
    manifest: &[String],
    cargo_toml: &str,
    package_json: &str,
    tauri_features: &[String],
) -> Vec<String> {
    let mut v = Vec::new();
    let plan: BTreeSet<&str> = PLAN_COMMANDS.iter().copied().collect();

    for h in handlers {
        if !plan.contains(h.as_str()) {
            v.push(format!("handler `{h}` is not in the plan command list"));
        }
    }
    for m in manifest {
        if !plan.contains(m.as_str()) {
            v.push(format!(
                "build.rs command `{m}` is not in the plan command list"
            ));
        }
    }
    let h: BTreeSet<&str> = handlers.iter().map(String::as_str).collect();
    let m: BTreeSet<&str> = manifest.iter().map(String::as_str).collect();
    if h != m {
        v.push(format!("handlers {h:?} and build.rs manifest {m:?} differ"));
    }

    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for c in caps {
        for p in &c.permissions {
            seen.insert(p.as_str());
            if let Some(b) = BANNED_PREFIXES.iter().find(|b| p.starts_with(**b)) {
                v.push(format!(
                    "capability `{}` grants banned `{p}` (prefix `{b}`)",
                    c.name
                ));
            } else if !EXPECTED_PERMISSIONS.contains(&p.as_str()) {
                v.push(format!("capability `{}` grants unexpected `{p}`", c.name));
            }
        }
    }
    for e in EXPECTED_PERMISSIONS {
        if !seen.contains(e) {
            v.push(format!("expected permission `{e}` is not granted"));
        }
    }

    for b in BANNED_PLUGINS {
        if cargo_toml.contains(b) {
            v.push(format!("Cargo.toml mentions banned plugin `{b}`"));
        }
        if package_json.contains(b) {
            v.push(format!("package.json mentions banned plugin `{b}`"));
        }
    }
    if tauri_features.iter().any(|f| f == "devtools") {
        v.push("tauri feature `devtools` is enabled".to_owned());
    }
    v
}

// ---- readers for the real repo -------------------------------------------------------------

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(p: &Path) -> String {
    fs::read_to_string(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

fn read_caps() -> Vec<Capability> {
    let dir = root().join("capabilities");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    files
        .iter()
        .map(|p| {
            let j: serde_json::Value = serde_json::from_str(&read(p))
                .unwrap_or_else(|e| panic!("parse {}: {e}", p.display()));
            let perms = j["permissions"]
                .as_array()
                .unwrap_or_else(|| panic!("{} has no permissions array", p.display()))
                .iter()
                .map(|x| {
                    x.as_str()
                        .unwrap_or_else(|| panic!("{} has a non-string permission", p.display()))
                        .to_owned()
                })
                .collect();
            Capability {
                name: p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                permissions: perms,
            }
        })
        .collect()
}

/// Identifiers (last path segment) inside `needle ... open ... close` in `src`.
fn idents_in(src: &str, needle: &str, open: char, close: char) -> Vec<String> {
    let Some(start) = src.find(needle) else {
        return Vec::new();
    };
    let rest = &src[start + needle.len()..];
    let Some(o) = rest.find(open) else {
        return Vec::new();
    };
    let Some(c) = rest[o..].find(close) else {
        return Vec::new();
    };
    rest[o + 1..o + c]
        .split(',')
        .map(|s| s.trim().trim_matches('"'))
        .filter(|s| !s.is_empty())
        .map(|s| s.rsplit("::").next().unwrap_or(s).to_owned())
        .collect()
}

/// Features switched on for `tauri` by this package's Cargo.toml (inline table form).
fn tauri_features(cargo_toml: &str) -> Vec<String> {
    cargo_toml
        .lines()
        .filter(|l| l.trim_start().starts_with("tauri ") || l.trim_start().starts_with("tauri="))
        .flat_map(|l| idents_in(l, "features", '[', ']'))
        .collect()
}

#[test]
fn the_real_shell_obeys_the_allowlist() {
    let lib = read(&root().join("src/lib.rs"));
    let build = read(&root().join("build.rs"));
    let cargo = read(&root().join("Cargo.toml"));
    let pkg = read(&root().join("../package.json"));

    let handlers = idents_in(&lib, "generate_handler!", '[', ']');
    let manifest = idents_in(&build, ".commands(", '[', ']');
    assert!(
        !handlers.is_empty(),
        "found no handlers; the reader is broken"
    );
    assert!(
        !manifest.is_empty(),
        "found no manifest commands; the reader is broken"
    );

    let violations = check(
        &read_caps(),
        &handlers,
        &manifest,
        &cargo,
        &pkg,
        &tauri_features(&cargo),
    );
    assert!(
        violations.is_empty(),
        "allowlist violations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn the_plan_list_has_28_distinct_commands() {
    let set: BTreeSet<&str> = PLAN_COMMANDS.iter().copied().collect();
    assert_eq!(set.len(), 28);
}

#[test]
fn the_csp_matches_the_design() {
    let conf: serde_json::Value = serde_json::from_str(&read(&root().join("tauri.conf.json")))
        .unwrap_or_else(|e| panic!("parse tauri.conf.json: {e}"));
    let csp = conf["app"]["security"]["csp"].as_str().unwrap_or_default();
    for must in [
        "default-src 'self'",
        "script-src 'self'",
        "connect-src ipc: http://ipc.localhost",
        "object-src 'none'",
        "frame-src 'none'",
    ] {
        assert!(csp.contains(must), "CSP lacks `{must}`: {csp}");
    }
    assert!(!csp.contains("unsafe-"), "CSP must not use unsafe-*: {csp}");
    assert_eq!(conf["app"]["security"]["freezePrototype"], true);
    assert_eq!(conf["app"]["withGlobalTauri"], false);
    assert_eq!(conf["bundle"]["macOS"]["signingIdentity"], "-");
}

// ---- self-tests: the guard must fail on bad inputs ----------------------------------------

fn good() -> (Vec<Capability>, Vec<String>, Vec<String>) {
    (
        vec![Capability {
            name: "main.json".into(),
            permissions: vec!["allow-app-info".into()],
        }],
        vec!["app_info".into()],
        vec!["app_info".into()],
    )
}

fn run(
    caps: &[Capability],
    h: &[String],
    m: &[String],
    cargo: &str,
    pkg: &str,
    feats: &[String],
) -> Vec<String> {
    check(caps, h, m, cargo, pkg, feats)
}

#[test]
fn self_test_accepts_the_good_input() {
    let (c, h, m) = good();
    assert!(run(&c, &h, &m, "", "", &[]).is_empty());
}

#[test]
fn self_test_rejects_fs_default() {
    let (mut c, h, m) = good();
    c[0].permissions.push("fs:default".into());
    let v = run(&c, &h, &m, "", "", &[]);
    assert!(v.iter().any(|x| x.contains("fs:default")), "{v:?}");
}

#[test]
fn self_test_rejects_core_path_and_unlisted_permissions() {
    let (mut c, h, m) = good();
    c[0].permissions.push("core:path:default".into());
    c[0].permissions.push("core:default".into());
    let v = run(&c, &h, &m, "", "", &[]);
    assert!(v.iter().any(|x| x.contains("core:path:default")), "{v:?}");
    assert!(
        v.iter().any(|x| x.contains("unexpected `core:default`")),
        "{v:?}"
    );
}

#[test]
fn self_test_rejects_a_missing_expected_permission() {
    let (mut c, h, m) = good();
    c[0].permissions.clear();
    let v = run(&c, &h, &m, "", "", &[]);
    assert!(v.iter().any(|x| x.contains("not granted")), "{v:?}");
}

#[test]
fn self_test_rejects_an_extra_handler() {
    let (c, mut h, m) = good();
    h.push("scope_pick".into());
    let v = run(&c, &h, &m, "", "", &[]);
    assert!(v.iter().any(|x| x.contains("differ")), "{v:?}");
}

#[test]
fn self_test_rejects_a_handler_outside_the_plan() {
    let (c, mut h, mut m) = good();
    h.push("read_any_file".into());
    m.push("read_any_file".into());
    let v = run(&c, &h, &m, "", "", &[]);
    assert!(v.iter().any(|x| x.contains("read_any_file")), "{v:?}");
}

#[test]
fn self_test_rejects_an_unlisted_manifest_command() {
    let (c, h, mut m) = good();
    m.push("sneaky".into());
    let v = run(&c, &h, &m, "", "", &[]);
    assert!(v.iter().any(|x| x.contains("sneaky")), "{v:?}");
}

#[test]
fn self_test_rejects_banned_plugins() {
    let (c, h, m) = good();
    let v = run(&c, &h, &m, "tauri-plugin-shell = \"2\"", "", &[]);
    assert!(v.iter().any(|x| x.contains("tauri-plugin-shell")), "{v:?}");
    let v = run(&c, &h, &m, "", "\"@tauri-apps/plugin-fs\": \"2\"", &[]);
    assert!(v.iter().any(|x| x.contains("@tauri-apps/plugin-")), "{v:?}");
}

#[test]
fn self_test_rejects_the_devtools_feature() {
    let (c, h, m) = good();
    let v = run(&c, &h, &m, "", "", &["devtools".to_owned()]);
    assert!(v.iter().any(|x| x.contains("devtools")), "{v:?}");
}

#[test]
fn self_test_feature_reader_sees_devtools() {
    let f = tauri_features("tauri = { workspace = true, features = [\"devtools\"] }");
    assert_eq!(f, vec!["devtools".to_owned()]);
}
