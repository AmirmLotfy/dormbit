// Task 3 spike (a2): a Tauri 2 window whose UI can only call two allowlisted
// commands. The folder picker is opened from Rust; the UI gets a grant ID and
// a display path, never the ability to open dialogs or touch the filesystem.

use serde::Serialize;
use tauri_plugin_dialog::DialogExt;

#[derive(Serialize)]
struct AppInfo {
    name: &'static str,
    version: &'static str,
    tauri: &'static str,
}

#[derive(Serialize)]
struct Grant {
    id: String,
    display: String,
}

#[tauri::command]
fn app_info() -> AppInfo {
    // Spike evidence: proves the WebView reached Rust through the allowlist.
    eprintln!("SPIKE app_info invoked from the UI");
    AppInfo { name: "DORMBIT spike", version: env!("CARGO_PKG_VERSION"), tauri: tauri::VERSION }
}

#[tauri::command]
async fn pick_folder(app: tauri::AppHandle) -> Result<Option<Grant>, String> {
    let Some(picked) = app.dialog().file().set_title("Choose a folder").blocking_pick_folder() else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    let display = match std::env::var_os("HOME") {
        Some(home) => match path.strip_prefix(&home) {
            Ok(rest) => format!("~/{}", rest.display()),
            Err(_) => path.display().to_string(),
        },
        None => path.display().to_string(),
    };
    Ok(Some(Grant { id: "grant-1".into(), display }))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![app_info, pick_folder])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
