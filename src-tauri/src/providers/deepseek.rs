use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;

use super::{
    AuthState, DataKind, DetectionResult, ProviderCapabilities, ProviderError, UsageProvider,
    UsageSnapshot,
};

const PROVIDER_ID: &str = "deepseek";
const DISPLAY_NAME: &str = "DeepSeek";
const API_BASE_URL: &str = "https://api.deepseek.com";
const BALANCE_PATH: &str = "/user/balance";
/// Prefer USD balances when the response reports multiple currencies.
const PREFERRED_CURRENCY: &str = "USD";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const FETCH_TIMEOUT: Duration = Duration::from_secs(60);

/// Reads the DeepSeek account balance and API availability through the
/// documented `GET /user/balance` endpoint. DeepSeek exposes no quota
/// windows or usage reports, so this provider deliberately shows only the
/// account balance (with its real currency) and never fabricates windows.
pub struct DeepSeekProvider;

#[async_trait]
impl UsageProvider for DeepSeekProvider {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }

    fn display_name(&self) -> &'static str {
        DISPLAY_NAME
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            quota_windows: false,
            token_usage: false,
            account_balance: true,
            credits: false,
            cost_tracking: false,
            local_history: false,
        }
    }

    async fn detect(&self) -> Result<DetectionResult, ProviderError> {
        Ok(detect_state().await)
    }

    async fn authenticate(&self) -> Result<AuthState, ProviderError> {
        // Key entry happens through the Settings UI; report the detected state.
        Ok(detect_state().await.auth_state)
    }

    async fn fetch_usage(&self) -> Result<UsageSnapshot, ProviderError> {
        let store = crate::credentials::WindowsCredentialStore;
        let key = tokio::task::spawn_blocking(move || {
            crate::credentials::provider_key(&store, PROVIDER_ID)
        })
        .await
        .map_err(|_| ProviderError::Unavailable)?
        .map_err(|_| ProviderError::Unavailable)?
        .ok_or(ProviderError::AuthenticationRequired)?;
        let api = DeepSeekApi::with_key(API_BASE_URL.to_string(), key);
        let balance = fetch_balance(&api).await?;
        Ok(balance_snapshot(balance))
    }
}

/// Query client for the balance endpoint. The key never leaves this struct
/// and is not logged.
struct DeepSeekApi {
    base_url: String,
    client: reqwest::Client,
    api_key: String,
}

impl DeepSeekApi {
    fn with_key(base_url: String, api_key: String) -> DeepSeekApi {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent(format!("ellie/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("reqwest client builds");
        DeepSeekApi {
            base_url,
            client,
            api_key,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Balance {
    value: Option<f64>,
    currency: Option<String>,
}

async fn fetch_balance(api: &DeepSeekApi) -> Result<Balance, ProviderError> {
    let url = reqwest::Url::parse(&format!("{}{}", api.base_url, BALANCE_PATH))
        .map_err(|_| ProviderError::Unavailable)?;
    let request = api
        .client
        .get(url)
        .header("Authorization", format!("Bearer {}", api.api_key))
        .header("Accept", "application/json");
    let response = tokio::time::timeout(FETCH_TIMEOUT, request.send())
        .await
        .map_err(|_| ProviderError::Unavailable)?
        .map_err(|_| ProviderError::Unavailable)?;
    let status = response.status();
    if !status.is_success() {
        return Err(classify_status(status.as_u16()));
    }
    let body: Value = tokio::time::timeout(FETCH_TIMEOUT, response.json())
        .await
        .map_err(|_| ProviderError::Unavailable)?
        .map_err(|_| ProviderError::Unavailable)?;
    parse_balance(&body)
}

fn balance_snapshot(balance: Balance) -> UsageSnapshot {
    UsageSnapshot {
        provider_id: PROVIDER_ID.to_string(),
        display_name: DISPLAY_NAME.to_string(),
        account_label: None,
        plan: None,
        has_subscription: None,
        capabilities: ProviderCapabilities {
            quota_windows: false,
            token_usage: false,
            account_balance: true,
            credits: false,
            cost_tracking: false,
            local_history: false,
        },
        auth_state: AuthState::Authenticated,
        data_kind: DataKind::Live,
        windows: Vec::new(),
        credits: None,
        balance: balance.value,
        balance_currency: balance.currency,
        spend_estimate: None,
        model: None,
        token_usage: None,
        fetched_at: Utc::now(),
    }
}

/// Extracts the balance: the USD entry when present, otherwise the first
/// currency reported. Unparseable or negative amounts become `None` rather
/// than a fabricated number; an empty `balance_infos` is "no balance known",
/// not zero.
fn parse_balance(body: &Value) -> Result<Balance, ProviderError> {
    let infos = body.get("balance_infos").and_then(Value::as_array);
    let Some(infos) = infos else {
        return Ok(Balance {
            value: None,
            currency: None,
        });
    };
    let preferred = infos
        .iter()
        .find(|info| info.get("currency").and_then(Value::as_str) == Some(PREFERRED_CURRENCY));
    let Some(info) = preferred.or_else(|| infos.first()) else {
        return Ok(Balance {
            value: None,
            currency: None,
        });
    };
    let currency = info
        .get("currency")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let value = match info.get("total_balance").and_then(Value::as_str) {
        Some(raw) => match raw.parse::<f64>() {
            Ok(value) if value.is_finite() && value >= 0.0 => Some(value),
            _ => None,
        },
        None => None,
    };
    Ok(Balance { value, currency })
}

async fn detect_state() -> DetectionResult {
    let store = crate::credentials::WindowsCredentialStore;
    let source = tokio::task::spawn_blocking(move || {
        crate::credentials::provider_key_status(&store, PROVIDER_ID).ok()
    })
    .await
    .unwrap_or(None)
    .unwrap_or(crate::credentials::KeySource::None);
    detection_from_source(source)
}

fn detection_from_source(source: crate::credentials::KeySource) -> DetectionResult {
    match source {
        crate::credentials::KeySource::Environment | crate::credentials::KeySource::CredentialManager => {
            DetectionResult {
                auth_state: AuthState::AuthenticationDetected,
                detail: Some(
                    "DeepSeek API key found (environment or Windows Credential Manager); account \
                     balance is read through the official /user/balance endpoint."
                        .to_string(),
                ),
            }
        }
        crate::credentials::KeySource::None => DetectionResult {
            auth_state: AuthState::AuthenticationRequired,
            detail: Some(
                "Add a DeepSeek key from platform.deepseek.com in Settings → Provider credentials. \
                 There are no quota windows on DeepSeek; Ellie shows your balance and an \
                 estimated spend."
                    .to_string(),
            ),
        },
    }
}

fn classify_status(status: u16) -> ProviderError {
    match status {
        401 => ProviderError::AuthenticationExpired,
        402 => ProviderError::Unavailable,
        403 => ProviderError::AuthenticationRequired,
        429 => ProviderError::Unavailable,
        400..=599 => ProviderError::Unavailable,
        _ => ProviderError::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{AuthState, DataKind, ProviderError};

    #[test]
    fn prefers_usd_balance_info() {
        let body: Value = serde_json::from_str(
            r#"{
                "is_available": true,
                "balance_infos": [
                    { "currency": "CNY", "total_balance": "110.00", "granted_balance": "10.00", "topped_up_balance": "100.00" },
                    { "currency": "USD", "total_balance": "12.34", "granted_balance": "0.00", "topped_up_balance": "12.34" }
                ]
            }"#,
        )
        .expect("fixture");
        let balance = parse_balance(&body).expect("balance");
        assert_eq!(balance.currency.as_deref(), Some("USD"));
        assert!(balance
            .value
            .is_some_and(|value| (value - 12.34).abs() < 1e-9));
    }

    #[test]
    fn falls_back_to_first_currency() {
        let body: Value = serde_json::from_str(
            r#"{"is_available": true, "balance_infos": [
                { "currency": "CNY", "total_balance": "110.00", "granted_balance": "0", "topped_up_balance": "110" }
            ]}"#,
        )
        .expect("fixture");
        let balance = parse_balance(&body).expect("balance");
        assert_eq!(balance.currency.as_deref(), Some("CNY"));
        assert!(balance
            .value
            .is_some_and(|value| (value - 110.0).abs() < 1e-9));
    }

    #[test]
    fn treats_missing_or_invalid_balance_as_unknown_not_zero() {
        for (fixture, expected) in [
            (r#"{"is_available": true, "balance_infos": []}"#, None),
            (r#"{"is_available": false}"#, None),
            (
                r#"{"balance_infos": [{ "currency": "USD", "total_balance": "not-a-number" }]}"#,
                None,
            ),
            (
                r#"{"balance_infos": [{ "currency": "USD", "total_balance": "-5.00" }]}"#,
                None,
            ),
            (
                r#"{"balance_infos": [{ "currency": "USD", "total_balance": "0.00" }]}"#,
                Some(0.0),
            ),
        ] {
            let body: Value = serde_json::from_str(fixture).expect("fixture");
            let balance = parse_balance(&body).expect("balance");
            assert_eq!(balance.value, expected, "fixture: {fixture}");
        }
    }

    #[test]
    fn classifies_balance_api_statuses() {
        assert_eq!(classify_status(401), ProviderError::AuthenticationExpired);
        assert_eq!(classify_status(403), ProviderError::AuthenticationRequired);
        assert_eq!(classify_status(402), ProviderError::Unavailable);
        assert_eq!(classify_status(429), ProviderError::Unavailable);
        assert_eq!(classify_status(500), ProviderError::Unavailable);
    }

    #[test]
    fn banner_snapshot_is_live_balance_only() {
        let snapshot = balance_snapshot(Balance {
            value: Some(110.0),
            currency: Some("CNY".into()),
        });
        assert_eq!(snapshot.provider_id, PROVIDER_ID);
        assert_eq!(snapshot.data_kind, DataKind::Live);
        assert_eq!(snapshot.auth_state, AuthState::Authenticated);
        assert_eq!(snapshot.balance, Some(110.0));
        assert_eq!(snapshot.balance_currency.as_deref(), Some("CNY"));
        assert!(snapshot.windows.is_empty());
        assert!(snapshot.token_usage.is_none());
        assert!(snapshot.capabilities.account_balance);
        assert!(!snapshot.capabilities.quota_windows);
        assert_eq!(snapshot.has_subscription, None);
    }

    #[test]
    fn detection_reports_missing_key_without_reading_real_credentials() {
        let state = detection_from_source(crate::credentials::KeySource::None);
        assert_eq!(state.auth_state, AuthState::AuthenticationRequired);
    }

    /// Hermetic fetch test: serve a canned /user/balance response over a
    /// local listener and assert the request path + aggregate result.
    #[tokio::test]
    async fn fetch_usage_against_local_server() {
        use std::io::{BufRead, Write};
        use std::net::TcpListener;

        const BODY: &str = r#"{"is_available": true, "balance_infos": [
            { "currency": "USD", "total_balance": "12.34", "granted_balance": "0", "topped_up_balance": "12.34" }
        ]}"#;
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let (sender, receiver) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut request = String::new();
            {
                let mut reader = std::io::BufReader::new(stream.try_clone().expect("clone"));
                let _ = reader.read_line(&mut request);
                let _ = reader.read_line(&mut request); // Authorization header
            }
            let _ = sender.send(request);
            let headers = format!(
                "HTTP/1.1 200\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                BODY.len()
            );
            let _ = stream.write_all(headers.as_bytes());
            let _ = stream.write_all(BODY.as_bytes());
            let _ = stream.flush();
        });

        let api = DeepSeekApi::with_key(format!("http://127.0.0.1:{port}"), "sk-test".to_string());
        let snapshot = balance_snapshot(fetch_balance(&api).await.expect("fetch"));
        let _ = thread.join();

        let request = receiver.try_recv().expect("request observed");
        assert!(request.starts_with("GET /user/balance"));
        assert!(request.contains("Bearer sk-test"));
        assert_eq!(snapshot.balance, Some(12.34));
        assert_eq!(snapshot.balance_currency.as_deref(), Some("USD"));
    }
}
