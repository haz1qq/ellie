use std::path::Path;

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{params, OptionalExtension};
use tauri_plugin_notification::NotificationExt;

use crate::{
    error::AppError,
    providers::{DataKind, MetricSource, ProviderOverview, UsageWindow},
    storage,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NotificationEvent {
    provider_name: String,
    window_label: String,
    threshold_percent: f64,
    remaining_percent: Option<f64>,
    reset_at: Option<DateTime<Utc>>,
}

impl NotificationEvent {
    fn body(&self) -> String {
        let remaining = self
            .remaining_percent
            .map(|value| format!(" {value:.0}% remaining."))
            .unwrap_or_default();
        let reset = self
            .reset_at
            .map(|value| {
                format!(
                    " Reset {}.",
                    value.to_rfc3339_opts(SecondsFormat::Secs, true)
                )
            })
            .unwrap_or_default();
        format!(
            "{} {} usage reached {:.0}%.{}{}",
            self.provider_name, self.window_label, self.threshold_percent, remaining, reset
        )
    }
}

/// Evaluates fresh provider results and sends at most one notification for a
/// threshold in a quota period. State is claimed before dispatch so repeated
/// polling cannot produce duplicate notifications if Windows is slow to show
/// the toast.
pub(crate) async fn notify_after_refresh(
    app: &tauri::AppHandle,
    database_path: &Path,
    providers: &[ProviderOverview],
) {
    let path = database_path.to_owned();
    let providers = providers.to_vec();
    let events =
        tauri::async_runtime::spawn_blocking(move || claim_notifications(&path, &providers)).await;
    let Ok(Ok(events)) = events else {
        tracing::warn!(event = "notification_evaluation_failed");
        return;
    };

    for event in events {
        if let Err(error) = app
            .notification()
            .builder()
            .title("Ellie · AI usage")
            .body(event.body())
            .show()
        {
            tracing::warn!(event = "notification_send_failed", error = ?error);
        }
    }
}

fn claim_notifications(
    database_path: &Path,
    providers: &[ProviderOverview],
) -> Result<Vec<NotificationEvent>, AppError> {
    let settings = storage::read_settings(database_path)?;
    if !settings.notifications_enabled {
        return Ok(vec![]);
    }
    let thresholds = settings.notification_thresholds;

    let mut connection = storage::connect(database_path)?;
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let mut events = Vec::new();

    for overview in providers {
        let Some(snapshot) = overview
            .snapshot
            .as_ref()
            .filter(|snapshot| snapshot.data_kind == DataKind::Live)
            .filter(|_| !overview.stale && overview.error.is_none())
        else {
            continue;
        };
        let provider_row_id: Option<i64> = transaction
            .query_row(
                "SELECT id FROM providers WHERE provider_key = ?1",
                params![snapshot.provider_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(provider_row_id) = provider_row_id else {
            continue;
        };

        for window in &snapshot.windows {
            let Some(used_percent) = window
                .used_percent
                .filter(|_| window.source == MetricSource::ProviderReported)
            else {
                continue;
            };
            let Some(period_key) = period_key(window) else {
                continue;
            };
            for threshold_percent in thresholds {
                if used_percent + f64::EPSILON < threshold_percent {
                    continue;
                }
                let inserted = transaction.execute(
                    "INSERT OR IGNORE INTO notification_state
                         (provider_id, window_key, period_key, threshold_percent, notified_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        provider_row_id,
                        window.id,
                        period_key,
                        threshold_percent,
                        Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
                    ],
                )?;
                if inserted == 1 {
                    events.push(NotificationEvent {
                        provider_name: snapshot.display_name.clone(),
                        window_label: window.label.clone(),
                        threshold_percent,
                        remaining_percent: window.remaining_percent,
                        reset_at: window.reset_at,
                    });
                }
            }
        }
    }

    transaction.commit()?;
    Ok(events)
}

fn period_key(window: &UsageWindow) -> Option<String> {
    window
        .reset_at
        .or(window.starts_at)
        .map(|timestamp| timestamp.to_rfc3339_opts(SecondsFormat::Millis, true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        history,
        providers::{AuthState, ProviderCapabilities, UsageSnapshot},
        storage,
    };
    use std::time::Duration;

    fn snapshot(reset_at: DateTime<Utc>, data_kind: DataKind) -> UsageSnapshot {
        UsageSnapshot {
            provider_id: "test-provider".into(),
            display_name: "Test Provider".into(),
            account_label: None,
            plan: None,
            has_subscription: None,
            capabilities: ProviderCapabilities {
                quota_windows: true,
                ..ProviderCapabilities::default()
            },
            auth_state: AuthState::Authenticated,
            data_kind,
            windows: vec![UsageWindow {
                id: "weekly".into(),
                label: "Weekly usage".into(),
                used_percent: Some(92.0),
                remaining_percent: Some(8.0),
                used_value: None,
                remaining_value: None,
                limit_value: None,
                unit: None,
                starts_at: Some(reset_at - Duration::from_secs(7 * 24 * 60 * 60)),
                reset_at: Some(reset_at),
                source: MetricSource::ProviderReported,
            }],
            credits: None,
            balance: None,
            balance_currency: None,
            spend_estimate: None,
            model: None,
            token_usage: None,
            fetched_at: Utc::now(),
        }
    }

    fn overview(snapshot: UsageSnapshot) -> ProviderOverview {
        ProviderOverview {
            provider_id: snapshot.provider_id.clone(),
            display_name: snapshot.display_name.clone(),
            snapshot: Some(snapshot),
            error: None,
            stale: false,
            last_successful_refresh: None,
            last_attempt_at: None,
            next_retry_at: None,
        }
    }

    #[test]
    fn claims_each_crossed_threshold_once_and_resets_with_period() -> Result<(), AppError> {
        let temp = tempfile::tempdir().map_err(|_| AppError::Storage)?;
        let path = temp.path().join("ellie.sqlite3");
        storage::initialize(&path)?;
        let mut settings = storage::read_settings(&path)?;
        settings.notification_thresholds = [50.0, 80.0, 95.0];
        storage::save_settings(&path, &settings)?;
        let first_reset = Utc::now() + Duration::from_secs(3_600);
        let first = snapshot(first_reset, DataKind::Live);
        history::insert_snapshot(&path, &first)?;
        let provider = overview(first);

        let events = claim_notifications(&path, std::slice::from_ref(&provider))?;
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].threshold_percent, 50.0);
        assert_eq!(events[1].threshold_percent, 80.0);
        assert!(claim_notifications(&path, std::slice::from_ref(&provider))?.is_empty());

        let second_reset = first_reset + Duration::from_secs(7 * 24 * 60 * 60);
        let mut next = provider.snapshot.clone().expect("snapshot");
        next.windows[0].reset_at = Some(second_reset);
        let events = claim_notifications(&path, &[overview(next)])?;
        assert_eq!(events.len(), 2);
        Ok(())
    }

    #[test]
    fn disabled_and_mock_data_do_not_claim_notification_state() -> Result<(), AppError> {
        let temp = tempfile::tempdir().map_err(|_| AppError::Storage)?;
        let path = temp.path().join("ellie.sqlite3");
        let mut settings = storage::initialize(&path)?;
        settings.notifications_enabled = false;
        storage::save_settings(&path, &settings)?;
        let live = snapshot(Utc::now() + Duration::from_secs(3_600), DataKind::Live);
        history::insert_snapshot(&path, &live)?;
        assert!(claim_notifications(&path, &[overview(live)])?.is_empty());

        settings.notifications_enabled = true;
        storage::save_settings(&path, &settings)?;
        let mock = snapshot(Utc::now() + Duration::from_secs(3_600), DataKind::Mock);
        assert!(claim_notifications(&path, &[overview(mock)])?.is_empty());
        let connection = storage::connect(&path)?;
        let count: i64 =
            connection.query_row("SELECT COUNT(*) FROM notification_state", [], |row| {
                row.get(0)
            })?;
        assert_eq!(count, 0);
        Ok(())
    }
}
