use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
};

use chrono::Utc;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{
    analytics::{AnalyticsRange, AnalyticsResponse},
    error::AppError,
    providers::{ProviderOverview, ProviderRegistry},
    refresh::{RefreshCoordinator, RefreshResponse},
    settings::Settings,
    storage,
};

pub struct AppState {
    pub database_path: PathBuf,
    pub local_api: Arc<crate::local_api::LocalApi>,
    pub github: Arc<crate::github::GitHubService>,
    pub close_to_tray: Arc<AtomicBool>,
    pub settings_view: AtomicBool,
    pub settings_write: tokio::sync::Mutex<()>,
    pub mini_move_generation: AtomicU64,
    pub provider_registry: ProviderRegistry,
    pub refresh: RefreshCoordinator,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    settings: Settings,
    view: &'static str,
    providers: Vec<ProviderOverview>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniBootstrap {
    settings: Settings,
    providers: Vec<ProviderOverview>,
}

#[tauri::command]
pub async fn get_bootstrap(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Bootstrap, AppError> {
    let path = state.database_path.clone();
    let settings = tauri::async_runtime::spawn_blocking(move || storage::read_settings(&path))
        .await
        .map_err(|_| AppError::Background)??;
    let refresh = refresh_all_from_app(&app, true).await;
    Ok(Bootstrap {
        settings,
        view: if state.settings_view.load(Ordering::Relaxed) {
            "settings"
        } else {
            "dashboard"
        },
        providers: refresh.providers,
    })
}

#[tauri::command]
pub async fn get_mini_bootstrap(state: State<'_, AppState>) -> Result<MiniBootstrap, AppError> {
    let path = state.database_path.clone();
    let settings = tauri::async_runtime::spawn_blocking(move || storage::read_settings(&path))
        .await
        .map_err(|_| AppError::Background)??;
    let providers = state
        .refresh
        .cached_response(&state.provider_registry, false)
        .await
        .providers;
    Ok(MiniBootstrap {
        settings,
        providers,
    })
}

pub async fn refresh_all_from_app(app: &AppHandle, force: bool) -> RefreshResponse {
    let state = app.state::<AppState>();
    let response = state
        .refresh
        .refresh_all(&state.provider_registry, &state.database_path, force)
        .await;
    crate::notifications::notify_after_refresh(app, &state.database_path, &response.providers)
        .await;
    emit_refresh(app, &response);
    response
}

fn emit_refresh(app: &AppHandle, response: &RefreshResponse) {
    if response.refreshed {
        if let Err(error) = app.emit("providers-updated", &response.providers) {
            tracing::warn!(event = "provider_update_emit_failed", error = ?error);
        }
    }
}

#[tauri::command]
pub async fn get_analytics(
    range: AnalyticsRange,
    state: State<'_, AppState>,
) -> Result<AnalyticsResponse, AppError> {
    let path = state.database_path.clone();
    tauri::async_runtime::spawn_blocking(move || crate::analytics::query(&path, range, Utc::now()))
        .await
        .map_err(|_| AppError::Background)?
}

#[tauri::command]
pub async fn refresh_all(app: AppHandle) -> Result<RefreshResponse, AppError> {
    Ok(refresh_all_from_app(&app, true).await)
}

#[tauri::command]
pub async fn refresh_provider(
    provider_id: String,
    app: AppHandle,
) -> Result<RefreshResponse, AppError> {
    let state = app.state::<AppState>();
    let response = state
        .refresh
        .refresh_one(&state.provider_registry, &state.database_path, &provider_id)
        .await;
    crate::notifications::notify_after_refresh(&app, &state.database_path, &response.providers)
        .await;
    emit_refresh(&app, &response);
    Ok(response)
}

#[tauri::command]
pub async fn save_settings(
    app: AppHandle,
    settings: Settings,
    state: State<'_, AppState>,
) -> Result<Settings, AppError> {
    settings.validate()?;
    let _guard = state.settings_write.lock().await;
    let path = state.database_path.clone();
    let value = settings.clone();
    let saved = tauri::async_runtime::spawn_blocking(move || {
        storage::save_settings_preserving_position(&path, &value)
    })
    .await
    .map_err(|_| AppError::Background)??;
    state
        .close_to_tray
        .store(saved.close_to_tray, Ordering::Relaxed);
    if let Err(error) = crate::mini_bar::apply(&app, &saved) {
        tracing::warn!(event = "mini_bar_visibility_failed", error = ?error);
    }
    if app.emit("mini-settings-updated", &saved).is_err() {
        tracing::warn!(event = "mini_bar_settings_emit_failed");
    }
    tracing::info!(event = "settings_saved");
    Ok(saved)
}

#[tauri::command]
pub fn open_main_window(app: tauri::AppHandle) -> Result<(), AppError> {
    crate::tray::open_main(&app, false).map_err(|_| AppError::Window)
}

#[tauri::command]
pub fn hide_to_tray(app: tauri::AppHandle) -> Result<(), AppError> {
    app.get_webview_window("main")
        .ok_or(AppError::Window)?
        .hide()
        .map_err(|_| AppError::Window)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderKeyStatus {
    pub provider_id: String,
    pub source: String,
}

/// Saves an API key for a key-backed provider to Windows Credential Manager.
/// The key is validated, never logged, and never returned.
#[tauri::command]
pub async fn save_provider_key(provider_id: String, key: String) -> Result<(), AppError> {
    validate_provider_key_input(&provider_id, &key)?;
    let account = crate::credentials::provider_account(&provider_id)
        .ok_or(AppError::Storage)?
        .to_string();
    let key = crate::credentials::validate_key(&key)?;
    let store = crate::credentials::WindowsCredentialStore;
    tauri::async_runtime::spawn_blocking(move || {
        crate::credentials::SecretStore::set(&store, &account, &key)
    })
    .await
    .map_err(|_| AppError::Background)??;
    tracing::info!(event = "provider_key_saved", provider = provider_id);
    Ok(())
}

/// Removes an API key for a key-backed provider from Windows Credential
/// Manager.
#[tauri::command]
pub async fn delete_provider_key(provider_id: String) -> Result<(), AppError> {
    let account = crate::credentials::provider_account(&provider_id)
        .ok_or(AppError::Storage)?
        .to_string();
    let store = crate::credentials::WindowsCredentialStore;
    tauri::async_runtime::spawn_blocking(move || {
        crate::credentials::SecretStore::delete(&store, &account)
    })
    .await
    .map_err(|_| AppError::Background)??;
    tracing::info!(event = "provider_key_deleted", provider = provider_id);
    Ok(())
}

/// Reports which key-backed providers are configured and where their key
/// lives. Secrets are never included.
#[tauri::command]
pub async fn provider_key_status() -> Result<Vec<ProviderKeyStatus>, AppError> {
    let store = crate::credentials::WindowsCredentialStore;
    let statuses = tauri::async_runtime::spawn_blocking(move || {
        crate::credentials::KEY_PROVIDERS
            .iter()
            .map(|(provider_id, _)| {
                let source = crate::credentials::provider_key_status(&store, provider_id)
                    .unwrap_or(crate::credentials::KeySource::None);
                ProviderKeyStatus {
                    provider_id: (*provider_id).to_string(),
                    source: match source {
                        crate::credentials::KeySource::CredentialManager => {
                            "credential_manager".into()
                        }
                        crate::credentials::KeySource::Environment => "environment".into(),
                        crate::credentials::KeySource::None => "none".into(),
                    },
                }
            })
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|_| AppError::Background)?;
    Ok(statuses)
}

fn validate_provider_key_input(provider_id: &str, key: &str) -> Result<(), AppError> {
    if provider_id.is_empty() || provider_id.len() > 64 || key.len() > 1_024 {
        return Err(AppError::Storage);
    }
    Ok(())
}

// Capability declarations and an explicit label guard keep auth controls main-window-only.
fn require_main_window(label: &str) -> Result<(), AppError> {
    if label != "main" {
        return Err(AppError::Window);
    }
    Ok(())
}

#[tauri::command]
pub async fn local_api_status(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<crate::local_api::ApiStatus, AppError> {
    require_main_window(window.label())?;
    Ok(state.local_api.status().await)
}

#[tauri::command]
pub async fn configure_local_api(
    window: tauri::WebviewWindow,
    app: AppHandle,
    state: State<'_, AppState>,
    action: crate::local_api::ApiAction,
) -> Result<crate::local_api::ApiStatus, AppError> {
    require_main_window(window.label())?;
    Ok(state
        .local_api
        .apply(Some(action), crate::api::router(app))
        .await)
}

fn require_github_main_window(label: &str) -> Result<(), crate::github::GitHubError> {
    require_main_window(label).map_err(|_| crate::github::GitHubError::window_denied())
}

#[tauri::command]
pub async fn github_connection_status(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
) -> Result<crate::github::GitHubConnectionStatus, crate::github::GitHubError> {
    require_github_main_window(window.label())?;
    app_state.github.connection_status().await
}

#[tauri::command]
pub async fn github_save_client_id(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
    client_id: String,
) -> Result<crate::github::GitHubConnectionStatus, crate::github::GitHubError> {
    require_github_main_window(window.label())?;
    app_state.github.save_client_id(client_id).await
}

#[tauri::command]
pub async fn github_save_client_secret(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
    client_secret: String,
) -> Result<crate::github::GitHubConnectionStatus, crate::github::GitHubError> {
    require_github_main_window(window.label())?;
    app_state.github.save_client_secret(client_secret).await
}

#[tauri::command]
pub async fn github_sign_in(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
) -> Result<crate::github::GitHubConnectionStatus, crate::github::GitHubError> {
    require_github_main_window(window.label())?;
    let app = window.app_handle().clone();
    app_state
        .github
        .sign_in(move |authorize_url| {
            use tauri_plugin_opener::OpenerExt;
            app.opener()
                .open_url(authorize_url, None::<&str>)
                .map_err(|_| crate::github::GitHubError::provider_unavailable())
        })
        .await
}

#[tauri::command]
pub async fn github_cancel_sign_in(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
) -> Result<crate::github::GitHubConnectionStatus, crate::github::GitHubError> {
    require_github_main_window(window.label())?;
    app_state.github.cancel_sign_in().await
}

#[tauri::command]
pub async fn github_connect_start(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
    client_id: String,
    redirect_port: u16,
) -> Result<String, crate::github::GitHubError> {
    require_github_main_window(window.label())?;
    app_state.github.connect_start(client_id, redirect_port)
}

#[tauri::command]
pub async fn github_connect_complete(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
    code: String,
    state: String,
) -> Result<crate::github::GitHubConnectionStatus, crate::github::GitHubError> {
    require_github_main_window(window.label())?;
    app_state.github.connect_complete(code, state).await
}

#[tauri::command]
pub async fn github_disconnect(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
) -> Result<crate::github::GitHubConnectionStatus, crate::github::GitHubError> {
    require_github_main_window(window.label())?;
    app_state.github.disconnect().await
}

#[tauri::command]
pub async fn github_list_repositories(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
) -> Result<Vec<crate::github::RepositorySummary>, crate::github::GitHubError> {
    require_github_main_window(window.label())?;
    app_state.github.list_repositories().await
}

#[tauri::command]
pub async fn github_list_commits(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
    owner: String,
    repo: String,
    branch: Option<String>,
) -> Result<Vec<crate::github::CommitSummary>, crate::github::GitHubError> {
    require_github_main_window(window.label())?;
    app_state
        .github
        .list_commits(&owner, &repo, branch.as_deref())
        .await
}

#[cfg(test)]
mod local_api_ipc_tests {
    #[test]
    fn only_main_window_can_manage_authentication() {
        assert!(super::require_main_window("main").is_ok());
        assert!(super::require_github_main_window("main").is_ok());
        for label in ["mini", "", "other"] {
            assert!(super::require_main_window(label).is_err());
            assert!(super::require_github_main_window(label).is_err());
        }
    }

    #[test]
    fn every_github_command_uses_the_main_window_guard() {
        let commands_source = include_str!("commands.rs");
        let runtime_source = include_str!("lib.rs");
        let build_manifest = include_str!("../build.rs");
        let main_capability = include_str!("../capabilities/main.json");
        let mini_capability = include_str!("../capabilities/mini.json");
        for command in [
            "github_connection_status",
            "github_save_client_id",
            "github_save_client_secret",
            "github_sign_in",
            "github_cancel_sign_in",
            "github_connect_start",
            "github_connect_complete",
            "github_disconnect",
            "github_list_repositories",
            "github_list_commits",
        ] {
            let start = commands_source
                .find(&format!("fn {command}"))
                .unwrap_or_else(|| panic!("missing command {command}"));
            let remainder = &commands_source[start..];
            let end = remainder
                .find("#[tauri::command]")
                .unwrap_or(remainder.len());
            assert!(
                remainder[..end].contains("require_github_main_window(window.label())?"),
                "{command} must enforce the main-window guard"
            );
            assert!(
                runtime_source.contains(&format!("commands::{command}")),
                "{command} must be registered in the invoke handler"
            );
            assert!(
                build_manifest.contains(&format!("\"{command}\"")),
                "{command} must be registered in the Tauri app manifest"
            );
            let permission = command.replace('_', "-");
            assert!(
                main_capability.contains(&format!("\"allow-{permission}\"")),
                "{command} must be allowed only by the main capability"
            );
            assert!(!mini_capability.contains(&permission));
        }
    }
}
