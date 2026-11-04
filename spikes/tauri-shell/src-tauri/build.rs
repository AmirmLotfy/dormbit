fn main() {
    // Only these commands exist in the app manifest. Each one must also be
    // granted to a window through a capability file before the UI can call it.
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&["app_info", "pick_folder"])),
    )
    .expect("tauri build script");
}
