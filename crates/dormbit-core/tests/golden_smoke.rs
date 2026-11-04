//! Proves the snapshot location and format with two tiny values. Real golden tests (capsules,
//! plans, briefs) use the same macro from their own crates.

#[path = "support/golden.rs"]
#[macro_use]
mod golden;

use dormbit_core::model::{AppInfoEvent, DormbitError, ErrorCode};

#[test]
fn golden_files_live_under_fixtures_golden() {
    golden!("smoke", "app_info_event_done", &AppInfoEvent::Done);
    golden!(
        "smoke",
        "dormbit_error_cancelled",
        &DormbitError::new(ErrorCode::Cancelled, "window closed")
    );
}
