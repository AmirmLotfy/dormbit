//! `app_info`: the first command, and the pattern later job commands copy.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum BuildProfile {
    Debug,
    Release,
}

/// What the app tells the UI about itself. Contains no filesystem path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AppInfo {
    /// Always `"DORMBIT"`.
    pub name: String,
    /// The `apps/desktop/src-tauri` package version.
    pub version: String,
    /// The `dormbit-core` package version.
    pub core_version: String,
    pub profile: BuildProfile,
    /// The minimum macOS version, `"13.0"`.
    pub min_macos: String,
}

/// Streamed over a Channel: exactly one `info`, then one `done`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum AppInfoEvent {
    Info { info: AppInfo },
    Done,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> AppInfo {
        AppInfo {
            name: "DORMBIT".into(),
            version: "0.1.0".into(),
            core_version: "0.1.0".into(),
            profile: BuildProfile::Debug,
            min_macos: "13.0".into(),
        }
    }

    #[test]
    fn info_event_is_tagged_with_kind() {
        let v = serde_json::to_value(AppInfoEvent::Info { info: sample() }).unwrap();
        assert_eq!(v["kind"], "info");
        assert_eq!(v["info"]["name"], "DORMBIT");
        assert_eq!(v["info"]["profile"], "debug");
    }

    #[test]
    fn done_event_has_only_a_kind() {
        let v = serde_json::to_value(AppInfoEvent::Done).unwrap();
        assert_eq!(v, serde_json::json!({ "kind": "done" }));
    }

    #[test]
    fn info_round_trips() {
        let e = AppInfoEvent::Info { info: sample() };
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(serde_json::from_str::<AppInfoEvent>(&json).unwrap(), e);
    }
}
