use std::{collections::BTreeMap, path::Path};

use chrono::{DateTime, Duration, NaiveDate, SecondsFormat, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::{
    providers::{DataKind, MetricSource},
    storage,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnalyticsRange {
    Today,
    SevenDays,
    ThirtyDays,
    NinetyDays,
}

impl AnalyticsRange {
    fn bounds(self, now: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>) {
        let start = match self {
            Self::Today => DateTime::from_naive_utc_and_offset(
                now.date_naive()
                    .and_hms_opt(0, 0, 0)
                    .expect("midnight is valid"),
                Utc,
            ),
            Self::SevenDays => now - Duration::days(7),
            Self::ThirtyDays => now - Duration::days(30),
            Self::NinetyDays => now - Duration::days(90),
        };
        (start, now)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalyticsSource {
    ProviderReported,
    LocallyCalculated,
    Mixed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsResponse {
    pub range: AnalyticsRange,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
    pub snapshot_count: u64,
    pub provider_count: u64,
    pub latest_total_tokens: Option<u64>,
    pub latest_request_count: Option<u64>,
    pub token_source: Option<AnalyticsSource>,
    pub estimated_spend: Vec<AnalyticsSpend>,
    pub providers: Vec<AnalyticsProvider>,
    pub token_series: Vec<TokenPoint>,
    pub quota_windows: Vec<QuotaPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsSpend {
    pub currency: String,
    pub amount: f64,
    pub source: AnalyticsSource,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsProvider {
    pub provider_id: String,
    pub display_name: String,
    pub model: Option<String>,
    pub latest_at: DateTime<Utc>,
    pub total_tokens: Option<u64>,
    pub request_count: Option<u64>,
    pub token_source: Option<MetricSource>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenPoint {
    pub date: NaiveDate,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaPoint {
    pub provider_id: String,
    pub display_name: String,
    pub window_id: String,
    pub window_label: String,
    pub used_percent: Option<f64>,
    pub remaining_percent: Option<f64>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug)]
struct RawObservation {
    provider_id: String,
    display_name: String,
    fetched_at: String,
    data_kind: String,
    model: Option<String>,
    total_tokens: Option<i64>,
    request_count: Option<i64>,
    spend_amount: Option<f64>,
    spend_currency: Option<String>,
    token_estimated_cost_usd: Option<f64>,
    token_source: Option<String>,
}

#[derive(Debug)]
struct RawQuota {
    provider_id: String,
    display_name: String,
    fetched_at: String,
    data_kind: String,
    window_id: String,
    window_label: String,
    used_percent: Option<f64>,
    remaining_percent: Option<f64>,
    source: String,
}

#[derive(Debug, Clone)]
struct LatestObservation {
    display_name: String,
    fetched_at: DateTime<Utc>,
    model: Option<String>,
    total_tokens: Option<u64>,
    request_count: Option<u64>,
    spend_amount: Option<f64>,
    spend_currency: Option<String>,
    token_estimated_cost_usd: Option<f64>,
    token_source: Option<MetricSource>,
}

pub fn query(
    database_path: &Path,
    range: AnalyticsRange,
    now: DateTime<Utc>,
) -> Result<AnalyticsResponse, crate::error::AppError> {
    let (start, end) = range.bounds(now);
    let connection = storage::connect(database_path)?;
    let mut observations = connection.prepare(
        "SELECT p.provider_key, p.display_name, s.fetched_at, s.data_kind, s.model,
                t.total_tokens, t.request_count, s.spend_estimate_amount,
                s.spend_estimate_currency, t.estimated_cost_usd, t.source
         FROM usage_snapshots s
         JOIN providers p ON p.id = s.provider_id
         LEFT JOIN token_usage t ON t.snapshot_id = s.id
         WHERE s.fetched_at >= ?1 AND s.fetched_at <= ?2
         ORDER BY s.fetched_at ASC, s.id ASC",
    )?;
    let rows = observations.query_map(params![timestamp(start), timestamp(end)], |row| {
        Ok(RawObservation {
            provider_id: row.get(0)?,
            display_name: row.get(1)?,
            fetched_at: row.get(2)?,
            data_kind: row.get(3)?,
            model: row.get(4)?,
            total_tokens: row.get(5)?,
            request_count: row.get(6)?,
            spend_amount: row.get(7)?,
            spend_currency: row.get(8)?,
            token_estimated_cost_usd: row.get(9)?,
            token_source: row.get(10)?,
        })
    })?;

    let mut latest_by_provider = BTreeMap::<String, LatestObservation>::new();
    let mut token_by_provider_date =
        BTreeMap::<(String, NaiveDate), (DateTime<Utc>, u64, MetricSource)>::new();
    let mut token_sources = Vec::new();
    let mut snapshot_count = 0_u64;
    for row in rows {
        let row = row?;
        if row.data_kind != data_kind_name(DataKind::Live) {
            continue;
        }
        let Some(fetched_at) = parse_timestamp(&row.fetched_at) else {
            continue;
        };
        snapshot_count = snapshot_count.saturating_add(1);
        let total_tokens = nonnegative_u64(row.total_tokens);
        let request_count = nonnegative_u64(row.request_count);
        let token_source = parse_metric_source(row.token_source.as_deref());
        let latest = LatestObservation {
            display_name: row.display_name,
            fetched_at,
            model: row.model,
            total_tokens,
            request_count,
            spend_amount: row.spend_amount,
            spend_currency: row.spend_currency,
            token_estimated_cost_usd: row.token_estimated_cost_usd,
            token_source,
        };
        latest_by_provider
            .entry(row.provider_id.clone())
            .and_modify(|current| {
                if latest.fetched_at >= current.fetched_at {
                    *current = latest.clone();
                }
            })
            .or_insert(latest);
        let source = token_source.unwrap_or(MetricSource::LocallyCalculated);
        if total_tokens.is_some() || request_count.is_some() {
            token_sources.push(source);
        }
        if let Some(total_tokens) = total_tokens {
            token_by_provider_date
                .entry((row.provider_id, fetched_at.date_naive()))
                .and_modify(|current| {
                    if fetched_at >= current.0 {
                        *current = (fetched_at, total_tokens, source);
                    }
                })
                .or_insert((fetched_at, total_tokens, source));
        }
    }
    drop(observations);

    let providers = latest_by_provider
        .iter()
        .map(|(provider_id, latest)| AnalyticsProvider {
            provider_id: provider_id.clone(),
            display_name: latest.display_name.clone(),
            model: latest.model.clone(),
            latest_at: latest.fetched_at,
            total_tokens: latest.total_tokens,
            request_count: latest.request_count,
            token_source: latest.token_source,
        })
        .collect::<Vec<_>>();
    let latest_total_tokens =
        sum_optional(latest_by_provider.values().map(|value| value.total_tokens));
    let latest_request_count =
        sum_optional(latest_by_provider.values().map(|value| value.request_count));
    let token_source = aggregate_source(token_sources.iter().copied().map(Some));

    let mut spend = BTreeMap::<String, (f64, Vec<MetricSource>)>::new();
    for latest in latest_by_provider.values() {
        if let (Some(amount), Some(currency)) =
            (latest.spend_amount, latest.spend_currency.as_ref())
        {
            if amount.is_finite() && amount >= 0.0 {
                let entry = spend.entry(currency.clone()).or_default();
                entry.0 += amount;
                entry.1.push(MetricSource::LocallyCalculated);
            }
        }
        if let Some(amount) = latest.token_estimated_cost_usd {
            if amount.is_finite() && amount >= 0.0 {
                let entry = spend.entry("USD".to_owned()).or_default();
                entry.0 += amount;
                entry.1.push(
                    latest
                        .token_source
                        .unwrap_or(MetricSource::LocallyCalculated),
                );
            }
        }
    }
    let estimated_spend = spend
        .into_iter()
        .map(|(currency, (amount, sources))| AnalyticsSpend {
            currency,
            amount,
            source: aggregate_source(sources.into_iter().map(Some))
                .unwrap_or(AnalyticsSource::LocallyCalculated),
        })
        .collect();

    let mut quotas = connection.prepare(
        "SELECT p.provider_key, p.display_name, s.fetched_at, s.data_kind,
                w.window_key, w.label, w.used_percent, w.remaining_percent, w.source
         FROM usage_windows w
         JOIN usage_snapshots s ON s.id = w.snapshot_id
         JOIN providers p ON p.id = s.provider_id
         WHERE s.fetched_at >= ?1 AND s.fetched_at <= ?2
         ORDER BY s.fetched_at ASC, w.id ASC",
    )?;
    let rows = quotas.query_map(params![timestamp(start), timestamp(end)], |row| {
        Ok(RawQuota {
            provider_id: row.get(0)?,
            display_name: row.get(1)?,
            fetched_at: row.get(2)?,
            data_kind: row.get(3)?,
            window_id: row.get(4)?,
            window_label: row.get(5)?,
            used_percent: row.get(6)?,
            remaining_percent: row.get(7)?,
            source: row.get(8)?,
        })
    })?;
    let mut latest_quotas = BTreeMap::<(String, String), QuotaPoint>::new();
    for row in rows {
        let row = row?;
        if row.data_kind != data_kind_name(DataKind::Live)
            || row.source != source_name(crate::providers::MetricSource::ProviderReported)
        {
            continue;
        }
        let Some(observed_at) = parse_timestamp(&row.fetched_at) else {
            continue;
        };
        let point = QuotaPoint {
            provider_id: row.provider_id.clone(),
            display_name: row.display_name,
            window_id: row.window_id.clone(),
            window_label: row.window_label,
            used_percent: row.used_percent,
            remaining_percent: row.remaining_percent,
            observed_at,
        };
        latest_quotas
            .entry((row.provider_id, row.window_id))
            .and_modify(|current| {
                if point.observed_at >= current.observed_at {
                    *current = point.clone();
                }
            })
            .or_insert(point);
    }

    let mut token_totals_by_date = BTreeMap::<NaiveDate, u64>::new();
    for ((_, date), (_, tokens, _)) in token_by_provider_date {
        let total = token_totals_by_date.entry(date).or_default();
        *total = total.saturating_add(tokens);
    }
    let token_series = token_totals_by_date
        .into_iter()
        .map(|(date, total_tokens)| TokenPoint { date, total_tokens })
        .collect();

    Ok(AnalyticsResponse {
        range,
        start_at: start,
        end_at: end,
        snapshot_count,
        provider_count: providers.len() as u64,
        latest_total_tokens,
        latest_request_count,
        token_source,
        estimated_spend,
        providers,
        token_series,
        quota_windows: latest_quotas.into_values().collect(),
    })
}

fn sum_optional(values: impl Iterator<Item = Option<u64>>) -> Option<u64> {
    let mut total = 0_u64;
    let mut found = false;
    for value in values.flatten() {
        found = true;
        total = total.saturating_add(value);
    }
    found.then_some(total)
}

fn nonnegative_u64(value: Option<i64>) -> Option<u64> {
    value.and_then(|value| u64::try_from(value).ok())
}

fn timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn parse_timestamp(text: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

fn parse_metric_source(value: Option<&str>) -> Option<MetricSource> {
    match value? {
        "provider_reported" => Some(MetricSource::ProviderReported),
        "locally_calculated" => Some(MetricSource::LocallyCalculated),
        _ => None,
    }
}

fn aggregate_source(values: impl Iterator<Item = Option<MetricSource>>) -> Option<AnalyticsSource> {
    let mut provider_reported = false;
    let mut locally_calculated = false;
    for source in values.flatten() {
        match source {
            MetricSource::ProviderReported => provider_reported = true,
            MetricSource::LocallyCalculated => locally_calculated = true,
        }
    }
    match (provider_reported, locally_calculated) {
        (false, false) => None,
        (true, false) => Some(AnalyticsSource::ProviderReported),
        (false, true) => Some(AnalyticsSource::LocallyCalculated),
        (true, true) => Some(AnalyticsSource::Mixed),
    }
}

fn data_kind_name(value: DataKind) -> &'static str {
    match value {
        DataKind::Live => "live",
        DataKind::Mock => "mock",
    }
}

fn source_name(value: MetricSource) -> &'static str {
    match value {
        MetricSource::ProviderReported => "provider_reported",
        MetricSource::LocallyCalculated => "locally_calculated",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        history,
        providers::{
            AuthState, DataKind, MetricSource, ProviderCapabilities, SpendEstimate, TokenUsage,
            UsageSnapshot, UsageWindow,
        },
    };

    fn snapshot(
        provider_id: &str,
        fetched_at: DateTime<Utc>,
        tokens: u64,
        requests: u64,
        used_percent: f64,
        data_kind: DataKind,
    ) -> UsageSnapshot {
        UsageSnapshot {
            provider_id: provider_id.into(),
            display_name: format!("{provider_id} Provider"),
            account_label: None,
            plan: None,
            has_subscription: None,
            capabilities: ProviderCapabilities {
                quota_windows: true,
                token_usage: true,
                ..ProviderCapabilities::default()
            },
            auth_state: AuthState::Authenticated,
            data_kind,
            windows: vec![UsageWindow {
                id: "weekly".into(),
                label: "Weekly".into(),
                used_percent: Some(used_percent),
                remaining_percent: Some(100.0 - used_percent),
                used_value: None,
                remaining_value: None,
                limit_value: None,
                unit: None,
                starts_at: None,
                reset_at: Some(fetched_at + Duration::days(7)),
                source: MetricSource::ProviderReported,
            }],
            credits: None,
            balance: None,
            balance_currency: None,
            spend_estimate: Some(SpendEstimate {
                amount: 1.5,
                currency: "USD".into(),
                window_days: 30,
            }),
            model: Some("test-model".into()),
            token_usage: Some(TokenUsage {
                total_tokens: Some(tokens),
                request_count: Some(requests),
                source: MetricSource::ProviderReported,
                ..TokenUsage::default()
            }),
            fetched_at,
        }
    }

    #[test]
    fn aggregates_latest_live_observations_without_counting_refresh_duplicates(
    ) -> Result<(), crate::error::AppError> {
        let temp = tempfile::tempdir().map_err(|_| crate::error::AppError::Storage)?;
        let path = temp.path().join("ellie.sqlite3");
        storage::initialize(&path)?;
        let now = DateTime::from_timestamp(1_700_086_400, 0).expect("test timestamp");
        history::insert_snapshot(
            &path,
            &snapshot(
                "alpha",
                now - Duration::hours(2),
                1_000,
                10,
                20.0,
                DataKind::Live,
            ),
        )?;
        history::insert_snapshot(
            &path,
            &snapshot(
                "alpha",
                now - Duration::hours(1),
                1_500,
                15,
                30.0,
                DataKind::Live,
            ),
        )?;
        let mut beta = snapshot(
            "beta",
            now - Duration::days(2),
            500,
            5,
            40.0,
            DataKind::Live,
        );
        if let Some(tokens) = beta.token_usage.as_mut() {
            tokens.estimated_cost_usd = Some(2.25);
        }
        history::insert_snapshot(&path, &beta)?;
        history::insert_snapshot(
            &path,
            &snapshot(
                "demo",
                now - Duration::hours(1),
                99_999,
                99,
                90.0,
                DataKind::Mock,
            ),
        )?;

        let result = query(&path, AnalyticsRange::SevenDays, now)?;
        assert_eq!(result.snapshot_count, 3);
        assert_eq!(result.provider_count, 2);
        assert_eq!(result.latest_total_tokens, Some(2_000));
        assert_eq!(result.latest_request_count, Some(20));
        assert_eq!(result.token_source, Some(AnalyticsSource::ProviderReported));
        assert_eq!(result.providers[0].total_tokens, Some(1_500));
        assert_eq!(result.token_series.len(), 2);
        assert_eq!(result.token_series[0].total_tokens, 500);
        assert_eq!(result.token_series[1].total_tokens, 1_500);
        assert_eq!(result.quota_windows.len(), 2);
        assert_eq!(result.estimated_spend[0].amount, 5.25);
        assert_eq!(result.estimated_spend[0].source, AnalyticsSource::Mixed);
        Ok(())
    }

    #[test]
    fn today_starts_at_utc_midnight() {
        let now = DateTime::from_timestamp(1_700_086_400, 0).expect("test timestamp");
        let (start, end) = AnalyticsRange::Today.bounds(now);
        assert_eq!(start.date_naive(), now.date_naive());
        assert_eq!(start.time().to_string(), "00:00:00");
        assert_eq!(end, now);
    }
}
