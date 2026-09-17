fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "local_api_status",
            "configure_local_api",
            "get_bootstrap",
            "get_mini_bootstrap",
            "open_main_window",
            "get_analytics",
            "save_settings",
            "hide_to_tray",
            "refresh_all",
            "refresh_provider",
            "save_provider_key",
            "delete_provider_key",
            "provider_key_status",
            // Custom commands are declared here so Tauri generates their
            // least-privilege permission manifests for the main capability.
            "github_connection_status",
            "github_save_client_id",
            "github_sign_in",
            "github_cancel_sign_in",
            "github_connect_start",
            "github_connect_complete",
            "github_disconnect",
            "github_list_repositories",
            "github_list_commits",
        ]),
    ))
    .expect("failed to build Ellie application resources");
}
