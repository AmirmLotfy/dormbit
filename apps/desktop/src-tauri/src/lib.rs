//! DORMBIT desktop shell. Exposes only the allowlisted commands (see `build.rs` and
//! `capabilities/main.json`); the Allowlist_Guard test keeps all three in step.
mod commands;
mod native;
pub mod redact;

use std::sync::Arc;

use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data = app.path().app_data_dir()?;
            let home = app.path().home_dir()?;
            let native = native::Native::open(&data, &home).map_err(|e| e.message)?;
            app.manage(Arc::new(native));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info::app_info,
            commands::journey::scope_pick,
            commands::journey::discovery_start,
            commands::journey::evidence_get,
            commands::journey::plan_create,
            commands::journey::plan_set_decision,
            commands::journey::plan_approve,
            commands::journey::capture_start,
            commands::journey::verify_start,
            commands::journey::restore_target_pick,
            commands::journey::restore_precheck,
            commands::journey::restore_start,
            commands::journey::brief_get,
            commands::journey::brief_export,
        ])
        .run(tauri::generate_context!())
        .expect("error while running DORMBIT");
}
