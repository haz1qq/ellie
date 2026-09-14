use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use tauri::Manager;

use crate::{
    commands::{self, AppState},
    local_api::Authorization,
    providers::{
        AuthState, DataKind, ProviderCapabilities, ProviderError, ProviderOverview, SpendEstimate,
        TokenUsage, UsageWindow,
    },
    refresh::RefreshResponse,
};

pub const API_ADDRESS: &str = "127.0.0.1:9876";
const API_VERSION: &str = "v1";

#[derive(Clone)]
struct ApiState {
    app: tauri::AppHandle,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiEnvelope {
    app: &'static str,
    version: &'static str,
    providers: Vec<ApiProvider>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiProvider {
    id: String,
    name: String,
    account_label: Option<String>,
    plan: Option<String>,
    has_subscription: Option<bool>,
    capabilities: Option<ProviderCapabilities>,
    auth_state: Option<AuthState>,
    data_kind: Option<DataKind>,
    windows: Vec<UsageWindow>,
    token_usage: Option<TokenUsage>,
    credits: Option<f64>,
    balance: Option<f64>,
    balance_currency: Option<String>,
    spend_estimate: Option<SpendEstimate>,
    model: Option<String>,
    error: Option<ProviderError>,
    stale: bool,
    last_successful_refresh: Option<chrono::DateTime<chrono::Utc>>,
    last_attempt_at: Option<chrono::DateTime<chrono::Utc>>,
    next_retry_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiHealth {
    app: &'static str,
    version: &'static str,
    status: &'static str,
    api_version: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiRefreshResponse {
    app: &'static str,
    version: &'static str,
    refreshed: bool,
    busy: bool,
    providers: Vec<ApiProvider>,
}

#[derive(Debug, Serialize)]
struct ApiError {
    error: &'static str,
}

impl From<&ProviderOverview> for ApiProvider {
    fn from(overview: &ProviderOverview) -> Self {
        let snapshot = overview.snapshot.as_ref();
        Self {
            id: overview.provider_id.clone(),
            name: overview.display_name.clone(),
            account_label: snapshot.and_then(|value| value.account_label.clone()),
            plan: snapshot.and_then(|value| value.plan.clone()),
            has_subscription: snapshot.and_then(|value| value.has_subscription),
            capabilities: snapshot.map(|value| value.capabilities.clone()),
            auth_state: snapshot.map(|value| value.auth_state.clone()),
            data_kind: snapshot.map(|value| value.data_kind),
            windows: snapshot
                .map(|value| value.windows.clone())
                .unwrap_or_default(),
            token_usage: snapshot.and_then(|value| value.token_usage.clone()),
            credits: snapshot.and_then(|value| value.credits),
            balance: snapshot.and_then(|value| value.balance),
            balance_currency: snapshot.and_then(|value| value.balance_currency.clone()),
            spend_estimate: snapshot.and_then(|value| value.spend_estimate.clone()),
            model: snapshot.and_then(|value| value.model.clone()),
            error: overview.error.clone(),
            stale: overview.stale,
            last_successful_refresh: overview.last_successful_refresh,
            last_attempt_at: overview.last_attempt_at,
            next_retry_at: overview.next_retry_at,
        }
    }
}

impl From<RefreshResponse> for ApiRefreshResponse {
    fn from(response: RefreshResponse) -> Self {
        Self {
            app: "ellie",
            version: env!("CARGO_PKG_VERSION"),
            refreshed: response.refreshed,
            busy: response.busy,
            providers: response.providers.iter().map(ApiProvider::from).collect(),
        }
    }
}

pub fn spawn(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let controller = app.state::<AppState>().local_api.clone();
        controller.apply(None, router(app.clone())).await;
    });
}

pub(crate) fn router(app: tauri::AppHandle) -> Router {
    let auth = app.state::<AppState>().local_api.authorization();
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/providers", get(providers))
        .route("/api/v1/usage", get(usage))
        .route("/api/v1/providers/{provider_id}/usage", get(provider_usage))
        .route("/api/v1/refresh", post(refresh))
        .layer(middleware::from_fn_with_state(auth, require_bearer_token))
        .with_state(ApiState { app })
}

pub(crate) async fn require_bearer_token(
    State(auth): State<Authorization>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    // No browser clients are supported. Reject Origin (including null) and
    // Fetch Metadata even if a browser has somehow obtained a bearer token.
    if request.headers().contains_key(header::ORIGIN)
        || request.headers().contains_key("sec-fetch-site")
    {
        return api_error(StatusCode::FORBIDDEN, "browser_origin_denied");
    }
    if !auth.accepts(bearer_token(request.headers())).await {
        return api_error(StatusCode::UNAUTHORIZED, "unauthorized");
    }
    next.run(request).await
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| !value.is_empty())
}

fn api_error(status: StatusCode, error: &'static str) -> Response {
    (status, Json(ApiError { error })).into_response()
}

async fn health() -> Json<ApiHealth> {
    Json(ApiHealth {
        app: "ellie",
        version: env!("CARGO_PKG_VERSION"),
        status: "ok",
        api_version: API_VERSION,
    })
}

async fn providers(State(state): State<ApiState>) -> Json<ApiEnvelope> {
    Json(cached_envelope(&state).await)
}

async fn usage(State(state): State<ApiState>) -> Json<ApiEnvelope> {
    Json(cached_envelope(&state).await)
}

async fn provider_usage(
    Path(provider_id): Path<String>,
    State(state): State<ApiState>,
) -> Result<Json<ApiProvider>, (StatusCode, Json<ApiError>)> {
    let response = cached_response(&state).await;
    response
        .providers
        .iter()
        .find(|provider| provider.provider_id == provider_id)
        .map(|provider| Json(ApiProvider::from(provider)))
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(ApiError {
                error: "provider_not_found",
            }),
        ))
}

async fn refresh(State(state): State<ApiState>) -> Json<ApiRefreshResponse> {
    let response = commands::refresh_all_from_app(&state.app, true).await;
    Json(response.into())
}

async fn cached_response(state: &ApiState) -> RefreshResponse {
    let app_state = state.app.state::<AppState>();
    app_state
        .refresh
        .cached_response(&app_state.provider_registry, false)
        .await
}

async fn cached_envelope(state: &ApiState) -> ApiEnvelope {
    let response = cached_response(state).await;
    ApiEnvelope {
        app: "ellie",
        version: env!("CARGO_PKG_VERSION"),
        providers: response.providers.iter().map(ApiProvider::from).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_a_bearer_prefix() {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "secret".parse().expect("header"));
        assert_eq!(bearer_token(&headers), None);
        headers.insert(
            header::AUTHORIZATION,
            "Bearer secret".parse().expect("header"),
        );
        assert_eq!(bearer_token(&headers), Some("secret"));
    }
}
