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
    pub tasks: Arc<crate::tasks::TaskService>,
    pub close_to_tray: Arc<AtomicBool>,
    pub settings_view: AtomicBool,
    pub settings_write: tokio::sync::Mutex<()>,
    pub mini_move_generation: AtomicU64,
    pub task_note_move_generation: AtomicU64,
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
    pub settings: Settings,
    pub providers: Vec<ProviderOverview>,
    /// Present only when the HUD task section is enabled.
    pub task: Option<MiniTaskProjection>,
    /// Present only when the HUD GitHub section is enabled.
    pub github: Option<MiniGitHubProjection>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniTaskProjection {
    pub task_id: i64,
    pub title: String,
    pub kind: crate::tasks::TaskKind,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniGitHubProjection {
    pub state: String,
    pub account_login: Option<String>,
    pub summary: Option<crate::github::HudCommitSummary>,
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
    // Disabled HUD sections never receive their payloads.
    let task = if settings.mini_bar_show_task {
        let tasks = Arc::clone(&state.tasks);
        tauri::async_runtime::spawn_blocking(move || tasks.pinned_task())
            .await
            .ok()
            .and_then(Result::ok)
            .flatten()
            .map(|task| MiniTaskProjection {
                task_id: task.id,
                title: task.title,
                kind: task.kind,
            })
    } else {
        None
    };
    let github = if settings.mini_bar_show_github {
        let status = state.github.connection_status().await.ok();
        let summary = state.github.hud_commit_summary().await;
        status.map(|status| {
            let state_label = match status.state {
                crate::github::GitHubConnectionState::Connected => "Connected",
                crate::github::GitHubConnectionState::Authorizing => "Authorizing",
                crate::github::GitHubConnectionState::Disconnected => "Disconnected",
            };
            MiniGitHubProjection {
                state: state_label.to_string(),
                account_login: status.account.map(|account| account.login),
                summary,
            }
        })
    } else {
        None
    };
    Ok(MiniBootstrap {
        settings,
        providers,
        task,
        github,
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
pub fn open_main_section(app: tauri::AppHandle, view: String) -> Result<(), AppError> {
    let view = view.as_str();
    if ![
        "dashboard",
        "ai-usage",
        "github",
        "todos",
        "history",
        "settings",
    ]
    .contains(&view)
    {
        return Ok(());
    }
    crate::tray::open_main_view(&app, view).map_err(|_| AppError::Window)
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

fn require_task_main_window(label: &str) -> Result<(), crate::tasks::TaskError> {
    require_main_window(label).map_err(|_| crate::tasks::TaskError::window_denied())
}

fn require_task_note_window(label: &str) -> Result<(), crate::tasks::TaskError> {
    if label != crate::task_note::WINDOW_LABEL {
        return Err(crate::tasks::TaskError::window_denied());
    }
    Ok(())
}

async fn sync_task_note(app: &AppHandle, tasks: Arc<crate::tasks::TaskService>) {
    if let Err(error) = crate::task_note::notify_main(app) {
        tracing::warn!(event = "task_main_notify_failed", error = ?error);
    }
    if let Err(error) = crate::task_note::notify_mini(app) {
        tracing::warn!(event = "task_mini_notify_failed", error = ?error);
    }
    let snapshot = tauri::async_runtime::spawn_blocking(move || tasks.task_note_snapshot()).await;
    match snapshot {
        Ok(Ok(snapshot)) => {
            if let Err(error) = crate::task_note::apply(app, &snapshot) {
                tracing::warn!(event = "task_note_sync_failed", error = ?error);
            }
        }
        _ => tracing::warn!(event = "task_note_sync_failed"),
    }
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
    max_rows: Option<usize>,
) -> Result<Vec<crate::github::CommitSummary>, crate::github::GitHubError> {
    require_github_main_window(window.label())?;
    match max_rows {
        Some(max_rows) => {
            app_state
                .github
                .list_commits_limited(&owner, &repo, branch.as_deref(), max_rows)
                .await
        }
        None => {
            app_state
                .github
                .list_commits(&owner, &repo, branch.as_deref())
                .await
        }
    }
}

#[tauri::command]
pub async fn github_contribution_calendar(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
    query: crate::github::ContributionCalendarQuery,
) -> Result<crate::github::ContributionCalendar, crate::github::GitHubError> {
    require_github_main_window(window.label())?;
    app_state.github.contribution_calendar(query).await
}

#[tauri::command]
pub async fn github_prepare_repository_creation(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
    input: crate::github::RepositoryCreationInput,
) -> Result<crate::github::RepositoryCreationReview, crate::github::GitHubError> {
    require_github_main_window(window.label())?;
    app_state.github.prepare_repository_creation(input).await
}

#[tauri::command]
pub async fn github_confirm_repository_creation(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
    review_id: String,
) -> Result<crate::github::RepositorySummary, crate::github::GitHubError> {
    require_github_main_window(window.label())?;
    app_state
        .github
        .confirm_repository_creation(&review_id)
        .await
}

#[tauri::command]
pub async fn github_repository_creation_status(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
) -> Result<Vec<crate::github::RepositoryCreationAttemptStatus>, crate::github::GitHubError> {
    require_github_main_window(window.label())?;
    app_state.github.repository_creation_status().await
}

#[tauri::command]
pub async fn github_resolve_repository_creation(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
    attempt_id: String,
    resolution: crate::github::RepositoryCreationResolution,
) -> Result<(), crate::github::GitHubError> {
    require_github_main_window(window.label())?;
    app_state
        .github
        .resolve_repository_creation(&attempt_id, resolution)
        .await
}

#[tauri::command]
pub async fn task_bootstrap(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
) -> Result<crate::tasks::TaskBootstrap, crate::tasks::TaskError> {
    require_task_main_window(window.label())?;
    let tasks = Arc::clone(&app_state.tasks);
    tauri::async_runtime::spawn_blocking(move || tasks.bootstrap())
        .await
        .map_err(|_| crate::tasks::TaskError::new(crate::tasks::TaskErrorCategory::Storage))?
}

#[tauri::command]
pub async fn task_list(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
    query: crate::tasks::TaskQuery,
) -> Result<Vec<crate::tasks::TaskItem>, crate::tasks::TaskError> {
    require_task_main_window(window.label())?;
    let tasks = Arc::clone(&app_state.tasks);
    tauri::async_runtime::spawn_blocking(move || tasks.list_tasks(query))
        .await
        .map_err(|_| crate::tasks::TaskError::new(crate::tasks::TaskErrorCategory::Storage))?
}

#[tauri::command]
pub async fn task_create_list(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
    name: String,
) -> Result<crate::tasks::TaskList, crate::tasks::TaskError> {
    require_task_main_window(window.label())?;
    let tasks = Arc::clone(&app_state.tasks);
    tauri::async_runtime::spawn_blocking(move || tasks.create_list(name))
        .await
        .map_err(|_| crate::tasks::TaskError::new(crate::tasks::TaskErrorCategory::Storage))?
}

#[tauri::command]
pub async fn task_rename_list(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
    list_id: i64,
    name: String,
) -> Result<crate::tasks::TaskList, crate::tasks::TaskError> {
    require_task_main_window(window.label())?;
    let tasks = Arc::clone(&app_state.tasks);
    tauri::async_runtime::spawn_blocking(move || tasks.rename_list(list_id, name))
        .await
        .map_err(|_| crate::tasks::TaskError::new(crate::tasks::TaskErrorCategory::Storage))?
}

#[tauri::command]
pub async fn task_list_delete_preview(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
    list_id: i64,
) -> Result<crate::tasks::ListDeletePreview, crate::tasks::TaskError> {
    require_task_main_window(window.label())?;
    let tasks = Arc::clone(&app_state.tasks);
    tauri::async_runtime::spawn_blocking(move || tasks.list_delete_preview(list_id))
        .await
        .map_err(|_| crate::tasks::TaskError::new(crate::tasks::TaskErrorCategory::Storage))?
}

#[tauri::command]
pub async fn task_delete_list(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
    list_id: i64,
    expected_task_count: u64,
) -> Result<(), crate::tasks::TaskError> {
    require_task_main_window(window.label())?;
    let tasks = Arc::clone(&app_state.tasks);
    let worker = Arc::clone(&tasks);
    tauri::async_runtime::spawn_blocking(move || worker.delete_list(list_id, expected_task_count))
        .await
        .map_err(|_| crate::tasks::TaskError::new(crate::tasks::TaskErrorCategory::Storage))??;
    sync_task_note(window.app_handle(), tasks).await;
    Ok(())
}

#[tauri::command]
pub async fn task_create(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
    input: crate::tasks::TaskInput,
) -> Result<crate::tasks::TaskItem, crate::tasks::TaskError> {
    require_task_main_window(window.label())?;
    let tasks = Arc::clone(&app_state.tasks);
    tauri::async_runtime::spawn_blocking(move || tasks.create_task(input))
        .await
        .map_err(|_| crate::tasks::TaskError::new(crate::tasks::TaskErrorCategory::Storage))?
}

#[tauri::command]
pub async fn task_update(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
    task_id: i64,
    input: crate::tasks::TaskInput,
) -> Result<crate::tasks::TaskItem, crate::tasks::TaskError> {
    require_task_main_window(window.label())?;
    let tasks = Arc::clone(&app_state.tasks);
    let worker = Arc::clone(&tasks);
    let task = tauri::async_runtime::spawn_blocking(move || worker.update_task(task_id, input))
        .await
        .map_err(|_| crate::tasks::TaskError::new(crate::tasks::TaskErrorCategory::Storage))??;
    sync_task_note(window.app_handle(), tasks).await;
    Ok(task)
}

#[tauri::command]
pub async fn task_set_completed(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
    task_id: i64,
    completed: bool,
) -> Result<crate::tasks::TaskItem, crate::tasks::TaskError> {
    require_task_main_window(window.label())?;
    let tasks = Arc::clone(&app_state.tasks);
    let worker = Arc::clone(&tasks);
    let task =
        tauri::async_runtime::spawn_blocking(move || worker.set_completed(task_id, completed))
            .await
            .map_err(|_| {
                crate::tasks::TaskError::new(crate::tasks::TaskErrorCategory::Storage)
            })??;
    sync_task_note(window.app_handle(), tasks).await;
    Ok(task)
}

#[tauri::command]
pub async fn task_delete(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
    task_id: i64,
) -> Result<(), crate::tasks::TaskError> {
    require_task_main_window(window.label())?;
    let tasks = Arc::clone(&app_state.tasks);
    let worker = Arc::clone(&tasks);
    tauri::async_runtime::spawn_blocking(move || worker.delete_task(task_id))
        .await
        .map_err(|_| crate::tasks::TaskError::new(crate::tasks::TaskErrorCategory::Storage))??;
    sync_task_note(window.app_handle(), tasks).await;
    Ok(())
}

#[tauri::command]
pub async fn task_set_pinned(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
    task_id: Option<i64>,
) -> Result<Option<i64>, crate::tasks::TaskError> {
    require_task_main_window(window.label())?;
    let tasks = Arc::clone(&app_state.tasks);
    let worker = Arc::clone(&tasks);
    let pinned = tauri::async_runtime::spawn_blocking(move || worker.set_pinned(task_id))
        .await
        .map_err(|_| crate::tasks::TaskError::new(crate::tasks::TaskErrorCategory::Storage))??;
    sync_task_note(window.app_handle(), tasks).await;
    Ok(pinned)
}

#[tauri::command]
pub async fn task_note_bootstrap(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
) -> Result<Option<crate::tasks::TaskItem>, crate::tasks::TaskError> {
    require_task_note_window(window.label())?;
    let tasks = Arc::clone(&app_state.tasks);
    tauri::async_runtime::spawn_blocking(move || tasks.pinned_task())
        .await
        .map_err(|_| crate::tasks::TaskError::new(crate::tasks::TaskErrorCategory::Storage))?
}

#[tauri::command]
pub async fn task_note_complete(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
) -> Result<crate::tasks::TaskItem, crate::tasks::TaskError> {
    require_task_note_window(window.label())?;
    let tasks = Arc::clone(&app_state.tasks);
    let worker = Arc::clone(&tasks);
    let task = tauri::async_runtime::spawn_blocking(move || worker.complete_pinned())
        .await
        .map_err(|_| crate::tasks::TaskError::new(crate::tasks::TaskErrorCategory::Storage))??;
    sync_task_note(window.app_handle(), tasks).await;
    if crate::task_note::notify_main(window.app_handle()).is_err() {
        tracing::warn!(event = "task_note_main_notify_failed");
    }
    Ok(task)
}

#[tauri::command]
pub async fn task_note_unpin(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
) -> Result<(), crate::tasks::TaskError> {
    require_task_note_window(window.label())?;
    let tasks = Arc::clone(&app_state.tasks);
    let worker = Arc::clone(&tasks);
    tauri::async_runtime::spawn_blocking(move || worker.set_pinned(None))
        .await
        .map_err(|_| crate::tasks::TaskError::new(crate::tasks::TaskErrorCategory::Storage))??;
    sync_task_note(window.app_handle(), tasks).await;
    if crate::task_note::notify_main(window.app_handle()).is_err() {
        tracing::warn!(event = "task_note_main_notify_failed");
    }
    Ok(())
}

#[cfg(test)]
mod local_api_ipc_tests {
    #[test]
    fn only_main_window_can_manage_authentication() {
        assert!(super::require_main_window("main").is_ok());
        assert!(super::require_github_main_window("main").is_ok());
        assert!(super::require_task_main_window("main").is_ok());
        assert!(super::require_task_note_window("task-note").is_ok());
        for label in ["mini", "task-note", "", "other"] {
            assert!(super::require_main_window(label).is_err());
            assert!(super::require_github_main_window(label).is_err());
            assert!(super::require_task_main_window(label).is_err());
        }
        for label in ["main", "mini", "", "other"] {
            assert!(super::require_task_note_window(label).is_err());
        }
    }

    #[test]
    fn every_github_command_uses_the_main_window_guard() {
        let commands_source = include_str!("commands.rs");
        let runtime_source = include_str!("lib.rs");
        let build_manifest = include_str!("../build.rs");
        let main_capability = include_str!("../capabilities/main.json");
        let mini_capability = include_str!("../capabilities/mini.json");
        let task_note_capability = include_str!("../capabilities/task-note.json");
        for command in [
            "github_connection_status",
            "github_save_client_id",
            "github_save_client_secret",
            "github_sign_in",
            "github_cancel_sign_in",
            "github_disconnect",
            "github_list_repositories",
            "github_list_commits",
            "github_contribution_calendar",
            "github_prepare_repository_creation",
            "github_confirm_repository_creation",
            "github_repository_creation_status",
            "github_resolve_repository_creation",
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
            assert!(!task_note_capability.contains(&permission));
        }
    }

    #[test]
    fn every_task_command_uses_the_main_window_guard() {
        let commands_source = include_str!("commands.rs");
        let runtime_source = include_str!("lib.rs");
        let build_manifest = include_str!("../build.rs");
        let main_capability = include_str!("../capabilities/main.json");
        let mini_capability = include_str!("../capabilities/mini.json");
        let task_note_capability = include_str!("../capabilities/task-note.json");
        for command in [
            "task_bootstrap",
            "task_list",
            "task_create_list",
            "task_rename_list",
            "task_list_delete_preview",
            "task_delete_list",
            "task_create",
            "task_update",
            "task_set_completed",
            "task_delete",
            "task_set_pinned",
        ] {
            let start = commands_source
                .find(&format!("fn {command}"))
                .unwrap_or_else(|| panic!("missing command {command}"));
            let remainder = &commands_source[start..];
            let end = remainder
                .find("#[tauri::command]")
                .unwrap_or(remainder.len());
            assert!(
                remainder[..end].contains("require_task_main_window(window.label())?"),
                "{command} must enforce the main-window guard"
            );
            assert!(runtime_source.contains(&format!("commands::{command}")));
            assert!(build_manifest.contains(&format!("\"{command}\"")));
            let permission = command.replace('_', "-");
            assert!(main_capability.contains(&format!("\"allow-{permission}\"")));
            assert!(!mini_capability.contains(&permission));
            assert!(!task_note_capability.contains(&permission));
        }
    }

    #[test]
    fn sticky_note_commands_use_the_dedicated_window_guard() {
        let commands_source = include_str!("commands.rs");
        let runtime_source = include_str!("lib.rs");
        let build_manifest = include_str!("../build.rs");
        let main_capability = include_str!("../capabilities/main.json");
        let mini_capability = include_str!("../capabilities/mini.json");
        let task_note_capability = include_str!("../capabilities/task-note.json");
        for command in [
            "task_note_bootstrap",
            "task_note_complete",
            "task_note_unpin",
        ] {
            let start = commands_source
                .find(&format!("fn {command}"))
                .unwrap_or_else(|| panic!("missing command {command}"));
            let remainder = &commands_source[start..];
            let end = remainder
                .find("#[tauri::command]")
                .unwrap_or(remainder.len());
            assert!(
                remainder[..end].contains("require_task_note_window(window.label())?"),
                "{command} must enforce the sticky-note window guard"
            );
            assert!(runtime_source.contains(&format!("commands::{command}")));
            assert!(build_manifest.contains(&format!("\"{command}\"")));
            let permission = command.replace('_', "-");
            assert!(task_note_capability.contains(&format!("\"allow-{permission}\"")));
            assert!(!main_capability.contains(&permission));
            assert!(!mini_capability.contains(&permission));
        }
    }
}
