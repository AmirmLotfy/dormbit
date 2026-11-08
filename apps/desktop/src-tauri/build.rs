fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "app_info",
            "scope_pick",
            "discovery_start",
            "evidence_get",
            "plan_create",
            "plan_set_decision",
            "plan_approve",
            "capture_start",
            "verify_start",
            "restore_target_pick",
            "restore_precheck",
            "restore_start",
            "brief_get",
            "brief_export",
        ]),
    ))
    .expect("tauri build"); // build scripts may panic; core lints don't apply here
}
