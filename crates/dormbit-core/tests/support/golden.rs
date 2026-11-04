//! Snapshot helper (foundation Requirement 16.2). Every golden file lives under
//! `fixtures/golden/<area>/<name>.snap`. In CI (`CI=true`, `INSTA_UPDATE=no`) a difference
//! fails the test and nothing is written; locally, `INSTA_UPDATE=always` accepts new values.

/// `golden!("area", "name", &value)` compares `value`, serialized as JSON, with its snapshot.
#[macro_export]
macro_rules! golden {
    ($area:literal, $name:literal, $value:expr) => {
        insta::with_settings!(
            {
                snapshot_path => concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/golden/", $area),
                prepend_module_to_snapshot => false,
                omit_expression => true,
            },
            { insta::assert_json_snapshot!($name, $value) }
        )
    };
}
