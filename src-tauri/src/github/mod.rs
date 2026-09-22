pub mod auth;
pub mod connection_store;
pub mod creation_store;
pub mod models;
mod sign_in;

use std::{
    collections::HashMap,
    fmt,
    future::Future,
    sync::{
        atomic::{AtomicU64, AtomicU8, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{DateTime, Datelike, Duration as ChronoDuration, Utc};
use reqwest::{
    header::{self, HeaderMap},
    Client, StatusCode, Url,
};
use serde::{Deserialize, Serialize};

use crate::credentials::SecretStore;
use auth::TokenSet;
use connection_store::{GitHubConnectionStore, StoredConnection};
use creation_store::{RepositoryCreationStore, StoredCreationAttempt, StoredCreationState};
use models::{
    contribution_calendar_window, parse_commits, parse_contribution_calendar, parse_repositories,
    parse_repository, parse_user, validate_branch, validate_new_repository_name,
    validate_repository_identifier, BoundedPagination,
};
pub use models::{
    CommitSummary, ContributionCalendar, ContributionCalendarQuery, ContributionDay,
    ContributionWeek, GitHubAccount, RepositoryCreationAttemptState,
    RepositoryCreationAttemptStatus, RepositoryCreationInput, RepositoryCreationResolution,
    RepositoryCreationReview, RepositorySummary,
};

pub const DEFAULT_API_BASE_URL: &str = "https://api.github.com";
pub const DEFAULT_AUTH_BASE_URL: &str = "https://github.com";
pub const REFRESH_TOKEN_ACCOUNT: &str = "github_refresh_token";
pub const CLIENT_SECRET_ACCOUNT: &str = "github_app_client_secret";

const API_VERSION: &str = "2022-11-28";
const CONTRIBUTION_CALENDAR_QUERY: &str = r#"
query EllieContributionCalendar($login: String!, $from: DateTime, $to: DateTime) {
  user(login: $login) {
    contributionsCollection(from: $from, to: $to) {
      contributionCalendar {
        totalContributions
        weeks {
          firstDay
          contributionDays {
            contributionCount
            contributionLevel
            date
            weekday
          }
        }
      }
    }
  }
}
"#;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_API_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const DEFAULT_PAGE_SIZE: usize = 100;
const DEFAULT_MAX_PAGES: usize = 5;
const DEFAULT_MAX_ROWS: usize = 500;
const ACCESS_TOKEN_EXPIRY_MARGIN: Duration = Duration::from_secs(30);
pub(crate) const AUTHORIZATION_LIFETIME: Duration = Duration::from_secs(15 * 60);
const LOOPBACK_BIND_ATTEMPTS: usize = 3;
const REPOSITORY_REVIEW_LIFETIME: Duration = Duration::from_secs(10 * 60);
const MAX_REPOSITORY_DESCRIPTION_CHARS: usize = 350;
const REVIEW_ID_RANDOM_BYTES: usize = 24;

const STATE_DISCONNECTED: u8 = 0;
const STATE_AUTHORIZING: u8 = 1;
const STATE_CONNECTED: u8 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GitHubErrorCategory {
    WindowDenied,
    InvalidInput,
    Busy,
    AuthorizationStateMismatch,
    AuthorizationDenied,
    AppCredentialsInvalid,
    TokenExpirationRequired,
    TokenResponseInvalid,
    AccountResponseInvalid,
    AuthenticationRequired,
    AuthenticationExpired,
    RateLimited,
    PermissionDenied,
    NotFound,
    ValidationFailed,
    Conflict,
    CreationOutcomeUnknown,
    NetworkUnavailable,
    ProviderUnavailable,
    MalformedResponse,
    CredentialStore,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubError {
    category: GitHubErrorCategory,
}

impl GitHubError {
    pub(crate) const fn new(category: GitHubErrorCategory) -> Self {
        Self { category }
    }

    pub const fn category(self) -> GitHubErrorCategory {
        self.category
    }

    pub(crate) const fn window_denied() -> Self {
        Self::new(GitHubErrorCategory::WindowDenied)
    }

    pub(crate) const fn invalid_input() -> Self {
        Self::new(GitHubErrorCategory::InvalidInput)
    }

    const fn busy() -> Self {
        Self::new(GitHubErrorCategory::Busy)
    }

    const fn state_mismatch() -> Self {
        Self::new(GitHubErrorCategory::AuthorizationStateMismatch)
    }

    pub(crate) const fn app_credentials_invalid() -> Self {
        Self::new(GitHubErrorCategory::AppCredentialsInvalid)
    }

    pub(crate) const fn token_expiration_required() -> Self {
        Self::new(GitHubErrorCategory::TokenExpirationRequired)
    }

    pub(crate) const fn token_response_invalid() -> Self {
        Self::new(GitHubErrorCategory::TokenResponseInvalid)
    }

    const fn account_response_invalid() -> Self {
        Self::new(GitHubErrorCategory::AccountResponseInvalid)
    }

    const fn authentication_required() -> Self {
        Self::new(GitHubErrorCategory::AuthenticationRequired)
    }

    pub(crate) const fn authentication_expired() -> Self {
        Self::new(GitHubErrorCategory::AuthenticationExpired)
    }

    pub(crate) const fn rate_limited() -> Self {
        Self::new(GitHubErrorCategory::RateLimited)
    }

    pub(crate) const fn permission_denied() -> Self {
        Self::new(GitHubErrorCategory::PermissionDenied)
    }

    pub(crate) const fn not_found() -> Self {
        Self::new(GitHubErrorCategory::NotFound)
    }

    const fn validation_failed() -> Self {
        Self::new(GitHubErrorCategory::ValidationFailed)
    }

    const fn conflict() -> Self {
        Self::new(GitHubErrorCategory::Conflict)
    }

    const fn creation_outcome_unknown() -> Self {
        Self::new(GitHubErrorCategory::CreationOutcomeUnknown)
    }

    pub(crate) const fn network_unavailable() -> Self {
        Self::new(GitHubErrorCategory::NetworkUnavailable)
    }

    pub(crate) const fn provider_unavailable() -> Self {
        Self::new(GitHubErrorCategory::ProviderUnavailable)
    }

    pub(crate) const fn malformed_response() -> Self {
        Self::new(GitHubErrorCategory::MalformedResponse)
    }

    const fn credential_store() -> Self {
        Self::new(GitHubErrorCategory::CredentialStore)
    }

    const fn cancelled() -> Self {
        Self::new(GitHubErrorCategory::Cancelled)
    }
}

impl fmt::Display for GitHubError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.category {
            GitHubErrorCategory::WindowDenied => "the command is not available to this window",
            GitHubErrorCategory::InvalidInput => "the GitHub request input is invalid",
            GitHubErrorCategory::Busy => "a GitHub authorization is already in progress",
            GitHubErrorCategory::AuthorizationStateMismatch => {
                "the GitHub authorization response did not match the request"
            }
            GitHubErrorCategory::AuthorizationDenied => "GitHub authorization was denied",
            GitHubErrorCategory::AppCredentialsInvalid => {
                "the GitHub App client credentials were rejected"
            }
            GitHubErrorCategory::TokenExpirationRequired => {
                "GitHub did not return an expiring user token"
            }
            GitHubErrorCategory::TokenResponseInvalid => {
                "GitHub returned unsupported token metadata"
            }
            GitHubErrorCategory::AccountResponseInvalid => {
                "GitHub returned an unexpected account response"
            }
            GitHubErrorCategory::AuthenticationRequired => "GitHub authentication is required",
            GitHubErrorCategory::AuthenticationExpired => "GitHub authentication has expired",
            GitHubErrorCategory::RateLimited => "GitHub rate limited the request",
            GitHubErrorCategory::PermissionDenied => "GitHub denied permission for the request",
            GitHubErrorCategory::NotFound => "the requested GitHub resource was not found",
            GitHubErrorCategory::ValidationFailed => "GitHub rejected the request",
            GitHubErrorCategory::Conflict => "a repository with that name already exists",
            GitHubErrorCategory::CreationOutcomeUnknown => {
                "the repository creation outcome is unknown"
            }
            GitHubErrorCategory::NetworkUnavailable => "GitHub is not reachable",
            GitHubErrorCategory::ProviderUnavailable => "GitHub is temporarily unavailable",
            GitHubErrorCategory::MalformedResponse => "GitHub returned an unexpected response",
            GitHubErrorCategory::CredentialStore => {
                "the GitHub credential could not be stored securely"
            }
            GitHubErrorCategory::Cancelled => "the GitHub operation was cancelled",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for GitHubError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum GitHubConnectionState {
    Disconnected,
    Authorizing,
    Connected,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubConnectionStatus {
    pub state: GitHubConnectionState,
    pub account: Option<GitHubAccount>,
    pub last_error: Option<GitHubErrorCategory>,
    pub token_present: bool,
    pub client_id_configured: bool,
    pub client_secret_configured: bool,
}

#[derive(Default)]
struct ConnectionRecord {
    client_id: Option<String>,
    account: Option<GitHubAccount>,
    last_error: Option<GitHubErrorCategory>,
}

struct PendingAuthorization {
    client_id: String,
    redirect_uri: String,
    verifier: String,
    state: String,
    generation: u64,
    expires_at: Instant,
}

struct AccessSession {
    client_id: String,
    access_token: String,
    access_expires_at: Instant,
    refresh_expires_at: Instant,
    generation: u64,
}

#[derive(Clone)]
struct PendingRepositoryReview {
    review: RepositoryCreationReview,
    account: GitHubAccount,
    generation: u64,
    expires_at: Instant,
}

#[derive(Clone, Copy)]
struct ServiceLimits {
    page_size: usize,
    max_pages: usize,
    max_rows: usize,
}

impl Default for ServiceLimits {
    fn default() -> Self {
        Self {
            page_size: DEFAULT_PAGE_SIZE,
            max_pages: DEFAULT_MAX_PAGES,
            max_rows: DEFAULT_MAX_ROWS,
        }
    }
}

pub struct GitHubService {
    client: Client,
    api_base_url: String,
    auth_base_url: String,
    secret_store: Arc<dyn SecretStore>,
    connection_store: Arc<dyn GitHubConnectionStore>,
    creation_store: Arc<dyn RepositoryCreationStore>,
    connection_state: AtomicU8,
    generation: AtomicU64,
    connection: Mutex<ConnectionRecord>,
    pending: Mutex<Option<PendingAuthorization>>,
    access_session: tokio::sync::Mutex<Option<AccessSession>>,
    restore_lock: tokio::sync::Mutex<()>,
    authorization_now: Arc<dyn Fn() -> Instant + Send + Sync>,
    authorization_lifetime: Duration,
    authorization_changed: tokio::sync::Notify,
    repository_reviews: Mutex<HashMap<String, PendingRepositoryReview>>,
    repository_creation_gate: tokio::sync::Mutex<()>,
    request_timeout: Duration,
    limits: ServiceLimits,
    hud_commits: std::sync::Mutex<HudCommitCache>,
}

/// Bounded in-memory record of the repositories whose commit lists were
/// actually loaded by the main window. The HUD reads this shared cache only;
/// it never triggers network work or SQLite access itself.
#[derive(Default)]
struct HudCommitCache {
    /// Repository full name → (loaded commit count, attributed commit count).
    repos: std::collections::HashMap<String, (u64, u64)>,
    last_fetched_at: Option<DateTime<Utc>>,
}

impl HudCommitCache {
    fn record(&mut self, full_name: String, loaded: usize, attributed: usize, now: DateTime<Utc>) {
        self.repos
            .insert(full_name, (loaded as u64, attributed as u64));
        self.last_fetched_at = Some(now);
    }

    fn clear(&mut self) {
        self.repos.clear();
        self.last_fetched_at = None;
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HudCommitSummary {
    pub total_loaded: u64,
    pub attributed: u64,
    pub repositories_checked: usize,
    pub fetched_at: DateTime<Utc>,
    pub age_seconds: u64,
}

impl GitHubService {
    pub fn new(
        client: Client,
        api_base_url: impl Into<String>,
        secret_store: Arc<dyn SecretStore>,
        connection_store: Arc<dyn GitHubConnectionStore>,
        creation_store: Arc<dyn RepositoryCreationStore>,
    ) -> Self {
        Self::with_auth_base(
            client,
            api_base_url,
            DEFAULT_AUTH_BASE_URL,
            secret_store,
            connection_store,
            creation_store,
        )
    }

    fn with_auth_base(
        client: Client,
        api_base_url: impl Into<String>,
        auth_base_url: impl Into<String>,
        secret_store: Arc<dyn SecretStore>,
        connection_store: Arc<dyn GitHubConnectionStore>,
        creation_store: Arc<dyn RepositoryCreationStore>,
    ) -> Self {
        Self {
            client,
            api_base_url: api_base_url.into().trim_end_matches('/').to_string(),
            auth_base_url: auth_base_url.into().trim_end_matches('/').to_string(),
            secret_store,
            connection_store,
            creation_store,
            connection_state: AtomicU8::new(STATE_DISCONNECTED),
            generation: AtomicU64::new(0),
            connection: Mutex::new(ConnectionRecord::default()),
            pending: Mutex::new(None),
            access_session: tokio::sync::Mutex::new(None),
            restore_lock: tokio::sync::Mutex::new(()),
            authorization_now: Arc::new(Instant::now),
            authorization_lifetime: AUTHORIZATION_LIFETIME,
            authorization_changed: tokio::sync::Notify::new(),
            repository_reviews: Mutex::new(HashMap::new()),
            repository_creation_gate: tokio::sync::Mutex::new(()),
            request_timeout: REQUEST_TIMEOUT,
            limits: ServiceLimits::default(),
            hud_commits: std::sync::Mutex::new(HudCommitCache::default()),
        }
    }

    pub async fn connection_status(&self) -> Result<GitHubConnectionStatus, GitHubError> {
        self.restore_if_needed().await?;
        let token_present = self.read_refresh_token().await?.is_some();
        let client_secret_configured = self.read_client_secret().await?.is_some();
        let client_id_configured = self.load_connection().await?.is_some();
        let connection = self
            .connection
            .lock()
            .map_err(|_| GitHubError::provider_unavailable())?;
        Ok(GitHubConnectionStatus {
            state: self.state(),
            account: connection.account.clone(),
            last_error: connection.last_error,
            token_present,
            client_id_configured,
            client_secret_configured,
        })
    }

    /// Shared HUD projection: never performs network or database work. Returns
    /// `None` when no commit list has been loaded since connect.
    pub async fn hud_commit_summary(&self) -> Option<HudCommitSummary> {
        let cache = self.hud_commits.lock().ok()?;
        let fetched_at = cache.last_fetched_at?;
        let total_loaded = cache.repos.values().map(|(loaded, _)| *loaded).sum();
        let attributed = cache
            .repos
            .values()
            .map(|(_, attributed)| *attributed)
            .sum();
        let age_seconds = (Utc::now() - fetched_at).num_seconds().max(0) as u64;
        Some(HudCommitSummary {
            total_loaded,
            attributed,
            repositories_checked: cache.repos.len(),
            fetched_at,
            age_seconds,
        })
    }

    fn record_hud_commits(&self, owner: &str, repository: &str, rows: &[CommitSummary]) {
        let account_id = self
            .connection
            .lock()
            .ok()
            .and_then(|connection| connection.account.clone())
            .map(|account| account.id);
        let attributed = account_id
            .map(|id| rows.iter().filter(|row| row.author_id == Some(id)).count())
            .unwrap_or(0);
        if let Ok(mut cache) = self.hud_commits.lock() {
            cache.record(
                format!("{owner}/{repository}"),
                rows.len(),
                attributed,
                Utc::now(),
            );
        }
    }

    pub async fn restore_if_needed(&self) -> Result<(), GitHubError> {
        if self.state() != GitHubConnectionState::Disconnected
            || self
                .pending
                .lock()
                .map_err(|_| GitHubError::provider_unavailable())?
                .is_some()
            || self.access_session.lock().await.is_some()
        {
            return Ok(());
        }

        let _restore_guard = self.restore_lock.lock().await;
        if self.state() != GitHubConnectionState::Disconnected
            || self
                .pending
                .lock()
                .map_err(|_| GitHubError::provider_unavailable())?
                .is_some()
            || self.access_session.lock().await.is_some()
        {
            return Ok(());
        }

        let generation = self.generation.load(Ordering::SeqCst);
        if self.read_refresh_token().await?.is_none() || self.read_client_secret().await?.is_none()
        {
            return Ok(());
        }
        let Some(stored) = self.load_connection().await? else {
            return Ok(());
        };
        if self.generation.load(Ordering::SeqCst) != generation
            || self.state() != GitHubConnectionState::Disconnected
            || self
                .pending
                .lock()
                .map_err(|_| GitHubError::provider_unavailable())?
                .is_some()
        {
            return Ok(());
        }

        let now = Instant::now();
        let refresh_expires_at = now
            .checked_add(Duration::from_secs(auth::REFRESH_TOKEN_LIFETIME_SECONDS))
            .ok_or_else(GitHubError::malformed_response)?;
        *self.access_session.lock().await = Some(AccessSession {
            client_id: stored.client_id.clone(),
            access_token: String::new(),
            access_expires_at: now,
            refresh_expires_at,
            generation,
        });
        {
            let mut connection = self
                .connection
                .lock()
                .map_err(|_| GitHubError::provider_unavailable())?;
            *connection = ConnectionRecord {
                client_id: Some(stored.client_id),
                account: stored.account,
                last_error: None,
            };
        }
        if self
            .connection_state
            .compare_exchange(
                STATE_DISCONNECTED,
                STATE_CONNECTED,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_err()
            || self.generation.load(Ordering::SeqCst) != generation
        {
            let mut session = self.access_session.lock().await;
            if session
                .as_ref()
                .is_some_and(|session| session.generation == generation)
            {
                *session = None;
            }
            let _ = self.connection_state.compare_exchange(
                STATE_CONNECTED,
                STATE_DISCONNECTED,
                Ordering::SeqCst,
                Ordering::SeqCst,
            );
        }
        Ok(())
    }

    pub async fn save_client_id(
        &self,
        client_id: String,
    ) -> Result<GitHubConnectionStatus, GitHubError> {
        auth::validate_client_id(&client_id)?;
        let stored = self.load_connection().await?;
        let active_client_id = self
            .access_session
            .lock()
            .await
            .as_ref()
            .map(|session| session.client_id.clone());
        let token_present = self.read_refresh_token().await?.is_some();
        let client_secret_configured = self.read_client_secret().await?.is_some();
        let connection_account = self
            .connection
            .lock()
            .map_err(|_| GitHubError::provider_unavailable())?
            .account
            .clone();
        let has_connection = stored
            .as_ref()
            .and_then(|value| value.account.as_ref())
            .is_some()
            || active_client_id.is_some()
            || token_present
            || client_secret_configured
            || self.state() == GitHubConnectionState::Connected;
        let mismatched = stored
            .as_ref()
            .is_some_and(|value| value.client_id != client_id)
            || active_client_id
                .as_ref()
                .is_some_and(|active| active != &client_id);
        let reset_session = has_connection && mismatched;
        let account = if reset_session {
            None
        } else {
            connection_account.or_else(|| stored.and_then(|value| value.account))
        };
        self.save_connection(StoredConnection {
            client_id: client_id.clone(),
            account: account.clone(),
            updated_at: Utc::now(),
        })
        .await?;

        if reset_session {
            self.bump_generation();
            self.connection_state
                .store(STATE_DISCONNECTED, Ordering::SeqCst);
            if let Ok(mut pending) = self.pending.lock() {
                *pending = None;
            }
            *self.access_session.lock().await = None;
            if let Ok(mut connection) = self.connection.lock() {
                *connection = ConnectionRecord {
                    client_id: Some(client_id),
                    account: None,
                    last_error: None,
                };
            }
            let token_result = self.delete_refresh_token().await;
            let secret_result = self.delete_client_secret().await;
            let account_result = self.clear_stored_account().await;
            token_result?;
            secret_result?;
            account_result?;
        } else if let Ok(mut connection) = self.connection.lock() {
            connection.client_id = Some(client_id);
            connection.account = account;
            connection.last_error = None;
        }
        self.connection_status().await
    }

    pub async fn save_client_secret(
        &self,
        client_secret: String,
    ) -> Result<GitHubConnectionStatus, GitHubError> {
        auth::validate_client_secret(&client_secret)?;
        self.store_client_secret(client_secret).await?;
        if let Ok(mut connection) = self.connection.lock() {
            connection.last_error = None;
        }
        self.connection_status().await
    }

    pub async fn sign_in<F>(&self, open_url: F) -> Result<GitHubConnectionStatus, GitHubError>
    where
        F: FnOnce(&str) -> Result<(), GitHubError>,
    {
        let stored = match self.load_connection().await? {
            Some(stored) => stored,
            None => {
                let error = GitHubError::authentication_required();
                self.record_error(error.category());
                return Err(error);
            }
        };
        if self.read_client_secret().await?.is_none() {
            let error = GitHubError::authentication_required();
            self.record_error(error.category());
            return Err(error);
        }
        let listener = bind_loopback_listener().await?;
        let port = listener
            .local_addr()
            .map_err(|_| GitHubError::network_unavailable())?
            .port();
        let authorize_url = self.connect_start(stored.client_id, port)?;
        let generation = self.pending_generation()?;
        let deadline = tokio::time::Instant::now() + self.authorization_lifetime;

        if let Err(error) = open_url(&authorize_url) {
            self.abort_authorization(generation, error.category()).await;
            return Err(error);
        }

        let accepted = self
            .wait_for_pending_authorization(generation, deadline, listener.accept())
            .await;
        let (mut stream, peer) = match accepted {
            Ok(Ok(accepted)) => accepted,
            Ok(Err(_)) => {
                let error = GitHubError::network_unavailable();
                self.abort_authorization(generation, error.category()).await;
                return Err(error);
            }
            Err(error) => {
                self.abort_authorization(generation, error.category()).await;
                return Err(error);
            }
        };
        if !peer.ip().is_loopback() {
            let error = GitHubError::invalid_input();
            sign_in::respond(&mut stream, false).await;
            self.abort_authorization(generation, error.category()).await;
            return Err(error);
        }

        let callback = self
            .wait_for_pending_authorization(
                generation,
                deadline,
                sign_in::read_callback(&mut stream),
            )
            .await;
        let callback = match callback {
            Ok(Ok(callback)) => callback,
            Ok(Err(error)) | Err(error) => {
                sign_in::respond(&mut stream, false).await;
                self.abort_authorization(generation, error.category()).await;
                return Err(error);
            }
        };

        let completion = self.connect_complete(callback.code, callback.state);
        tokio::pin!(completion);
        let result = tokio::select! {
            biased;
            _ = self.wait_for_generation_change(generation) => Err(GitHubError::cancelled()),
            result = &mut completion => result,
        };
        sign_in::respond(&mut stream, result.is_ok()).await;
        if let Err(error) = result {
            self.abort_authorization(generation, error.category()).await;
            return Err(error);
        }
        result
    }

    pub async fn cancel_sign_in(&self) -> Result<GitHubConnectionStatus, GitHubError> {
        self.bump_generation();
        self.connection_state
            .store(STATE_DISCONNECTED, Ordering::SeqCst);
        if let Ok(mut pending) = self.pending.lock() {
            *pending = None;
        }
        *self.access_session.lock().await = None;

        let stored = self.load_connection().await?;
        if let Ok(mut connection) = self.connection.lock() {
            *connection = ConnectionRecord {
                client_id: stored.as_ref().map(|value| value.client_id.clone()),
                account: None,
                last_error: Some(GitHubErrorCategory::Cancelled),
            };
        }
        let token_present = self.read_refresh_token().await?.is_some();
        let client_secret_configured = self.read_client_secret().await?.is_some();
        Ok(GitHubConnectionStatus {
            state: GitHubConnectionState::Disconnected,
            account: None,
            last_error: Some(GitHubErrorCategory::Cancelled),
            token_present,
            client_id_configured: stored.is_some(),
            client_secret_configured,
        })
    }

    pub fn connect_start(
        &self,
        client_id: String,
        redirect_port: u16,
    ) -> Result<String, GitHubError> {
        self.reset_expired_pending()?;
        self.connection_state
            .compare_exchange(
                STATE_DISCONNECTED,
                STATE_AUTHORIZING,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .map_err(|_| GitHubError::busy())?;

        let result: Result<String, GitHubError> = (|| {
            let pkce = auth::generate_pkce()?;
            let state = auth::generate_state()?;
            let (authorize_url, redirect_uri) = auth::build_authorize_url(
                &self.auth_base_url,
                &client_id,
                redirect_port,
                &pkce.challenge,
                &state,
            )?;
            let generation = self.bump_generation();
            let expires_at = (self.authorization_now)()
                .checked_add(self.authorization_lifetime)
                .ok_or_else(GitHubError::provider_unavailable)?;
            let pending = PendingAuthorization {
                client_id,
                redirect_uri,
                verifier: pkce.verifier,
                state,
                generation,
                expires_at,
            };
            *self
                .pending
                .lock()
                .map_err(|_| GitHubError::provider_unavailable())? = Some(pending);
            let mut connection = self
                .connection
                .lock()
                .map_err(|_| GitHubError::provider_unavailable())?;
            *connection = ConnectionRecord::default();
            Ok(authorize_url)
        })();

        if let Err(error) = result {
            self.connection_state
                .store(STATE_DISCONNECTED, Ordering::SeqCst);
            self.record_error(error.category());
        }
        result
    }

    pub async fn connect_complete(
        &self,
        code: String,
        returned_state: String,
    ) -> Result<GitHubConnectionStatus, GitHubError> {
        let pending = {
            let mut pending_guard = self
                .pending
                .lock()
                .map_err(|_| GitHubError::provider_unavailable())?;
            let Some(pending) = pending_guard.as_ref() else {
                let error = GitHubError::authentication_required();
                self.record_error(error.category());
                return Err(error);
            };
            if pending.expires_at <= (self.authorization_now)() {
                *pending_guard = None;
                self.bump_generation();
                self.connection_state
                    .store(STATE_DISCONNECTED, Ordering::SeqCst);
                let error = GitHubError::cancelled();
                self.record_error(error.category());
                return Err(error);
            }
            if !auth::state_matches(&pending.state, &returned_state) {
                let error = GitHubError::state_mismatch();
                self.record_error(error.category());
                return Err(error);
            }
            pending_guard
                .take()
                .ok_or_else(GitHubError::authentication_required)?
        };

        let result = self.finish_connection(code, pending).await;
        if let Err(error) = result {
            self.connection_state
                .store(STATE_DISCONNECTED, Ordering::SeqCst);
            self.record_error(error.category());
        }
        result?;
        self.connection_status().await
    }

    async fn finish_connection(
        &self,
        code: String,
        pending: PendingAuthorization,
    ) -> Result<(), GitHubError> {
        self.ensure_generation(pending.generation)?;
        let client_secret = self
            .read_client_secret()
            .await?
            .ok_or_else(GitHubError::authentication_required)?;
        let tokens = auth::exchange_code(
            &self.client,
            &self.auth_base_url,
            &pending.client_id,
            &client_secret,
            &code,
            &pending.redirect_uri,
            &pending.verifier,
        )
        .await?;
        self.ensure_generation(pending.generation)?;
        let user = self
            .get_user_with_access_token(&tokens.access_token)
            .await?;
        self.ensure_generation(pending.generation)?;
        self.store_refresh_token(tokens.refresh_token.clone())
            .await?;

        if self.ensure_generation(pending.generation).is_err() {
            let _ = self.delete_refresh_token().await;
            return Err(GitHubError::cancelled());
        }

        let client_id = pending.client_id;
        let session = session_from_tokens(client_id.clone(), pending.generation, tokens)?;
        *self.access_session.lock().await = Some(session);
        {
            let mut connection = self
                .connection
                .lock()
                .map_err(|_| GitHubError::provider_unavailable())?;
            connection.client_id = Some(client_id.clone());
            connection.account = Some(user.clone());
            connection.last_error = None;
        }
        self.save_connection(StoredConnection {
            client_id,
            account: Some(user),
            updated_at: Utc::now(),
        })
        .await?;
        if self
            .connection_state
            .compare_exchange(
                STATE_AUTHORIZING,
                STATE_CONNECTED,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_err()
        {
            *self.access_session.lock().await = None;
            if let Ok(mut connection) = self.connection.lock() {
                connection.account = None;
            }
            let _ = self.delete_refresh_token().await;
            let _ = self.clear_stored_account().await;
            return Err(GitHubError::cancelled());
        }
        Ok(())
    }

    pub async fn disconnect(&self) -> Result<GitHubConnectionStatus, GitHubError> {
        self.bump_generation();
        self.connection_state
            .store(STATE_DISCONNECTED, Ordering::SeqCst);
        if let Ok(mut pending) = self.pending.lock() {
            *pending = None;
        }
        if let Ok(mut reviews) = self.repository_reviews.lock() {
            reviews.clear();
        }
        *self.access_session.lock().await = None;
        if let Ok(mut connection) = self.connection.lock() {
            *connection = ConnectionRecord::default();
        }
        if let Ok(mut hud) = self.hud_commits.lock() {
            hud.clear();
        }

        let token_result = self.delete_refresh_token().await;
        let secret_result = self.delete_client_secret().await;
        let account_result = self.clear_stored_account().await;
        if let Err(error) = token_result.and(secret_result).and(account_result) {
            self.record_error(error.category());
            return Err(error);
        }
        self.connection_status().await
    }

    pub async fn get_user(&self) -> Result<GitHubAccount, GitHubError> {
        let result = async {
            self.restore_if_needed().await?;
            let url = self.api_url("/user")?;
            let response = self.authorized_get(url).await?;
            parse_user(&response.body)
        }
        .await;
        self.record_result(&result);
        result
    }

    pub async fn list_repositories(&self) -> Result<Vec<RepositorySummary>, GitHubError> {
        let result = self.list_repositories_inner().await;
        self.record_result(&result);
        result
    }

    async fn list_repositories_inner(&self) -> Result<Vec<RepositorySummary>, GitHubError> {
        self.restore_if_needed().await?;
        let mut output = Vec::new();
        let mut pagination = BoundedPagination::new(self.limits.max_pages, self.limits.max_rows);
        loop {
            let mut url = self.api_url("/user/repos")?;
            url.query_pairs_mut()
                .append_pair("per_page", &self.limits.page_size.to_string())
                .append_pair("page", &pagination.current_page().to_string());
            let response = self.authorized_get(url).await?;
            let page = parse_repositories(&response.body)?;
            let has_next = response.has_next || page.len() == self.limits.page_size;
            let decision = pagination.accept_page(page.len(), has_next);
            output.extend(page.into_iter().take(decision.take_rows));
            if decision.next_page.is_none() {
                break;
            }
        }
        Ok(output)
    }

    pub async fn list_commits(
        &self,
        owner: &str,
        repository: &str,
        branch: Option<&str>,
    ) -> Result<Vec<CommitSummary>, GitHubError> {
        let result = self
            .list_commits_inner(owner, repository, branch, self.limits.max_rows)
            .await;
        self.record_result(&result);
        if let Ok(rows) = &result {
            self.record_hud_commits(owner, repository, rows);
        }
        result
    }

    pub async fn list_commits_limited(
        &self,
        owner: &str,
        repository: &str,
        branch: Option<&str>,
        max_rows: usize,
    ) -> Result<Vec<CommitSummary>, GitHubError> {
        let result = self
            .list_commits_inner(owner, repository, branch, max_rows)
            .await;
        self.record_result(&result);
        if let Ok(rows) = &result {
            self.record_hud_commits(owner, repository, rows);
        }
        result
    }

    async fn list_commits_inner(
        &self,
        owner: &str,
        repository: &str,
        branch: Option<&str>,
        max_rows: usize,
    ) -> Result<Vec<CommitSummary>, GitHubError> {
        self.restore_if_needed().await?;
        validate_repository_identifier(owner)?;
        validate_repository_identifier(repository)?;
        if let Some(branch) = branch {
            validate_branch(branch)?;
        }

        let max_rows = max_rows.clamp(1, self.limits.max_rows);
        let page_size = self.limits.page_size.min(max_rows);
        let mut output = Vec::new();
        let mut pagination = BoundedPagination::new(self.limits.max_pages, max_rows);
        loop {
            let mut url = self.repository_commits_url(owner, repository)?;
            let page_number = pagination.current_page().to_string();
            let page_size_query = page_size.to_string();
            {
                let mut query = url.query_pairs_mut();
                query
                    .append_pair("per_page", &page_size_query)
                    .append_pair("page", &page_number);
                if let Some(branch) = branch {
                    query.append_pair("sha", branch);
                }
            }

            let response = self.authorized_get(url).await?;
            let page = parse_commits(&response.body)?;
            let has_next = response.has_next || page.len() == page_size;
            let decision = pagination.accept_page(page.len(), has_next);
            output.extend(page.into_iter().take(decision.take_rows));
            if decision.next_page.is_none() {
                break;
            }
        }
        Ok(output)
    }

    pub async fn contribution_calendar(
        &self,
        query: ContributionCalendarQuery,
    ) -> Result<ContributionCalendar, GitHubError> {
        let result = self.contribution_calendar_inner(query).await;
        self.record_result(&result);
        result
    }

    async fn contribution_calendar_inner(
        &self,
        query: ContributionCalendarQuery,
    ) -> Result<ContributionCalendar, GitHubError> {
        self.restore_if_needed().await?;
        let login = self
            .connection
            .lock()
            .map_err(|_| GitHubError::provider_unavailable())?
            .account
            .as_ref()
            .map(|account| account.login.clone())
            .ok_or_else(GitHubError::authentication_required)?;
        let current_year = Utc::now().year();
        let window = contribution_calendar_window(query.year, current_year)?;
        let (from, to) = match window {
            Some((start, end)) => (
                Some(format!("{start}T00:00:00Z")),
                Some(format!("{end}T23:59:59Z")),
            ),
            None => (None, None),
        };
        let url = self.api_url("/graphql")?;
        let payload = GraphQlRequest {
            query: CONTRIBUTION_CALENDAR_QUERY,
            variables: GraphQlVariables {
                login: &login,
                from: from.as_deref(),
                to: to.as_deref(),
            },
        };
        let response = self.authorized_post_json(url, &payload).await?;
        parse_contribution_calendar(&response.body)
    }

    pub async fn prepare_repository_creation(
        &self,
        input: RepositoryCreationInput,
    ) -> Result<RepositoryCreationReview, GitHubError> {
        self.restore_if_needed().await?;
        if self.state() != GitHubConnectionState::Connected {
            return Err(GitHubError::authentication_required());
        }
        let input = validate_repository_creation_input(input)?;
        let generation = self.generation.load(Ordering::SeqCst);
        let account = self
            .connection
            .lock()
            .map_err(|_| GitHubError::provider_unavailable())?
            .account
            .clone()
            .ok_or_else(GitHubError::authentication_required)?;
        let review_id = generate_review_id()?;
        let expires_at_instant = (self.authorization_now)()
            .checked_add(REPOSITORY_REVIEW_LIFETIME)
            .ok_or_else(GitHubError::provider_unavailable)?;
        let review = RepositoryCreationReview {
            review_id: review_id.clone(),
            owner: account.login.clone(),
            name: input.name,
            description: input.description,
            private: input.private,
            initialize_readme: input.initialize_readme,
            expires_at: Utc::now()
                + ChronoDuration::from_std(REPOSITORY_REVIEW_LIFETIME)
                    .map_err(|_| GitHubError::provider_unavailable())?,
        };
        let mut reviews = self
            .repository_reviews
            .lock()
            .map_err(|_| GitHubError::provider_unavailable())?;
        let now = (self.authorization_now)();
        reviews.retain(|_, pending| pending.expires_at > now);
        reviews.insert(
            review_id,
            PendingRepositoryReview {
                review: review.clone(),
                account,
                generation,
                expires_at: expires_at_instant,
            },
        );
        Ok(review)
    }

    pub async fn confirm_repository_creation(
        &self,
        review_id: &str,
    ) -> Result<RepositorySummary, GitHubError> {
        creation_store::validate_attempt_id(review_id)?;
        let _gate = self
            .repository_creation_gate
            .try_lock()
            .map_err(|_| GitHubError::busy())?;
        let pending = self.take_repository_review(review_id)?;
        let now = Utc::now();
        let attempt = StoredCreationAttempt {
            id: review_id.to_string(),
            account: pending.account.clone(),
            repository_name: pending.review.name.clone(),
            state: StoredCreationState::Dispatching,
            created_at: now,
            updated_at: now,
        };
        self.begin_creation_attempt(attempt).await?;

        let result = self.send_repository_creation(&pending).await;
        match result {
            Ok(repository) => {
                // The remote side effect is already confirmed. A local cleanup
                // failure must never turn this into a retryable creation error.
                if self.remove_creation_attempt(review_id).await.is_err() {
                    tracing::warn!(event = "github_repository_creation_cleanup_failed");
                }
                self.record_result(&Ok::<(), GitHubError>(()));
                Ok(repository)
            }
            Err(error) if error.category() == GitHubErrorCategory::CreationOutcomeUnknown => {
                if self
                    .mark_creation_outcome_unknown(review_id, Utc::now())
                    .await
                    .is_err()
                {
                    tracing::warn!(event = "github_repository_creation_uncertainty_save_failed");
                }
                self.record_error(error.category());
                Err(error)
            }
            Err(error) => {
                self.remove_creation_attempt(review_id).await?;
                if error.category() == GitHubErrorCategory::AuthenticationExpired {
                    self.mark_authentication_expired().await;
                } else {
                    self.record_error(error.category());
                }
                Err(error)
            }
        }
    }

    pub async fn repository_creation_status(
        &self,
    ) -> Result<Vec<RepositoryCreationAttemptStatus>, GitHubError> {
        let attempts = self.load_creation_attempts().await?;
        Ok(attempts
            .into_iter()
            .map(|attempt| RepositoryCreationAttemptStatus {
                attempt_id: attempt.id,
                owner: attempt.account.login.clone(),
                name: attempt.repository_name.clone(),
                state: RepositoryCreationAttemptState::OutcomeUnknown,
                repository_url: format!(
                    "https://github.com/{}/{}",
                    attempt.account.login, attempt.repository_name
                ),
                created_at: attempt.created_at,
                updated_at: attempt.updated_at,
            })
            .collect())
    }

    pub async fn resolve_repository_creation(
        &self,
        attempt_id: &str,
        resolution: RepositoryCreationResolution,
    ) -> Result<(), GitHubError> {
        creation_store::validate_attempt_id(attempt_id)?;
        self.restore_if_needed().await?;
        if self.state() != GitHubConnectionState::Connected {
            return Err(GitHubError::authentication_required());
        }
        let account = self
            .connection
            .lock()
            .map_err(|_| GitHubError::provider_unavailable())?
            .account
            .clone()
            .ok_or_else(GitHubError::authentication_required)?;
        let attempts = self.load_creation_attempts().await?;
        let attempt = attempts
            .into_iter()
            .find(|attempt| attempt.id == attempt_id)
            .ok_or_else(GitHubError::not_found)?;
        if attempt.account.id != account.id {
            return Err(GitHubError::permission_denied());
        }
        self.remove_creation_attempt(attempt_id).await?;
        let outcome = match resolution {
            RepositoryCreationResolution::Exists => "exists",
            RepositoryCreationResolution::NotFound => "not_found",
        };
        tracing::info!(
            event = "github_repository_creation_resolved",
            resolution = outcome
        );
        Ok(())
    }

    fn take_repository_review(
        &self,
        review_id: &str,
    ) -> Result<PendingRepositoryReview, GitHubError> {
        let pending = self
            .repository_reviews
            .lock()
            .map_err(|_| GitHubError::provider_unavailable())?
            .remove(review_id)
            .ok_or_else(GitHubError::invalid_input)?;
        if pending.expires_at <= (self.authorization_now)() {
            return Err(GitHubError::invalid_input());
        }
        self.ensure_generation(pending.generation)?;
        if self.state() != GitHubConnectionState::Connected {
            return Err(GitHubError::authentication_required());
        }
        let account = self
            .connection
            .lock()
            .map_err(|_| GitHubError::provider_unavailable())?
            .account
            .clone()
            .ok_or_else(GitHubError::authentication_required)?;
        if account != pending.account {
            return Err(GitHubError::cancelled());
        }
        Ok(pending)
    }

    async fn send_repository_creation(
        &self,
        pending: &PendingRepositoryReview,
    ) -> Result<RepositorySummary, GitHubError> {
        self.ensure_generation(pending.generation)?;
        let access_token = self.access_token(false, pending.generation).await?;
        self.ensure_generation(pending.generation)?;
        let url = self.api_url("/user/repos")?;
        let payload = CreateRepositoryRequest {
            name: &pending.review.name,
            description: pending.review.description.as_deref(),
            private: pending.review.private,
            auto_init: pending.review.initialize_readme,
        };
        let response = self
            .client
            .post(url)
            .bearer_auth(access_token)
            .header(header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", API_VERSION)
            .json(&payload)
            .timeout(self.request_timeout)
            .send()
            .await
            .map_err(|error| {
                if error.is_connect() {
                    GitHubError::network_unavailable()
                } else {
                    GitHubError::creation_outcome_unknown()
                }
            })?;
        let status = response.status();
        let headers = response.headers().clone();
        if status != StatusCode::CREATED {
            let body = read_bounded(response, MAX_API_RESPONSE_BYTES)
                .await
                .unwrap_or_default();
            if status.is_success() {
                return Err(GitHubError::creation_outcome_unknown());
            }
            if status == StatusCode::UNPROCESSABLE_ENTITY || status == StatusCode::CONFLICT {
                return Err(GitHubError::conflict());
            }
            return Err(classify_api_error(status, &headers, &body));
        }
        let body = read_bounded(response, MAX_API_RESPONSE_BYTES)
            .await
            .map_err(|_| GitHubError::creation_outcome_unknown())?;
        let repository =
            parse_repository(&body).map_err(|_| GitHubError::creation_outcome_unknown())?;
        let expected_full_name = format!("{}/{}", pending.account.login, pending.review.name);
        if repository.name != pending.review.name || repository.full_name != expected_full_name {
            return Err(GitHubError::creation_outcome_unknown());
        }
        if self.ensure_generation(pending.generation).is_err() {
            return Err(GitHubError::creation_outcome_unknown());
        }
        Ok(repository)
    }

    async fn begin_creation_attempt(
        &self,
        attempt: StoredCreationAttempt,
    ) -> Result<(), GitHubError> {
        let store = Arc::clone(&self.creation_store);
        tokio::task::spawn_blocking(move || store.begin(&attempt))
            .await
            .map_err(|_| GitHubError::credential_store())?
    }

    async fn mark_creation_outcome_unknown(
        &self,
        id: &str,
        updated_at: chrono::DateTime<Utc>,
    ) -> Result<(), GitHubError> {
        let id = id.to_string();
        let store = Arc::clone(&self.creation_store);
        tokio::task::spawn_blocking(move || store.mark_outcome_unknown(&id, updated_at))
            .await
            .map_err(|_| GitHubError::credential_store())?
    }

    async fn remove_creation_attempt(&self, id: &str) -> Result<(), GitHubError> {
        let id = id.to_string();
        let store = Arc::clone(&self.creation_store);
        tokio::task::spawn_blocking(move || store.remove(&id))
            .await
            .map_err(|_| GitHubError::credential_store())?
    }

    async fn load_creation_attempts(&self) -> Result<Vec<StoredCreationAttempt>, GitHubError> {
        let store = Arc::clone(&self.creation_store);
        tokio::task::spawn_blocking(move || store.list_unresolved())
            .await
            .map_err(|_| GitHubError::credential_store())?
    }

    async fn get_user_with_access_token(
        &self,
        access_token: &str,
    ) -> Result<GitHubAccount, GitHubError> {
        let url = self.api_url("/user")?;
        let response = self.send_api_get(url, access_token).await?;
        ensure_api_success(&response)?;
        parse_user(&response.body).map_err(|_| GitHubError::account_response_invalid())
    }

    async fn authorized_get(&self, url: Url) -> Result<ApiResponse, GitHubError> {
        if self.state() != GitHubConnectionState::Connected {
            return Err(GitHubError::authentication_required());
        }
        let generation = self.generation.load(Ordering::SeqCst);
        let access_token = match self.access_token(false, generation).await {
            Ok(token) => token,
            Err(error) => {
                if is_authentication_failure(error.category()) {
                    self.mark_authentication_expired().await;
                }
                return Err(error);
            }
        };
        let mut response = self.send_api_get(url.clone(), &access_token).await?;
        if response.status == StatusCode::UNAUTHORIZED {
            let refreshed_token = match self.access_token(true, generation).await {
                Ok(token) => token,
                Err(error) => {
                    if is_authentication_failure(error.category()) {
                        self.mark_authentication_expired().await;
                    }
                    return Err(error);
                }
            };
            response = self.send_api_get(url, &refreshed_token).await?;
        }
        self.ensure_generation(generation)?;
        if let Err(error) = ensure_api_success(&response) {
            if error.category() == GitHubErrorCategory::AuthenticationExpired {
                self.mark_authentication_expired().await;
            }
            return Err(error);
        }
        Ok(response)
    }

    async fn authorized_post_json<T: Serialize + ?Sized>(
        &self,
        url: Url,
        payload: &T,
    ) -> Result<ApiResponse, GitHubError> {
        if self.state() != GitHubConnectionState::Connected {
            return Err(GitHubError::authentication_required());
        }
        let generation = self.generation.load(Ordering::SeqCst);
        let access_token = match self.access_token(false, generation).await {
            Ok(token) => token,
            Err(error) => {
                if is_authentication_failure(error.category()) {
                    self.mark_authentication_expired().await;
                }
                return Err(error);
            }
        };
        let mut response = self
            .send_api_post_json(url.clone(), &access_token, payload)
            .await?;
        if response.status == StatusCode::UNAUTHORIZED {
            let refreshed_token = match self.access_token(true, generation).await {
                Ok(token) => token,
                Err(error) => {
                    if is_authentication_failure(error.category()) {
                        self.mark_authentication_expired().await;
                    }
                    return Err(error);
                }
            };
            response = self
                .send_api_post_json(url, &refreshed_token, payload)
                .await?;
        }
        self.ensure_generation(generation)?;
        if let Err(error) = ensure_api_success(&response) {
            if error.category() == GitHubErrorCategory::AuthenticationExpired {
                self.mark_authentication_expired().await;
            }
            return Err(error);
        }
        Ok(response)
    }

    async fn access_token(
        &self,
        force_refresh: bool,
        generation: u64,
    ) -> Result<String, GitHubError> {
        let mut session_guard = self.access_session.lock().await;
        let session = session_guard
            .as_mut()
            .ok_or_else(GitHubError::authentication_required)?;
        if session.generation != generation {
            return Err(GitHubError::cancelled());
        }
        if !force_refresh
            && session
                .access_expires_at
                .checked_duration_since(Instant::now())
                .is_some_and(|remaining| remaining > ACCESS_TOKEN_EXPIRY_MARGIN)
        {
            return Ok(session.access_token.clone());
        }
        if session.refresh_expires_at <= Instant::now() {
            return Err(GitHubError::authentication_expired());
        }

        let refresh_token = self
            .read_refresh_token()
            .await?
            .ok_or_else(GitHubError::authentication_required)?;
        let client_secret = self
            .read_client_secret()
            .await?
            .ok_or_else(GitHubError::authentication_required)?;
        let tokens = auth::refresh_token(
            &self.client,
            &self.auth_base_url,
            &session.client_id,
            &client_secret,
            &refresh_token,
        )
        .await?;
        self.ensure_generation(generation)?;
        self.store_refresh_token(tokens.refresh_token.clone())
            .await?;
        if self.ensure_generation(generation).is_err() {
            let _ = self.delete_refresh_token().await;
            return Err(GitHubError::cancelled());
        }

        session.access_token = tokens.access_token;
        session.access_expires_at = instant_after(tokens.expires_in)?;
        session.refresh_expires_at = instant_after(tokens.refresh_token_expires_in)?;
        Ok(session.access_token.clone())
    }

    async fn send_api_get(&self, url: Url, access_token: &str) -> Result<ApiResponse, GitHubError> {
        let response = self
            .client
            .get(url)
            .bearer_auth(access_token)
            .header(header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", API_VERSION)
            .timeout(self.request_timeout)
            .send()
            .await
            .map_err(map_transport_error)?;
        let status = response.status();
        let headers = response.headers().clone();
        let has_next = headers
            .get(header::LINK)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.contains("rel=\"next\""));
        let body = read_bounded(response, MAX_API_RESPONSE_BYTES).await?;
        Ok(ApiResponse {
            status,
            headers,
            body,
            has_next,
        })
    }

    async fn send_api_post_json<T: Serialize + ?Sized>(
        &self,
        url: Url,
        access_token: &str,
        payload: &T,
    ) -> Result<ApiResponse, GitHubError> {
        let response = self
            .client
            .post(url)
            .bearer_auth(access_token)
            .header(header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", API_VERSION)
            .json(payload)
            .timeout(self.request_timeout)
            .send()
            .await
            .map_err(map_transport_error)?;
        let status = response.status();
        let headers = response.headers().clone();
        let body = read_bounded(response, MAX_API_RESPONSE_BYTES).await?;
        Ok(ApiResponse {
            status,
            headers,
            body,
            has_next: false,
        })
    }

    fn api_url(&self, path: &str) -> Result<Url, GitHubError> {
        Url::parse(&format!("{}{path}", self.api_base_url))
            .map_err(|_| GitHubError::invalid_input())
    }

    fn repository_commits_url(&self, owner: &str, repository: &str) -> Result<Url, GitHubError> {
        let mut url = self.api_url("")?;
        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| GitHubError::invalid_input())?;
            segments
                .pop_if_empty()
                .push("repos")
                .push(owner)
                .push(repository)
                .push("commits");
        }
        Ok(url)
    }

    fn reset_expired_pending(&self) -> Result<(), GitHubError> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| GitHubError::provider_unavailable())?;
        if pending
            .as_ref()
            .is_some_and(|pending| pending.expires_at <= (self.authorization_now)())
        {
            *pending = None;
            self.bump_generation();
            self.connection_state
                .store(STATE_DISCONNECTED, Ordering::SeqCst);
        }
        Ok(())
    }

    fn pending_generation(&self) -> Result<u64, GitHubError> {
        self.pending
            .lock()
            .map_err(|_| GitHubError::provider_unavailable())?
            .as_ref()
            .map(|pending| pending.generation)
            .ok_or_else(GitHubError::cancelled)
    }

    async fn wait_for_pending_authorization<F, T>(
        &self,
        generation: u64,
        deadline: tokio::time::Instant,
        future: F,
    ) -> Result<T, GitHubError>
    where
        F: Future<Output = T>,
    {
        tokio::pin!(future);
        loop {
            self.ensure_pending_authorization(generation)?;
            tokio::select! {
                biased;
                _ = self.authorization_changed.notified() => continue,
                _ = tokio::time::sleep_until(deadline) => return Err(GitHubError::cancelled()),
                output = &mut future => return Ok(output),
            }
        }
    }

    async fn wait_for_generation_change(&self, generation: u64) {
        loop {
            if self.generation.load(Ordering::SeqCst) != generation {
                return;
            }
            self.authorization_changed.notified().await;
        }
    }

    fn ensure_pending_authorization(&self, generation: u64) -> Result<(), GitHubError> {
        self.ensure_generation(generation)?;
        let is_current = self
            .pending
            .lock()
            .map_err(|_| GitHubError::provider_unavailable())?
            .as_ref()
            .is_some_and(|pending| pending.generation == generation);
        if self.state() != GitHubConnectionState::Authorizing || !is_current {
            return Err(GitHubError::cancelled());
        }
        Ok(())
    }

    async fn abort_authorization(&self, generation: u64, category: GitHubErrorCategory) {
        let removed = if let Ok(mut pending) = self.pending.lock() {
            if pending
                .as_ref()
                .is_some_and(|pending| pending.generation == generation)
            {
                *pending = None;
                true
            } else {
                false
            }
        } else {
            false
        };
        if removed {
            self.bump_generation();
            self.connection_state
                .store(STATE_DISCONNECTED, Ordering::SeqCst);
            *self.access_session.lock().await = None;
            self.record_error(category);
        }
    }

    fn bump_generation(&self) -> u64 {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.authorization_changed.notify_one();
        generation
    }

    fn state(&self) -> GitHubConnectionState {
        match self.connection_state.load(Ordering::SeqCst) {
            STATE_AUTHORIZING => GitHubConnectionState::Authorizing,
            STATE_CONNECTED => GitHubConnectionState::Connected,
            _ => GitHubConnectionState::Disconnected,
        }
    }

    fn ensure_generation(&self, generation: u64) -> Result<(), GitHubError> {
        if self.generation.load(Ordering::SeqCst) != generation {
            return Err(GitHubError::cancelled());
        }
        Ok(())
    }

    async fn mark_authentication_expired(&self) {
        self.bump_generation();
        self.connection_state
            .store(STATE_DISCONNECTED, Ordering::SeqCst);
        *self.access_session.lock().await = None;
        self.record_error(GitHubErrorCategory::AuthenticationExpired);
    }

    fn record_error(&self, category: GitHubErrorCategory) {
        if let Ok(mut connection) = self.connection.lock() {
            connection.last_error = Some(category);
        }
    }

    fn record_result<T>(&self, result: &Result<T, GitHubError>) {
        match result {
            Ok(_) => {
                if let Ok(mut connection) = self.connection.lock() {
                    connection.last_error = None;
                }
            }
            Err(error) => self.record_error(error.category()),
        }
    }

    async fn load_connection(&self) -> Result<Option<StoredConnection>, GitHubError> {
        let store = Arc::clone(&self.connection_store);
        tokio::task::spawn_blocking(move || store.load())
            .await
            .map_err(|_| GitHubError::credential_store())?
    }

    async fn save_connection(&self, connection: StoredConnection) -> Result<(), GitHubError> {
        let store = Arc::clone(&self.connection_store);
        tokio::task::spawn_blocking(move || store.save(&connection))
            .await
            .map_err(|_| GitHubError::credential_store())?
    }

    async fn clear_stored_account(&self) -> Result<(), GitHubError> {
        let store = Arc::clone(&self.connection_store);
        tokio::task::spawn_blocking(move || store.clear_account())
            .await
            .map_err(|_| GitHubError::credential_store())?
    }

    async fn read_refresh_token(&self) -> Result<Option<String>, GitHubError> {
        let store = Arc::clone(&self.secret_store);
        tokio::task::spawn_blocking(move || store.get(REFRESH_TOKEN_ACCOUNT))
            .await
            .map_err(|_| GitHubError::credential_store())?
            .map_err(|_| GitHubError::credential_store())
    }

    async fn store_refresh_token(&self, refresh_token: String) -> Result<(), GitHubError> {
        let store = Arc::clone(&self.secret_store);
        tokio::task::spawn_blocking(move || store.set(REFRESH_TOKEN_ACCOUNT, &refresh_token))
            .await
            .map_err(|_| GitHubError::credential_store())?
            .map_err(|_| GitHubError::credential_store())
    }

    async fn delete_refresh_token(&self) -> Result<(), GitHubError> {
        let store = Arc::clone(&self.secret_store);
        tokio::task::spawn_blocking(move || {
            if store.get(REFRESH_TOKEN_ACCOUNT)?.is_some() {
                store.delete(REFRESH_TOKEN_ACCOUNT)?;
            }
            Ok::<(), crate::error::AppError>(())
        })
        .await
        .map_err(|_| GitHubError::credential_store())?
        .map_err(|_| GitHubError::credential_store())
    }

    async fn read_client_secret(&self) -> Result<Option<String>, GitHubError> {
        let store = Arc::clone(&self.secret_store);
        tokio::task::spawn_blocking(move || store.get(CLIENT_SECRET_ACCOUNT))
            .await
            .map_err(|_| GitHubError::credential_store())?
            .map_err(|_| GitHubError::credential_store())
    }

    async fn store_client_secret(&self, client_secret: String) -> Result<(), GitHubError> {
        let store = Arc::clone(&self.secret_store);
        tokio::task::spawn_blocking(move || store.set(CLIENT_SECRET_ACCOUNT, &client_secret))
            .await
            .map_err(|_| GitHubError::credential_store())?
            .map_err(|_| GitHubError::credential_store())
    }

    async fn delete_client_secret(&self) -> Result<(), GitHubError> {
        let store = Arc::clone(&self.secret_store);
        tokio::task::spawn_blocking(move || {
            if store.get(CLIENT_SECRET_ACCOUNT)?.is_some() {
                store.delete(CLIENT_SECRET_ACCOUNT)?;
            }
            Ok::<(), crate::error::AppError>(())
        })
        .await
        .map_err(|_| GitHubError::credential_store())?
        .map_err(|_| GitHubError::credential_store())
    }
}

async fn bind_loopback_listener() -> Result<tokio::net::TcpListener, GitHubError> {
    for _ in 0..LOOPBACK_BIND_ATTEMPTS {
        if let Ok(listener) = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await {
            return Ok(listener);
        }
    }
    Err(GitHubError::network_unavailable())
}

#[derive(Serialize)]
struct GraphQlRequest<'a> {
    query: &'static str,
    variables: GraphQlVariables<'a>,
}

#[derive(Serialize)]
struct GraphQlVariables<'a> {
    login: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<&'a str>,
}

#[derive(Serialize)]
struct CreateRepositoryRequest<'a> {
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
    private: bool,
    auto_init: bool,
}

fn validate_repository_creation_input(
    input: RepositoryCreationInput,
) -> Result<RepositoryCreationInput, GitHubError> {
    let name = validate_new_repository_name(&input.name)?;
    let description = match input.description {
        None => None,
        Some(description) => {
            if description.chars().count() > MAX_REPOSITORY_DESCRIPTION_CHARS
                || description.chars().any(char::is_control)
            {
                return Err(GitHubError::invalid_input());
            }
            let description = description.trim();
            (!description.is_empty()).then(|| description.to_string())
        }
    };
    Ok(RepositoryCreationInput {
        name,
        description,
        private: input.private,
        initialize_readme: input.initialize_readme,
    })
}

fn generate_review_id() -> Result<String, GitHubError> {
    let mut random = [0_u8; REVIEW_ID_RANDOM_BYTES];
    getrandom::fill(&mut random).map_err(|_| GitHubError::provider_unavailable())?;
    Ok(URL_SAFE_NO_PAD.encode(random))
}

struct ApiResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
    has_next: bool,
}

fn session_from_tokens(
    client_id: String,
    generation: u64,
    tokens: TokenSet,
) -> Result<AccessSession, GitHubError> {
    Ok(AccessSession {
        client_id,
        access_token: tokens.access_token,
        access_expires_at: instant_after(tokens.expires_in)?,
        refresh_expires_at: instant_after(tokens.refresh_token_expires_in)?,
        generation,
    })
}

fn instant_after(seconds: u64) -> Result<Instant, GitHubError> {
    Instant::now()
        .checked_add(Duration::from_secs(seconds))
        .ok_or_else(GitHubError::malformed_response)
}

async fn read_bounded(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, GitHubError> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(GitHubError::malformed_response());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(map_transport_error)? {
        let next_len = body
            .len()
            .checked_add(chunk.len())
            .ok_or_else(GitHubError::malformed_response)?;
        if next_len > limit {
            return Err(GitHubError::malformed_response());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn ensure_api_success(response: &ApiResponse) -> Result<(), GitHubError> {
    if response.status.is_success() {
        return Ok(());
    }
    Err(classify_api_error(
        response.status,
        &response.headers,
        &response.body,
    ))
}

#[derive(Deserialize)]
struct ApiErrorBody {
    message: Option<String>,
}

fn classify_api_error(status: StatusCode, headers: &HeaderMap, body: &[u8]) -> GitHubError {
    let provider_message = serde_json::from_slice::<ApiErrorBody>(body)
        .ok()
        .and_then(|error| error.message)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let rate_limited = status == StatusCode::TOO_MANY_REQUESTS
        || headers.contains_key(header::RETRY_AFTER)
        || headers
            .get("x-ratelimit-remaining")
            .and_then(|value| value.to_str().ok())
            == Some("0")
        || provider_message.contains("rate limit");
    if rate_limited {
        return GitHubError::rate_limited();
    }
    match status.as_u16() {
        401 => GitHubError::authentication_expired(),
        403 => GitHubError::permission_denied(),
        404 => GitHubError::not_found(),
        422 => GitHubError::validation_failed(),
        400 => GitHubError::invalid_input(),
        500..=599 => GitHubError::provider_unavailable(),
        _ => GitHubError::provider_unavailable(),
    }
}

fn map_transport_error(error: reqwest::Error) -> GitHubError {
    if error.is_connect() || error.is_timeout() {
        GitHubError::network_unavailable()
    } else {
        GitHubError::provider_unavailable()
    }
}

fn is_authentication_failure(category: GitHubErrorCategory) -> bool {
    matches!(
        category,
        GitHubErrorCategory::AuthenticationRequired
            | GitHubErrorCategory::AuthenticationExpired
            | GitHubErrorCategory::AuthorizationDenied
            | GitHubErrorCategory::CredentialStore
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::{MemoryStore, SecretStore};
    use connection_store::MemoryGitHubConnectionStore;
    use creation_store::MemoryRepositoryCreationStore;
    use std::sync::{atomic::AtomicBool, Mutex as StdMutex};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        sync::oneshot,
        task::JoinHandle,
    };

    const TEST_CLIENT_SECRET: &str = "sanitized-test-client-secret";

    struct MockResponse {
        status: &'static str,
        headers: &'static str,
        body: String,
    }

    struct FailingSaveConnectionStore;

    #[derive(Default)]
    struct FailingRemoveCreationStore {
        inner: MemoryRepositoryCreationStore,
    }

    impl RepositoryCreationStore for FailingRemoveCreationStore {
        fn begin(&self, attempt: &StoredCreationAttempt) -> Result<(), GitHubError> {
            self.inner.begin(attempt)
        }

        fn mark_outcome_unknown(
            &self,
            id: &str,
            updated_at: chrono::DateTime<Utc>,
        ) -> Result<(), GitHubError> {
            self.inner.mark_outcome_unknown(id, updated_at)
        }

        fn remove(&self, _id: &str) -> Result<(), GitHubError> {
            Err(GitHubError::credential_store())
        }

        fn list_unresolved(&self) -> Result<Vec<StoredCreationAttempt>, GitHubError> {
            self.inner.list_unresolved()
        }
    }

    impl GitHubConnectionStore for FailingSaveConnectionStore {
        fn load(&self) -> Result<Option<StoredConnection>, GitHubError> {
            Ok(None)
        }

        fn save(&self, _connection: &StoredConnection) -> Result<(), GitHubError> {
            Err(GitHubError::credential_store())
        }

        fn clear_account(&self) -> Result<(), GitHubError> {
            Ok(())
        }
    }

    async fn serve_sequence(
        responses: Vec<MockResponse>,
    ) -> (String, Arc<StdMutex<Vec<String>>>, JoinHandle<()>) {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind mock server");
        let port = listener.local_addr().expect("address").port();
        let requests = Arc::new(StdMutex::new(Vec::new()));
        let captured = Arc::clone(&requests);
        let handle = tokio::spawn(async move {
            for response in responses {
                let (mut socket, _) = listener.accept().await.expect("accept request");
                let mut request = vec![0_u8; 16 * 1024];
                let read = socket.read(&mut request).await.expect("read request");
                captured
                    .lock()
                    .expect("captured requests")
                    .push(String::from_utf8_lossy(&request[..read]).into_owned());
                let wire = format!(
                    "HTTP/1.1 {}\r\nContent-Type: application/json\r\n{}Content-Length: {}\r\nConnection: close\r\n\r\n{}",
                    response.status,
                    response.headers,
                    response.body.len(),
                    response.body
                );
                socket.write_all(wire.as_bytes()).await.expect("response");
                let _ = socket.shutdown().await;
            }
        });
        (format!("http://127.0.0.1:{port}"), requests, handle)
    }

    async fn serve_hanging_request(
        hold: Duration,
    ) -> (String, Arc<StdMutex<Vec<String>>>, JoinHandle<()>) {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind hanging mock server");
        let port = listener.local_addr().expect("address").port();
        let requests = Arc::new(StdMutex::new(Vec::new()));
        let captured = Arc::clone(&requests);
        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept request");
            let mut request = vec![0_u8; 16 * 1024];
            let read = socket.read(&mut request).await.expect("read request");
            captured
                .lock()
                .expect("captured requests")
                .push(String::from_utf8_lossy(&request[..read]).into_owned());
            tokio::time::sleep(hold).await;
        });
        (format!("http://127.0.0.1:{port}"), requests, handle)
    }

    fn test_client() -> Client {
        Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("test client")
    }

    async fn request_loopback_callback(authorize_url: String, callback_path: &str) -> String {
        let authorize_url = Url::parse(&authorize_url).expect("authorize URL");
        let redirect_uri = authorize_url
            .query_pairs()
            .find(|(key, _)| key == "redirect_uri")
            .map(|(_, value)| value.into_owned())
            .expect("redirect URI");
        let state = authorize_url
            .query_pairs()
            .find(|(key, _)| key == "state")
            .map(|(_, value)| value.into_owned())
            .expect("state");
        let redirect = Url::parse(&redirect_uri).expect("redirect URL");
        let port = redirect.port().expect("redirect port");
        let target = format!(
            "{callback_path}?code=sanitized-code&iss=https%3A%2F%2Fgithub.com%2Flogin%2Foauth&state={state}"
        );
        let mut stream = TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("connect loopback callback");
        let request =
            format!("GET {target} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
        stream
            .write_all(request.as_bytes())
            .await
            .expect("write callback");
        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .await
            .expect("read callback response");
        String::from_utf8(response).expect("UTF-8 callback response")
    }

    fn test_service(
        base_url: &str,
        store: Arc<dyn SecretStore>,
        limits: ServiceLimits,
    ) -> GitHubService {
        test_service_with_connection(
            base_url,
            store,
            Arc::new(MemoryGitHubConnectionStore::default()),
            limits,
        )
    }

    fn test_service_with_connection(
        base_url: &str,
        store: Arc<dyn SecretStore>,
        connection_store: Arc<dyn GitHubConnectionStore>,
        limits: ServiceLimits,
    ) -> GitHubService {
        if store
            .get(CLIENT_SECRET_ACCOUNT)
            .expect("read test client secret")
            .is_none()
        {
            store
                .set(CLIENT_SECRET_ACCOUNT, TEST_CLIENT_SECRET)
                .expect("store test client secret");
        }
        let mut service = GitHubService::with_auth_base(
            test_client(),
            base_url,
            base_url,
            store,
            connection_store,
            Arc::new(MemoryRepositoryCreationStore::default()),
        );
        service.limits = limits;
        service
    }

    async fn mark_connected(service: &GitHubService, store: &dyn SecretStore) {
        let generation = service.generation.fetch_add(1, Ordering::SeqCst) + 1;
        store
            .set(REFRESH_TOKEN_ACCOUNT, "ghr_sanitized_refresh_value")
            .expect("refresh credential");
        *service.access_session.lock().await = Some(AccessSession {
            client_id: "Iv1.sanitized-client".to_string(),
            access_token: "ghu_sanitized_access_value".to_string(),
            access_expires_at: Instant::now() + Duration::from_secs(3_600),
            refresh_expires_at: Instant::now() + Duration::from_secs(86_400),
            generation,
        });
        let account = GitHubAccount {
            id: 42,
            login: "octo-cat".to_string(),
        };
        *service.connection.lock().expect("connection") = ConnectionRecord {
            client_id: Some("Iv1.sanitized-client".to_string()),
            account: Some(account.clone()),
            last_error: None,
        };
        service
            .save_connection(StoredConnection {
                client_id: "Iv1.sanitized-client".to_string(),
                account: Some(account),
                updated_at: Utc::now(),
            })
            .await
            .expect("stored connection");
        service
            .connection_state
            .store(STATE_CONNECTED, Ordering::SeqCst);
    }

    #[tokio::test]
    async fn orchestrated_sign_in_completes_through_a_real_loopback_callback() {
        let (base, requests, server) = serve_sequence(vec![
            MockResponse {
                status: "200 OK",
                headers: "",
                body: r#"{"access_token":"ghu_sanitized_access_value","expires_in":28800,"refresh_token":"ghr_sanitized_refresh_value","refresh_token_expires_in":15897600,"token_type":"bearer"}"#.to_string(),
            },
            MockResponse {
                status: "200 OK",
                headers: "",
                body: r#"{"id":42,"login":"octo-cat"}"#.to_string(),
            },
        ])
        .await;
        let secrets = Arc::new(MemoryStore::default());
        let service = test_service(&base, secrets.clone(), ServiceLimits::default());
        service
            .save_client_id("Iv1.sanitized-client".to_string())
            .await
            .expect("configure client ID");
        let (response_tx, response_rx) = oneshot::channel();

        let status = service
            .sign_in(move |authorize_url| {
                let authorize_url = authorize_url.to_string();
                tokio::spawn(async move {
                    let response =
                        request_loopback_callback(authorize_url, sign_in::CALLBACK_PATH).await;
                    let _ = response_tx.send(response);
                });
                Ok(())
            })
            .await
            .expect("orchestrated sign-in");
        server.await.expect("mock server");
        let browser_response = response_rx.await.expect("browser response");

        assert!(browser_response.starts_with("HTTP/1.1 200 OK"));
        assert!(browser_response.contains("GitHub sign-in complete"));
        assert_eq!(status.state, GitHubConnectionState::Connected);
        assert_eq!(status.account.expect("account").login, "octo-cat");
        assert!(status.token_present);
        assert_eq!(
            secrets
                .get(REFRESH_TOKEN_ACCOUNT)
                .expect("stored refresh token")
                .as_deref(),
            Some("ghr_sanitized_refresh_value")
        );
        let requests = requests.lock().expect("requests");
        assert!(requests[0].contains("code=sanitized-code"));
        assert!(requests[0].contains("code_verifier="));
        assert!(requests[0].contains("client_secret=sanitized-test-client-secret"));
    }

    #[tokio::test]
    async fn orchestrated_sign_in_times_out_without_leaving_pending_state() {
        let mut service = test_service(
            "http://127.0.0.1:9",
            Arc::new(MemoryStore::default()),
            ServiceLimits::default(),
        );
        service.authorization_lifetime = Duration::from_millis(25);
        service
            .save_client_id("Iv1.sanitized-client".to_string())
            .await
            .expect("configure client ID");

        let error = service
            .sign_in(|_| Ok(()))
            .await
            .expect_err("authorization timeout");
        assert_eq!(error.category(), GitHubErrorCategory::Cancelled);
        assert_eq!(service.state(), GitHubConnectionState::Disconnected);
        assert!(service
            .pending
            .lock()
            .expect("pending authorization")
            .is_none());
        assert!(service.access_session.lock().await.is_none());
        let status = service.connection_status().await.expect("status");
        assert_eq!(status.state, GitHubConnectionState::Disconnected);
        assert_eq!(status.last_error, Some(GitHubErrorCategory::Cancelled));
        assert!(!status.token_present);
    }

    #[tokio::test]
    async fn cancelling_orchestrated_sign_in_aborts_the_wait_and_preserves_refresh_token() {
        let secrets = Arc::new(MemoryStore::default());
        let mut service = test_service(
            "http://127.0.0.1:9",
            secrets.clone(),
            ServiceLimits::default(),
        );
        service.authorization_lifetime = Duration::from_secs(5);
        service
            .save_client_id("Iv1.sanitized-client".to_string())
            .await
            .expect("configure client ID");
        secrets
            .set(REFRESH_TOKEN_ACCOUNT, "ghr_sanitized_existing_value")
            .expect("existing refresh token");
        let service = Arc::new(service);
        let (opened_tx, opened_rx) = oneshot::channel();
        let sign_in_service = Arc::clone(&service);
        let sign_in = tokio::spawn(async move {
            sign_in_service
                .sign_in(move |_| {
                    let _ = opened_tx.send(());
                    Ok(())
                })
                .await
        });
        opened_rx.await.expect("browser open callback");

        let cancelled = service.cancel_sign_in().await.expect("cancel sign-in");
        let error = tokio::time::timeout(Duration::from_secs(1), sign_in)
            .await
            .expect("sign-in aborts promptly")
            .expect("sign-in task")
            .expect_err("cancelled sign-in");
        assert_eq!(error.category(), GitHubErrorCategory::Cancelled);
        assert_eq!(cancelled.state, GitHubConnectionState::Disconnected);
        assert!(cancelled.token_present);
        assert!(service
            .pending
            .lock()
            .expect("pending authorization")
            .is_none());
        assert!(service.access_session.lock().await.is_none());
        assert_eq!(
            secrets
                .get(REFRESH_TOKEN_ACCOUNT)
                .expect("preserved refresh token")
                .as_deref(),
            Some("ghr_sanitized_existing_value")
        );
    }

    #[tokio::test]
    async fn orchestrated_sign_in_requires_a_configured_client_id() {
        let service = test_service(
            "http://127.0.0.1:9",
            Arc::new(MemoryStore::default()),
            ServiceLimits::default(),
        );
        let opened = Arc::new(AtomicBool::new(false));
        let opened_in_callback = Arc::clone(&opened);
        let error = service
            .sign_in(move |_| {
                opened_in_callback.store(true, Ordering::SeqCst);
                Ok(())
            })
            .await
            .expect_err("missing client ID");
        assert_eq!(
            error.category(),
            GitHubErrorCategory::AuthenticationRequired
        );
        assert!(!opened.load(Ordering::SeqCst));
        assert_eq!(service.state(), GitHubConnectionState::Disconnected);
    }

    #[tokio::test]
    async fn orchestrated_sign_in_requires_a_configured_client_secret() {
        let secrets = Arc::new(MemoryStore::default());
        let service = test_service(
            "http://127.0.0.1:9",
            secrets.clone(),
            ServiceLimits::default(),
        );
        service
            .save_client_id("Iv1.sanitized-client".to_string())
            .await
            .expect("configure client ID");
        secrets
            .delete(CLIENT_SECRET_ACCOUNT)
            .expect("remove test client secret");
        let opened = Arc::new(AtomicBool::new(false));
        let opened_in_callback = Arc::clone(&opened);
        let error = service
            .sign_in(move |_| {
                opened_in_callback.store(true, Ordering::SeqCst);
                Ok(())
            })
            .await
            .expect_err("missing client secret");
        assert_eq!(
            error.category(),
            GitHubErrorCategory::AuthenticationRequired
        );
        assert!(!opened.load(Ordering::SeqCst));
        let status = service.connection_status().await.expect("status");
        assert!(status.client_id_configured);
        assert!(!status.client_secret_configured);
        assert_eq!(service.state(), GitHubConnectionState::Disconnected);
    }

    #[tokio::test]
    async fn orchestrated_sign_in_rejects_the_wrong_callback_path() {
        let service = test_service(
            "http://127.0.0.1:9",
            Arc::new(MemoryStore::default()),
            ServiceLimits::default(),
        );
        service
            .save_client_id("Iv1.sanitized-client".to_string())
            .await
            .expect("configure client ID");
        let (response_tx, response_rx) = oneshot::channel();
        let error = service
            .sign_in(move |authorize_url| {
                let authorize_url = authorize_url.to_string();
                tokio::spawn(async move {
                    let response = request_loopback_callback(authorize_url, "/wrong").await;
                    let _ = response_tx.send(response);
                });
                Ok(())
            })
            .await
            .expect_err("wrong callback path");
        let browser_response = response_rx.await.expect("browser response");

        assert_eq!(error.category(), GitHubErrorCategory::InvalidInput);
        assert!(browser_response.starts_with("HTTP/1.1 400 Bad Request"));
        assert!(browser_response.contains("could not be completed"));
        assert_eq!(service.state(), GitHubConnectionState::Disconnected);
        assert!(service
            .pending
            .lock()
            .expect("pending authorization")
            .is_none());
    }

    #[tokio::test]
    async fn completes_pkce_connection_and_keeps_only_refresh_token_in_store() {
        let token_body = r#"{"access_token":"ghu_sanitized_access_value","expires_in":28800,"refresh_token":"ghr_sanitized_refresh_value","refresh_token_expires_in":15897600,"token_type":"bearer"}"#.to_string();
        let user_body = r#"{"id":42,"login":"octo-cat"}"#.to_string();
        let (base, requests, server) = serve_sequence(vec![
            MockResponse {
                status: "200 OK",
                headers: "",
                body: token_body,
            },
            MockResponse {
                status: "200 OK",
                headers: "",
                body: user_body,
            },
        ])
        .await;
        let store = Arc::new(MemoryStore::default());
        let service = test_service(&base, store.clone(), ServiceLimits::default());
        let authorize_url = service
            .connect_start("Iv1.sanitized-client".into(), 58210)
            .expect("connect start");
        let state = Url::parse(&authorize_url)
            .expect("authorize URL")
            .query_pairs()
            .find(|(key, _)| key == "state")
            .map(|(_, value)| value.into_owned())
            .expect("state");
        let status = service
            .connect_complete("sanitized-code".into(), state)
            .await
            .expect("complete connection");
        server.await.expect("mock server");

        assert_eq!(status.state, GitHubConnectionState::Connected);
        let serialized_status = serde_json::to_string(&status).expect("serialize status");
        for secret in [
            "ghu_sanitized_access_value",
            "ghr_sanitized_refresh_value",
            "sanitized-code",
        ] {
            assert!(!serialized_status.contains(secret));
        }
        assert_eq!(status.account.expect("account").login, "octo-cat");
        assert!(status.token_present);
        assert!(status.client_id_configured);
        let persisted = service
            .load_connection()
            .await
            .expect("load connection")
            .expect("persisted connection");
        assert_eq!(persisted.client_id, "Iv1.sanitized-client");
        assert_eq!(persisted.account.expect("persisted account").id, 42);
        assert_eq!(
            store
                .get(REFRESH_TOKEN_ACCOUNT)
                .expect("stored token")
                .as_deref(),
            Some("ghr_sanitized_refresh_value")
        );
        let requests = requests.lock().expect("requests");
        assert!(requests[0].starts_with("POST /login/oauth/access_token"));
        assert!(requests[0].contains("grant_type=authorization_code"));
        assert!(requests[0].contains("client_secret=sanitized-test-client-secret"));
        assert!(requests[1].starts_with("GET /user"));
        assert!(requests[1]
            .to_ascii_lowercase()
            .contains("authorization: bearer ghu_sanitized_access_value"));
    }

    #[tokio::test]
    async fn connection_persistence_failure_is_closed_but_keeps_the_retry_session() {
        let (base, _, server) = serve_sequence(vec![
            MockResponse {
                status: "200 OK",
                headers: "",
                body: r#"{"access_token":"ghu_sanitized_access_value","expires_in":28800,"refresh_token":"ghr_sanitized_refresh_value","refresh_token_expires_in":15897600,"token_type":"bearer"}"#.to_string(),
            },
            MockResponse {
                status: "200 OK",
                headers: "",
                body: r#"{"id":42,"login":"octo-cat"}"#.to_string(),
            },
        ])
        .await;
        let secrets = Arc::new(MemoryStore::default());
        let service = test_service_with_connection(
            &base,
            secrets.clone(),
            Arc::new(FailingSaveConnectionStore),
            ServiceLimits::default(),
        );
        let authorize_url = service
            .connect_start("Iv1.sanitized-client".to_string(), 58210)
            .expect("connect start");
        let returned_state = Url::parse(&authorize_url)
            .expect("authorize URL")
            .query_pairs()
            .find(|(key, _)| key == "state")
            .map(|(_, value)| value.into_owned())
            .expect("state");

        let error = service
            .connect_complete("sanitized-code".to_string(), returned_state)
            .await
            .expect_err("connection persistence failure");
        server.await.expect("mock server");
        assert_eq!(error.category(), GitHubErrorCategory::CredentialStore);
        assert_eq!(service.state(), GitHubConnectionState::Disconnected);
        assert!(service.access_session.lock().await.is_some());
        assert_eq!(
            service
                .connection
                .lock()
                .expect("in-memory connection")
                .account
                .as_ref()
                .map(|account| account.id),
            Some(42)
        );
        assert_eq!(
            secrets
                .get(REFRESH_TOKEN_ACCOUNT)
                .expect("refresh token retained")
                .as_deref(),
            Some("ghr_sanitized_refresh_value")
        );
    }

    #[tokio::test]
    async fn refreshes_expired_access_and_rotates_the_stored_refresh_token() {
        let token_body = r#"{"access_token":"ghu_rotated_access_value","expires_in":28800,"refresh_token":"ghr_rotated_refresh_value","refresh_token_expires_in":15897600,"token_type":"bearer"}"#.to_string();
        let user_body = r#"{"id":42,"login":"octo-cat"}"#.to_string();
        let (base, requests, server) = serve_sequence(vec![
            MockResponse {
                status: "200 OK",
                headers: "",
                body: token_body,
            },
            MockResponse {
                status: "200 OK",
                headers: "",
                body: user_body,
            },
        ])
        .await;
        let store = Arc::new(MemoryStore::default());
        let service = test_service(&base, store.clone(), ServiceLimits::default());
        mark_connected(&service, store.as_ref()).await;
        service
            .access_session
            .lock()
            .await
            .as_mut()
            .expect("access session")
            .access_expires_at = Instant::now();

        let user = service.get_user().await.expect("refreshed user request");
        server.await.expect("mock server");
        assert_eq!(user.login, "octo-cat");
        assert_eq!(
            store
                .get(REFRESH_TOKEN_ACCOUNT)
                .expect("rotated token")
                .as_deref(),
            Some("ghr_rotated_refresh_value")
        );
        let requests = requests.lock().expect("requests");
        assert!(requests[0].starts_with("POST /login/oauth/access_token"));
        assert!(requests[0].contains("grant_type=refresh_token"));
        assert!(requests[0].contains("refresh_token=ghr_sanitized_refresh_value"));
        assert!(requests[0].contains("client_secret=sanitized-test-client-secret"));
        assert!(requests[1]
            .to_ascii_lowercase()
            .contains("authorization: bearer ghu_rotated_access_value"));
    }

    #[tokio::test]
    async fn retries_one_unauthorized_api_response_after_refresh() {
        let (base, requests, server) = serve_sequence(vec![
            MockResponse {
                status: "401 Unauthorized",
                headers: "",
                body: r#"{"message":"Bad credentials"}"#.to_string(),
            },
            MockResponse {
                status: "200 OK",
                headers: "",
                body: r#"{"access_token":"ghu_retry_access_value","expires_in":28800,"refresh_token":"ghr_retry_refresh_value","refresh_token_expires_in":15897600,"token_type":"bearer"}"#.to_string(),
            },
            MockResponse {
                status: "200 OK",
                headers: "",
                body: r#"{"id":42,"login":"octo-cat"}"#.to_string(),
            },
        ])
        .await;
        let store = Arc::new(MemoryStore::default());
        let service = test_service(&base, store.clone(), ServiceLimits::default());
        mark_connected(&service, store.as_ref()).await;

        let user = service.get_user().await.expect("retry after refresh");
        server.await.expect("mock server");
        assert_eq!(user.id, 42);
        let requests = requests.lock().expect("requests");
        assert_eq!(requests.len(), 3);
        assert!(requests[0].starts_with("GET /user"));
        assert!(requests[1].starts_with("POST /login/oauth/access_token"));
        assert!(requests[2]
            .to_ascii_lowercase()
            .contains("authorization: bearer ghu_retry_access_value"));
    }

    #[tokio::test]
    async fn rejects_state_mismatch_before_any_exchange() {
        let store = Arc::new(MemoryStore::default());
        let service = test_service("http://127.0.0.1:9", store, ServiceLimits::default());
        service
            .connect_start("Iv1.sanitized-client".into(), 58210)
            .expect("connect start");
        let error = service
            .connect_complete("sanitized-code".into(), "wrong-state".into())
            .await
            .expect_err("state mismatch");
        assert_eq!(
            error.category(),
            GitHubErrorCategory::AuthorizationStateMismatch
        );
        assert_eq!(service.state(), GitHubConnectionState::Authorizing);
    }

    #[tokio::test]
    async fn restores_without_network_and_refreshes_on_the_first_api_call() {
        let (base, requests, server) = serve_sequence(vec![
            MockResponse {
                status: "200 OK",
                headers: "",
                body: r#"{"access_token":"ghu_restored_access_value","expires_in":28800,"refresh_token":"ghr_restored_refresh_value","refresh_token_expires_in":15897600,"token_type":"bearer"}"#.to_string(),
            },
            MockResponse {
                status: "200 OK",
                headers: "",
                body: r#"{"id":42,"login":"octo-cat"}"#.to_string(),
            },
        ])
        .await;
        let secrets = Arc::new(MemoryStore::default());
        secrets
            .set(REFRESH_TOKEN_ACCOUNT, "ghr_sanitized_refresh_value")
            .expect("refresh token");
        let connections = Arc::new(MemoryGitHubConnectionStore::default());
        connections
            .save(&StoredConnection {
                client_id: "Iv1.sanitized-client".to_string(),
                account: Some(GitHubAccount {
                    id: 42,
                    login: "octo-cat".to_string(),
                }),
                updated_at: Utc::now(),
            })
            .expect("stored connection");
        let service = test_service_with_connection(
            &base,
            secrets.clone(),
            connections,
            ServiceLimits::default(),
        );

        let status = service.connection_status().await.expect("restore status");
        assert_eq!(status.state, GitHubConnectionState::Connected);
        assert_eq!(status.account.expect("restored account").login, "octo-cat");
        assert!(status.client_id_configured);
        assert!(requests
            .lock()
            .expect("requests before API call")
            .is_empty());

        let account = service.get_user().await.expect("first restored API call");
        server.await.expect("mock server");
        assert_eq!(account.id, 42);
        let requests = requests.lock().expect("requests");
        assert!(requests[0].starts_with("POST /login/oauth/access_token"));
        assert!(requests[0].contains("grant_type=refresh_token"));
        assert!(requests[1].starts_with("GET /user"));
        assert_eq!(
            secrets
                .get(REFRESH_TOKEN_ACCOUNT)
                .expect("rotated refresh token")
                .as_deref(),
            Some("ghr_restored_refresh_value")
        );
    }

    #[tokio::test]
    async fn restore_requires_both_refresh_token_and_stored_client_id() {
        let connection = StoredConnection {
            client_id: "Iv1.sanitized-client".to_string(),
            account: Some(GitHubAccount {
                id: 42,
                login: "octo-cat".to_string(),
            }),
            updated_at: Utc::now(),
        };

        let no_token_connections = Arc::new(MemoryGitHubConnectionStore::default());
        no_token_connections
            .save(&connection)
            .expect("stored connection");
        let no_token = test_service_with_connection(
            "http://127.0.0.1:9",
            Arc::new(MemoryStore::default()),
            no_token_connections,
            ServiceLimits::default(),
        );
        let status = no_token.connection_status().await.expect("status");
        assert_eq!(status.state, GitHubConnectionState::Disconnected);
        assert!(status.client_id_configured);
        assert!(no_token.access_session.lock().await.is_none());

        let token_only_secrets = Arc::new(MemoryStore::default());
        token_only_secrets
            .set(REFRESH_TOKEN_ACCOUNT, "ghr_sanitized_refresh_value")
            .expect("refresh token");
        let token_only = test_service(
            "http://127.0.0.1:9",
            token_only_secrets,
            ServiceLimits::default(),
        );
        let status = token_only.connection_status().await.expect("status");
        assert_eq!(status.state, GitHubConnectionState::Disconnected);
        assert!(!status.client_id_configured);
        assert!(token_only.access_session.lock().await.is_none());
    }

    #[tokio::test]
    async fn saving_a_different_client_id_resets_the_connected_session() {
        let secrets = Arc::new(MemoryStore::default());
        let connections = Arc::new(MemoryGitHubConnectionStore::default());
        let service = test_service_with_connection(
            "http://127.0.0.1:9",
            secrets.clone(),
            connections.clone(),
            ServiceLimits::default(),
        );
        mark_connected(&service, secrets.as_ref()).await;

        let status = service
            .save_client_id("Iv1.sanitized-replacement".to_string())
            .await
            .expect("save replacement client id");
        assert_eq!(status.state, GitHubConnectionState::Disconnected);
        assert!(status.client_id_configured);
        assert!(!status.client_secret_configured);
        assert!(!status.token_present);
        assert_eq!(status.account, None);
        assert!(service.access_session.lock().await.is_none());
        assert_eq!(secrets.get(REFRESH_TOKEN_ACCOUNT).expect("token"), None);
        assert_eq!(secrets.get(CLIENT_SECRET_ACCOUNT).expect("secret"), None);
        let stored = connections
            .load()
            .expect("connection")
            .expect("stored client id");
        assert_eq!(stored.client_id, "Iv1.sanitized-replacement");
        assert_eq!(stored.account, None);
    }

    #[tokio::test]
    async fn status_reports_a_saved_client_id_without_a_connection() {
        let service = test_service(
            "http://127.0.0.1:9",
            Arc::new(MemoryStore::default()),
            ServiceLimits::default(),
        );
        let status = service
            .save_client_id("Iv1.sanitized-client".to_string())
            .await
            .expect("save client id");
        assert!(status.client_id_configured);
        assert_eq!(status.state, GitHubConnectionState::Disconnected);
        assert!(!status.token_present);
    }

    #[tokio::test]
    async fn expired_pending_authorization_is_replaced_on_start_and_cancelled_on_complete() {
        let now = Arc::new(StdMutex::new(Instant::now()));
        let mut service = test_service(
            "http://127.0.0.1:9",
            Arc::new(MemoryStore::default()),
            ServiceLimits::default(),
        );
        let clock = Arc::clone(&now);
        service.authorization_now = Arc::new(move || *clock.lock().expect("test clock"));

        service
            .connect_start("Iv1.sanitized-client".to_string(), 58210)
            .expect("first start");
        *now.lock().expect("test clock") += AUTHORIZATION_LIFETIME + Duration::from_secs(1);
        service
            .connect_start("Iv1.sanitized-client".to_string(), 58210)
            .expect("expired authorization is replaced");

        let returned_state = service
            .pending
            .lock()
            .expect("pending authorization")
            .as_ref()
            .expect("replacement authorization")
            .state
            .clone();
        *now.lock().expect("test clock") += AUTHORIZATION_LIFETIME + Duration::from_secs(1);
        let error = service
            .connect_complete("sanitized-code".to_string(), returned_state)
            .await
            .expect_err("expired completion");
        assert_eq!(error.category(), GitHubErrorCategory::Cancelled);
        assert_eq!(service.state(), GitHubConnectionState::Disconnected);
        assert!(service
            .pending
            .lock()
            .expect("pending authorization")
            .is_none());
    }

    #[tokio::test]
    async fn repository_pagination_obeys_page_and_row_bounds() {
        let repository = |id: u64, name: &str| {
            serde_json::json!({
                "id": id,
                "name": name,
                "full_name": format!("octo-cat/{name}"),
                "private": true,
                "default_branch": "main",
                "html_url": format!("https://github.com/octo-cat/{name}")
            })
        };
        let (base, requests, server) = serve_sequence(vec![
            MockResponse {
                status: "200 OK",
                headers: "Link: <https://api.github.com/user/repos?page=2>; rel=\"next\"\r\n",
                body: serde_json::to_string(&vec![repository(1, "one"), repository(2, "two")])
                    .expect("fixture"),
            },
            MockResponse {
                status: "200 OK",
                headers: "Link: <https://api.github.com/user/repos?page=3>; rel=\"next\"\r\n",
                body: serde_json::to_string(&vec![repository(3, "three"), repository(4, "four")])
                    .expect("fixture"),
            },
        ])
        .await;
        let store = Arc::new(MemoryStore::default());
        let service = test_service(
            &base,
            store.clone(),
            ServiceLimits {
                page_size: 2,
                max_pages: 2,
                max_rows: 3,
            },
        );
        mark_connected(&service, store.as_ref()).await;

        let repositories = service.list_repositories().await.expect("repositories");
        server.await.expect("mock server");
        assert_eq!(repositories.len(), 3);
        assert_eq!(repositories[2].name, "three");
        let requests = requests.lock().expect("requests");
        assert_eq!(requests.len(), 2);
        assert!(requests[0].starts_with("GET /user/repos?per_page=2&page=1"));
        assert!(requests[1].starts_with("GET /user/repos?per_page=2&page=2"));
    }

    #[tokio::test]
    async fn lists_default_or_explicit_branch_commits_without_user_filter() {
        let commit_body = r#"[{"sha":"0123456789abcdef0123456789abcdef01234567","commit":{"message":"A plain subject\nbody is not retained","author":{"date":"2026-01-02T03:04:05Z"},"committer":{"date":"2026-01-02T04:05:06Z"}},"author":{"id":42,"login":"octo-cat"}}]"#;
        for (branch, expected_query) in [
            (None, "per_page=100&page=1"),
            (Some("feature/workspace"), "sha=feature%2Fworkspace"),
        ] {
            let (base, requests, server) = serve_sequence(vec![MockResponse {
                status: "200 OK",
                headers: "",
                body: commit_body.to_string(),
            }])
            .await;
            let store = Arc::new(MemoryStore::default());
            let service = test_service(&base, store.clone(), ServiceLimits::default());
            mark_connected(&service, store.as_ref()).await;
            let commits = service
                .list_commits("octo-cat", "ellie", branch)
                .await
                .expect("commits");
            server.await.expect("mock server");
            assert_eq!(commits[0].subject, "A plain subject");
            let request = &requests.lock().expect("requests")[0];
            assert!(request.starts_with("GET /repos/octo-cat/ellie/commits?"));
            assert!(request.contains(expected_query));
            assert!(!request.contains("author="));
        }
    }

    #[tokio::test]
    async fn limits_commit_rows_and_request_page_size_for_overview_reads() {
        let commit = |sha: &str, subject: &str| {
            serde_json::json!({
                "sha": sha,
                "commit": {
                    "message": subject,
                    "author": { "date": "2026-01-02T03:04:05Z" },
                    "committer": { "date": "2026-01-02T04:05:06Z" }
                },
                "author": { "id": 42, "login": "octo-cat" }
            })
        };
        let body = serde_json::to_string(&vec![
            commit("1111111111111111111111111111111111111111", "First"),
            commit("2222222222222222222222222222222222222222", "Second"),
        ])
        .expect("fixture");
        let (base, requests, server) = serve_sequence(vec![MockResponse {
            status: "200 OK",
            headers: "",
            body,
        }])
        .await;
        let store = Arc::new(MemoryStore::default());
        let service = test_service(&base, store.clone(), ServiceLimits::default());
        mark_connected(&service, store.as_ref()).await;

        let commits = service
            .list_commits_limited("octo-cat", "ellie", None, 2)
            .await
            .expect("commits");
        server.await.expect("mock server");

        assert_eq!(commits.len(), 2);
        let request = &requests.lock().expect("requests")[0];
        assert!(request.starts_with("GET /repos/octo-cat/ellie/commits?"));
        assert!(request.contains("per_page=2"));
        assert!(request.contains("page=1"));
    }

    #[tokio::test]
    async fn exposes_a_shared_hud_commit_summary_without_extra_requests() {
        let body_with_two_attributed = r#"[{"sha":"0123456789abcdef0123456789abcdef01234567","commit":{"message":"Mine","author":{"date":"2026-01-02T03:04:05Z"},"committer":{"date":"2026-01-02T04:05:06Z"}},"author":{"id":42,"login":"octo-cat"}},{"sha":"1123456789abcdef0123456789abcdef01234567","commit":{"message":"Also mine","author":{"date":"2026-01-03T03:04:05Z"},"committer":{"date":"2026-01-03T04:05:06Z"}},"author":{"id":42,"login":"octo-cat"}}]"#;
        let body_with_one_foreign = r#"[{"sha":"2123456789abcdef0123456789abcdef01234567","commit":{"message":"Someone else","author":{"date":"2026-01-04T03:04:05Z"},"committer":{"date":"2026-01-04T04:05:06Z"}},"author":{"id":7,"login":"friend"}}]"#;
        let (base, requests, server) = serve_sequence(vec![
            MockResponse {
                status: "200 OK",
                headers: "",
                body: body_with_two_attributed.to_string(),
            },
            MockResponse {
                status: "200 OK",
                headers: "",
                body: body_with_one_foreign.to_string(),
            },
        ])
        .await;
        let store = Arc::new(MemoryStore::default());
        let service = test_service(&base, store.clone(), ServiceLimits::default());
        mark_connected(&service, store.as_ref()).await;

        assert!(service.hud_commit_summary().await.is_none());

        service
            .list_commits("octo-cat", "ellie", None)
            .await
            .expect("ellie commits");
        service
            .list_commits("octo-cat", "notes", None)
            .await
            .expect("notes commits");
        server.await.expect("mock server");

        let summary = service.hud_commit_summary().await.expect("cached summary");
        assert_eq!(summary.total_loaded, 3);
        assert_eq!(summary.attributed, 2);
        assert_eq!(summary.repositories_checked, 2);
        assert!(summary.age_seconds < 60);
        // The HUD projection is a cache read; it issues no further requests.
        let requests = requests.lock().expect("requests");
        assert_eq!(requests.len(), 2);
    }

    #[tokio::test]
    async fn loads_the_connected_accounts_profile_contribution_calendar() {
        let body = r#"{"data":{"user":{"contributionsCollection":{"contributionCalendar":{"totalContributions":8,"weeks":[{"firstDay":"2026-09-06","contributionDays":[{"contributionCount":3,"contributionLevel":"SECOND_QUARTILE","date":"2026-09-06","weekday":0},{"contributionCount":5,"contributionLevel":"FOURTH_QUARTILE","date":"2026-09-07","weekday":1}]}]}}}}}"#;
        let (base, requests, server) = serve_sequence(vec![MockResponse {
            status: "200 OK",
            headers: "",
            body: body.to_string(),
        }])
        .await;
        let store = Arc::new(MemoryStore::default());
        let service = test_service(&base, store.clone(), ServiceLimits::default());
        mark_connected(&service, store.as_ref()).await;

        let calendar = service
            .contribution_calendar(crate::github::models::ContributionCalendarQuery::default())
            .await
            .expect("contribution calendar");
        server.await.expect("mock server");

        assert_eq!(calendar.total_contributions, 8);
        assert_eq!(calendar.weeks[0].days[0].contribution_count, 3);
        assert_eq!(calendar.weeks[0].days[1].level, 4);
        let request = &requests.lock().expect("requests")[0];
        assert!(request.starts_with("POST /graphql HTTP/1.1"));
        assert!(request.contains("EllieContributionCalendar"));
        assert!(request.contains(r#""login":"octo-cat""#));
        assert!(!request.contains(r#"\"from\":\""#));
        assert!(!request.contains(r#"\"to\":\""#));
    }

    #[tokio::test]
    async fn passes_a_calendar_year_window_to_graphql_and_omits_it_by_default() {
        let body = r#"{"data":{"user":{"contributionsCollection":{"contributionCalendar":{"totalContributions":2,"weeks":[{"firstDay":"2025-01-01","contributionDays":[{"contributionCount":2,"contributionLevel":"FIRST_QUARTILE","date":"2025-01-01","weekday":3}]}]}}}}}"#;
        let y = |query: crate::github::models::ContributionCalendarQuery| async move {
            let (base, requests, server) = serve_sequence(vec![MockResponse {
                status: "200 OK",
                headers: "",
                body: body.to_string(),
            }])
            .await;
            let store = Arc::new(MemoryStore::default());
            let service = test_service(&base, store.clone(), ServiceLimits::default());
            mark_connected(&service, store.as_ref()).await;
            service
                .contribution_calendar(query)
                .await
                .expect("contribution calendar");
            server.await.expect("mock server");
            let request = &requests.lock().expect("requests")[0];
            (
                request.contains(r#""from":"2025-01-01T00:00:00Z""#),
                request.contains(r#""to":"2025-12-31T23:59:59Z""#),
            )
        };
        let with_year =
            y(crate::github::models::ContributionCalendarQuery { year: Some(2025) }).await;
        assert!(with_year.0, "from boundary present");
        assert!(with_year.1, "to boundary present");
    }

    #[tokio::test]
    async fn endpoint_errors_map_to_redacted_categories() {
        for (status, headers, body, expected) in [
            (
                "401 Unauthorized",
                "",
                r#"{"message":"Bad credentials"}"#,
                GitHubErrorCategory::AuthenticationExpired,
            ),
            (
                "403 Forbidden",
                "",
                r#"{"message":"Resource not accessible by integration"}"#,
                GitHubErrorCategory::PermissionDenied,
            ),
            (
                "403 Forbidden",
                "",
                r#"{"message":"You have exceeded a secondary rate limit"}"#,
                GitHubErrorCategory::RateLimited,
            ),
            (
                "404 Not Found",
                "",
                r#"{"message":"Not Found"}"#,
                GitHubErrorCategory::NotFound,
            ),
            (
                "422 Unprocessable Entity",
                "",
                r#"{"message":"Validation Failed"}"#,
                GitHubErrorCategory::ValidationFailed,
            ),
        ] {
            let (base, _, server) = serve_sequence(vec![MockResponse {
                status,
                headers,
                body: body.to_string(),
            }])
            .await;
            let service = test_service(
                &base,
                Arc::new(MemoryStore::default()),
                ServiceLimits::default(),
            );
            let error = service
                .get_user_with_access_token("ghu_sanitized_access_value")
                .await
                .expect_err("endpoint error");
            server.await.expect("mock server");
            assert_eq!(error.category(), expected);
            assert!(!error.to_string().contains(body));
        }
    }

    #[tokio::test]
    async fn malformed_and_missing_endpoint_fields_are_rejected() {
        for body in [r#"{"id":42}"#, r#"{"id":"forty-two","login":"octo-cat"}"#] {
            let (base, _, server) = serve_sequence(vec![MockResponse {
                status: "200 OK",
                headers: "",
                body: body.to_string(),
            }])
            .await;
            let service = test_service(
                &base,
                Arc::new(MemoryStore::default()),
                ServiceLimits::default(),
            );
            let error = service
                .get_user_with_access_token("ghu_sanitized_access_value")
                .await
                .expect_err("malformed user");
            server.await.expect("mock server");
            assert_eq!(
                error.category(),
                GitHubErrorCategory::AccountResponseInvalid
            );
        }
    }

    fn repository_creation_input() -> RepositoryCreationInput {
        RepositoryCreationInput {
            name: "ellie-workspace".to_string(),
            description: Some("Private workspace repository".to_string()),
            private: true,
            initialize_readme: true,
        }
    }

    #[tokio::test]
    async fn repository_creation_uses_reviewed_personal_request_once() {
        let repository = serde_json::json!({
            "id": 99,
            "name": "ellie-workspace",
            "full_name": "octo-cat/ellie-workspace",
            "private": true,
            "default_branch": "main",
            "html_url": "https://github.com/octo-cat/ellie-workspace"
        });
        let (base, requests, server) = serve_sequence(vec![MockResponse {
            status: "201 Created",
            headers: "",
            body: repository.to_string(),
        }])
        .await;
        let store = Arc::new(MemoryStore::default());
        let service = test_service(&base, store.clone(), ServiceLimits::default());
        mark_connected(&service, store.as_ref()).await;

        let review = service
            .prepare_repository_creation(repository_creation_input())
            .await
            .expect("prepare repository");
        assert_eq!(review.owner, "octo-cat");
        assert!(review.private);
        assert!(review.initialize_readme);
        let created = service
            .confirm_repository_creation(&review.review_id)
            .await
            .expect("create repository");
        server.await.expect("mock server");
        assert_eq!(created.id, 99);
        assert!(service
            .repository_creation_status()
            .await
            .expect("creation status")
            .is_empty());
        assert_eq!(
            service
                .confirm_repository_creation(&review.review_id)
                .await
                .expect_err("review is single-use")
                .category(),
            GitHubErrorCategory::InvalidInput
        );
        let request = &requests.lock().expect("requests")[0];
        assert!(request.starts_with("POST /user/repos HTTP/1.1"));
        assert!(request.contains("authorization: Bearer ghu_sanitized_access_value"));
        assert!(request.contains("\"name\":\"ellie-workspace\""));
        assert!(request.contains("\"description\":\"Private workspace repository\""));
        assert!(request.contains("\"private\":true"));
        assert!(request.contains("\"auto_init\":true"));
    }

    #[tokio::test]
    async fn confirmed_creation_stays_successful_when_local_cleanup_fails() {
        let repository = serde_json::json!({
            "id": 99,
            "name": "ellie-workspace",
            "full_name": "octo-cat/ellie-workspace",
            "private": true,
            "default_branch": "main",
            "html_url": "https://github.com/octo-cat/ellie-workspace"
        });
        let (base, _, server) = serve_sequence(vec![MockResponse {
            status: "201 Created",
            headers: "",
            body: repository.to_string(),
        }])
        .await;
        let store = Arc::new(MemoryStore::default());
        store
            .set(CLIENT_SECRET_ACCOUNT, TEST_CLIENT_SECRET)
            .expect("store client secret");
        let service = GitHubService::with_auth_base(
            test_client(),
            &base,
            &base,
            store.clone(),
            Arc::new(MemoryGitHubConnectionStore::default()),
            Arc::new(FailingRemoveCreationStore::default()),
        );
        mark_connected(&service, store.as_ref()).await;
        let review = service
            .prepare_repository_creation(repository_creation_input())
            .await
            .expect("prepare repository");

        let created = service
            .confirm_repository_creation(&review.review_id)
            .await
            .expect("remote success must remain success");

        server.await.expect("mock server");
        assert_eq!(created.id, 99);
        assert_eq!(
            service
                .repository_creation_status()
                .await
                .expect("cleanup warning remains resolvable")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn repository_review_expires_and_is_bound_to_the_prepared_account() {
        let now = Arc::new(StdMutex::new(Instant::now()));
        let store = Arc::new(MemoryStore::default());
        let mut service = test_service(
            "http://127.0.0.1:9",
            store.clone(),
            ServiceLimits::default(),
        );
        let clock = Arc::clone(&now);
        service.authorization_now = Arc::new(move || *clock.lock().expect("test clock"));
        mark_connected(&service, store.as_ref()).await;
        let review = service
            .prepare_repository_creation(repository_creation_input())
            .await
            .expect("prepare expiring review");
        *now.lock().expect("clock") += REPOSITORY_REVIEW_LIFETIME + Duration::from_secs(1);
        assert_eq!(
            service
                .confirm_repository_creation(&review.review_id)
                .await
                .expect_err("expired review")
                .category(),
            GitHubErrorCategory::InvalidInput
        );

        *now.lock().expect("clock") = Instant::now();
        let review = service
            .prepare_repository_creation(repository_creation_input())
            .await
            .expect("prepare account-bound review");
        service.connection.lock().expect("connection").account = Some(GitHubAccount {
            id: 7,
            login: "different-account".to_string(),
        });
        assert_eq!(
            service
                .confirm_repository_creation(&review.review_id)
                .await
                .expect_err("account changed")
                .category(),
            GitHubErrorCategory::Cancelled
        );
    }

    #[tokio::test]
    async fn repository_creation_gate_does_not_consume_a_busy_review() {
        let repository = serde_json::json!({
            "id": 99,
            "name": "ellie-workspace",
            "full_name": "octo-cat/ellie-workspace",
            "private": true,
            "default_branch": "main",
            "html_url": "https://github.com/octo-cat/ellie-workspace"
        });
        let (base, _, server) = serve_sequence(vec![MockResponse {
            status: "201 Created",
            headers: "",
            body: repository.to_string(),
        }])
        .await;
        let store = Arc::new(MemoryStore::default());
        let service = test_service(&base, store.clone(), ServiceLimits::default());
        mark_connected(&service, store.as_ref()).await;
        let review = service
            .prepare_repository_creation(repository_creation_input())
            .await
            .expect("prepare");
        let guard = service.repository_creation_gate.lock().await;
        assert_eq!(
            service
                .confirm_repository_creation(&review.review_id)
                .await
                .expect_err("busy")
                .category(),
            GitHubErrorCategory::Busy
        );
        drop(guard);
        service
            .confirm_repository_creation(&review.review_id)
            .await
            .expect("review remains usable");
        server.await.expect("mock server");
    }

    #[tokio::test]
    async fn repository_creation_errors_are_redacted_and_classified() {
        for (status, headers, expected) in [
            (
                "422 Unprocessable Entity",
                "",
                GitHubErrorCategory::Conflict,
            ),
            (
                "403 Forbidden",
                "X-RateLimit-Remaining: 0\r\n",
                GitHubErrorCategory::RateLimited,
            ),
            ("403 Forbidden", "", GitHubErrorCategory::PermissionDenied),
            (
                "401 Unauthorized",
                "",
                GitHubErrorCategory::AuthenticationExpired,
            ),
        ] {
            let secret_body = r#"{"message":"secret provider detail token-value"}"#;
            let (base, _, server) = serve_sequence(vec![MockResponse {
                status,
                headers,
                body: secret_body.to_string(),
            }])
            .await;
            let store = Arc::new(MemoryStore::default());
            let service = test_service(&base, store.clone(), ServiceLimits::default());
            mark_connected(&service, store.as_ref()).await;
            let review = service
                .prepare_repository_creation(repository_creation_input())
                .await
                .expect("prepare");
            let error = service
                .confirm_repository_creation(&review.review_id)
                .await
                .expect_err("creation error");
            server.await.expect("mock server");
            assert_eq!(error.category(), expected);
            let serialized = serde_json::to_string(&error).expect("redacted error");
            assert!(!serialized.contains("secret provider detail"));
            assert!(service
                .repository_creation_status()
                .await
                .expect("status")
                .is_empty());
        }
    }

    #[tokio::test]
    async fn timed_out_repository_creation_is_persisted_as_outcome_unknown() {
        let (base, requests, server) = serve_hanging_request(Duration::from_millis(150)).await;
        let store = Arc::new(MemoryStore::default());
        let mut service = test_service(&base, store.clone(), ServiceLimits::default());
        service.request_timeout = Duration::from_millis(25);
        mark_connected(&service, store.as_ref()).await;
        let review = service
            .prepare_repository_creation(repository_creation_input())
            .await
            .expect("prepare");
        let error = service
            .confirm_repository_creation(&review.review_id)
            .await
            .expect_err("timeout is uncertain");
        assert_eq!(
            error.category(),
            GitHubErrorCategory::CreationOutcomeUnknown
        );
        let unresolved = service
            .repository_creation_status()
            .await
            .expect("unresolved status");
        assert_eq!(unresolved.len(), 1);
        assert_eq!(unresolved[0].attempt_id, review.review_id);
        assert_eq!(unresolved[0].owner, "octo-cat");
        assert_eq!(
            unresolved[0].state,
            RepositoryCreationAttemptState::OutcomeUnknown
        );
        server.await.expect("hanging server");
        assert_eq!(requests.lock().expect("requests").len(), 1);
    }

    #[tokio::test]
    async fn uncertain_creation_requires_matching_account_resolution() {
        let (base, _, server) = serve_hanging_request(Duration::from_millis(150)).await;
        let store = Arc::new(MemoryStore::default());
        let mut service = test_service(&base, store.clone(), ServiceLimits::default());
        service.request_timeout = Duration::from_millis(25);
        mark_connected(&service, store.as_ref()).await;
        let review = service
            .prepare_repository_creation(repository_creation_input())
            .await
            .expect("prepare");
        service
            .confirm_repository_creation(&review.review_id)
            .await
            .expect_err("timeout is uncertain");
        server.await.expect("hanging server");

        service.connection.lock().expect("connection").account = Some(GitHubAccount {
            id: 7,
            login: "different-account".to_string(),
        });
        assert_eq!(
            service
                .resolve_repository_creation(
                    &review.review_id,
                    RepositoryCreationResolution::NotFound,
                )
                .await
                .expect_err("different account cannot resolve")
                .category(),
            GitHubErrorCategory::PermissionDenied
        );
        service.connection.lock().expect("connection").account = Some(GitHubAccount {
            id: 42,
            login: "octo-cat".to_string(),
        });
        service
            .resolve_repository_creation(&review.review_id, RepositoryCreationResolution::Exists)
            .await
            .expect("matching account resolves locally");
        assert!(service
            .repository_creation_status()
            .await
            .expect("status")
            .is_empty());
    }

    #[tokio::test]
    async fn credential_round_trip_disconnect_and_absence_are_safe() {
        let store = Arc::new(MemoryStore::default());
        assert_eq!(store.get(REFRESH_TOKEN_ACCOUNT).expect("absence"), None);
        store
            .set(REFRESH_TOKEN_ACCOUNT, "ghr_sanitized_refresh_value")
            .expect("store refresh token");
        assert_eq!(
            store
                .get(REFRESH_TOKEN_ACCOUNT)
                .expect("round trip")
                .as_deref(),
            Some("ghr_sanitized_refresh_value")
        );

        let connections = Arc::new(MemoryGitHubConnectionStore::default());
        let service = test_service_with_connection(
            "http://127.0.0.1:9",
            store.clone(),
            connections.clone(),
            ServiceLimits::default(),
        );
        mark_connected(&service, store.as_ref()).await;
        let status = service.disconnect().await.expect("disconnect");
        assert_eq!(status.state, GitHubConnectionState::Disconnected);
        assert_eq!(status.account, None);
        assert!(!status.token_present);
        assert!(status.client_id_configured);
        assert!(!status.client_secret_configured);
        assert_eq!(store.get(REFRESH_TOKEN_ACCOUNT).expect("deleted"), None);
        assert_eq!(store.get(CLIENT_SECRET_ACCOUNT).expect("deleted"), None);
        assert!(service.access_session.lock().await.is_none());
        let persisted = connections
            .load()
            .expect("stored connection")
            .expect("client id remains");
        assert_eq!(persisted.client_id, "Iv1.sanitized-client");
        assert_eq!(persisted.account, None);
    }

    #[tokio::test]
    async fn invalid_identifiers_are_rejected_without_network_access() {
        let store = Arc::new(MemoryStore::default());
        let service = test_service(
            "http://127.0.0.1:9",
            store.clone(),
            ServiceLimits::default(),
        );
        mark_connected(&service, store.as_ref()).await;
        for (owner, repository) in [
            ("owner/repo", "repo"),
            ("owner", "repo\\other"),
            ("", "repo"),
            ("owner", "bad\nrepo"),
        ] {
            assert_eq!(
                service
                    .list_commits(owner, repository, None)
                    .await
                    .expect_err("invalid identifier")
                    .category(),
                GitHubErrorCategory::InvalidInput
            );
        }
    }
}
