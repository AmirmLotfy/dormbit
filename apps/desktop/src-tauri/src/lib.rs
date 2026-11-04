//! DORMBIT desktop shell. Exposes only the allowlisted commands (see `build.rs` and
//! `capabilities/main.json`); the Allowlist_Guard test keeps all three in step.
mod commands;
pub mod redact;

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::app_info::app_info])
        .run(tauri::generate_context!())
        .expect("error while running DORMBIT");
}
