use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use serde::Serialize;
use tauri::{Manager, State};

use crate::{
    error::AppError,
    history,
    providers::{ProviderOverview, ProviderRegistry, UsageSnapshot},
    settings::Settings,
    storage,
};

pub struct AppState {
    pub database_path: PathBuf,
    pub close_to_tray: Arc<AtomicBool>,
    pub settings_view: AtomicBool,
    pub settings_write: tokio::sync::Mutex<()>,
    pub provider_registry: ProviderRegistry,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    settings: Settings,
    view: &'static str,
    providers: Vec<ProviderOverview>,
}

#[tauri::command]
pub async fn get_bootstrap(state: State<'_, AppState>) -> Result<Bootstrap, AppError> {
    let path = state.database_path.clone();
    let settings = tauri::async_runtime::spawn_blocking(move || storage::read_settings(&path))
        .await
        .map_err(|_| AppError::Background)??;
    let mut providers = state.provider_registry.refresh_all().await;
    attach_spend_estimates(&state, &mut providers).await;
    persist_snapshots(&state, &providers).await;
    Ok(Bootstrap {
        settings,
        view: if state.settings_view.load(Ordering::Relaxed) {
            "settings"
        } else {
            "dashboard"
        },
        providers,
    })
}

/// Best-effort history write for successful refreshes. A failing write never
/// fails bootstrap; it runs on a blocking worker, bounded by a timeout.
async fn persist_snapshots(state: &AppState, overviews: &[ProviderOverview]) {
    let path = state.database_path.clone();
    let snapshots: Vec<UsageSnapshot> = overviews
        .iter()
        .filter_map(|overview| overview.snapshot.clone())
        .collect();
    let _ = tokio::time::timeout(
        history::HISTORY_CLEANUP_TIMEOUT,
        tauri::async_runtime::spawn_blocking(move || {
            for snapshot in &snapshots {
                if let Err(error) = history::insert_snapshot(&path, snapshot) {
                    tracing::warn!(
                        event = "history_insert_failed",
                        provider = ?snapshot.provider_id,
                        error = ?error
                    );
                }
            }
        }),
    )
    .await;
}

/// Attaches a locally-calculated spend estimate to balance-based snapshots
/// (e.g. DeepSeek) by comparing against the oldest stored balance inside the
/// trailing window. Best-effort: a failure only skips the estimate.
async fn attach_spend_estimates(state: &AppState, overviews: &mut [ProviderOverview]) {
    let path = state.database_path.clone();
    let candidates: Vec<(String, f64, String)> = overviews
        .iter()
        .filter_map(|overview| {
            let snapshot = overview.snapshot.as_ref()?;
            Some((
                snapshot.provider_id.clone(),
                snapshot.balance?,
                snapshot.balance_currency.clone()?,
            ))
        })
        .collect();
    let estimates = tauri::async_runtime::spawn_blocking(move || {
        let mut estimates = std::collections::BTreeMap::new();
        for (provider_id, balance, currency) in candidates {
            match history::spend_estimate(
                &path,
                &provider_id,
                balance,
                &currency,
                history::SPEND_ESTIMATE_WINDOW,
            ) {
                Ok(Some(estimate)) => {
                    estimates.insert(provider_id, estimate);
                }
                Ok(None) => {}
                Err(error) => tracing::warn!(
                    event = "spend_estimate_failed",
                    provider = ?provider_id,
                    error = ?error
                ),
            }
        }
        estimates
    })
    .await
    .unwrap_or_default();
    for overview in overviews {
        let Some(snapshot) = overview.snapshot.as_mut() else {
            continue;
        };
        snapshot.spend_estimate = estimates.get(&snapshot.provider_id).cloned();
    }
}

#[tauri::command]
pub async fn save_settings(
    settings: Settings,
    state: State<'_, AppState>,
) -> Result<Settings, AppError> {
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
