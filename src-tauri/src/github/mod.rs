pub mod auth;
pub mod models;

use std::{
    fmt,
    sync::{
        atomic::{AtomicU64, AtomicU8, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use reqwest::{
    header::{self, HeaderMap},
    Client, StatusCode, Url,
};
use serde::{Deserialize, Serialize};

use crate::credentials::SecretStore;
use auth::TokenSet;
use models::{
    parse_commits, parse_repositories, parse_user, validate_branch, validate_repository_identifier,
    BoundedPagination,
};
pub use models::{CommitSummary, GitHubAccount, RepositorySummary};

pub const DEFAULT_API_BASE_URL: &str = "https://api.github.com";
pub const DEFAULT_AUTH_BASE_URL: &str = "https://github.com";
pub const REFRESH_TOKEN_ACCOUNT: &str = "github_refresh_token";

const API_VERSION: &str = "2022-11-28";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_API_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const DEFAULT_PAGE_SIZE: usize = 100;
const DEFAULT_MAX_PAGES: usize = 5;
const DEFAULT_MAX_ROWS: usize = 500;
const ACCESS_TOKEN_EXPIRY_MARGIN: Duration = Duration::from_secs(30);

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
    AuthenticationRequired,
    AuthenticationExpired,
    RateLimited,
    PermissionDenied,
    NotFound,
    ValidationFailed,
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

    const fn not_found() -> Self {
        Self::new(GitHubErrorCategory::NotFound)
    }

    const fn validation_failed() -> Self {
        Self::new(GitHubErrorCategory::ValidationFailed)
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
            GitHubErrorCategory::AuthenticationRequired => "GitHub authentication is required",
            GitHubErrorCategory::AuthenticationExpired => "GitHub authentication has expired",
            GitHubErrorCategory::RateLimited => "GitHub rate limited the request",
            GitHubErrorCategory::PermissionDenied => "GitHub denied permission for the request",
            GitHubErrorCategory::NotFound => "the requested GitHub resource was not found",
            GitHubErrorCategory::ValidationFailed => "GitHub rejected the request",
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
}

#[derive(Default)]
struct ConnectionRecord {
    account: Option<GitHubAccount>,
    last_error: Option<GitHubErrorCategory>,
}

struct PendingAuthorization {
    client_id: String,
    redirect_uri: String,
    verifier: String,
    state: String,
    generation: u64,
}

struct AccessSession {
    client_id: String,
    access_token: String,
    access_expires_at: Instant,
    refresh_expires_at: Instant,
    generation: u64,
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
    connection_state: AtomicU8,
    generation: AtomicU64,
    connection: Mutex<ConnectionRecord>,
    pending: Mutex<Option<PendingAuthorization>>,
    access_session: tokio::sync::Mutex<Option<AccessSession>>,
    limits: ServiceLimits,
}

impl GitHubService {
    pub fn new(
        client: Client,
        api_base_url: impl Into<String>,
        secret_store: Arc<dyn SecretStore>,
    ) -> Self {
        Self::with_auth_base(client, api_base_url, DEFAULT_AUTH_BASE_URL, secret_store)
    }

    fn with_auth_base(
        client: Client,
        api_base_url: impl Into<String>,
        auth_base_url: impl Into<String>,
        secret_store: Arc<dyn SecretStore>,
    ) -> Self {
        Self {
            client,
            api_base_url: api_base_url.into().trim_end_matches('/').to_string(),
            auth_base_url: auth_base_url.into().trim_end_matches('/').to_string(),
            secret_store,
            connection_state: AtomicU8::new(STATE_DISCONNECTED),
            generation: AtomicU64::new(0),
            connection: Mutex::new(ConnectionRecord::default()),
            pending: Mutex::new(None),
            access_session: tokio::sync::Mutex::new(None),
            limits: ServiceLimits::default(),
        }
    }

    pub async fn connection_status(&self) -> Result<GitHubConnectionStatus, GitHubError> {
        let token_present = self.read_refresh_token().await?.is_some();
        let connection = self
            .connection
            .lock()
            .map_err(|_| GitHubError::provider_unavailable())?;
        Ok(GitHubConnectionStatus {
            state: self.state(),
            account: connection.account.clone(),
            last_error: connection.last_error,
            token_present,
        })
    }

    pub fn connect_start(
        &self,
        client_id: String,
        redirect_port: u16,
    ) -> Result<String, GitHubError> {
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
            let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
            let pending = PendingAuthorization {
                client_id,
                redirect_uri,
                verifier: pkce.verifier,
                state,
                generation,
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
        let tokens = auth::exchange_code(
            &self.client,
            &self.auth_base_url,
            &pending.client_id,
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

        let session = session_from_tokens(pending.client_id, pending.generation, tokens)?;
        *self.access_session.lock().await = Some(session);
        {
            let mut connection = self
                .connection
                .lock()
                .map_err(|_| GitHubError::provider_unavailable())?;
            connection.account = Some(user);
            connection.last_error = None;
        }
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
            return Err(GitHubError::cancelled());
        }
        Ok(())
    }

    pub async fn disconnect(&self) -> Result<GitHubConnectionStatus, GitHubError> {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.connection_state
            .store(STATE_DISCONNECTED, Ordering::SeqCst);
        if let Ok(mut pending) = self.pending.lock() {
            *pending = None;
        }
        *self.access_session.lock().await = None;
        if let Ok(mut connection) = self.connection.lock() {
            *connection = ConnectionRecord::default();
        }

        if let Err(error) = self.delete_refresh_token().await {
            self.record_error(error.category());
            return Err(error);
        }
        self.connection_status().await
    }

    pub async fn get_user(&self) -> Result<GitHubAccount, GitHubError> {
        let result = async {
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
        let result = self.list_commits_inner(owner, repository, branch).await;
        self.record_result(&result);
        result
    }

    async fn list_commits_inner(
        &self,
        owner: &str,
        repository: &str,
        branch: Option<&str>,
    ) -> Result<Vec<CommitSummary>, GitHubError> {
        validate_repository_identifier(owner)?;
        validate_repository_identifier(repository)?;
        if let Some(branch) = branch {
            validate_branch(branch)?;
        }

        let mut output = Vec::new();
        let mut pagination = BoundedPagination::new(self.limits.max_pages, self.limits.max_rows);
        loop {
            let mut url = self.repository_commits_url(owner, repository)?;
            let page_number = pagination.current_page().to_string();
            let page_size = self.limits.page_size.to_string();
            {
                let mut query = url.query_pairs_mut();
                query
                    .append_pair("per_page", &page_size)
                    .append_pair("page", &page_number);
                if let Some(branch) = branch {
                    query.append_pair("sha", branch);
                }
            }

            let response = self.authorized_get(url).await?;
            let page = parse_commits(&response.body)?;
            let has_next = response.has_next || page.len() == self.limits.page_size;
            let decision = pagination.accept_page(page.len(), has_next);
            output.extend(page.into_iter().take(decision.take_rows));
            if decision.next_page.is_none() {
                break;
            }
        }
        Ok(output)
    }

    async fn get_user_with_access_token(
        &self,
        access_token: &str,
    ) -> Result<GitHubAccount, GitHubError> {
        let url = self.api_url("/user")?;
        let response = self.send_api_get(url, access_token).await?;
        ensure_api_success(&response)?;
        parse_user(&response.body)
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
        let tokens = auth::refresh_token(
            &self.client,
            &self.auth_base_url,
            &session.client_id,
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
            .timeout(REQUEST_TIMEOUT)
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
        self.generation.fetch_add(1, Ordering::SeqCst);
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
    use std::sync::Mutex as StdMutex;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        task::JoinHandle,
    };

    struct MockResponse {
        status: &'static str,
        headers: &'static str,
        body: String,
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

    fn test_client() -> Client {
        Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("test client")
    }

    fn test_service(
        base_url: &str,
        store: Arc<dyn SecretStore>,
        limits: ServiceLimits,
    ) -> GitHubService {
        let mut service = GitHubService::with_auth_base(test_client(), base_url, base_url, store);
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
        *service.connection.lock().expect("connection") = ConnectionRecord {
            account: Some(GitHubAccount {
                id: 42,
                login: "octo-cat".to_string(),
            }),
            last_error: None,
        };
        service
            .connection_state
            .store(STATE_CONNECTED, Ordering::SeqCst);
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
        assert!(!requests[0].contains("client_secret"));
        assert!(requests[1].starts_with("GET /user"));
        assert!(requests[1]
            .to_ascii_lowercase()
            .contains("authorization: bearer ghu_sanitized_access_value"));
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
            assert_eq!(error.category(), GitHubErrorCategory::MalformedResponse);
        }
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

        let service = test_service(
            "http://127.0.0.1:9",
            store.clone(),
            ServiceLimits::default(),
        );
        mark_connected(&service, store.as_ref()).await;
        let status = service.disconnect().await.expect("disconnect");
        assert_eq!(status.state, GitHubConnectionState::Disconnected);
        assert_eq!(status.account, None);
        assert!(!status.token_present);
        assert_eq!(store.get(REFRESH_TOKEN_ACCOUNT).expect("deleted"), None);
        assert!(service.access_session.lock().await.is_none());
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
