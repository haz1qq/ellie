use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{
    error::AppError,
    providers::{ProviderOverview, ProviderRegistry},
    refresh::{RefreshCoordinator, RefreshResponse},
    settings::Settings,
    storage,
};

pub struct AppState {
    pub database_path: PathBuf,
    pub close_to_tray: Arc<AtomicBool>,
    pub settings_view: AtomicBool,
    pub settings_write: tokio::sync::Mutex<()>,
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

#[tauri::command]
pub async fn get_bootstrap(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Bootstrap, AppError> {
    let path = state.database_path.clone();
    let settings = tauri::async_runtime::spawn_blocking(move || storage::read_settings(&path))
        .await
        .map_err(|_| AppError::Background)??;
    let refresh = state
        .refresh
        .refresh_all(&state.provider_registry, &state.database_path, true)
        .await;
    crate::notifications::notify_after_refresh(&app, &state.database_path, &refresh.providers)
        .await;
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
    settings: Settings,
    state: State<'_, AppState>,
) -> Result<Settings, AppError> {
    settings.validate()?;
    let _guard = state.settings_write.lock().await;
    let path = state.database_path.clone();
    let value = settings.clone();
    tauri::async_runtime::spawn_blocking(move || storage::save_settings(&path, &value))
        .await
        .map_err(|_| AppError::Background)??;
    state
        .close_to_tray
        .store(settings.close_to_tray, Ordering::Relaxed);
    tracing::info!(event = "settings_saved");
    Ok(settings)
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
