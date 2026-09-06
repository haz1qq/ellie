use std::{path::Path, time::Duration};

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::{
    error::AppError,
    providers::{
        AuthState, DataKind, MetricSource, ProviderCapabilities, SpendEstimate, TokenUsage,
        UsageSnapshot, UsageWindow,
    },
    storage,
};

/// Default history retention window from the specification.
pub const HISTORY_RETENTION: Duration = Duration::from_secs(90 * 24 * 60 * 60);
/// Window used for balance-derived spend estimates.
pub const SPEND_ESTIMATE_WINDOW: Duration = Duration::from_secs(30 * 24 * 60 * 60);
/// Upper bound on balance snapshots examined for one spend estimate.
pub const SPEND_ESTIMATE_MAX_SNAPSHOTS: u64 = 1_000;
/// How often expired history is removed while the app runs.
pub const HISTORY_CLEANUP_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
/// Upper bound for one retention cleanup pass.
pub const HISTORY_CLEANUP_TIMEOUT: Duration = Duration::from_secs(30);

/// Persists a validated snapshot (provider/account identity, usage windows,
/// token usage) in one transaction. Returns the new snapshot row id.
pub fn insert_snapshot(path: &Path, snapshot: &UsageSnapshot) -> Result<i64, AppError> {
    snapshot.validate().map_err(|_| AppError::Storage)?;
    let mut connection = storage::connect(path)?;
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

    let provider_id = upsert_provider(&transaction, snapshot)?;
    let account_id = upsert_account(&transaction, provider_id, snapshot)?;

    transaction.execute(
        "INSERT INTO usage_snapshots
             (provider_id, account_id, fetched_at, data_kind, auth_state,
              capabilities_json, credits, balance, has_subscription, balance_currency,
              spend_estimate_amount, spend_estimate_currency, spend_estimate_window_days, model)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            provider_id,
            account_id,
            timestamp(snapshot.fetched_at),
            data_kind_name(snapshot.data_kind),
            auth_state_name(&snapshot.auth_state),
            serde_json::to_string(&snapshot.capabilities).map_err(|_| AppError::Storage)?,
            snapshot.credits,
            snapshot.balance,
            snapshot.has_subscription.map(|value| value as i64),
            snapshot.balance_currency,
            snapshot
                .spend_estimate
                .as_ref()
                .map(|estimate| estimate.amount),
            snapshot
                .spend_estimate
                .as_ref()
                .map(|estimate| estimate.currency.clone()),
            snapshot
                .spend_estimate
                .as_ref()
                .map(|estimate| estimate.window_days),
            snapshot.model,
        ],
    )?;
    let snapshot_id = transaction.last_insert_rowid();

    for window in &snapshot.windows {
        transaction.execute(
            "INSERT INTO usage_windows
                 (snapshot_id, window_key, label, used_percent, remaining_percent,
                  used_value, remaining_value, limit_value, unit, starts_at, reset_at, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                snapshot_id,
                window.id,
                window.label,
                window.used_percent,
                window.remaining_percent,
                window.used_value,
                window.remaining_value,
                window.limit_value,
                window.unit,
                window.starts_at.map(timestamp),
                window.reset_at.map(timestamp),
                source_name(&window.source),
            ],
        )?;
    }

    if let Some(tokens) = &snapshot.token_usage {
        transaction.execute(
            "INSERT INTO token_usage
                 (snapshot_id, input_tokens, output_tokens, cached_input_tokens,
                  cached_output_tokens, reasoning_tokens, total_tokens, request_count,
                  estimated_cost_usd, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                snapshot_id,
                i64_tokens(tokens.input_tokens),
                i64_tokens(tokens.output_tokens),
                i64_tokens(tokens.cached_input_tokens),
                i64_tokens(tokens.cached_output_tokens),
                i64_tokens(tokens.reasoning_tokens),
                i64_tokens(tokens.total_tokens),
                i64_tokens(tokens.request_count),
                tokens.estimated_cost_usd,
                source_name(&tokens.source),
            ],
        )?;
    }

    transaction.commit()?;
    Ok(snapshot_id)
}

/// Returns the most recent snapshot for a provider, if any.
pub fn latest_snapshot(path: &Path, provider_key: &str) -> Result<Option<UsageSnapshot>, AppError> {
    let connection = storage::connect(path)?;
    let snapshot_id: Option<i64> = connection
        .query_row(
            "SELECT s.id FROM usage_snapshots s
             JOIN providers p ON p.id = s.provider_id
             WHERE p.provider_key = ?1
             ORDER BY s.fetched_at DESC, s.id DESC
             LIMIT 1",
            params![provider_key],
            |row| row.get(0),
        )
        .optional()?;
    snapshot_id
        .map(|id| load_snapshot(&connection, id))
        .transpose()
}

/// Returns snapshots newest first. `provider_key` filters to one provider;
/// `limit` bounds the result.
pub fn snapshot_history(
    path: &Path,
    provider_key: Option<&str>,
    limit: u64,
) -> Result<Vec<UsageSnapshot>, AppError> {
    let connection = storage::connect(path)?;
    let ids: Vec<i64> = match provider_key {
        Some(provider_key) => {
            let mut statement = connection.prepare(
                "SELECT s.id FROM usage_snapshots s
                 JOIN providers p ON p.id = s.provider_id
                 WHERE p.provider_key = ?1
                 ORDER BY s.fetched_at DESC, s.id DESC
                 LIMIT ?2",
            )?;
            let rows =
                statement.query_map(params![provider_key, limit as i64], |row| row.get(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        }
        None => {
            let mut statement = connection.prepare(
                "SELECT s.id FROM usage_snapshots s
                 ORDER BY s.fetched_at DESC, s.id DESC
                 LIMIT ?1",
            )?;
            let rows = statement.query_map(params![limit as i64], |row| row.get(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        }
    };
    ids.iter()
        .map(|id| load_snapshot(&connection, *id))
        .collect()
}

/// Removes snapshots older than `retention`, cascading to their windows and
/// token usage. Returns the number of deleted snapshots.
pub fn cleanup_history(path: &Path, retention: Duration) -> Result<usize, AppError> {
    let connection = storage::connect(path)?;
    let cutoff = Utc::now() - retention;
    Ok(connection.execute(
        "DELETE FROM usage_snapshots WHERE fetched_at < ?1",
        params![timestamp(cutoff)],
    )?)
}

/// Locally-calculated spend estimate for a balance-based provider: the
/// positive balance decrease between the oldest stored snapshot inside
/// `window` (with a matching currency) and the current balance. Returns
/// `None` when no suitable prior snapshot exists (fewer than two data
/// points), the currencies differ, or the balance did not decrease
/// (top-ups/grants). `window_days` reflects the actual span covered.
pub fn spend_estimate(
    path: &Path,
    provider_key: &str,
    current_balance: f64,
    currency: &str,
    window: Duration,
) -> Result<Option<SpendEstimate>, AppError> {
    let connection = storage::connect(path)?;
    let cutoff = Utc::now() - window;
    // An estimate needs at least two prior observations to establish a trend;
    // a single point is likely the initial top-up, not spend.
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM usage_snapshots s
         JOIN providers p ON p.id = s.provider_id
         WHERE p.provider_key = ?1 AND s.balance IS NOT NULL
           AND s.balance_currency = ?2 AND s.fetched_at >= ?3",
        params![provider_key, currency, timestamp(cutoff)],
        |row| row.get(0),
    )?;
    if count < 2 {
        return Ok(None);
    }
    let earliest: Option<(String, f64)> = connection
        .query_row(
            "SELECT s.fetched_at, s.balance
             FROM usage_snapshots s
             JOIN providers p ON p.id = s.provider_id
             WHERE p.provider_key = ?1
               AND s.balance IS NOT NULL
               AND s.balance_currency = ?2
               AND s.fetched_at >= ?3
             ORDER BY s.fetched_at ASC
             LIMIT 1",
            params![provider_key, currency, timestamp(cutoff)],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((fetched_at, previous_balance)) = earliest else {
        return Ok(None);
    };
    let Some(previous_at) = parse_timestamp(&fetched_at) else {
        return Ok(None);
    };
    let spent = previous_balance - current_balance;
    if spent <= 0.0 || !spent.is_finite() {
        return Ok(None);
    }
    let window_days = (Utc::now() - previous_at).num_days().clamp(1, i64::MAX);
    Ok(Some(SpendEstimate {
        amount: spent,
        currency: currency.to_string(),
        window_days,
    }))
}

fn upsert_provider(connection: &Connection, snapshot: &UsageSnapshot) -> Result<i64, AppError> {
    connection.execute(
        "INSERT INTO providers (provider_key, display_name)
         VALUES (?1, ?2)
         ON CONFLICT(provider_key) DO UPDATE SET
             display_name = excluded.display_name,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        params![snapshot.provider_id, snapshot.display_name],
    )?;
    Ok(connection.query_row(
        "SELECT id FROM providers WHERE provider_key = ?1",
        params![snapshot.provider_id],
        |row| row.get(0),
    )?)
}

fn upsert_account(
    connection: &Connection,
    provider_id: i64,
    snapshot: &UsageSnapshot,
) -> Result<i64, AppError> {
    let label = snapshot.account_label.as_deref().unwrap_or("");
    let existing: Option<i64> = connection
        .query_row(
            "SELECT id FROM accounts
             WHERE provider_id = ?1 AND COALESCE(account_label, '') = ?2",
            params![provider_id, label],
            |row| row.get(0),
        )
        .optional()?;
    Ok(match existing {
        Some(id) => {
            connection.execute(
                "UPDATE accounts
                 SET plan = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?2",
                params![snapshot.plan, id],
            )?;
            id
        }
        None => {
            connection.execute(
                "INSERT INTO accounts (provider_id, account_label, plan)
                 VALUES (?1, ?2, ?3)",
                params![provider_id, snapshot.account_label, snapshot.plan],
            )?;
            connection.last_insert_rowid()
        }
    })
}

#[derive(Debug)]
struct RawWindow {
    window_key: String,
    label: String,
    used_percent: Option<f64>,
    remaining_percent: Option<f64>,
    used_value: Option<f64>,
    remaining_value: Option<f64>,
    limit_value: Option<f64>,
    unit: Option<String>,
    starts_at: Option<String>,
    reset_at: Option<String>,
    source: String,
}

#[derive(Debug)]
struct RawTokenUsage {
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    cached_input_tokens: Option<i64>,
    cached_output_tokens: Option<i64>,
    reasoning_tokens: Option<i64>,
    total_tokens: Option<i64>,
    request_count: Option<i64>,
    estimated_cost_usd: Option<f64>,
    source: String,
}

#[derive(Debug)]
struct RawSnapshot {
    provider_id: i64,
    account_id: Option<i64>,
    fetched_at: String,
    data_kind: String,
    auth_state: String,
    capabilities_json: String,
    credits: Option<f64>,
    balance: Option<f64>,
    has_subscription: Option<bool>,
    balance_currency: Option<String>,
    spend_estimate_amount: Option<f64>,
    spend_estimate_currency: Option<String>,
    spend_estimate_window_days: Option<i64>,
    model: Option<String>,
}

fn load_snapshot(connection: &Connection, snapshot_id: i64) -> Result<UsageSnapshot, AppError> {
    let raw: RawSnapshot = connection.query_row(
        "SELECT provider_id, account_id, fetched_at, data_kind, auth_state,
                capabilities_json, credits, balance, has_subscription, balance_currency,
                spend_estimate_amount, spend_estimate_currency, spend_estimate_window_days, model
         FROM usage_snapshots WHERE id = ?1",
        params![snapshot_id],
        |row| {
            Ok(RawSnapshot {
                provider_id: row.get(0)?,
                account_id: row.get(1)?,
                fetched_at: row.get(2)?,
                data_kind: row.get(3)?,
                auth_state: row.get(4)?,
                capabilities_json: row.get(5)?,
                credits: row.get(6)?,
                balance: row.get(7)?,
                has_subscription: row
                    .get(8)
                    .map(|value: Option<i64>| value.map(|value| value != 0))?,
                balance_currency: row.get(9)?,
                spend_estimate_amount: row.get(10)?,
                spend_estimate_currency: row.get(11)?,
                spend_estimate_window_days: row.get(12)?,
                model: row.get(13)?,
            })
        },
    )?;

    let (provider_key, display_name): (String, String) = connection.query_row(
        "SELECT provider_key, display_name FROM providers WHERE id = ?1",
        params![raw.provider_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    let (account_label, plan): (Option<String>, Option<String>) = match raw.account_id {
        Some(account_id) => connection.query_row(
            "SELECT account_label, plan FROM accounts WHERE id = ?1",
            params![account_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?,
        None => (None, None),
    };

    let capabilities: ProviderCapabilities =
        serde_json::from_str(&raw.capabilities_json).map_err(|_| AppError::Storage)?;

    let windows = load_windows(connection, snapshot_id)?;
    let token_usage = load_token_usage(connection, snapshot_id)?;

    Ok(UsageSnapshot {
        provider_id: provider_key,
        display_name,
        account_label,
        plan,
        has_subscription: raw.has_subscription,
        capabilities,
        auth_state: parse_auth_state(&raw.auth_state)?,
        data_kind: parse_data_kind(&raw.data_kind)?,
        windows,
        credits: raw.credits,
        balance: raw.balance,
        balance_currency: raw.balance_currency,
        spend_estimate: raw.spend_estimate_amount.and_then(|amount| {
            let currency = raw.spend_estimate_currency?;
            let window_days = raw.spend_estimate_window_days?;
            Some(SpendEstimate {
                amount,
                currency,
                window_days,
            })
        }),
        model: raw.model,
        token_usage,
        fetched_at: parse_timestamp(&raw.fetched_at).ok_or(AppError::Storage)?,
    })
}

fn load_windows(connection: &Connection, snapshot_id: i64) -> Result<Vec<UsageWindow>, AppError> {
    let mut statement = connection.prepare(
        "SELECT window_key, label, used_percent, remaining_percent, used_value,
                remaining_value, limit_value, unit, starts_at, reset_at, source
         FROM usage_windows WHERE snapshot_id = ?1 ORDER BY id",
    )?;
    let rows = statement.query_map(params![snapshot_id], |row| {
        Ok(RawWindow {
            window_key: row.get(0)?,
            label: row.get(1)?,
            used_percent: row.get(2)?,
            remaining_percent: row.get(3)?,
            used_value: row.get(4)?,
            remaining_value: row.get(5)?,
            limit_value: row.get(6)?,
            unit: row.get(7)?,
            starts_at: row.get(8)?,
            reset_at: row.get(9)?,
            source: row.get(10)?,
        })
    })?;
    let mut windows = Vec::new();
    for raw in rows {
        let raw = raw?;
        windows.push(UsageWindow {
            id: raw.window_key,
            label: raw.label,
            used_percent: raw.used_percent,
            remaining_percent: raw.remaining_percent,
            used_value: raw.used_value,
            remaining_value: raw.remaining_value,
            limit_value: raw.limit_value,
            unit: raw.unit,
            starts_at: raw.starts_at.as_deref().and_then(parse_timestamp),
            reset_at: raw.reset_at.as_deref().and_then(parse_timestamp),
            source: parse_source(&raw.source)?,
        });
    }
    Ok(windows)
}

fn load_token_usage(
    connection: &Connection,
    snapshot_id: i64,
) -> Result<Option<TokenUsage>, AppError> {
    let raw: Option<RawTokenUsage> = connection
        .query_row(
            "SELECT input_tokens, output_tokens, cached_input_tokens, cached_output_tokens,
                    reasoning_tokens, total_tokens, request_count, estimated_cost_usd, source
             FROM token_usage WHERE snapshot_id = ?1",
            params![snapshot_id],
            |row| {
                Ok(RawTokenUsage {
                    input_tokens: row.get(0)?,
                    output_tokens: row.get(1)?,
                    cached_input_tokens: row.get(2)?,
                    cached_output_tokens: row.get(3)?,
                    reasoning_tokens: row.get(4)?,
                    total_tokens: row.get(5)?,
                    request_count: row.get(6)?,
                    estimated_cost_usd: row.get(7)?,
                    source: row.get(8)?,
                })
            },
        )
        .optional()?;
    let Some(raw) = raw else { return Ok(None) };
    let source = parse_source(&raw.source)?;
    Ok(Some(TokenUsage {
        input_tokens: raw.input_tokens.map(|v| v as u64),
        output_tokens: raw.output_tokens.map(|v| v as u64),
        cached_input_tokens: raw.cached_input_tokens.map(|v| v as u64),
        cached_output_tokens: raw.cached_output_tokens.map(|v| v as u64),
        reasoning_tokens: raw.reasoning_tokens.map(|v| v as u64),
        total_tokens: raw.total_tokens.map(|v| v as u64),
        request_count: raw.request_count.map(|v| v as u64),
        estimated_cost_usd: raw.estimated_cost_usd,
        source,
    }))
}

/// Fixed millisecond precision so stored timestamps compare lexicographically
/// in the same order as chronologically.
fn timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn parse_timestamp(text: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

fn i64_tokens(value: Option<u64>) -> Option<i64> {
    value.map(|value| value as i64)
}

fn data_kind_name(value: DataKind) -> &'static str {
    match value {
        DataKind::Live => "live",
        DataKind::Mock => "mock",
    }
}

fn parse_data_kind(text: &str) -> Result<DataKind, AppError> {
    match text {
        "live" => Ok(DataKind::Live),
        "mock" => Ok(DataKind::Mock),
        _ => Err(AppError::Storage),
    }
}

fn auth_state_name(value: &AuthState) -> &'static str {
    match value {
        AuthState::Authenticated => "authenticated",
        AuthState::AuthenticationDetected => "authentication_detected",
        AuthState::AuthenticationRequired => "authentication_required",
        AuthState::Expired => "expired",
        AuthState::Unsupported => "unsupported",
    }
}

fn parse_auth_state(text: &str) -> Result<AuthState, AppError> {
    match text {
        "authenticated" => Ok(AuthState::Authenticated),
        "authentication_detected" => Ok(AuthState::AuthenticationDetected),
        "authentication_required" => Ok(AuthState::AuthenticationRequired),
        "expired" => Ok(AuthState::Expired),
        "unsupported" => Ok(AuthState::Unsupported),
        _ => Err(AppError::Storage),
    }
}

fn source_name(value: &MetricSource) -> &'static str {
    match value {
        MetricSource::ProviderReported => "provider_reported",
        MetricSource::LocallyCalculated => "locally_calculated",
    }
}

fn parse_source(text: &str) -> Result<MetricSource, AppError> {
    match text {
        "provider_reported" => Ok(MetricSource::ProviderReported),
        "locally_calculated" => Ok(MetricSource::LocallyCalculated),
        _ => Err(AppError::Storage),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{DataKind, MockProvider, ProviderCapabilities, UsageProvider};
    use chrono::Duration as ChronoDuration;

    fn utc_ms(timestamp_seconds: i64, millis: u32) -> DateTime<Utc> {
        DateTime::from_timestamp(timestamp_seconds, millis * 1_000_000).expect("valid timestamp")
    }

    fn sample_snapshot(
        provider_key: &str,
        label: &str,
        fetched_at: DateTime<Utc>,
    ) -> UsageSnapshot {
        UsageSnapshot {
            provider_id: provider_key.into(),
            display_name: format!("{label} Provider"),
            account_label: Some("alice".into()),
            plan: Some("Pro".into()),
            capabilities: ProviderCapabilities {
                quota_windows: true,
                token_usage: true,
                cost_tracking: true,
                ..ProviderCapabilities::default()
            },
            auth_state: AuthState::Authenticated,
            has_subscription: None,
            data_kind: DataKind::Live,
            windows: vec![UsageWindow {
                id: "weekly".into(),
                label: "Weekly allowance".into(),
                used_percent: Some(25.0),
                remaining_percent: Some(75.0),
                used_value: Some(250.0),
                remaining_value: Some(750.0),
                limit_value: Some(1000.0),
                unit: Some("requests".into()),
                starts_at: None,
                reset_at: Some(fetched_at + ChronoDuration::days(7)),
                source: MetricSource::ProviderReported,
            }],
            credits: None,
            balance: Some(12.5),
            balance_currency: Some("USD".into()),
            spend_estimate: None,
            model: Some("alpha-model".into()),
            token_usage: Some(TokenUsage {
                input_tokens: Some(1_000),
                output_tokens: Some(500),
                total_tokens: Some(1_500),
                request_count: Some(3),
                estimated_cost_usd: Some(0.05),
                source: MetricSource::LocallyCalculated,
                ..TokenUsage::default()
            }),
            fetched_at,
        }
    }

    fn initialize(path: &Path) {
        crate::storage::initialize(path).expect("schema initialize");
    }

    fn truncate_millis(value: DateTime<Utc>) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(value.timestamp_millis()).expect("valid timestamp")
    }

    #[test]
    fn insert_then_read_roundtrip_preserves_provenance() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("ellie.sqlite3");
        initialize(&path);
        let snapshot = sample_snapshot("alpha", "Alpha", utc_ms(1_700_000_000, 0));
        insert_snapshot(&path, &snapshot)?;
        let loaded = latest_snapshot(&path, "alpha")?.expect("snapshot present");
        assert_eq!(loaded, snapshot);
        assert_eq!(loaded.data_kind, DataKind::Live);
        assert_eq!(loaded.windows[0].source, MetricSource::ProviderReported);
        assert_eq!(
            loaded.token_usage.expect("token row").source,
            MetricSource::LocallyCalculated
        );
        Ok(())
    }

    #[tokio::test]
    async fn mock_provider_snapshot_roundtrips_with_mock_provenance(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("ellie.sqlite3");
        initialize(&path);
        let mut snapshot = MockProvider.fetch_usage().await?;
        snapshot.fetched_at = truncate_millis(snapshot.fetched_at);
        for window in &mut snapshot.windows {
            window.starts_at = window.starts_at.map(truncate_millis);
            window.reset_at = window.reset_at.map(truncate_millis);
        }
        insert_snapshot(&path, &snapshot)?;
        let loaded = latest_snapshot(&path, "ellie-demo")?.expect("mock snapshot present");
        assert_eq!(loaded, snapshot);
        assert_eq!(loaded.data_kind, DataKind::Mock);
        assert_eq!(loaded.auth_state, AuthState::Unsupported);
        Ok(())
    }

    #[test]
    fn history_is_newest_first_and_respects_limit_and_filter(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("ellie.sqlite3");
        initialize(&path);
        let template = sample_snapshot("alpha", "Alpha", utc_ms(1_700_000_000, 0));
        let first = UsageSnapshot {
            has_subscription: Some(true),
            ..template.clone()
        };
        let second = UsageSnapshot {
            has_subscription: Some(false),
            ..template
        };
        let third = sample_snapshot("alpha", "Alpha", utc_ms(1_700_000_120, 0));
        let other = sample_snapshot("beta", "Beta", utc_ms(1_700_000_180, 0));
        for snapshot in [&first, &second, &third, &other] {
            insert_snapshot(&path, snapshot)?;
        }

        let all = snapshot_history(&path, None, 10)?;
        assert_eq!(all.len(), 4);
        assert!(
            all.windows(2)
                .all(|pair| pair[0].fetched_at >= pair[1].fetched_at),
            "history must be newest first"
        );

        let limited = snapshot_history(&path, None, 2)?;
        assert_eq!(limited.len(), 2);

        let alpha_only = snapshot_history(&path, Some("alpha"), 10)?;
        assert_eq!(alpha_only.len(), 3);
        assert!(alpha_only.iter().all(|s| s.provider_id == "alpha"));

        Ok(())
    }

    #[test]
    fn cleanup_removes_only_expired_snapshots() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("ellie.sqlite3");
        initialize(&path);
        let expired_at = truncate_millis(Utc::now() - ChronoDuration::days(91));
        let fresh_at = truncate_millis(Utc::now());
        insert_snapshot(&path, &sample_snapshot("alpha", "Alpha", expired_at))?;
        insert_snapshot(&path, &sample_snapshot("alpha", "Alpha", fresh_at))?;

        let deleted = cleanup_history(&path, HISTORY_RETENTION)?;
        assert_eq!(deleted, 1);
        assert_eq!(snapshot_history(&path, None, 10)?.len(), 1);
        assert!(matches!(
            latest_snapshot(&path, "alpha")?,
            Some(ref snapshot) if snapshot.fetched_at == fresh_at
        ));
        Ok(())
    }

    #[test]
    fn repeated_inserts_reuse_one_account_row_per_label() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("ellie.sqlite3");
        initialize(&path);
        let first = sample_snapshot("alpha", "Alpha", utc_ms(1_700_000_000, 0));
        let second = sample_snapshot("alpha", "Alpha", utc_ms(1_700_000_060, 0));
        insert_snapshot(&path, &first)?;
        insert_snapshot(&path, &second)?;
        let changed = sample_snapshot("alpha", "Alpha", utc_ms(1_700_000_120, 0));
        let changed = UsageSnapshot {
            account_label: Some("bob".into()),
            ..changed
        };
        insert_snapshot(&path, &changed)?;

        let connection = storage::connect(&path)?;
        let count: i64 =
            connection.query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))?;
        assert_eq!(count, 2);
        Ok(())
    }

    #[test]
    fn subscription_flag_roundtrips_through_storage() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("ellie.sqlite3");
        initialize(&path);
        let subscribed = UsageSnapshot {
            has_subscription: Some(true),
            ..sample_snapshot("alpha", "Alpha", utc_ms(1_700_000_000, 0))
        };
        let unsubscribed = UsageSnapshot {
            has_subscription: Some(false),
            ..sample_snapshot("beta", "Beta", utc_ms(1_700_000_060, 0))
        };
        insert_snapshot(&path, &subscribed)?;
        insert_snapshot(&path, &unsubscribed)?;
        assert_eq!(
            latest_snapshot(&path, "alpha")?
                .expect("alpha")
                .has_subscription,
            Some(true)
        );
        assert_eq!(
            latest_snapshot(&path, "beta")?
                .expect("beta")
                .has_subscription,
            Some(false)
        );
        Ok(())
    }

    #[test]
    fn spend_estimate_derives_positive_balance_decrease() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("ellie.sqlite3");
        initialize(&path);
        let now = Utc::now();
        let ten_days_ago = truncate_millis(now - ChronoDuration::days(10));
        let yesterday = truncate_millis(now - ChronoDuration::days(1));
        let mut earlier = sample_snapshot("alpha", "Alpha", ten_days_ago);
        earlier.balance = Some(110.0);
        let mut later = sample_snapshot("alpha", "Alpha", yesterday);
        later.balance = Some(90.0);
        insert_snapshot(&path, &earlier)?;
        insert_snapshot(&path, &later)?;

        let estimate = spend_estimate(&path, "alpha", 90.0, "USD", SPEND_ESTIMATE_WINDOW)?
            .expect("estimate computed");
        assert!((estimate.amount - 20.0).abs() < 1e-9);
        assert_eq!(estimate.currency, "USD");
        assert!(
            (8..=10).contains(&estimate.window_days),
            "actual span reported"
        );

        // Balance went up (top-up): no estimate.
        assert!(spend_estimate(&path, "alpha", 120.0, "USD", SPEND_ESTIMATE_WINDOW)?.is_none());
        // Currency mismatch: no estimate.
        assert!(spend_estimate(&path, "alpha", 90.0, "CNY", SPEND_ESTIMATE_WINDOW)?.is_none());
        // No prior snapshot for an unknown provider.
        assert!(spend_estimate(&path, "missing", 90.0, "USD", SPEND_ESTIMATE_WINDOW)?.is_none());
        Ok(())
    }

    #[test]
    fn spend_estimate_requires_two_balance_points() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("ellie.sqlite3");
        initialize(&path);
        let mut single = sample_snapshot("alpha", "Alpha", truncate_millis(Utc::now()));
        single.balance = Some(50.0);
        insert_snapshot(&path, &single)?;
        assert!(spend_estimate(&path, "alpha", 40.0, "USD", SPEND_ESTIMATE_WINDOW)?.is_none());
        Ok(())
    }

    #[test]
    fn invalid_snapshot_is_rejected_before_any_write() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("ellie.sqlite3");
        initialize(&path);
        let mut snapshot = sample_snapshot("alpha", "Alpha", utc_ms(1_700_000_000, 0));
        snapshot.windows[0].used_percent = Some(101.0);
        assert!(insert_snapshot(&path, &snapshot).is_err());
        assert_eq!(snapshot_history(&path, None, 10)?.len(), 0);
        let connection = storage::connect(&path)?;
        let providers: i64 =
            connection.query_row("SELECT COUNT(*) FROM providers", [], |row| row.get(0))?;
        let accounts: i64 =
            connection.query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))?;
        assert_eq!(providers, 0);
        assert_eq!(accounts, 0);
        Ok(())
    }

    #[test]
    fn history_of_unknown_provider_is_empty() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("ellie.sqlite3");
        initialize(&path);
        assert!(latest_snapshot(&path, "missing")?.is_none());
        assert!(snapshot_history(&path, Some("missing"), 10)?.is_empty());
        Ok(())
    }
}
