pub mod analytics;
mod api;
mod commands;
pub mod credentials;
pub mod error;
pub mod history;
mod mini_bar;
mod notifications;
pub mod providers;
mod refresh;
mod settings;
mod storage;
mod tray;

use commands::AppState;
use error::AppError;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use tauri::Manager;

#[derive(Clone, Copy, Debug, PartialEq)]
enum MainCloseAction {
    Hide,
    Exit,
}

fn main_close_action(close_to_tray: bool) -> MainCloseAction {
    if close_to_tray {
        MainCloseAction::Hide
    } else {
        MainCloseAction::Exit
    }
}

pub fn run() -> Result<(), AppError> {
    let _ = tracing_subscriber::fmt()
        .json()
        .with_target(false)
        .with_max_level(tracing::Level::INFO)
        .try_init();
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
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
                mini_move_generation: AtomicU64::new(0),
                provider_registry: {
                    let mut registry = providers::ProviderRegistry::default();
                    registry.register(Arc::new(providers::OpenAiProvider));
                    registry.register(Arc::new(providers::OpenAiApiProvider));
                    registry.register(Arc::new(providers::AnthropicProvider));
                    registry.register(Arc::new(providers::DeepSeekProvider));
                    registry.register(Arc::new(providers::MockProvider));
                    registry
                },
                refresh: refresh::RefreshCoordinator::default(),
            });
            if let Err(error) = mini_bar::apply(app.handle(), &settings) {
                tracing::warn!(event = "mini_bar_startup_failed", error = ?error);
            }
            api::spawn(app.handle().clone());
            refresh::spawn_poller(app.handle().clone());
            spawn_history_cleanup(database_path);
            tray::create(app.handle()).map_err(|_| AppError::Startup)?;
            tracing::info!(event = "app_started", schema_version = 10);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_bootstrap,
            commands::get_mini_bootstrap,
            commands::open_main_window,
            commands::get_analytics,
            commands::save_settings,
            commands::hide_to_tray,
            commands::refresh_all,
            commands::refresh_provider,
            commands::save_provider_key,
            commands::delete_provider_key,
            commands::provider_key_status
        ])
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    let close_to_tray = window
                        .state::<AppState>()
                        .close_to_tray
                        .load(Ordering::Relaxed);
                    api.prevent_close();
                    match main_close_action(close_to_tray) {
                        MainCloseAction::Hide => {
                            if window.hide().is_err() {
                                tracing::warn!(event = "window_hide_failed");
                            }
                        }
                        MainCloseAction::Exit => {
                            // Exit closes auxiliary windows such as the always-on-top mini bar.
                            tracing::info!(event = "window_close_requested");
                            window.app_handle().exit(0);
                        }
                    }
                }
            } else if window.label() == mini_bar::WINDOW_LABEL {
                if let tauri::WindowEvent::Moved(position) = event {
                    let state = window.state::<AppState>();
                    let generation = state
                        .mini_move_generation
                        .fetch_add(1, Ordering::Relaxed)
                        .wrapping_add(1);
                    let app = window.app_handle().clone();
                    let (x, y) = (position.x, position.y);
                    tauri::async_runtime::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                        let state = app.state::<AppState>();
                        if state.mini_move_generation.load(Ordering::Relaxed) != generation {
                            return;
                        }
                        let _guard = state.settings_write.lock().await;
                        let path = state.database_path.clone();
                        let result = tauri::async_runtime::spawn_blocking(move || {
                            storage::save_mini_bar_position(&path, x, y)
                        })
                        .await;
                        if !matches!(result, Ok(Ok(()))) {
                            tracing::warn!(event = "mini_bar_position_save_failed");
                        }
                    });
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

#[cfg(test)]
mod tests {
    use super::{main_close_action, MainCloseAction};

    #[test]
    fn main_close_hides_only_when_close_to_tray_is_enabled() {
        assert_eq!(main_close_action(true), MainCloseAction::Hide);
        assert_eq!(main_close_action(false), MainCloseAction::Exit);
    }
}
