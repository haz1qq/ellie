use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;

use super::{
    AuthState, DataKind, DetectionResult, MetricSource, ProviderCapabilities, ProviderError,
    TokenUsage, UsageProvider, UsageSnapshot,
};

const PROVIDER_ID: &str = "anthropic-claude";
const DISPLAY_NAME: &str = "Anthropic / Claude";
const API_BASE_URL: &str = "https://api.anthropic.com";
const API_VERSION: &str = "2023-06-01";
/// Report window: the trailing 30 days, one bucket per day.
const REPORT_DAYS: i64 = 30;
/// Maximum number of paginated pages followed per report (bounded work).
const MAX_PAGES: usize = 4;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const FETCH_TIMEOUT: Duration = Duration::from_secs(60);

/// Reads Anthropic API usage and cost through the documented Admin API
/// (Usage and Cost reports). This is the pay-as-you-go surface: there are no
/// subscription windows (5-hour/weekly) on API billing, so the provider
/// reports token activity and cost, not quota windows.
pub struct AnthropicProvider;

#[async_trait]
impl UsageProvider for AnthropicProvider {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }

    fn display_name(&self) -> &'static str {
        DISPLAY_NAME
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            quota_windows: false,
            token_usage: true,
            account_balance: false,
            credits: false,
            cost_tracking: true,
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
        let api = AnthropicApi::with_key(API_BASE_URL.to_string(), key);
        let snapshot = anthropic_api_fetch(api).await?;
        Ok(snapshot)
    }
}

/// Query client for the Admin API usage and cost reports. The key never
/// leaves this struct and is not logged.
struct AnthropicApi {
    base_url: String,
    client: reqwest::Client,
    api_key: String,
}

impl AnthropicApi {
    fn with_key(base_url: String, api_key: String) -> AnthropicApi {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent(format!("ellie/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("reqwest client builds");
        AnthropicApi {
            base_url,
            client,
            api_key,
        }
    }
}

async fn anthropic_api_fetch(api: AnthropicApi) -> Result<UsageSnapshot, ProviderError> {
    let now = Utc::now();
    let starting_at = now - chrono::Duration::days(REPORT_DAYS);

    let (usage_calls, cost_calls) = tokio::join!(
        fetch_usage_report(&api, &starting_at, &now),
        fetch_cost_report(&api, &starting_at, &now),
    );

    let usage = usage_calls?;
    let cost = cost_calls?;

    let input_tokens = usage.iter().map(|row| row.input).sum::<u64>();
    let output_tokens = usage.iter().map(|row| row.output).sum::<u64>();
    let cached_input_tokens = usage.iter().map(|row| row.cached_input).sum::<u64>();
    let total_tokens = input_tokens + output_tokens;
    let estimated_cost_usd = cost.iter().map(|point| point.amount_usd).sum::<f64>();
    let model = dominant_model(&usage);

    Ok(UsageSnapshot {
        provider_id: PROVIDER_ID.to_string(),
        display_name: DISPLAY_NAME.to_string(),
        account_label: None,
        plan: None,
        has_subscription: None,
        capabilities: ProviderCapabilities {
            quota_windows: false,
            token_usage: true,
            account_balance: false,
            credits: false,
            cost_tracking: true,
            local_history: false,
        },
        auth_state: AuthState::Authenticated,
        data_kind: DataKind::Live,
        windows: Vec::new(),
        credits: None,
        balance: None,
        balance_currency: None,
        spend_estimate: None,
        model,
        token_usage: Some(TokenUsage {
            input_tokens: Some(input_tokens),
            output_tokens: Some(output_tokens),
            cached_input_tokens: Some(cached_input_tokens),
            cached_output_tokens: None,
            reasoning_tokens: None,
            total_tokens: Some(total_tokens),
            request_count: None,
            estimated_cost_usd: Some(estimated_cost_usd),
            source: MetricSource::LocallyCalculated,
        }),
        fetched_at: now,
    })
}

#[derive(Default)]
struct UsageRow {
    input: u64,
    output: u64,
    cached_input: u64,
    model: Option<String>,
}

#[derive(Default)]
struct CostPoint {
    amount_usd: f64,
}

/// The model with the most tokens in the report window, when any model is
/// named. `None` when the source does not identify models — never invented.
fn dominant_model(rows: &[UsageRow]) -> Option<String> {
    let mut totals: std::collections::BTreeMap<&str, u64> = std::collections::BTreeMap::new();
    for row in rows {
        if let Some(model) = row.model.as_deref() {
            *totals.entry(model).or_default() += row.input + row.output;
        }
    }
    totals
        .into_iter()
        .max_by_key(|(_, total)| *total)
        .map(|(model, _)| model.to_string())
}

fn parse_usage_rows(data: &[Value]) -> Result<Vec<UsageRow>, ProviderError> {
    let mut rows = Vec::new();
    for bucket in data {
        let results = bucket.get("results").and_then(Value::as_array);
        let Some(results) = results else { continue };
        for result in results {
            let row = parse_usage_row(result)?;
            rows.push(row);
        }
    }
    Ok(rows)
}

/// Extracts the token fields of one usage result row. Missing fields
/// contribute zero; present-but-negative values reject the report, and
/// non-numeric values are treated as missing rather than crashing the parse.
fn parse_usage_row(result: &Value) -> Result<UsageRow, ProviderError> {
    let uncached = token_field(result, "uncached_input_tokens")?;
    let cache_read = token_field(result, "cache_read_input_tokens")?;
    let cache_creation = creation_tokens(result)?;
    let output = token_field(result, "output_tokens")?;
    Ok(UsageRow {
        input: uncached + cache_read + cache_creation,
        output,
        cached_input: cache_read + cache_creation,
        model: result
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

fn token_field(value: &Value, key: &str) -> Result<u64, ProviderError> {
    match value.get(key) {
        None => Ok(0),
        Some(Value::Number(number)) => match number.as_i64() {
            Some(amount) if amount >= 0 => Ok(amount as u64),
            Some(_) => Err(ProviderError::InvalidSnapshot),
            None => Ok(0), // non-integer numerics are treated as missing
        },
        Some(_) => Ok(0), // non-numeric shapes are treated as missing
    }
}

fn creation_tokens(value: &Value) -> Result<u64, ProviderError> {
    let Some(creation) = value.get("cache_creation") else {
        return Ok(0);
    };
    let ephemeral_1h = token_field(creation, "ephemeral_1h_input_tokens")?;
    let ephemeral_5m = token_field(creation, "ephemeral_5m_input_tokens")?;
    Ok(ephemeral_1h + ephemeral_5m)
}

fn parse_cost_points(data: &[Value]) -> Result<Vec<CostPoint>, ProviderError> {
    let mut points = Vec::new();
    for bucket in data {
        let results = bucket.get("results").and_then(Value::as_array);
        let Some(results) = results else { continue };
        for result in results {
            points.push(CostPoint {
                amount_usd: cost_amount(result)?,
            });
        }
    }
    Ok(points)
}

/// Cost amounts are decimal strings in the lowest currency units (cents);
/// `"123.45"` in USD means `$1.23`. Non-numeric amounts are treated as
/// missing (zero); negative amounts pass through (e.g. refunds).
fn cost_amount(result: &Value) -> Result<f64, ProviderError> {
    let Some(amount) = result.get("amount") else {
        return Ok(0.0);
    };
    let raw = match amount {
        Value::String(raw) => raw.clone(),
        Value::Number(number) => number.to_string(),
        _ => return Ok(0.0),
    };
    match raw.parse::<f64>() {
        Ok(value) if value.is_finite() => Ok(value / 100.0),
        _ => Ok(0.0),
    }
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
        crate::credentials::KeySource::Environment
        | crate::credentials::KeySource::CredentialManager => DetectionResult {
            auth_state: AuthState::AuthenticationDetected,
            detail: Some(
                "Anthropic admin API key found (environment or Windows Credential Manager); \
                     usage and cost reports are read through the Admin API."
                    .to_string(),
            ),
        },
        crate::credentials::KeySource::None => DetectionResult {
            auth_state: AuthState::AuthenticationRequired,
            detail: Some(
                "Add an Anthropic admin key (sk-ant-admin) scoped for the usage and cost reports \
                 in Settings → Provider credentials."
                    .to_string(),
            ),
        },
    }
}

async fn fetch_usage_report(
    api: &AnthropicApi,
    starting_at: &DateTime<Utc>,
    ending_at: &DateTime<Utc>,
) -> Result<Vec<UsageRow>, ProviderError> {
    let data = fetch_paginated(
        api,
        "/v1/organizations/usage_report/messages",
        &[
            ("starting_at", &starting_at.to_rfc3339()),
            ("ending_at", &ending_at.to_rfc3339()),
            ("bucket_width", &"1d".to_string()),
        ],
    )
    .await?;
    parse_usage_rows(&data)
}

async fn fetch_cost_report(
    api: &AnthropicApi,
    starting_at: &DateTime<Utc>,
    ending_at: &DateTime<Utc>,
) -> Result<Vec<CostPoint>, ProviderError> {
    let data = fetch_paginated(
        api,
        "/v1/organizations/cost_report",
        &[
            ("starting_at", &starting_at.to_rfc3339()),
            ("ending_at", &ending_at.to_rfc3339()),
            ("bucket_width", &"1d".to_string()),
            ("limit", &"31".to_string()),
        ],
    )
    .await?;
    parse_cost_points(&data)
}

/// Follows `data` + `next_page` pagination for a report endpoint, bounded by
/// `MAX_PAGES`. Returns the concatenated `data` arrays.
async fn fetch_paginated(
    api: &AnthropicApi,
    path: &str,
    params: &[(&str, &String)],
) -> Result<Vec<Value>, ProviderError> {
    let mut data = Vec::new();
    let mut page: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let (page_data, next_page) = fetch_page(api, path, params, page.as_deref()).await?;
        data.extend(page_data);
        match next_page {
            Some(next) => page = Some(next),
            None => break,
        }
    }
    Ok(data)
}

async fn fetch_page(
    api: &AnthropicApi,
    path: &str,
    params: &[(&str, &String)],
    page: Option<&str>,
) -> Result<(Vec<Value>, Option<String>), ProviderError> {
    let mut pairs: Vec<(&str, &str)> = params
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect();
    if let Some(page) = page {
        pairs.push(("page", page));
    }
    let url = reqwest::Url::parse_with_params(&format!("{}{}", api.base_url, path), pairs)
        .map_err(|_| ProviderError::Unavailable)?;
    let request = api
        .client
        .get(url)
        .header("anthropic-version", API_VERSION)
        .header("x-api-key", &api.api_key);
    let response = tokio::time::timeout(FETCH_TIMEOUT, request.send())
        .await
        .map_err(|_| ProviderError::Unavailable)?
        .map_err(classify_http_error)?;
    let status = response.status();
    if !status.is_success() {
        return Err(classify_status(status.as_u16()));
    }
    let body: Value = tokio::time::timeout(FETCH_TIMEOUT, response.json())
        .await
        .map_err(|_| ProviderError::Unavailable)?
        .map_err(|_| ProviderError::Unavailable)?;
    let page_data = body
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let next_page = body
        .get("next_page")
        .and_then(Value::as_str)
        .map(str::to_owned);
    Ok((page_data, next_page))
}

/// Transport-level send failures (DNS, connect, TLS, timeout) surface as
/// unavailable; HTTP statuses are classified separately from the response.
fn classify_http_error(_error: reqwest::Error) -> ProviderError {
    ProviderError::Unavailable
}

fn classify_status(status: u16) -> ProviderError {
    match status {
        401 => ProviderError::AuthenticationExpired,
        403 => ProviderError::AuthenticationRequired,
        429 => ProviderError::Unavailable,
        400..=599 => ProviderError::Unavailable,
        _ => ProviderError::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{DataKind, MetricSource};

    const USAGE_FIXTURE: &str = r#"{
        "data": [
            {
                "starting_at": "2026-08-07T00:00:00Z",
                "ending_at": "2026-08-08T00:00:00Z",
                "results": [
                    { "model": "claude-opus-5", "uncached_input_tokens": 1500,
                      "cache_read_input_tokens": 200,
                      "cache_creation": { "ephemeral_1h_input_tokens": 1000, "ephemeral_5m_input_tokens": 500 },
                      "output_tokens": 500 },
                    { "model": "claude-sonnet-5", "uncached_input_tokens": 4000,
                      "output_tokens": 300 }
                ]
            }
        ],
        "has_more": false,
        "next_page": null
    }"#;

    const COST_FIXTURE: &str = r#"{
        "data": [
            { "results": [ { "amount": "123.45", "cost_type": "tokens" },
                           { "amount": "76.55", "cost_type": "tokens" } ] }
        ],
        "has_more": false,
        "next_page": null
    }"#;

    #[test]
    fn sums_usage_rows_and_maps_cache_fields() {
        let data: Value = serde_json::from_str(USAGE_FIXTURE).expect("fixture");
        let rows = parse_usage_rows(data["data"].as_array().expect("data")).expect("rows");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].input, 1500 + 200 + 1000 + 500);
        assert_eq!(rows[0].cached_input, 200 + 1000 + 500);
        assert_eq!(rows[0].output, 500);
        assert_eq!(rows[1].input, 4000);
        assert_eq!(rows[1].cached_input, 0);
        assert_eq!(rows[1].output, 300);
        let input: u64 = rows.iter().map(|row| row.input).sum();
        let output: u64 = rows.iter().map(|row| row.output).sum();
        assert_eq!(input, 7200);
        assert_eq!(output, 800);
    }

    #[test]
    fn converts_cost_cents_to_dollars() {
        let data: Value = serde_json::from_str(COST_FIXTURE).expect("fixture");
        let points = parse_cost_points(data["data"].as_array().expect("data")).expect("points");
        assert_eq!(points.len(), 2);
        let total: f64 = points.iter().map(|point| point.amount_usd).sum();
        assert!(
            (total - 2.0).abs() < 1e-9,
            "123.45c + 76.55c = $2.00, got {total}"
        );
    }

    #[test]
    fn treats_missing_and_non_numeric_as_zero_but_rejects_negative() {
        let clean: Value = serde_json::from_str(
            r#"{"results":[{ "output_tokens": 12, "cache_read_input_tokens": "n/a" }]}"#,
        )
        .unwrap();
        let row = parse_usage_row(&clean["results"][0]).expect("tolerant row");
        assert_eq!(row.output, 12);
        assert_eq!(row.cached_input, 0);
        assert_eq!(row.input, 0);

        let negative: Value =
            serde_json::from_str(r#"{"results":[{ "output_tokens": -1 }]}"#).unwrap();
        assert!(parse_usage_row(&negative["results"][0]).is_err());
    }

    #[test]
    fn empty_reports_mean_zero_usage() {
        assert_eq!(
            parse_usage_rows(&[])
                .expect("empty")
                .iter()
                .map(|r| r.input)
                .sum::<u64>(),
            0
        );
        assert_eq!(
            parse_cost_points(&[])
                .expect("empty")
                .iter()
                .map(|p| p.amount_usd)
                .sum::<f64>(),
            0.0
        );
    }

    #[test]
    fn classifies_admin_api_statuses() {
        assert_eq!(classify_status(401), ProviderError::AuthenticationExpired);
        assert_eq!(classify_status(403), ProviderError::AuthenticationRequired);
        assert_eq!(classify_status(429), ProviderError::Unavailable);
        assert_eq!(classify_status(500), ProviderError::Unavailable);
    }

    #[test]
    fn detection_reports_missing_key_without_reading_real_credentials() {
        let state = detection_from_source(crate::credentials::KeySource::None);
        assert_eq!(state.auth_state, AuthState::AuthenticationRequired);
    }

    /// Minimal blocking HTTP server serving canned report responses so the
    /// full client path (URLs, headers, pagination, parsing) runs hermetically.
    fn serve_fixtures(
        responses: Vec<(&'static str, String)>,
    ) -> (String, std::sync::mpsc::Receiver<String>) {
        use std::io::{BufRead, Write};
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let items: Vec<(&'static str, &String)> =
                responses.iter().map(|(path, body)| (*path, body)).collect();
            for (stream, (path, body)) in listener.incoming().take(items.len()).zip(items) {
                let Ok(mut stream) = stream else { continue };
                let mut request = String::new();
                {
                    let mut reader = std::io::BufReader::new(stream.try_clone().expect("clone"));
                    let _ = reader.read_line(&mut request);
                }
                let path_received = request
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or_default()
                    .to_string();
                let _ = sender.send(path_received.clone());
                let status = if path_received.starts_with(path) {
                    200
                } else {
                    404
                };
                let headers = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(headers.as_bytes());
                let _ = stream.write_all(body.as_bytes());
                let _ = stream.flush();
            }
        });
        (format!("http://127.0.0.1:{port}"), receiver)
    }

    #[tokio::test]
    async fn fetch_usage_hits_both_reports_and_aggregates() {
        let (base_url, receiver) = serve_fixtures(vec![
            (
                "/v1/organizations/usage_report/messages",
                USAGE_FIXTURE.to_string(),
            ),
            ("/v1/organizations/cost_report", COST_FIXTURE.to_string()),
        ]);
        let api = AnthropicApi::with_key(base_url, "sk-ant-admin-test".to_string());
        let snapshot = anthropic_api_fetch(api).await.expect("fetch");
        assert_eq!(snapshot.provider_id, PROVIDER_ID);
        assert_eq!(snapshot.data_kind, DataKind::Live);
        assert_eq!(snapshot.auth_state, AuthState::Authenticated);
        assert_eq!(snapshot.has_subscription, None);
        assert!(snapshot.capabilities.token_usage);
        assert!(snapshot.capabilities.cost_tracking);
        assert!(!snapshot.capabilities.quota_windows);
        assert!(snapshot.windows.is_empty());
        let tokens = snapshot.token_usage.expect("token usage");
        assert_eq!(tokens.input_tokens, Some(7200));
        assert_eq!(tokens.output_tokens, Some(800));
        assert_eq!(tokens.total_tokens, Some(8000));
        assert_eq!(tokens.cached_input_tokens, Some(1700));
        assert!((tokens.estimated_cost_usd.expect("cost") - 2.0).abs() < 1e-9);
        assert_eq!(tokens.source, MetricSource::LocallyCalculated);

        let seen: Vec<String> = receiver.try_iter().collect();
        assert!(seen[0].contains("/usage_report/messages"));
        assert!(seen[0].contains("bucket_width=1d"));
        assert!(seen[1].contains("/cost_report"));
        assert!(seen[1].contains("limit=31"));
    }
}
