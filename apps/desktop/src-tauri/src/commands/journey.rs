//! The desktop journey's commands. Each runs its work on a blocking thread and returns the
//! core's own result as JSON (see `crate::native`). Folder and save panels are opened by Rust
//! with `rfd`; the WebView has no dialog, fs, or shell permission.

use std::path::PathBuf;
use std::sync::Arc;

use dormbit_core::model::{Decision, DormbitError, ErrorCode};
use serde_json::{Value, json};
use tauri::{AppHandle, State};

use crate::native::Native;

type Shared<'a> = State<'a, Arc<Native>>;

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, DormbitError> + Send + 'static,
) -> Result<T, DormbitError> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|_| DormbitError::new(ErrorCode::IoFailed, "The task stopped unexpectedly."))?
}

/// Recording builds (`--features test-picker`) take the folder from an environment variable
/// instead of showing the panel. Release builds never compile this in.
#[cfg(feature = "test-picker")]
fn stub(var: &str) -> Option<Option<PathBuf>> {
    std::env::var_os(var).map(|v| Some(PathBuf::from(v)))
}
#[cfg(not(feature = "test-picker"))]
fn stub(_var: &str) -> Option<Option<PathBuf>> {
    None
}

/// Shows the native folder panel on the main thread and waits for the user.
fn pick_folder(app: &AppHandle, title: &'static str, var: &str) -> Option<PathBuf> {
    if let Some(p) = stub(var) {
        return p;
    }
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(rfd::FileDialog::new().set_title(title).pick_folder());
    })
    .ok()?;
    rx.recv().ok().flatten()
}

fn save_file(app: &AppHandle, var: &str) -> Option<PathBuf> {
    if let Some(p) = stub(var) {
        return p.map(|d| d.join("RESURRECTION-BRIEF.md"));
    }
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(
            rfd::FileDialog::new()
                .set_title("Save the Resurrection Brief")
                .set_file_name("RESURRECTION-BRIEF.md")
                .save_file(),
        );
    })
    .ok()?;
    rx.recv().ok().flatten()
}

#[tauri::command]
pub async fn scope_pick(
    app: AppHandle,
    state: Shared<'_>,
    kind: String,
) -> Result<Option<Value>, DormbitError> {
    if kind != "Read" {
        return Err(DormbitError::new(
            ErrorCode::ScopeDenied,
            "Only a folder to read from can be granted here.",
        ));
    }
    let n = Arc::clone(&state);
    blocking(move || {
        match pick_folder(
            &app,
            "Choose a folder for DORMBIT to look in",
            "DORMBIT_TEST_PICK_READ",
        ) {
            Some(p) => n.grant_read(&p).map(Some),
            None => Ok(None),
        }
    })
    .await
}

#[tauri::command]
pub async fn discovery_start(state: Shared<'_>, scope_id: String) -> Result<Value, DormbitError> {
    let n = Arc::clone(&state);
    blocking(move || n.discover(&scope_id)).await
}

#[tauri::command]
pub async fn evidence_get(state: Shared<'_>, project_id: String) -> Result<Value, DormbitError> {
    let n = Arc::clone(&state);
    blocking(move || n.evidence(&project_id)).await
}

#[tauri::command]
pub async fn plan_create(state: Shared<'_>, project_id: String) -> Result<Value, DormbitError> {
    let n = Arc::clone(&state);
    blocking(move || n.plan_create(&project_id)).await
}

#[tauri::command]
pub async fn plan_set_decision(
    state: Shared<'_>,
    plan_id: String,
    item_path: String,
    decision: Decision,
) -> Result<Value, DormbitError> {
    let n = Arc::clone(&state);
    blocking(move || n.plan_set_decision(&plan_id, &item_path, decision)).await
}

#[tauri::command]
pub async fn plan_approve(
    state: Shared<'_>,
    plan_id: String,
    digest: String,
    acks: Vec<String>,
) -> Result<Value, DormbitError> {
    let n = Arc::clone(&state);
    blocking(move || n.plan_approve(&plan_id, &digest, &acks)).await
}

#[tauri::command]
pub async fn capture_start(state: Shared<'_>, plan_id: String) -> Result<Value, DormbitError> {
    let n = Arc::clone(&state);
    blocking(move || n.capture(&plan_id)).await
}

#[tauri::command]
pub async fn verify_start(state: Shared<'_>, capsule_id: String) -> Result<Value, DormbitError> {
    let n = Arc::clone(&state);
    blocking(move || n.verify(&capsule_id)).await
}

#[tauri::command]
pub async fn restore_target_pick(
    app: AppHandle,
    state: Shared<'_>,
) -> Result<Option<Value>, DormbitError> {
    let n = Arc::clone(&state);
    blocking(move || {
        match pick_folder(
            &app,
            "Choose where DORMBIT creates the restored folder",
            "DORMBIT_TEST_PICK_RESTORE",
        ) {
            Some(p) => n.grant_target(&p).map(Some),
            None => Ok(None),
        }
    })
    .await
}

#[tauri::command]
pub async fn restore_precheck(state: Shared<'_>, dest_name: String) -> Result<Value, DormbitError> {
    let n = Arc::clone(&state);
    blocking(move || n.restore_precheck(&dest_name)).await
}

#[tauri::command]
pub async fn restore_start(state: Shared<'_>, dest_name: String) -> Result<Value, DormbitError> {
    let n = Arc::clone(&state);
    blocking(move || n.restore(&dest_name)).await
}

#[tauri::command]
pub async fn brief_get(state: Shared<'_>) -> Result<Value, DormbitError> {
    let n = Arc::clone(&state);
    blocking(move || n.brief().map(|(v, _)| v)).await
}

/// Writes the brief to a new file the user names in the save panel. An existing file is
/// never replaced.
#[tauri::command]
pub async fn brief_export(
    app: AppHandle,
    state: Shared<'_>,
) -> Result<Option<Value>, DormbitError> {
    let n = Arc::clone(&state);
    blocking(move || {
        let (_, md) = n.brief()?;
        let Some(path) = save_file(&app, "DORMBIT_TEST_SAVE_DIR") else {
            return Ok(None);
        };
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|_| {
                DormbitError::new(
                    ErrorCode::RestoreDestExists,
                    "A file with that name already exists, and DORMBIT never replaces files. Choose a new name.",
                )
            })?;
        f.write_all(md.as_bytes())
            .map_err(|_| DormbitError::new(ErrorCode::IoFailed, "The brief could not be written."))?;
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(Some(json!({ "file_name": name, "bytes": md.len() })))
    })
    .await
}
