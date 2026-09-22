pub mod analytics;
mod api;
mod commands;
pub mod credentials;
pub mod error;
pub mod github;
pub mod history;
mod local_api;
pub mod local_api_token;
mod mini_bar;
mod notifications;
pub mod providers;
mod refresh;
mod settings;
mod storage;
mod task_note;
pub mod tasks;
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
        .plugin(
            tauri_plugin_opener::Builder::new()
                .open_js_links_on_click(false)
                .build(),
        )
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
            let github_client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .user_agent(format!("ellie/{}", env!("CARGO_PKG_VERSION")))
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .https_only(true)
                .build()
                .map_err(|_| AppError::Startup)?;
            let tasks = Arc::new(tasks::TaskService::new(database_path.clone()));
            app.manage(AppState {
                database_path: database_path.clone(),
                github: Arc::new(github::GitHubService::new(
                    github_client,
                    github::DEFAULT_API_BASE_URL,
                    Arc::new(credentials::WindowsCredentialStore),
                    Arc::new(github::connection_store::SqliteGitHubConnectionStore::new(
                        database_path.clone(),
                    )),
                    Arc::new(github::creation_store::SqliteRepositoryCreationStore::new(
                        database_path.clone(),
                    )),
                )),
                local_api: local_api::LocalApi::new(
                    database_path.clone(),
                    Arc::new(credentials::WindowsCredentialStore),
                    std::env::var_os(local_api_token::ENVIRONMENT),
                    api::API_ADDRESS,
                ),
                tasks: Arc::clone(&tasks),
                close_to_tray: Arc::new(AtomicBool::new(settings.close_to_tray)),
                settings_view: AtomicBool::new(false),
                settings_write: tokio::sync::Mutex::new(()),
                mini_move_generation: AtomicU64::new(0),
                mini_resize_generation: AtomicU64::new(0),
                task_note_move_generation: AtomicU64::new(0),
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
            let note_tasks = Arc::clone(&tasks);
            let note_snapshot =
                tauri::async_runtime::block_on(tauri::async_runtime::spawn_blocking(move || {
                    note_tasks.task_note_snapshot()
                }));
            match note_snapshot {
                Ok(Ok(snapshot)) => {
                    if let Err(error) = task_note::apply(app.handle(), &snapshot) {
                        tracing::warn!(event = "task_note_startup_failed", error = ?error);
                    }
                }
                _ => tracing::warn!(event = "task_note_startup_failed"),
            }
            api::spawn(app.handle().clone());
            refresh::spawn_poller(app.handle().clone());
            spawn_history_cleanup(database_path);
            tray::create(app.handle()).map_err(|_| AppError::Startup)?;
            tracing::info!(event = "app_started", schema_version = 19);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::local_api_status,
            commands::configure_local_api,
            commands::get_bootstrap,
            commands::get_mini_bootstrap,
            commands::open_main_window,
            commands::open_main_section,
            commands::get_analytics,
            commands::save_settings,
            commands::hide_to_tray,
            commands::refresh_all,
            commands::refresh_provider,
            commands::save_provider_key,
            commands::delete_provider_key,
            commands::provider_key_status,
            commands::github_connection_status,
            commands::github_save_client_id,
            commands::github_save_client_secret,
            commands::github_sign_in,
            commands::github_cancel_sign_in,
            commands::github_disconnect,
            commands::github_list_repositories,
            commands::github_list_commits,
            commands::github_contribution_calendar,
            commands::github_prepare_repository_creation,
            commands::github_confirm_repository_creation,
            commands::github_repository_creation_status,
            commands::github_resolve_repository_creation,
            commands::task_bootstrap,
            commands::task_list,
            commands::task_create_list,
            commands::task_rename_list,
            commands::task_list_delete_preview,
            commands::task_delete_list,
            commands::task_create,
            commands::task_update,
            commands::task_set_completed,
            commands::task_delete,
            commands::task_set_pinned,
            commands::task_note_bootstrap,
            commands::task_note_complete,
            commands::task_note_unpin
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
                match event {
                    tauri::WindowEvent::Moved(position) => {
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
                    tauri::WindowEvent::Resized(size) => {
                        // User-driven resizing is persisted so it survives a
                        // restart and is never overwritten by a settings save.
                        let width = size.width.clamp(
                            crate::settings::MIN_MINI_BAR_WIDTH,
                            crate::settings::MAX_MINI_BAR_WIDTH,
                        );
                        let height = size.height.clamp(
                            crate::settings::MIN_MINI_BAR_HEIGHT,
                            crate::settings::MAX_MINI_BAR_HEIGHT,
                        );
                        let state = window.state::<AppState>();
                        let generation = state
                            .mini_resize_generation
                            .fetch_add(1, Ordering::Relaxed)
                            .wrapping_add(1);
                        let app = window.app_handle().clone();
                        tauri::async_runtime::spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                            let state = app.state::<AppState>();
                            if state.mini_resize_generation.load(Ordering::Relaxed) != generation {
                                return;
                            }
                            let _guard = state.settings_write.lock().await;
                            let path = state.database_path.clone();
                            let result = tauri::async_runtime::spawn_blocking(move || {
                                storage::save_mini_bar_size(&path, width, height)
                            })
                            .await;
                            if !matches!(result, Ok(Ok(()))) {
                                tracing::warn!(event = "mini_bar_size_save_failed");
                            }
                        });
                    }
                    _ => {}
                }
            } else if window.label() == task_note::WINDOW_LABEL {
                match event {
                    tauri::WindowEvent::Moved(position) => {
                        let state = window.state::<AppState>();
                        let generation = state
                            .task_note_move_generation
                            .fetch_add(1, Ordering::Relaxed)
                            .wrapping_add(1);
                        let app = window.app_handle().clone();
                        let (x, y) = (position.x, position.y);
                        tauri::async_runtime::spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                            let state = app.state::<AppState>();
                            if state.task_note_move_generation.load(Ordering::Relaxed) != generation
                            {
                                return;
                            }
                            let tasks = Arc::clone(&state.tasks);
                            let result = tauri::async_runtime::spawn_blocking(move || {
                                tasks.save_sticky_position(x, y)
                            })
                            .await;
                            if !matches!(result, Ok(Ok(()))) {
                                tracing::warn!(event = "task_note_position_save_failed");
                            }
                        });
                    }
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        if window.hide().is_err() {
                            tracing::warn!(event = "task_note_hide_failed");
                        }
                    }
                    _ => {}
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
