use dormbit_core::model::{AppInfo, AppInfoEvent, BuildProfile, DormbitError};
use tauri::ipc::Channel;

/// What this build reports about itself. Holds no filesystem path.
pub fn current() -> AppInfo {
    AppInfo {
        name: "DORMBIT".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        core_version: dormbit_core::VERSION.to_owned(),
        profile: if cfg!(debug_assertions) {
            BuildProfile::Debug
        } else {
            BuildProfile::Release
        },
        min_macos: "13.0".to_owned(),
    }
}

/// Streams one `info` then one `done` over the Channel, and returns the same info.
#[tauri::command]
pub async fn app_info(on_event: Channel<AppInfoEvent>) -> Result<AppInfo, DormbitError> {
    let info = current();
    on_event
        .send(AppInfoEvent::Info { info: info.clone() })
        .map_err(DormbitError::ipc)?;
    on_event
        .send(AppInfoEvent::Done)
        .map_err(DormbitError::ipc)?;
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_reports_the_plan_values() {
        let i = current();
        assert_eq!(i.name, "DORMBIT");
        assert_eq!(i.min_macos, "13.0");
        assert_eq!(i.core_version, dormbit_core::VERSION);
    }

    #[test]
    fn current_contains_no_path_separator() {
        let json = serde_json::to_string(&current()).unwrap_or_default();
        assert!(!json.contains('/'), "AppInfo must not carry a path: {json}");
    }
}
