fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_bootstrap",
            "save_settings",
            "hide_to_tray",
        ]),
    ))
    .expect("failed to build Ellie application resources");
}
