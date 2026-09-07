mod commands;
pub mod credentials;
pub mod error;
pub mod history;
pub mod providers;
mod settings;
mod storage;
mod tray;

use commands::AppState;
use error::AppError;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::Manager;

pub fn run() -> Result<(), AppError> {
    let _ = tracing_subscriber::fmt()
        .json()
        .with_target(false)
        .with_max_level(tracing::Level::INFO)
        .try_init();
    tauri::Builder::default()
        .setup(|app| {
            let database_path = app
                .path()
                .app_local_data_dir()
                .map_err(|_| AppError::Storage)?
                .join("ellie.sqlite3");
            let path = database_path.clone();
            // Setup must finish before exposing IPC; SQLite itself runs on a blocking worker.
            let settings =
                tauri::async_runtime::block_on(tauri::async_runtime::spawn_blocking(move || {
                    storage::initialize(&path)
                }))
                .map_err(|_| AppError::Background)??;
            app.manage(AppState {
                database_path: database_path.clone(),
                close_to_tray: Arc::new(AtomicBool::new(settings.close_to_tray)),
                settings_view: AtomicBool::new(false),
                settings_write: tokio::sync::Mutex::new(()),
                provider_registry: {
                    let mut registry = providers::ProviderRegistry::default();
                    registry.register(Arc::new(providers::OpenAiProvider));
                    registry.register(Arc::new(providers::OpenAiApiProvider));
                    registry.register(Arc::new(providers::AnthropicProvider));
                    registry.register(Arc::new(providers::DeepSeekProvider));
                    registry.register(Arc::new(providers::MockProvider));
                    registry
                },
            });
            spawn_history_cleanup(database_path);
            tray::create(app.handle()).map_err(|_| AppError::Startup)?;
            tracing::info!(event = "app_started", schema_version = 1);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_bootstrap,
            commands::save_settings,
            commands::hide_to_tray,
            commands::save_provider_key,
            commands::delete_provider_key,
            commands::provider_key_status
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window
                    .state::<AppState>()
                    .close_to_tray
                    .load(Ordering::Relaxed)
                {
                    api.prevent_close();
                    if window.hide().is_err() {
                        tracing::warn!(event = "window_hide_failed");
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .map_err(|_| AppError::Startup)
}

/// Runs retention cleanup on a blocking worker while the app is alive. The
/// first tick fires immediately, so expired history is also removed at
/// startup. Cleanup never blocks the UI or the async executor.
fn spawn_history_cleanup(database_path: std::path::PathBuf) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(history::HISTORY_CLEANUP_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            let path = database_path.clone();
            let result = tokio::time::timeout(
                history::HISTORY_CLEANUP_TIMEOUT,
                tauri::async_runtime::spawn_blocking(move || {
                    history::cleanup_history(&path, history::HISTORY_RETENTION)
                }),
            )
            .await;
            match result {
                Ok(Ok(Ok(deleted))) => {
                    tracing::info!(event = "history_cleanup", deleted = deleted);
                }
                Ok(Ok(Err(error))) => {
                    tracing::warn!(event = "history_cleanup_failed", error = ?error);
                }
                Ok(Err(_)) => {
                    tracing::warn!(event = "history_cleanup_failed");
                }
                Err(_) => {
                    tracing::warn!(event = "history_cleanup_timed_out");
                }
            }
        }
    });
}
