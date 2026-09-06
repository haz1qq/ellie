use std::{
    io::BufRead,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc::{channel, Receiver},
    thread,
    time::Duration,
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};

use super::{
    AuthState, DataKind, DetectionResult, MetricSource, ProviderCapabilities, ProviderError,
    UsageProvider, UsageSnapshot, UsageWindow,
};

const PROVIDER_ID: &str = "openai-codex";
const DISPLAY_NAME: &str = "OpenAI / Codex";
/// Upper bound for one request/reply exchange over app-server stdio.
const EXCHANGE_STEP_TIMEOUT: Duration = Duration::from_secs(30);
/// Upper bound for the whole fetch (process spawn plus exchange).
pub const FETCH_TIMEOUT: Duration = Duration::from_secs(60);

/// Keeps the child console window from flashing during a fetch.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Illustrative window labels derived from the backend window duration.
fn window_label(duration_mins: Option<i64>) -> String {
    match duration_mins {
        Some(300) => "5-hour limit".to_string(),
        Some(1440) => "Daily limit".to_string(),
        Some(10_080) => "Weekly limit".to_string(),
        Some(43_200) => "Monthly limit".to_string(),
        Some(525_600) => "Annual limit".to_string(),
        Some(minutes) => format!("Quota window ({minutes} minutes)"),
        None => "Quota window".to_string(),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetAccountRateLimitsResponse {
    account_id: Option<String>,
    #[allow(dead_code)]
    ordinary_usage_allowed: Option<bool>,
    rate_limits: RateLimitSnapshot,
    #[allow(dead_code)]
    rate_limits_by_limit_id: Option<std::collections::BTreeMap<String, RateLimitSnapshot>>,
    #[allow(dead_code)]
    rate_limit_reset_credits: Option<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RateLimitSnapshot {
    #[allow(dead_code)]
    limit_id: Option<String>,
    #[allow(dead_code)]
    limit_name: Option<String>,
    plan_type: Option<String>,
    primary: Option<RateLimitWindow>,
    secondary: Option<RateLimitWindow>,
    credits: Option<CreditsSnapshot>,
    #[allow(dead_code)]
    individual_limit: Option<Value>,
    #[allow(dead_code)]
    spend_control_reached: Option<bool>,
    #[allow(dead_code)]
    rate_limit_reached_type: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RateLimitWindow {
    resets_at: Option<i64>,
    used_percent: f64,
    window_duration_mins: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreditsSnapshot {
    balance: Option<String>,
    #[allow(dead_code)]
    has_credits: Option<bool>,
    #[allow(dead_code)]
    unlimited: Option<bool>,
}

/// Reads ChatGPT plan quota through the Codex CLI's own `app-server`
/// (`account/rateLimits/read` over stdio JSON-RPC). Codex owns login and
/// token refresh; this provider never receives or stores credentials.
pub struct OpenAiProvider;

#[async_trait]
impl UsageProvider for OpenAiProvider {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }

    fn display_name(&self) -> &'static str {
        DISPLAY_NAME
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            quota_windows: true,
            token_usage: false,
            account_balance: false,
            credits: true,
            cost_tracking: false,
            local_history: false,
        }
    }

    async fn detect(&self) -> Result<DetectionResult, ProviderError> {
        if !codex_present() {
            return Ok(DetectionResult {
                auth_state: AuthState::Unsupported,
                detail: Some(
                    "Codex CLI was not found on PATH; install it and run `codex login`."
                        .to_string(),
                ),
            });
        }
        if codex_auth_present() {
            Ok(DetectionResult {
                auth_state: AuthState::AuthenticationDetected,
                detail: Some(
                    "Codex CLI login found. Ellie reuses the codex app-server; no token is \
                     stored by Ellie."
                        .to_string(),
                ),
            })
        } else {
            Ok(DetectionResult {
                auth_state: AuthState::AuthenticationRequired,
                detail: Some("No Codex login found; run `codex login`.".to_string()),
            })
        }
    }

    async fn authenticate(&self) -> Result<AuthState, ProviderError> {
        // Reuse-only provider: report the detected state; logins happen through
        // the official `codex login` flows, not inside Ellie.
        Ok(self.detect().await?.auth_state)
    }

    async fn fetch_usage(&self) -> Result<UsageSnapshot, ProviderError> {
        let handle = tokio::task::spawn_blocking(fetch_usage_blocking);
        tokio::time::timeout(FETCH_TIMEOUT, handle)
            .await
            .map_err(|_| ProviderError::Unavailable)?
            .map_err(|_| ProviderError::Unavailable)?
    }
}

fn fetch_usage_blocking() -> Result<UsageSnapshot, ProviderError> {
    if !codex_present() {
        return Err(ProviderError::Unavailable);
    }
    let mut command = codex_app_server_command()?;
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|_| ProviderError::Unavailable)?;

    let reply = match run_exchange(&mut child) {
        Ok(reply) => reply,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };

    let response: GetAccountRateLimitsResponse =
        serde_json::from_value(reply).map_err(|_| ProviderError::Unavailable)?;
    snapshot_from_response(response, Utc::now()).map_err(|_| ProviderError::Unavailable)
}

/// Performs the app-server handshake and the `account/rateLimits/read` call
/// over newline-delimited JSON-RPC (no `jsonrpc` header on the wire). The
/// reply payload for the rate-limit request is returned.
fn run_exchange(child: &mut Child) -> Result<Value, ProviderError> {
    let stdin = child.stdin.take().ok_or(ProviderError::Unavailable)?;
    let stdout = child.stdout.take().ok_or(ProviderError::Unavailable)?;
    let stderr = child.stderr.take().ok_or(ProviderError::Unavailable)?;

    // Drain stderr so the child never blocks on a full pipe. Content is not
    // logged; diagnostics stay on the JSON-RPC channel.
    thread::spawn(move || {
        let _ = std::io::Read::read_to_end(&mut std::io::BufReader::new(stderr), &mut Vec::new());
    });

    let (sender, receiver) = channel::<Result<String, ProviderError>>();
    let reader_thread = thread::spawn(move || {
        let reader = std::io::BufReader::new(stdout);
        for line in reader.lines() {
            if sender
                .send(
                    line.map_err(|_| ProviderError::Unavailable)
                        .map(|line| line.trim().to_string()),
                )
                .is_err()
            {
                break;
            }
        }
    });

    let mut wire = Wire { stdin, receiver };
    let result = wire
        .send(json!({
            "method": "initialize",
            "id": 1,
            "params": {
                "clientInfo": {
                    "name": "ellie",
                    "title": "Ellie",
                    "version": env!("CARGO_PKG_VERSION"),
                }
            }
        }))
        .and_then(|_| wire.recv(child, 1))
        .and_then(|_| wire.send(json!({ "method": "notifications/initialized" })))
        .and_then(|_| wire.send(json!({ "method": "account/rateLimits/read", "id": 2 })))
        .and_then(|_| wire.recv(child, 2));
    // Kill the child before joining the reader thread so stdout reaches EOF and
    // the thread can exit; without this the exchange would hang forever.
    let _ = child.kill();
    let _ = child.wait();
    let _ = reader_thread.join();
    result
}

struct Wire {
    stdin: std::process::ChildStdin,
    receiver: Receiver<Result<String, ProviderError>>,
}

impl Wire {
    fn send(&mut self, message: Value) -> Result<(), ProviderError> {
        use std::io::Write;
        serde_json::to_writer(&mut self.stdin, &message).map_err(|_| ProviderError::Unavailable)?;
        self.stdin
            .write_all(b"\n")
            .and_then(|_| self.stdin.flush())
            .map_err(|_| ProviderError::Unavailable)
    }

    /// Reads newline-delimited messages until one matches `target_id`,
    /// skipping unsolicited notifications. Returns the `result` payload or
    /// maps a JSON-RPC error.
    fn recv(&self, child: &mut Child, target_id: i64) -> Result<Value, ProviderError> {
        loop {
            let line = match self.receiver.recv_timeout(EXCHANGE_STEP_TIMEOUT) {
                Ok(Ok(line)) => line,
                Ok(Err(error)) => return Err(error),
                Err(_) => return Err(kill_and(child, ProviderError::Unavailable)),
            };
            let message: Value = match serde_json::from_str(&line) {
                Ok(message) => message,
                Err(_) => continue, // tolerate non-JSON diagnostic lines
            };
            if message["id"].as_i64() != Some(target_id) {
                continue; // notification or another request's reply
            }
            if let Some(error) = message.get("error") {
                let detail = error["message"].as_str().unwrap_or("").to_string();
                return Err(kill_and(child, classify_error(&detail)));
            }
            return message
                .get("result")
                .cloned()
                .ok_or_else(|| kill_and(child, ProviderError::Unavailable));
        }
    }
}

fn kill_and(child: &mut Child, error: ProviderError) -> ProviderError {
    let _ = child.kill();
    let _ = child.wait();
    error
}

fn classify_error(detail: &str) -> ProviderError {
    let detail = detail.to_lowercase();
    if detail.contains("expire") || detail.contains("unauthorized") || detail.contains("401") {
        ProviderError::AuthenticationExpired
    } else if detail.contains("login") || detail.contains("authenticate") || detail.contains("auth")
    {
        ProviderError::AuthenticationRequired
    } else {
        ProviderError::Unavailable
    }
}

fn snapshot_from_response(
    response: GetAccountRateLimitsResponse,
    fetched_at: DateTime<Utc>,
) -> Result<UsageSnapshot, ProviderError> {
    let windows = windows_from_snapshot(&response.rate_limits)?;
    let credits = response
        .rate_limits
        .credits
        .as_ref()
        .and_then(|credits| credits.balance.as_deref())
        .and_then(parse_balance);
    Ok(UsageSnapshot {
        provider_id: PROVIDER_ID.to_string(),
        display_name: DISPLAY_NAME.to_string(),
        account_label: response.account_id,
        plan: response.rate_limits.plan_type.clone(),
        has_subscription: match response.rate_limits.plan_type.as_deref() {
            Some("free") => Some(false),
            Some(_) => Some(true),
            None => None,
        },
        capabilities: ProviderCapabilities {
            quota_windows: true,
            token_usage: false,
            account_balance: false,
            credits: true,
            cost_tracking: false,
            local_history: false,
        },
        auth_state: AuthState::Authenticated,
        data_kind: DataKind::Live,
        windows,
        credits,
        balance: None,
        balance_currency: None,
        token_usage: None,
        fetched_at,
    })
}

fn windows_from_snapshot(snapshot: &RateLimitSnapshot) -> Result<Vec<UsageWindow>, ProviderError> {
    let mut windows = Vec::with_capacity(2);
    if let Some(window) = &snapshot.primary {
        windows.push(usage_window("primary", window)?);
    }
    if let Some(window) = &snapshot.secondary {
        windows.push(usage_window("secondary", window)?);
    }
    Ok(windows)
}

fn usage_window(key: &str, window: &RateLimitWindow) -> Result<UsageWindow, ProviderError> {
    if !(0.0..=100.0).contains(&window.used_percent) {
        return Err(ProviderError::InvalidSnapshot);
    }
    Ok(UsageWindow {
        id: key.to_string(),
        label: window_label(window.window_duration_mins),
        used_percent: Some(window.used_percent),
        remaining_percent: Some(100.0 - window.used_percent),
        used_value: None,
        remaining_value: None,
        limit_value: None,
        unit: None,
        starts_at: None,
        reset_at: window
            .resets_at
            .and_then(|seconds| DateTime::from_timestamp(seconds, 0)),
        source: MetricSource::ProviderReported,
    })
}

/// `credits.balance` is a display string such as "$766.76". Only currency,
/// digits, commas, spaces, and a single decimal point are accepted; anything
/// else (including a minus sign) becomes `None` rather than a fabricated number.
fn parse_balance(balance: &str) -> Option<f64> {
    if balance.is_empty() {
        return None;
    }
    let allowed = |character: char| {
        character.is_ascii_digit() || matches!(character, '.' | ',' | ' ' | '$' | '€' | '£')
    };
    if balance.chars().any(|character| !allowed(character)) {
        return None;
    }
    let cleaned: String = balance
        .chars()
        .filter(|character| character.is_ascii_digit() || matches!(character, '.' | ','))
        .collect();
    cleaned
        .replace(',', "")
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value >= 0.0)
}

fn codex_home() -> PathBuf {
    if let Ok(home) = std::env::var("CODEX_HOME") {
        return PathBuf::from(home);
    }
    let base = if cfg!(windows) {
        std::env::var("USERPROFILE")
    } else {
        std::env::var("HOME")
    };
    base.map(|base| PathBuf::from(base).join(".codex"))
        .unwrap_or_default()
}

fn codex_auth_present() -> bool {
    // Codex may store credentials in the OS keyring instead; this is a
    // best-effort signal, and fetch_usage remains the authoritative check.
    codex_home().join("auth.json").is_file()
}

/// Whether the Codex CLI is available at all. Windows npm installs expose
/// `codex` only as `.cmd`/`.js` shims that `CreateProcess` cannot run, so
/// presence is checked via `where`, not by attempting to spawn `codex`.
fn codex_present() -> bool {
    #[cfg(windows)]
    {
        !codex_where_lines().is_empty()
    }
    #[cfg(not(windows))]
    {
        Command::new("codex")
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .is_ok()
    }
}

fn codex_app_server_command() -> Result<std::process::Command, ProviderError> {
    let mut command = codex_base_command()?;
    command.arg("app-server").arg("--stdio");
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    Ok(command)
}

#[cfg(windows)]
fn codex_base_command() -> Result<std::process::Command, ProviderError> {
    // Prefer the real native binary so the child can be terminated directly.
    if let Some(path) = resolve_native_codex(&codex_where_lines()) {
        return Ok(Command::new(path));
    }
    if codex_where_lines().is_empty() {
        return Err(ProviderError::Unavailable);
    }
    // Last resort: let cmd.exe resolve the npm shim. Only fixed, literal
    // arguments are passed, so no user input reaches the command line.
    let mut command = Command::new("cmd");
    command.args(["/C", "codex"]);
    Ok(command)
}

#[cfg(not(windows))]
fn codex_base_command() -> Result<std::process::Command, ProviderError> {
    Ok(Command::new("codex"))
}

#[cfg(windows)]
fn codex_where_lines() -> Vec<PathBuf> {
    let output = match Command::new("where")
        .arg("codex")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// Resolves the codex launcher from `where codex` output: a native `.exe`
/// directly, or the npm-installed native binary next to a `.cmd` shim.
#[cfg(windows)]
fn resolve_native_codex(where_lines: &[PathBuf]) -> Option<PathBuf> {
    if let Some(path) = where_lines.iter().find(|line| {
        line.extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
    }) {
        return Some(path.clone());
    }
    let shim_dir = where_lines.iter().find_map(|line| {
        let extension = line.extension()?.to_str()?.to_ascii_lowercase();
        (extension == "cmd" || extension == "bat").then(|| line.parent().map(Path::to_path_buf))?
    })?;
    find_native_codex_in(&shim_dir.join("node_modules").join("@openai"))
}

/// Bounded search for a vendored native `codex.exe` beneath an `@openai`
/// node_modules scope, for example
/// `@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe`.
#[cfg(windows)]
fn find_native_codex_in(root: &Path) -> Option<PathBuf> {
    fn walk(dir: &Path, depth: usize) -> Option<PathBuf> {
        if depth > 8 {
            return None;
        }
        let entries = std::fs::read_dir(dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = walk(&path, depth + 1) {
                    return Some(found);
                }
            } else if path
                .file_name()
                .is_some_and(|name| name.eq_ignore_ascii_case("codex.exe"))
                && path
                    .components()
                    .any(|component| component.as_os_str().eq_ignore_ascii_case("vendor"))
            {
                return Some(path);
            }
        }
        None
    }
    if !root.is_dir() {
        return None;
    }
    walk(root, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SANITIZED_RESPONSE: &str = r#"{
        "accountId": "acct_test_0000",
        "ordinaryUsageAllowed": true,
        "rateLimits": {
            "limitId": "codex",
            "planType": "plus",
            "primary": {
                "usedPercent": 25,
                "windowDurationMins": 300,
                "resetsAt": 1730947200
            },
            "secondary": {
                "usedPercent": 40,
                "windowDurationMins": 10080,
                "resetsAt": 1731542400
            },
            "credits": {
                "balance": "$766.76",
                "hasCredits": true,
                "unlimited": false
            },
            "rateLimitReachedType": null
        },
        "rateLimitResetCredits": { "availableCount": 2, "credits": null }
    }"#;

    fn parse_response() -> GetAccountRateLimitsResponse {
        serde_json::from_str(SANITIZED_RESPONSE).expect("sanitized fixture")
    }

    #[test]
    fn normalizes_sanitized_response_into_live_snapshot() {
        let snapshot = snapshot_from_response(parse_response(), Utc::now()).expect("snapshot");
        assert_eq!(snapshot.provider_id, PROVIDER_ID);
        assert_eq!(snapshot.display_name, DISPLAY_NAME);
        assert_eq!(snapshot.plan.as_deref(), Some("plus"));
        assert_eq!(snapshot.account_label.as_deref(), Some("acct_test_0000"));
        assert_eq!(snapshot.data_kind, DataKind::Live);
        assert_eq!(snapshot.auth_state, AuthState::Authenticated);
        assert!(snapshot.capabilities.quota_windows);
        assert!(snapshot.capabilities.credits);
        assert!(!snapshot.capabilities.token_usage);
        assert_eq!(snapshot.windows.len(), 2);

        let primary = &snapshot.windows[0];
        assert_eq!(primary.id, "primary");
        assert_eq!(primary.label, "5-hour limit");
        assert_eq!(primary.used_percent, Some(25.0));
        assert_eq!(primary.remaining_percent, Some(75.0));
        assert_eq!(primary.reset_at, DateTime::from_timestamp(1_730_947_200, 0));
        assert_eq!(primary.source, MetricSource::ProviderReported);

        let secondary = &snapshot.windows[1];
        assert_eq!(secondary.id, "secondary");
        assert_eq!(secondary.label, "Weekly limit");

        assert_eq!(snapshot.credits, Some(766.76));
        assert_eq!(snapshot.balance, None);
        assert_eq!(snapshot.token_usage, None);
    }

    #[test]
    fn handles_null_secondary_and_missing_credits() {
        let response: GetAccountRateLimitsResponse = serde_json::from_str(
            r#"{
                "rateLimits": {
                    "planType": null,
                    "primary": { "usedPercent": 100, "windowDurationMins": 300, "resetsAt": null },
                    "secondary": null,
                    "credits": null
                }
            }"#,
        )
        .expect("fixture");
        let snapshot = snapshot_from_response(response, Utc::now()).expect("snapshot");
        assert_eq!(snapshot.windows.len(), 1);
        assert_eq!(snapshot.windows[0].used_percent, Some(100.0));
        assert_eq!(snapshot.windows[0].remaining_percent, Some(0.0));
        assert!(snapshot.windows[0].reset_at.is_none());
        assert_eq!(snapshot.plan, None);
        assert_eq!(snapshot.credits, None);
        assert!(snapshot.account_label.is_none());
    }

    #[test]
    fn falls_back_to_descriptive_window_labels() {
        let response: GetAccountRateLimitsResponse = serde_json::from_str(
            r#"{"rateLimits": { "primary": { "usedPercent": 10, "windowDurationMins": 137, "resetsAt": null } } }"#,
        )
        .expect("fixture");
        let snapshot = snapshot_from_response(response, Utc::now()).expect("snapshot");
        assert_eq!(snapshot.windows[0].label, "Quota window (137 minutes)");
        assert_eq!(snapshot.windows.len(), 1);
    }

    #[test]
    fn rejects_out_of_range_percent() {
        let response: GetAccountRateLimitsResponse = serde_json::from_str(
            r#"{"rateLimits": { "primary": { "usedPercent": 150, "windowDurationMins": 300, "resetsAt": null } } }"#,
        )
        .expect("fixture");
        assert!(snapshot_from_response(response, Utc::now()).is_err());
    }

    #[test]
    fn parse_balance_accepts_currency_and_rejects_garbage() {
        assert_eq!(parse_balance("$766.76"), Some(766.76));
        assert_eq!(parse_balance("1,234.50"), Some(1234.5));
        assert_eq!(parse_balance("0"), Some(0.0));
        assert_eq!(parse_balance("unlimited"), None);
        assert_eq!(parse_balance(""), None);
        assert_eq!(parse_balance("-5"), None);
        assert_eq!(parse_balance("1.2.3"), None);
    }

    #[test]
    fn classifies_authentication_errors() {
        assert_eq!(
            classify_error("authentication required, please run `codex login`"),
            ProviderError::AuthenticationRequired
        );
        assert_eq!(
            classify_error("token expired or invalid"),
            ProviderError::AuthenticationExpired
        );
        assert_eq!(
            classify_error("Method not found: account/nope"),
            ProviderError::Unavailable
        );
    }

    #[cfg(windows)]
    #[test]
    fn resolve_native_codex_handles_exe_and_npm_shim_layouts(
    ) -> Result<(), Box<dyn std::error::Error>> {
        // A native .exe on PATH wins directly.
        let native = PathBuf::from(r"C:\tools\codex.exe");
        assert_eq!(
            resolve_native_codex(std::slice::from_ref(&native)),
            Some(native),
            "exe path used directly"
        );

        // npm global install: shim .cmd next to node_modules/@openai with a
        // vendored native binary beneath the platform package.
        let temp = tempfile::tempdir()?;
        let exe = temp.path().join(
            "node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe",
        );
        std::fs::create_dir_all(exe.parent().expect("exe dir"))?;
        std::fs::write(&exe, b"native")?;
        let shim = temp.path().join("codex.cmd");
        std::fs::write(&shim, "@echo off\n")?;
        let resolved = resolve_native_codex(&[shim]).expect("npm layout resolves to native binary");
        assert_eq!(resolved, exe);
        assert!(resolved.extension().is_some_and(|ext| ext == "exe"));

        // Nothing resolvable reports None (caller falls back to cmd /C).
        assert_eq!(resolve_native_codex(&[]), None);
        Ok(())
    }

    /// Manual live smoke check (requires an installed, logged-in Codex CLI):
    /// `ELLIE_LIVE_CODEX=1 cargo test live_codex_fetch_round_trip -- --ignored`
    /// is not used; set the env var and run the test normally -- it no-ops
    /// when the variable is absent so the offline suite stays hermetic.
    #[test]
    fn live_codex_fetch_round_trip() {
        if std::env::var("ELLIE_LIVE_CODEX").is_err() {
            return;
        }
        let snapshot = fetch_usage_blocking().expect("live codex fetch");
        assert_eq!(snapshot.provider_id, PROVIDER_ID);
        assert_eq!(snapshot.data_kind, DataKind::Live);
        assert_eq!(snapshot.auth_state, AuthState::Authenticated);
        assert!(snapshot.capabilities.quota_windows);
        assert!(
            !snapshot.windows.is_empty(),
            "expected at least one quota window from the live account"
        );
    }

    /// When run with the mock-server env var, acts as a canned codex
    /// app-server for the exchange test; otherwise it is a no-op test.
    #[test]
    fn mock_codex_app_server() {
        let Some(mode) = std::env::var("ELLIE_MOCK_CODEX_SERVER").ok() else {
            return;
        };
        assert_eq!(mode, "rate-limits", "unknown mock mode");
        let stdin = std::io::stdin();
        use std::io::{BufRead, Write};
        for line in stdin.lock().lines() {
            let line = line.expect("mock stdin line");
            let message: Value = serde_json::from_str(&line).expect("mock request");
            let method = message["method"].as_str().unwrap_or("");
            let Some(id) = message["id"].as_i64() else {
                continue; // notifications
            };
            let response = match method {
                "initialize" => json!({
                    "id": id,
                    "result": {
                        "userAgent": "codex-mock",
                        "codexHome": "/tmp/mock",
                        "platformFamily": "test",
                        "platformOs": "test"
                    }
                }),
                "account/rateLimits/read" => {
                    json!({ "id": id, "result": serde_json::from_str::<Value>(SANITIZED_RESPONSE).unwrap() })
                }
                _ => {
                    json!({ "id": id, "error": { "code": -32601, "message": "Method not found" } })
                }
            };
            let _ = std::io::stdout().write_all(response.to_string().as_bytes());
            let _ = std::io::stdout().write_all(b"\n");
            let _ = std::io::stdout().flush();
        }
    }

    /// Spawns this test binary as a mock app-server over real stdio pipes and
    /// drives the real handshake + read exchange through `run_exchange`.
    #[test]
    fn exchange_completes_against_mock_codex_app_server() {
        if std::env::var("ELLIE_MOCK_CODEX_SERVER").is_ok() {
            return; // running as the mock child: skip
        }
        let mut child = Command::new(std::env::current_exe().expect("test exe"))
            .arg("--exact")
            .arg("providers::openai::tests::mock_codex_app_server")
            .arg("--nocapture")
            .env("ELLIE_MOCK_CODEX_SERVER", "rate-limits")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn mock app-server");

        let reply = run_exchange(&mut child).expect("exchange");
        let _ = child.kill();
        let _ = child.wait();

        let snapshot = snapshot_from_response(
            serde_json::from_value(reply).expect("structured reply"),
            Utc::now(),
        )
        .expect("snapshot");
        assert_eq!(snapshot.windows.len(), 2);
        assert_eq!(snapshot.windows[0].used_percent, Some(25.0));
        assert_eq!(snapshot.plan.as_deref(), Some("plus"));
    }
}
