//! `ellie-cli` — a thin command-line consumer of Ellie's local REST API.
//!
//! Route B design (see PROJECT.md §38): this binary talks only to the loopback
//! API at `http://127.0.0.1:9876/api/v1` using the `ELLIE_API_TOKEN` bearer
//! token. It never touches credentials, SQLite, or Ellie core internals, and
//! it requires the Ellie tray app to be running with the token set.
//!
//! Security rules that are enforced here:
//! - the token is sent only in the `Authorization: Bearer` header;
//! - the token is never printed, logged, or persisted;
//! - raw response bodies and API error bodies are never printed — HTTP and
//!   transport failures are shown as static friendly copy;
//! - provider errors are redacted to friendly copy with distinct exit codes
//!   (see `docs/cli.md`).

use std::env;
use std::process::ExitCode;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Deserialize;

const API_BASE_URL: &str = "http://127.0.0.1:9876/api/v1";
const TOKEN_ENV_VAR: &str = "ELLIE_API_TOKEN";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const STATUS_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
// A forced refresh runs the configured providers sequentially. Six minutes
// covers their bounded request/page limits plus snapshot persistence.
const REFRESH_REQUEST_TIMEOUT: Duration = Duration::from_secs(6 * 60);

// Exit codes (documented in docs/cli.md and the help text).
const EXIT_ELLIE_UNREACHABLE: u8 = 1;
const EXIT_USAGE_OR_ENV: u8 = 2;
const EXIT_UNAUTHORIZED: u8 = 3;
const EXIT_SERVER_TOKEN_UNCONFIGURED: u8 = 4;
const EXIT_PROVIDER_NOT_FOUND: u8 = 5;
const EXIT_RESPONSE_ERROR: u8 = 6;

const USAGE: &str = "Ellie CLI — check AI usage through the running Ellie tray app

Usage:
  ellie-cli status     Show current quota usage per provider
  ellie-cli refresh    Trigger a refresh in the running Ellie app, then show usage
  ellie-cli version    Print the ellie-cli version
  ellie-cli help       Show this help

The Ellie tray app must be running so its local API is reachable at
127.0.0.1:9876, and the ELLIE_API_TOKEN environment variable must be set to
the same bearer token the app was started with.

Exit codes:
  0  success
  1  Ellie is unreachable (not running, network error)
  2  usage or environment error (unknown command, missing ELLIE_API_TOKEN)
  3  unauthorized: ELLIE_API_TOKEN does not match the running app
  4  the app's local API has no token configured
  5  requested provider not found
  6  Ellie returned an unexpected response or server error
";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Status,
    Refresh,
    Version,
    Help,
}

// ---------------------------------------------------------------------------
// API response types (camelCase, mirroring src-tauri/src/api.rs). Fields that
// the CLI does not render are simply ignored by serde, so additive API changes
// stay compatible. `#[serde(default)]` keeps the CLI tolerant of missing
// optional fields while required identity fields still fail loudly.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiEnvelope {
    providers: Vec<ApiProvider>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiProvider {
    name: String,
    auth_state: Option<AuthState>,
    data_kind: Option<DataKind>,
    #[serde(default)]
    windows: Vec<UsageWindow>,
    balance: Option<f64>,
    balance_currency: Option<String>,
    error: Option<ProviderError>,
    #[serde(default)]
    stale: bool,
    last_successful_refresh: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiRefreshResponse {
    busy: bool,
    providers: Vec<ApiProvider>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsageWindow {
    label: String,
    used_percent: Option<f64>,
    reset_at: Option<DateTime<Utc>>,
    source: MetricSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AuthState {
    Authenticated,
    AuthenticationDetected,
    AuthenticationRequired,
    Expired,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DataKind {
    Live,
    Mock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ProviderError {
    InvalidSnapshot,
    AuthenticationRequired,
    AuthenticationExpired,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MetricSource {
    ProviderReported,
    LocallyCalculated,
}

// ---------------------------------------------------------------------------
// Errors: friendly copy only, never raw bodies or credentials.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum CliError {
    EllieNotRunning,
    Timeout,
    NetworkUnavailable,
    Unauthorized,
    ServerTokenNotConfigured,
    ProviderNotFound,
    ServerError(u16),
    MalformedResponse,
}

impl CliError {
    fn friendly(&self) -> String {
        match self {
            CliError::EllieNotRunning => {
                "Ellie is not running or its local API is not reachable; start the Ellie tray app and try again"
                    .to_string()
            }
            CliError::Timeout => "Ellie's local API did not respond in time".to_string(),
            CliError::NetworkUnavailable => {
                "could not reach Ellie's local API (network error)".to_string()
            }
            CliError::Unauthorized => format!(
                "unauthorized: {TOKEN_ENV_VAR} does not match the token the running Ellie app was started with"
            ),
            CliError::ServerTokenNotConfigured => format!(
                "Ellie's local API has no token configured; start the Ellie tray app with {TOKEN_ENV_VAR} set"
            ),
            CliError::ProviderNotFound => "provider not found".to_string(),
            CliError::ServerError(code) => {
                format!("Ellie's local API returned an error (HTTP {code})")
            }
            CliError::MalformedResponse => {
                "Ellie returned an unexpected response; try 'ellie-cli refresh'".to_string()
            }
        }
    }

    fn exit_code(&self) -> u8 {
        match self {
            CliError::EllieNotRunning | CliError::Timeout | CliError::NetworkUnavailable => {
                EXIT_ELLIE_UNREACHABLE
            }
            CliError::Unauthorized => EXIT_UNAUTHORIZED,
            CliError::ServerTokenNotConfigured => EXIT_SERVER_TOKEN_UNCONFIGURED,
            CliError::ProviderNotFound => EXIT_PROVIDER_NOT_FOUND,
            CliError::ServerError(_) | CliError::MalformedResponse => EXIT_RESPONSE_ERROR,
        }
    }
}

// ---------------------------------------------------------------------------
// HTTP layer (separable from formatting and parsing).
// ---------------------------------------------------------------------------

fn build_client() -> Result<reqwest::Client, CliError> {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        // This client sends a bearer credential only to Ellie's fixed loopback
        // API. Never delegate that request to system or environment proxies.
        .no_proxy()
        .build()
        .map_err(|_| CliError::NetworkUnavailable)
}

fn transport_error(error: reqwest::Error) -> CliError {
    if error.is_connect() {
        CliError::EllieNotRunning
    } else if error.is_timeout() {
        CliError::Timeout
    } else {
        CliError::NetworkUnavailable
    }
}

fn ensure_success(status: reqwest::StatusCode) -> Result<(), CliError> {
    if status.is_success() {
        Ok(())
    } else {
        Err(match status.as_u16() {
            401 | 403 => CliError::Unauthorized,
            404 => CliError::ProviderNotFound,
            503 => CliError::ServerTokenNotConfigured,
            code => CliError::ServerError(code),
        })
    }
}

async fn get_usage(
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<ApiEnvelope, CliError> {
    get_usage_with_timeout(client, base_url, token, STATUS_REQUEST_TIMEOUT).await
}

async fn get_usage_with_timeout(
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
    timeout: Duration,
) -> Result<ApiEnvelope, CliError> {
    let response = client
        .get(format!("{base_url}/usage"))
        .bearer_auth(token)
        .timeout(timeout)
        .send()
        .await
        .map_err(transport_error)?;
    ensure_success(response.status())?;
    response
        .json::<ApiEnvelope>()
        .await
        .map_err(|_| CliError::MalformedResponse)
}

async fn post_refresh(
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<ApiRefreshResponse, CliError> {
    post_refresh_with_timeout(client, base_url, token, REFRESH_REQUEST_TIMEOUT).await
}

async fn post_refresh_with_timeout(
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
    timeout: Duration,
) -> Result<ApiRefreshResponse, CliError> {
    let response = client
        .post(format!("{base_url}/refresh"))
        .bearer_auth(token)
        .timeout(timeout)
        .send()
        .await
        .map_err(transport_error)?;
    ensure_success(response.status())?;
    response
        .json::<ApiRefreshResponse>()
        .await
        .map_err(|_| CliError::MalformedResponse)
}

// ---------------------------------------------------------------------------
// Formatting (pure functions, unit-testable without a running app).
// ---------------------------------------------------------------------------

fn format_status(providers: &[ApiProvider], now: DateTime<Utc>) -> String {
    let mut out = String::from("Ellie\n");
    let mut rendered_provider = false;
    for provider in providers {
        // The demo provider is clearly marked mock data and is omitted from
        // reports (matches the §38 example, which shows only live providers).
        if provider.data_kind == Some(DataKind::Mock) {
            continue;
        }
        let Some(lines) = provider_lines(provider, now) else {
            continue;
        };
        rendered_provider = true;
        out.push('\n');
        out.push_str(&provider.name);
        out.push('\n');
        for line in lines {
            out.push_str("  ");
            out.push_str(&line);
            out.push('\n');
        }
    }
    if !rendered_provider {
        out.push_str("\nNo provider usage data available\n");
    }
    out
}

/// Renders one provider's report lines (without the name or indentation).
/// Returns `None` for providers that have never produced data and carry no
/// error (they are omitted, matching the dashboard's hidden-provider policy).
fn provider_lines(provider: &ApiProvider, now: DateTime<Utc>) -> Option<Vec<String>> {
    if provider.auth_state.is_none() && provider.error.is_none() {
        return None;
    }

    let mut lines = Vec::new();
    if provider.auth_state.is_some() {
        for window in &provider.windows {
            lines.push(format_window_line(window, now));
        }
        if let Some(balance) = provider.balance {
            lines.push(format_balance_line(
                balance,
                provider.balance_currency.as_deref(),
            ));
        }
        if provider.windows.is_empty() && provider.balance.is_none() {
            lines.push("No usage data available".to_string());
        }
        if let Some(state) = provider.auth_state {
            if let Some(copy) = auth_state_issue(state) {
                lines.push(copy.to_string());
            }
        }
    }
    if let Some(error) = provider.error {
        let copy = unavailable_copy(error);
        if !lines.iter().any(|line| line == copy) {
            lines.push(copy.to_string());
        }
    }

    if provider.stale {
        let age = provider
            .last_successful_refresh
            .map(|then| format!(" (data from {})", relative_age(then, now)))
            .unwrap_or_default();
        lines.push(format!("Stale — showing data from a previous refresh{age}"));
    }

    Some(lines)
}

fn format_window_line(window: &UsageWindow, now: DateTime<Utc>) -> String {
    let used = window.used_percent.map_or_else(
        || "used: Unavailable".to_string(),
        |percent| format!("{percent:.0}% used"),
    );
    let reset = window.reset_at.map_or_else(
        || "Reset unknown".to_string(),
        |reset| format!("Reset {}", reset_countdown(reset, now)),
    );
    // Provenance label: locally calculated windows are always marked as
    // estimates; provider-reported windows carry no suffix.
    let provenance = match window.source {
        MetricSource::LocallyCalculated => " (Ellie estimate)",
        MetricSource::ProviderReported => "",
    };
    // Matches the §38 example exactly: `  5 Hour    63% used     Reset 2h 14m`,
    // i.e. label padded to 6, then 4 spaces, the used segment (+ provenance),
    // then 5 spaces before the Reset column.
    format!("{:<6}    {used}{provenance}     {reset}", window.label)
}

fn format_balance_line(balance: f64, currency: Option<&str>) -> String {
    let rendered = match currency {
        Some("USD") => format!("${balance:.2}"),
        Some("CNY") => format!("¥{balance:.2}"),
        Some(other) if !other.trim().is_empty() => format!("{other} {balance:.2}"),
        Some(_) | None => format!("{balance:.2} (currency unavailable)"),
    };
    format!("Balance   {rendered}")
}

/// Human countdown until `reset_at` from `now`, e.g. "2h 14m", "3d 7h".
fn reset_countdown(reset_at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let seconds = reset_at.signed_duration_since(now).num_seconds();
    if seconds <= 0 {
        "now".to_string()
    } else {
        compact_duration(seconds)
    }
}

/// Human age of a past timestamp from `now`, e.g. "2h 14m", "3d 7h", "45s".
fn relative_age(then: DateTime<Utc>, now: DateTime<Utc>) -> String {
    compact_duration(now.signed_duration_since(then).num_seconds().max(0))
}

fn compact_duration(total_seconds: i64) -> String {
    let days = total_seconds / 86_400;
    let hours = (total_seconds % 86_400) / 3_600;
    let minutes = (total_seconds % 3_600) / 60;
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else if minutes > 0 {
        format!("{minutes}m")
    } else {
        format!("{total_seconds}s")
    }
}

fn auth_state_issue(state: AuthState) -> Option<&'static str> {
    match state {
        AuthState::AuthenticationRequired => Some("Authentication not configured"),
        AuthState::Expired => Some("Authentication has expired"),
        AuthState::Authenticated | AuthState::AuthenticationDetected | AuthState::Unsupported => {
            None
        }
    }
}

fn unavailable_copy(error: ProviderError) -> &'static str {
    match error {
        ProviderError::InvalidSnapshot => "Invalid provider data",
        ProviderError::AuthenticationRequired => "Authentication not configured",
        ProviderError::AuthenticationExpired => "Authentication has expired",
        ProviderError::Unavailable => "Data unavailable",
    }
}

// ---------------------------------------------------------------------------
// Command parsing and entry point.
// ---------------------------------------------------------------------------

fn parse_command(args: &[String]) -> Result<Command, String> {
    match args {
        [] => Ok(Command::Help),
        [arg] if arg == "status" => Ok(Command::Status),
        [arg] if arg == "refresh" => Ok(Command::Refresh),
        [arg] if arg == "version" => Ok(Command::Version),
        [arg] if arg == "help" || arg == "--help" || arg == "-h" => Ok(Command::Help),
        [arg] => Err(format!("unknown command '{arg}'")),
        _ => Err("expected a single command".to_string()),
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let command = match parse_command(&args) {
        Ok(command) => command,
        Err(message) => {
            eprintln!("ellie-cli: {message}\n");
            eprint!("{USAGE}");
            return ExitCode::from(EXIT_USAGE_OR_ENV);
        }
    };

    match command {
        Command::Help => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Command::Version => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Command::Status | Command::Refresh => {
            let Some(token) = env::var(TOKEN_ENV_VAR)
                .ok()
                .filter(|value| !value.trim().is_empty())
            else {
                eprintln!(
                    "ellie-cli: set the {TOKEN_ENV_VAR} environment variable to the bearer token the Ellie tray app is running with"
                );
                return ExitCode::from(EXIT_USAGE_OR_ENV);
            };
            let client = match build_client() {
                Ok(client) => client,
                Err(error) => {
                    report_error(&error);
                    return ExitCode::from(error.exit_code());
                }
            };
            let output = match command {
                Command::Status => cmd_status(&client, &token).await,
                Command::Refresh => cmd_refresh(&client, &token).await,
                Command::Help | Command::Version => unreachable!("handled above"),
            };
            match output {
                Ok(text) => {
                    print!("{text}");
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    report_error(&error);
                    ExitCode::from(error.exit_code())
                }
            }
        }
    }
}

fn report_error(error: &CliError) {
    eprintln!("ellie-cli: {}", error.friendly());
}

async fn cmd_status(client: &reqwest::Client, token: &str) -> Result<String, CliError> {
    let envelope = get_usage(client, API_BASE_URL, token).await?;
    Ok(format_status(&envelope.providers, Utc::now()))
}

async fn cmd_refresh(client: &reqwest::Client, token: &str) -> Result<String, CliError> {
    let response = post_refresh(client, API_BASE_URL, token).await?;
    Ok(format_refresh_response(&response, Utc::now()))
}

fn format_refresh_response(response: &ApiRefreshResponse, now: DateTime<Utc>) -> String {
    let heading = if response.busy {
        "Refresh already in progress."
    } else {
        "Refreshed."
    };
    let report = format_status(&response.providers, now);
    format!("{heading}\n\n{report}")
}

// ---------------------------------------------------------------------------
// Tests: serde fixtures, pure formatting, error mapping, and a tiny local
// mock HTTP server (no live Ellie app required).
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::sync::Mutex;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::task::JoinHandle;

    static PROXY_ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvVarGuard {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: Option<&str>) -> Self {
            let previous = env::var_os(key);
            match value {
                Some(value) => env::set_var(key, value),
                None => env::remove_var(key),
            }
            Self { key, previous }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => env::set_var(self.key, value),
                None => env::remove_var(self.key),
            }
        }
    }

    fn fixed_now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-09-06T08:00:00Z")
            .expect("fixture timestamp")
            .with_timezone(&Utc)
    }

    fn parse_envelope(json: &str) -> ApiEnvelope {
        serde_json::from_str(json).expect("fixture envelope")
    }

    const SECTION38_ENVELOPE: &str = r#"{
      "app": "ellie",
      "version": "0.1.0",
      "providers": [
        {
          "id": "openai-codex",
          "name": "OpenAI / Codex",
          "accountLabel": "personal",
          "plan": "Plus",
          "hasSubscription": true,
          "capabilities": {
            "quotaWindows": true,
            "tokenUsage": true,
            "accountBalance": false,
            "credits": true,
            "costTracking": false,
            "localHistory": false
          },
          "authState": "authenticated",
          "dataKind": "live",
          "windows": [
            {
              "id": "session",
              "label": "5 Hour",
              "usedPercent": 63.0,
              "remainingPercent": 37.0,
              "usedValue": null,
              "remainingValue": null,
              "limitValue": null,
              "unit": null,
              "startsAt": null,
              "resetAt": "2026-09-06T10:14:00Z",
              "source": "provider_reported"
            },
            {
              "id": "weekly",
              "label": "Weekly",
              "usedPercent": 42.0,
              "remainingPercent": 58.0,
              "usedValue": null,
              "remainingValue": null,
              "limitValue": null,
              "unit": null,
              "startsAt": null,
              "resetAt": "2026-09-09T15:00:00Z",
              "source": "provider_reported"
            }
          ],
          "tokenUsage": {
            "inputTokens": 120000,
            "outputTokens": 8000,
            "cachedInputTokens": null,
            "cachedOutputTokens": null,
            "reasoningTokens": null,
            "totalTokens": 128000,
            "requestCount": 320,
            "estimatedCostUsd": null,
            "source": "locally_calculated"
          },
          "credits": null,
          "balance": null,
          "balanceCurrency": null,
          "spendEstimate": null,
          "model": "gpt-5-codex",
          "error": null,
          "stale": false,
          "lastSuccessfulRefresh": "2026-09-06T08:00:00Z",
          "lastAttemptAt": "2026-09-06T08:00:00Z",
          "nextRetryAt": null
        },
        {
          "id": "anthropic-claude",
          "name": "Anthropic / Claude",
          "accountLabel": null,
          "plan": null,
          "hasSubscription": null,
          "capabilities": null,
          "authState": "authenticated",
          "dataKind": "live",
          "windows": [
            {
              "id": "session",
              "label": "5 Hour",
              "usedPercent": 81.0,
              "remainingPercent": 19.0,
              "usedValue": null,
              "remainingValue": null,
              "limitValue": null,
              "unit": null,
              "startsAt": null,
              "resetAt": "2026-09-06T11:22:00Z",
              "source": "provider_reported"
            },
            {
              "id": "weekly",
              "label": "Weekly",
              "usedPercent": 54.0,
              "remainingPercent": 46.0,
              "usedValue": null,
              "remainingValue": null,
              "limitValue": null,
              "unit": null,
              "startsAt": null,
              "resetAt": "2026-09-10T10:00:00Z",
              "source": "provider_reported"
            }
          ],
          "tokenUsage": null,
          "credits": null,
          "balance": null,
          "balanceCurrency": null,
          "spendEstimate": null,
          "model": "claude-sonnet-4-5",
          "error": null,
          "stale": false,
          "lastSuccessfulRefresh": "2026-09-06T08:00:00Z",
          "lastAttemptAt": "2026-09-06T08:00:00Z",
          "nextRetryAt": null
        },
        {
          "id": "deepseek",
          "name": "DeepSeek",
          "accountLabel": null,
          "plan": null,
          "hasSubscription": null,
          "capabilities": {
            "quotaWindows": false,
            "tokenUsage": false,
            "accountBalance": true,
            "credits": false,
            "costTracking": true,
            "localHistory": false
          },
          "authState": "authenticated",
          "dataKind": "live",
          "windows": [],
          "tokenUsage": null,
          "credits": null,
          "balance": 8.42,
          "balanceCurrency": "USD",
          "spendEstimate": { "amount": 1.3, "currency": "USD", "windowDays": 14 },
          "model": "deepseek-chat",
          "error": null,
          "stale": false,
          "lastSuccessfulRefresh": "2026-09-06T08:00:00Z",
          "lastAttemptAt": "2026-09-06T08:00:00Z",
          "nextRetryAt": null
        }
      ]
    }"#;

    // -- serde parsing -----------------------------------------------------

    #[test]
    fn parses_multiple_windows_and_balance_rows() {
        let envelope = parse_envelope(SECTION38_ENVELOPE);
        assert_eq!(envelope.providers.len(), 3);
        let openai = &envelope.providers[0];
        assert_eq!(openai.windows.len(), 2);
        assert_eq!(openai.windows[0].label, "5 Hour");
        assert_eq!(openai.windows[0].used_percent, Some(63.0));
        assert_eq!(openai.windows[0].source, MetricSource::ProviderReported);
        let deepseek = &envelope.providers[2];
        assert_eq!(deepseek.balance, Some(8.42));
        assert_eq!(deepseek.balance_currency.as_deref(), Some("USD"));
        assert!(deepseek.windows.is_empty());
    }

    #[test]
    fn parses_missing_and_null_optional_fields() {
        let json = r#"{
          "app": "ellie",
          "version": "0.1.0",
          "providers": [
            {
              "id": "deepseek",
              "name": "DeepSeek",
              "accountLabel": null,
              "plan": null,
              "hasSubscription": null,
              "capabilities": null,
              "authState": "authenticated",
              "dataKind": "live",
              "windows": [],
              "tokenUsage": null,
              "credits": null,
              "balance": 8.42,
              "balanceCurrency": "CNY",
              "spendEstimate": null,
              "model": null,
              "error": null,
              "stale": false,
              "lastSuccessfulRefresh": null,
              "lastAttemptAt": null,
              "nextRetryAt": null
            },
            {
              "id": "openai-api",
              "name": "OpenAI API",
              "authState": null,
              "dataKind": null,
              "windows": [],
              "balance": null,
              "balanceCurrency": null,
              "error": null,
              "stale": false
            }
          ]
        }"#;
        let envelope = parse_envelope(json);
        let deepseek = &envelope.providers[0];
        assert_eq!(deepseek.balance_currency.as_deref(), Some("CNY"));
        assert_eq!(deepseek.last_successful_refresh, None);
        // Provider with most optional fields missing still parses.
        assert_eq!(envelope.providers[1].name, "OpenAI API");
        assert_eq!(envelope.providers[1].auth_state, None);
    }

    #[test]
    fn parses_empty_provider_list() {
        let envelope = parse_envelope(r#"{"app": "ellie", "version": "0.1.0", "providers": []}"#);
        assert!(envelope.providers.is_empty());
        assert_eq!(
            format_status(&envelope.providers, fixed_now()),
            "Ellie\n\nNo provider usage data available\n"
        );
    }

    #[test]
    fn parses_refresh_response() {
        let json = r#"{
          "app": "ellie",
          "version": "0.1.0",
          "refreshed": true,
          "busy": false,
          "providers": [
            {
              "id": "deepseek",
              "name": "DeepSeek",
              "authState": null,
              "dataKind": null,
              "windows": [],
              "balance": null,
              "balanceCurrency": null,
              "error": "unavailable",
              "stale": false
            }
          ]
        }"#;
        let response: ApiRefreshResponse = serde_json::from_str(json).expect("refresh response");
        assert!(!response.busy);
        assert_eq!(response.providers.len(), 1);
    }

    #[test]
    fn rejects_malformed_json() {
        let result: Result<ApiEnvelope, _> = serde_json::from_str("not json at all");
        assert!(result.is_err());
        // `providers` is required — a missing list is a malformed envelope.
        assert!(serde_json::from_str::<ApiEnvelope>(r#"{"app": "ellie"}"#).is_err());
        // Unknown fields (app/version and future additions) are ignored.
        let envelope = parse_envelope(r#"{"app": "ellie", "version": "0.1.0", "providers": []}"#);
        assert!(envelope.providers.is_empty());
    }

    #[test]
    fn parses_error_envelope_without_surfacing_raw_copy() {
        // The error contract is `{"error": "<string>"}`; the CLI never prints
        // the string itself, but parsing it proves the contract stays parseable.
        #[derive(Deserialize)]
        struct ApiError {
            error: String,
        }
        let body: ApiError =
            serde_json::from_str(r#"{"error": "api_token_not_configured"}"#).expect("error body");
        assert_eq!(body.error, "api_token_not_configured");
        assert!(!CliError::ServerTokenNotConfigured
            .friendly()
            .contains(&body.error));
    }

    // -- formatting --------------------------------------------------------

    #[test]
    fn status_matches_section38_example_shape() {
        let envelope = parse_envelope(SECTION38_ENVELOPE);
        let expected = "\
Ellie

OpenAI / Codex
  5 Hour    63% used     Reset 2h 14m
  Weekly    42% used     Reset 3d 7h

Anthropic / Claude
  5 Hour    81% used     Reset 3h 22m
  Weekly    54% used     Reset 4d 2h

DeepSeek
  Balance   $8.42
";
        assert_eq!(format_status(&envelope.providers, fixed_now()), expected);
    }

    #[test]
    fn balance_without_a_currency_is_explicitly_unavailable() {
        assert_eq!(
            format_balance_line(8.42, None),
            "Balance   8.42 (currency unavailable)"
        );
        assert_eq!(
            format_balance_line(8.42, Some("")),
            "Balance   8.42 (currency unavailable)"
        );
        assert_eq!(format_balance_line(8.42, Some("EUR")), "Balance   EUR 8.42");
    }

    #[test]
    fn reset_countdown_derives_human_labels() {
        let now = fixed_now();
        assert_eq!(
            reset_countdown(now + chrono::Duration::minutes(134), now),
            "2h 14m"
        );
        assert_eq!(
            reset_countdown(
                now + chrono::Duration::days(3) + chrono::Duration::hours(7),
                now
            ),
            "3d 7h"
        );
        assert_eq!(
            reset_countdown(now + chrono::Duration::minutes(45), now),
            "45m"
        );
        assert_eq!(
            reset_countdown(now + chrono::Duration::seconds(20), now),
            "20s"
        );
        assert_eq!(
            reset_countdown(now - chrono::Duration::seconds(1), now),
            "now"
        );
    }

    #[test]
    fn stale_providers_are_labeled_with_age() {
        let json = r#"{
          "app": "ellie",
          "version": "0.1.0",
          "providers": [
            {
              "id": "openai-codex",
              "name": "OpenAI / Codex",
              "authState": "authenticated",
              "dataKind": "live",
              "windows": [
                {
                  "id": "session",
                  "label": "5 Hour",
                  "usedPercent": 63.0,
                  "resetAt": "2026-09-06T10:14:00Z",
                  "source": "provider_reported"
                }
              ],
              "balance": null,
              "balanceCurrency": null,
              "error": "unavailable",
              "stale": true,
              "lastSuccessfulRefresh": "2026-09-06T06:00:00Z"
            }
          ]
        }"#;
        let envelope = parse_envelope(json);
        let output = format_status(&envelope.providers, fixed_now());
        assert!(output.contains("5 Hour    63% used     Reset 2h 14m"));
        assert!(output.contains("Data unavailable"));
        assert!(output.contains("Stale — showing data from a previous refresh (data from 2h 0m)"));
        // The typed error is friendly static copy, not the raw serialized value.
        assert!(!output.contains("\n  unavailable\n"));
    }

    #[test]
    fn stale_cached_usage_includes_current_authentication_error() {
        let json = r#"{
          "providers": [
            {
              "name": "OpenAI / Codex",
              "authState": "authenticated",
              "dataKind": "live",
              "windows": [
                {
                  "label": "5 Hour",
                  "usedPercent": 63.0,
                  "resetAt": "2026-09-06T10:14:00Z",
                  "source": "provider_reported"
                }
              ],
              "balance": null,
              "balanceCurrency": null,
              "error": "authentication_expired",
              "stale": true,
              "lastSuccessfulRefresh": "2026-09-06T06:00:00Z"
            }
          ]
        }"#;
        let envelope = parse_envelope(json);
        assert_eq!(
            format_status(&envelope.providers, fixed_now()),
            "Ellie\n\nOpenAI / Codex\n  5 Hour    63% used     Reset 2h 14m\n  Authentication has expired\n  Stale — showing data from a previous refresh (data from 2h 0m)\n"
        );
    }

    #[test]
    fn equivalent_auth_state_and_error_copy_is_not_duplicated() {
        let json = r#"{
          "providers": [
            {
              "name": "Anthropic / Claude",
              "authState": "expired",
              "dataKind": "live",
              "windows": [],
              "balance": null,
              "balanceCurrency": null,
              "error": "authentication_expired",
              "stale": true,
              "lastSuccessfulRefresh": null
            }
          ]
        }"#;
        let envelope = parse_envelope(json);
        let output = format_status(&envelope.providers, fixed_now());
        assert_eq!(output.matches("Authentication has expired").count(), 1);
        assert_eq!(
            output,
            "Ellie\n\nAnthropic / Claude\n  No usage data available\n  Authentication has expired\n  Stale — showing data from a previous refresh\n"
        );
    }

    #[test]
    fn unavailable_and_auth_state_providers_show_friendly_copy() {
        let json = r#"{
          "app": "ellie",
          "version": "0.1.0",
          "providers": [
            {
              "id": "deepseek",
              "name": "DeepSeek",
              "authState": null,
              "dataKind": null,
              "windows": [],
              "balance": null,
              "balanceCurrency": null,
              "error": "authentication_required",
              "stale": false
            },
            {
              "id": "anthropic-claude",
              "name": "Anthropic / Claude",
              "authState": "expired",
              "dataKind": "live",
              "windows": [],
              "balance": null,
              "balanceCurrency": null,
              "error": null,
              "stale": false
            }
          ]
        }"#;
        let envelope = parse_envelope(json);
        let output = format_status(&envelope.providers, fixed_now());
        assert!(output.contains("DeepSeek\n  Authentication not configured\n"));
        assert!(output.contains(
            "Anthropic / Claude\n  No usage data available\n  Authentication has expired\n"
        ));
        assert!(!output.contains("authentication_required"));
        assert!(!output.contains("authentication_expired"));
    }

    #[test]
    fn empty_windows_render_explicit_empty_state() {
        let json = r#"{
          "app": "ellie",
          "version": "0.1.0",
          "providers": [
            {
              "id": "deepseek",
              "name": "DeepSeek",
              "authState": "authenticated",
              "dataKind": "live",
              "windows": [],
              "balance": null,
              "balanceCurrency": null,
              "error": null,
              "stale": false
            }
          ]
        }"#;
        let envelope = parse_envelope(json);
        assert!(format_status(&envelope.providers, fixed_now())
            .contains("DeepSeek\n  No usage data available\n"));
    }

    #[test]
    fn locally_calculated_windows_are_labelled_as_estimates() {
        let json = r#"{
          "app": "ellie",
          "version": "0.1.0",
          "providers": [
            {
              "id": "openai-api",
              "name": "OpenAI API",
              "authState": "authenticated",
              "dataKind": "live",
              "windows": [
                {
                  "id": "monthly",
                  "label": "Monthly",
                  "usedPercent": 30.0,
                  "resetAt": "2026-09-10T10:00:00Z",
                  "source": "locally_calculated"
                }
              ],
              "balance": null,
              "balanceCurrency": null,
              "error": null,
              "stale": false
            }
          ]
        }"#;
        let envelope = parse_envelope(json);
        let output = format_status(&envelope.providers, fixed_now());
        assert!(output.contains("Monthly    30% used (Ellie estimate)     Reset 4d 2h\n"));
    }

    #[test]
    fn missing_window_values_render_explicitly() {
        let json = r#"{
          "app": "ellie",
          "version": "0.1.0",
          "providers": [
            {
              "id": "openai-codex",
              "name": "OpenAI / Codex",
              "authState": "authenticated",
              "dataKind": "live",
              "windows": [
                {
                  "id": "session",
                  "label": "5 Hour",
                  "usedPercent": null,
                  "resetAt": null,
                  "source": "provider_reported"
                }
              ],
              "balance": null,
              "balanceCurrency": null,
              "error": null,
              "stale": false
            }
          ]
        }"#;
        let envelope = parse_envelope(json);
        assert!(format_status(&envelope.providers, fixed_now())
            .contains("5 Hour    used: Unavailable     Reset unknown\n"));
    }

    #[test]
    fn mock_providers_and_never_fetched_providers_are_omitted() {
        let json = r#"{
          "app": "ellie",
          "version": "0.1.0",
          "providers": [
            {
              "id": "ellie-demo",
              "name": "Ellie Demo",
              "authState": "unsupported",
              "dataKind": "mock",
              "windows": [
                {
                  "id": "sample",
                  "label": "Sample allowance",
                  "usedPercent": 41.0,
                  "resetAt": "2026-09-10T10:00:00Z",
                  "source": "provider_reported"
                }
              ],
              "balance": null,
              "balanceCurrency": null,
              "error": null,
              "stale": false
            },
            {
              "id": "openai-api",
              "name": "OpenAI API",
              "authState": null,
              "dataKind": null,
              "windows": [],
              "balance": null,
              "balanceCurrency": null,
              "error": null,
              "stale": false
            }
          ]
        }"#;
        let envelope = parse_envelope(json);
        let output = format_status(&envelope.providers, fixed_now());
        assert_eq!(output, "Ellie\n\nNo provider usage data available\n");
        assert!(!output.contains("Ellie Demo"));
        assert!(!output.contains("Sample allowance"));
    }

    // -- error mapping and exit codes --------------------------------------

    #[test]
    fn maps_http_statuses_to_cli_errors() {
        use reqwest::StatusCode;
        assert_eq!(
            ensure_success(StatusCode::UNAUTHORIZED),
            Err(CliError::Unauthorized)
        );
        assert_eq!(
            ensure_success(StatusCode::FORBIDDEN),
            Err(CliError::Unauthorized)
        );
        assert_eq!(
            ensure_success(StatusCode::SERVICE_UNAVAILABLE),
            Err(CliError::ServerTokenNotConfigured)
        );
        assert_eq!(
            ensure_success(StatusCode::NOT_FOUND),
            Err(CliError::ProviderNotFound)
        );
        assert_eq!(
            ensure_success(StatusCode::INTERNAL_SERVER_ERROR),
            Err(CliError::ServerError(500))
        );
        assert!(ensure_success(StatusCode::OK).is_ok());
    }

    #[test]
    fn maps_errors_to_distinct_exit_codes() {
        assert_eq!(CliError::EllieNotRunning.exit_code(), 1);
        assert_eq!(CliError::Timeout.exit_code(), 1);
        assert_eq!(CliError::NetworkUnavailable.exit_code(), 1);
        assert_eq!(CliError::Unauthorized.exit_code(), 3);
        assert_eq!(CliError::ServerTokenNotConfigured.exit_code(), 4);
        assert_eq!(CliError::ProviderNotFound.exit_code(), 5);
        assert_eq!(CliError::ServerError(500).exit_code(), 6);
        assert_eq!(CliError::MalformedResponse.exit_code(), 6);
    }

    #[test]
    fn friendly_errors_never_contain_raw_bodies_or_tokens() {
        let friendly = format!(
            "{} {} {} {}",
            CliError::Unauthorized.friendly(),
            CliError::ServerTokenNotConfigured.friendly(),
            CliError::MalformedResponse.friendly(),
            CliError::ServerError(500).friendly()
        );
        for forbidden in ["sk-proj", "Bearer", "api_token_not_configured"] {
            assert!(
                !friendly.contains(forbidden),
                "forbidden text leaked: {forbidden}"
            );
        }
    }

    // -- tiny mock HTTP server --------------------------------------------

    /// Serves one canned response and returns the base URL plus the captured
    /// request text so tests can assert path and Authorization header.
    async fn serve_once(status_line: &'static str, body: String) -> (String, JoinHandle<String>) {
        serve_once_after(status_line, body, Duration::ZERO).await
    }

    async fn serve_once_after(
        status_line: &'static str,
        body: String,
        delay: Duration,
    ) -> (String, JoinHandle<String>) {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind mock server");
        let port = listener.local_addr().expect("bound port").port();
        let handle = tokio::spawn(async move {
            let (mut socket, _peer) = listener.accept().await.expect("accept mock request");
            let mut request = vec![0u8; 4096];
            let read = socket.read(&mut request).await.expect("read mock request");
            let request_text = String::from_utf8_lossy(&request[..read]).into_owned();
            tokio::time::sleep(delay).await;
            let response = format!(
                "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.shutdown().await;
            request_text
        });
        (format!("http://127.0.0.1:{port}/api/v1"), handle)
    }

    fn test_client() -> reqwest::Client {
        build_client().expect("test client")
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetches_and_formats_usage_from_local_api() {
        let (base, handle) = serve_once("200 OK", SECTION38_ENVELOPE.to_string()).await;
        let envelope = get_usage(&test_client(), &base, "test-token")
            .await
            .expect("usage fetch");
        assert_eq!(envelope.providers.len(), 3);
        let output = format_status(&envelope.providers, fixed_now());
        assert!(output.contains("Balance   $8.42"));
        let request = handle.await.expect("mock server finished");
        assert!(request.starts_with("GET /api/v1/usage"));
        // HTTP header names are case-insensitive; reqwest writes them lowercase.
        assert!(request
            .to_lowercase()
            .contains("authorization: bearer test-token"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn posts_refresh_with_auth_and_formats_both_headings_exactly() {
        let body = r#"{
          "busy": false,
          "providers": []
        }"#;
        let (base, handle) = serve_once("200 OK", body.to_string()).await;
        let response = post_refresh(&test_client(), &base, "refresh-token")
            .await
            .expect("refresh request");
        assert_eq!(
            format_refresh_response(&response, fixed_now()),
            "Refreshed.\n\nEllie\n\nNo provider usage data available\n"
        );
        let request = handle.await.expect("mock server finished");
        assert!(request.starts_with("POST /api/v1/refresh"));
        assert!(request
            .to_lowercase()
            .contains("authorization: bearer refresh-token"));

        let busy = ApiRefreshResponse {
            busy: true,
            providers: Vec::new(),
        };
        assert_eq!(
            format_refresh_response(&busy, fixed_now()),
            "Refresh already in progress.\n\nEllie\n\nNo provider usage data available\n"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn refresh_timeout_is_independently_configurable() {
        let body = r#"{"busy":false,"providers":[]}"#;
        let (base, handle) =
            serve_once_after("200 OK", body.to_string(), Duration::from_millis(40)).await;
        let error = post_refresh_with_timeout(
            &test_client(),
            &base,
            "test-token",
            Duration::from_millis(5),
        )
        .await
        .expect_err("short injected deadline should time out");
        assert_eq!(error, CliError::Timeout);
        let _ = handle.await.expect("mock server finished");

        let (base, handle) =
            serve_once_after("200 OK", body.to_string(), Duration::from_millis(20)).await;
        let response = post_refresh_with_timeout(
            &test_client(),
            &base,
            "test-token",
            Duration::from_millis(200),
        )
        .await
        .expect("longer injected deadline should allow refresh");
        assert!(!response.busy);
        let _ = handle.await.expect("mock server finished");
        assert!(REFRESH_REQUEST_TIMEOUT > STATUS_REQUEST_TIMEOUT);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn configured_http_proxy_never_receives_loopback_token() {
        let proxy = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind proxy trap");
        let proxy_url = format!(
            "http://127.0.0.1:{}",
            proxy.local_addr().expect("proxy address").port()
        );
        let proxy_trap = tokio::spawn(async move {
            let Ok(Ok((mut socket, _peer))) =
                tokio::time::timeout(Duration::from_millis(200), proxy.accept()).await
            else {
                return None;
            };
            let mut request = vec![0u8; 4096];
            let read = socket.read(&mut request).await.expect("read proxy request");
            let request_text = String::from_utf8_lossy(&request[..read]).into_owned();
            let response =
                b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            let _ = socket.write_all(response).await;
            Some(request_text)
        });
        let (base, destination) = serve_once("200 OK", SECTION38_ENVELOPE.to_string()).await;

        let client = {
            let _lock = PROXY_ENV_LOCK.lock().expect("proxy environment lock");
            let _http_proxy = EnvVarGuard::set("HTTP_PROXY", Some(&proxy_url));
            let _all_proxy = EnvVarGuard::set("ALL_PROXY", Some(&proxy_url));
            let _no_proxy = EnvVarGuard::set("NO_PROXY", None);
            #[cfg(not(windows))]
            let _http_proxy_lower = EnvVarGuard::set("http_proxy", Some(&proxy_url));
            #[cfg(not(windows))]
            let _all_proxy_lower = EnvVarGuard::set("all_proxy", Some(&proxy_url));
            #[cfg(not(windows))]
            let _no_proxy_lower = EnvVarGuard::set("no_proxy", None);
            build_client().expect("proxy-disabled client")
        };

        let envelope = get_usage_with_timeout(
            &client,
            &base,
            "proxy-secret-token",
            Duration::from_millis(500),
        )
        .await
        .expect("direct loopback request");
        assert_eq!(envelope.providers.len(), 3);
        let direct_request = destination.await.expect("destination finished");
        assert!(direct_request
            .to_lowercase()
            .contains("authorization: bearer proxy-secret-token"));
        assert_eq!(proxy_trap.await.expect("proxy trap finished"), None);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn rejects_503_unconfigured_token() {
        let (base, handle) = serve_once(
            "503 Service Unavailable",
            r#"{"error": "api_token_not_configured"}"#.to_string(),
        )
        .await;
        let error = get_usage(&test_client(), &base, "ignored")
            .await
            .expect_err("should fail");
        assert_eq!(error, CliError::ServerTokenNotConfigured);
        assert!(handle
            .await
            .expect("mock server finished")
            .contains("/api/v1/usage"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn rejects_401_and_403_auth_failures() {
        for status in ["401 Unauthorized", "403 Forbidden"] {
            let (base, handle) =
                serve_once(status, r#"{"error": "unauthorized"}"#.to_string()).await;
            let error = get_usage(&test_client(), &base, "wrong-token")
                .await
                .expect_err("should fail");
            assert_eq!(error, CliError::Unauthorized);
            assert!(handle
                .await
                .expect("mock server finished")
                .contains("Bearer wrong-token"));
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn rejects_malformed_response_body() {
        let (base, handle) = serve_once("200 OK", "definitely not json".to_string()).await;
        let error = get_usage(&test_client(), &base, "test-token")
            .await
            .expect_err("should fail");
        assert_eq!(error, CliError::MalformedResponse);
        let _ = handle.await.expect("mock server finished");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reports_connection_refused_when_app_not_running() {
        // Bind and immediately drop so nothing is listening on that port.
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind temporary port");
        let port = listener.local_addr().expect("bound port").port();
        drop(listener);
        let base = format!("http://127.0.0.1:{port}/api/v1");
        let error = get_usage(&test_client(), &base, "test-token")
            .await
            .expect_err("should fail");
        assert_eq!(error, CliError::EllieNotRunning);
    }

    #[test]
    fn parses_commands() {
        assert_eq!(parse_command(&[]), Ok(Command::Help));
        assert_eq!(parse_command(&["status".to_string()]), Ok(Command::Status));
        assert_eq!(
            parse_command(&["refresh".to_string()]),
            Ok(Command::Refresh)
        );
        assert_eq!(
            parse_command(&["version".to_string()]),
            Ok(Command::Version)
        );
        assert_eq!(parse_command(&["help".to_string()]), Ok(Command::Help));
        assert!(parse_command(&["bogus".to_string()]).is_err());
        assert!(parse_command(&["status".to_string(), "extra".to_string()]).is_err());
    }
}
