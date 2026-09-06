use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use serde::Serialize;
use tauri::{Manager, State};

use crate::{error::AppError, settings::Settings, storage};

pub struct AppState {
    pub database_path: PathBuf,
    pub close_to_tray: Arc<AtomicBool>,
    pub settings_view: AtomicBool,
    pub settings_write: tokio::sync::Mutex<()>,
}

#[derive(Serialize)]
pub struct Bootstrap {
    settings: Settings,
    view: &'static str,
}

#[tauri::command]
pub async fn get_bootstrap(state: State<'_, AppState>) -> Result<Bootstrap, AppError> {
    let path = state.database_path.clone();
    let settings = tauri::async_runtime::spawn_blocking(move || storage::read_settings(&path))
        .await
        .map_err(|_| AppError::Background)??;
    Ok(Bootstrap {
        settings,
        view: if state.settings_view.load(Ordering::Relaxed) {
            "settings"
        } else {
            "dashboard"
        },
    })
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
