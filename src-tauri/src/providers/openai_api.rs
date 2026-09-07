use std::{collections::BTreeMap, time::Duration};

use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;

use super::{
    AuthState, DataKind, DetectionResult, MetricSource, ProviderCapabilities, ProviderError,
    TokenUsage, UsageProvider, UsageSnapshot,
};

const PROVIDER_ID: &str = "openai-api";
const DISPLAY_NAME: &str = "OpenAI API";
const API_BASE_URL: &str = "https://api.openai.com";
const USAGE_PATH: &str = "/v1/organization/usage/completions";
const REPORT_DAYS: i64 = 30;
const MAX_PAGES: usize = 4;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const FETCH_TIMEOUT: Duration = Duration::from_secs(60);

/// Reads separately billed OpenAI API completion activity through the
/// documented Organization Usage API. This is deliberately a different
/// provider from OpenAI / Codex: API billing is not ChatGPT subscription
/// quota or Codex activity. The endpoint requires an OpenAI Admin API key.
pub struct OpenAiApiProvider;

#[async_trait]
impl UsageProvider for OpenAiApiProvider {
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
            cost_tracking: false,
            local_history: false,
        }
    }

    async fn detect(&self) -> Result<DetectionResult, ProviderError> {
        Ok(detect_state().await)
    }

    async fn authenticate(&self) -> Result<AuthState, ProviderError> {
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
        let api = OpenAiUsageApi::with_key(API_BASE_URL.to_string(), key);
        fetch_snapshot(&api).await
    }
}

/// Query client for the Organization Usage API. The Admin API key never
/// leaves this struct and is neither logged nor returned through IPC.
struct OpenAiUsageApi {
    base_url: String,
    client: reqwest::Client,
    admin_key: String,
}

impl OpenAiUsageApi {
    fn with_key(base_url: String, admin_key: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent(format!("ellie/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("fixed reqwest configuration builds");
        Self {
            base_url,
            client,
            admin_key,
        }
    }
}

#[derive(Default)]
struct UsageTotals {
    input_tokens: u64,
    output_tokens: u64,
    cached_input_tokens: u64,
    request_count: u64,
    model_tokens: BTreeMap<String, u64>,
}

impl UsageTotals {
    fn add(&mut self, row: UsageRow) -> Result<(), ProviderError> {
        self.input_tokens = self
            .input_tokens
            .checked_add(row.input_tokens)
            .ok_or(ProviderError::InvalidSnapshot)?;
        self.output_tokens = self
            .output_tokens
            .checked_add(row.output_tokens)
            .ok_or(ProviderError::InvalidSnapshot)?;
        self.cached_input_tokens = self
            .cached_input_tokens
            .checked_add(row.cached_input_tokens)
            .ok_or(ProviderError::InvalidSnapshot)?;
        self.request_count = self
            .request_count
            .checked_add(row.request_count)
            .ok_or(ProviderError::InvalidSnapshot)?;
        if let Some(model) = row.model {
            let row_total = row
                .input_tokens
                .checked_add(row.output_tokens)
                .ok_or(ProviderError::InvalidSnapshot)?;
            let model_total = self.model_tokens.entry(model).or_default();
            *model_total = model_total
                .checked_add(row_total)
                .ok_or(ProviderError::InvalidSnapshot)?;
        }
        Ok(())
    }

    fn dominant_model(&self) -> Option<String> {
        self.model_tokens
            .iter()
            .max_by_key(|(_, tokens)| *tokens)
            .map(|(model, _)| model.clone())
    }
}

struct UsageRow {
    input_tokens: u64,
    output_tokens: u64,
    cached_input_tokens: u64,
    request_count: u64,
    model: Option<String>,
}

async fn fetch_snapshot(api: &OpenAiUsageApi) -> Result<UsageSnapshot, ProviderError> {
    let now = Utc::now();
    let start_time = (now - chrono::Duration::days(REPORT_DAYS)).timestamp();
    let totals = fetch_usage(api, start_time, now.timestamp()).await?;
    let total_tokens = totals
        .input_tokens
        .checked_add(totals.output_tokens)
        .ok_or(ProviderError::InvalidSnapshot)?;

    Ok(UsageSnapshot {
        provider_id: PROVIDER_ID.to_string(),
        display_name: DISPLAY_NAME.to_string(),
        account_label: None,
        plan: Some("API billing".to_string()),
        has_subscription: None,
        capabilities: OpenAiApiProvider.capabilities(),
        auth_state: AuthState::Authenticated,
        data_kind: DataKind::Live,
        windows: Vec::new(),
        credits: None,
        balance: None,
        balance_currency: None,
        spend_estimate: None,
        model: totals.dominant_model(),
        token_usage: Some(TokenUsage {
            input_tokens: Some(totals.input_tokens),
            output_tokens: Some(totals.output_tokens),
            cached_input_tokens: Some(totals.cached_input_tokens),
            cached_output_tokens: None,
            reasoning_tokens: None,
            total_tokens: Some(total_tokens),
            request_count: Some(totals.request_count),
            estimated_cost_usd: None,
            // The API reports buckets; Ellie chooses and sums the trailing
            // 30-day window, so this aggregate is locally calculated.
            source: MetricSource::LocallyCalculated,
        }),
        fetched_at: now,
    })
}

async fn fetch_usage(
    api: &OpenAiUsageApi,
    start_time: i64,
    end_time: i64,
) -> Result<UsageTotals, ProviderError> {
    let mut totals = UsageTotals::default();
    let mut page: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let result = fetch_usage_page(api, start_time, end_time, page.as_deref()).await?;
        let has_more = result.has_more;
        let next_page = result.next_page;
        for row in result.rows {
            totals.add(row)?;
        }
        if !has_more {
            return Ok(totals);
        }
        page = Some(next_page.ok_or(ProviderError::InvalidSnapshot)?);
    }
    // Do not silently display an incomplete report when pagination exceeds
    // the bounded work limit.
    Err(ProviderError::Unavailable)
}

struct UsagePage {
    rows: Vec<UsageRow>,
    has_more: bool,
    next_page: Option<String>,
}

async fn fetch_usage_page(
    api: &OpenAiUsageApi,
    start_time: i64,
    end_time: i64,
    page: Option<&str>,
) -> Result<UsagePage, ProviderError> {
    let mut url = reqwest::Url::parse(&format!("{}{}", api.base_url, USAGE_PATH))
        .map_err(|_| ProviderError::Unavailable)?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("start_time", &start_time.to_string());
        query.append_pair("end_time", &end_time.to_string());
        query.append_pair("bucket_width", "1d");
        query.append_pair("limit", "31");
        query.append_pair("group_by", "model");
        if let Some(page) = page {
            query.append_pair("page", page);
        }
    }
    let request = api
        .client
        .get(url)
        .header("Authorization", format!("Bearer {}", api.admin_key))
        .header("Accept", "application/json");
    let response = tokio::time::timeout(FETCH_TIMEOUT, request.send())
        .await
        .map_err(|_| ProviderError::Unavailable)?
        .map_err(|_| ProviderError::Unavailable)?;
    if !response.status().is_success() {
        return Err(classify_status(response.status().as_u16()));
    }
    let body: Value = tokio::time::timeout(FETCH_TIMEOUT, response.json())
        .await
        .map_err(|_| ProviderError::Unavailable)?
        .map_err(|_| ProviderError::Unavailable)?;
    parse_usage_page(&body)
}

fn parse_usage_page(body: &Value) -> Result<UsagePage, ProviderError> {
    let data = body
        .get("data")
        .and_then(Value::as_array)
        .ok_or(ProviderError::InvalidSnapshot)?;
    let mut rows = Vec::new();
    for bucket in data {
        let results = bucket
            .get("results")
            .and_then(Value::as_array)
            .ok_or(ProviderError::InvalidSnapshot)?;
        for result in results {
            rows.push(parse_usage_row(result)?);
        }
    }
    let has_more = body
        .get("has_more")
        .and_then(Value::as_bool)
        .ok_or(ProviderError::InvalidSnapshot)?;
    let next_page = body
        .get("next_page")
        .and_then(Value::as_str)
        .map(str::to_owned);
    Ok(UsagePage {
        rows,
        has_more,
        next_page,
    })
}

fn parse_usage_row(result: &Value) -> Result<UsageRow, ProviderError> {
    Ok(UsageRow {
        input_tokens: required_token(result, "input_tokens")?,
        output_tokens: required_token(result, "output_tokens")?,
        cached_input_tokens: required_token(result, "input_cached_tokens")?,
        request_count: required_token(result, "num_model_requests")?,
        model: result
            .get("model")
            .and_then(Value::as_str)
            .filter(|model| !model.is_empty())
            .map(str::to_owned),
    })
}

fn required_token(value: &Value, field: &str) -> Result<u64, ProviderError> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or(ProviderError::InvalidSnapshot)
}

async fn detect_state() -> DetectionResult {
    let store = crate::credentials::WindowsCredentialStore;
    let source = tokio::task::spawn_blocking(move || {
        crate::credentials::provider_key_status(&store, PROVIDER_ID).ok()
    })
    .await
    .unwrap_or(None)
    .unwrap_or(crate::credentials::KeySource::None);
    match source {
        crate::credentials::KeySource::Environment | crate::credentials::KeySource::CredentialManager => {
            DetectionResult {
                auth_state: AuthState::AuthenticationDetected,
                detail: Some(
                    "OpenAI Admin API key found (environment or Windows Credential Manager); Ellie reads separately billed API completion usage."
                        .to_string(),
                ),
            }
        }
        crate::credentials::KeySource::None => DetectionResult {
            auth_state: AuthState::AuthenticationRequired,
            detail: Some(
                "Add an OpenAI Admin API key in Settings → Provider credentials to read API-billed completion usage. This is separate from ChatGPT/Codex subscription usage."
                    .to_string(),
            ),
        },
    }
}

fn classify_status(status: u16) -> ProviderError {
    match status {
        401 => ProviderError::AuthenticationExpired,
        // An ordinary API key or insufficient Admin role should remain visible
        // as an actionable provider failure rather than being hidden as
        // unconfigured.
        403 => ProviderError::Unavailable,
        400..=599 => ProviderError::Unavailable,
        _ => ProviderError::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_aggregates_provider_usage_rows() {
        let body: Value = serde_json::from_str(
            r#"{
                "data": [{"results": [
                    {"input_tokens": 100, "input_cached_tokens": 25, "output_tokens": 40, "num_model_requests": 2, "model": "gpt-5"},
                    {"input_tokens": 50, "input_cached_tokens": 5, "output_tokens": 10, "num_model_requests": 1, "model": "gpt-5-mini"}
                ]}], "has_more": false, "next_page": null
            }"#,
        ).expect("fixture");
        let page = parse_usage_page(&body).expect("page");
        let mut totals = UsageTotals::default();
        for row in page.rows {
            totals.add(row).expect("totals");
        }
        assert_eq!(totals.input_tokens, 150);
        assert_eq!(totals.cached_input_tokens, 30);
        assert_eq!(totals.output_tokens, 50);
        assert_eq!(totals.request_count, 3);
        assert_eq!(totals.dominant_model().as_deref(), Some("gpt-5"));
    }

    #[test]
    fn rejects_malformed_or_negative_usage_without_fabricating_zero() {
        for fixture in [
            r#"{"has_more": false, "next_page": null}"#,
            r#"{"data": [{"results": [{}]}], "has_more": false, "next_page": null}"#,
            r#"{"data": [{"results": [{"input_tokens": -1, "input_cached_tokens": 0, "output_tokens": 0, "num_model_requests": 0}]}], "has_more": false, "next_page": null}"#,
            r#"{"data": [{"results": []}], "has_more": "false", "next_page": null}"#,
        ] {
            let body: Value = serde_json::from_str(fixture).expect("fixture JSON");
            assert!(parse_usage_page(&body).is_err(), "fixture: {fixture}");
        }
    }

    #[test]
    fn classifies_openai_api_statuses() {
        assert_eq!(classify_status(401), ProviderError::AuthenticationExpired);
        assert_eq!(classify_status(403), ProviderError::Unavailable);
        assert_eq!(classify_status(429), ProviderError::Unavailable);
        assert_eq!(classify_status(500), ProviderError::Unavailable);
    }

    #[tokio::test]
    async fn fetches_paginated_usage_from_a_local_server() {
        use std::io::{BufRead, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("address").port();
        let thread = std::thread::spawn(move || {
            for (expected_page, body) in [
                (
                    None,
                    r#"{"data":[{"results":[{"input_tokens":100,"input_cached_tokens":10,"output_tokens":20,"num_model_requests":1,"model":"gpt-5"}]}],"has_more":true,"next_page":"next"}"#,
                ),
                (
                    Some("next"),
                    r#"{"data":[{"results":[{"input_tokens":30,"input_cached_tokens":3,"output_tokens":7,"num_model_requests":2,"model":"gpt-5-mini"}]}],"has_more":false,"next_page":null}"#,
                ),
            ] {
                let (mut stream, _) = listener.accept().expect("request");
                let mut request = String::new();
                let mut reader = std::io::BufReader::new(stream.try_clone().expect("clone"));
                reader.read_line(&mut request).expect("request line");
                assert!(request.starts_with("GET /v1/organization/usage/completions?"));
                assert!(request.contains("group_by=model"));
                match expected_page {
                    Some(page) => assert!(request.contains(&format!("page={page}"))),
                    None => assert!(!request.contains("page=")),
                }
                let headers = format!(
                    "HTTP/1.1 200\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(headers.as_bytes()).expect("headers");
                stream.write_all(body.as_bytes()).expect("body");
                stream.flush().expect("flush");
            }
        });

        let api = OpenAiUsageApi::with_key(format!("http://127.0.0.1:{port}"), "admin-test".into());
        let totals = fetch_usage(&api, 1_700_000_000, 1_700_100_000)
            .await
            .expect("usage");
        thread.join().expect("server");
        assert_eq!(totals.input_tokens, 130);
        assert_eq!(totals.cached_input_tokens, 13);
        assert_eq!(totals.output_tokens, 27);
        assert_eq!(totals.request_count, 3);
    }
}
