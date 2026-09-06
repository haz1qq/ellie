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
    let providers = state.provider_registry.refresh_all().await;
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
