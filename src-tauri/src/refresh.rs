use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    time::Duration,
};

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::{
    history,
    providers::{ProviderError, ProviderOverview, ProviderRegistry, UsageSnapshot},
};

/// Normal automatic refresh interval. Providers may still be skipped while
/// their individual failure backoff is active.
pub const POLL_INTERVAL: Duration = Duration::from_secs(5 * 60);
/// Bounded retry delay after a provider failure.
const RETRY_BASE: Duration = Duration::from_secs(30);
const RETRY_MAX: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshResponse {
    pub providers: Vec<ProviderOverview>,
    pub refreshed: bool,
    pub busy: bool,
}

#[derive(Debug, Clone, Default)]
struct FailureBackoff {
    failures: u32,
    next_retry_at: Option<DateTime<Utc>>,
}

/// Owns the refresh critical section, last successful provider results, and
/// per-provider retry state. All refresh entry points use this coordinator.
pub struct RefreshCoordinator {
    operation: tokio::sync::Mutex<()>,
    cache: tokio::sync::RwLock<BTreeMap<String, ProviderOverview>>,
    backoff: tokio::sync::Mutex<BTreeMap<String, FailureBackoff>>,
}

impl Default for RefreshCoordinator {
    fn default() -> Self {
        Self {
            operation: tokio::sync::Mutex::new(()),
            cache: tokio::sync::RwLock::new(BTreeMap::new()),
            backoff: tokio::sync::Mutex::new(BTreeMap::new()),
        }
    }
}

impl RefreshCoordinator {
    /// Refreshes every due provider. `force` is used by startup and explicit
    /// user refreshes; polling honors per-provider backoff.
    pub async fn refresh_all(
        &self,
        registry: &ProviderRegistry,
        database_path: &Path,
        force: bool,
    ) -> RefreshResponse {
        let Some(_operation) = self.operation.try_lock().ok() else {
            return self.cached_response(registry, true).await;
        };
        let ids = registry.identities();
        let results = self.refresh_ids(registry, database_path, ids, force).await;
        self.response(results, false).await
    }

    /// Refreshes one provider without allowing it to overlap a full refresh.
    /// Explicit per-provider refreshes bypass that provider's backoff.
    pub async fn refresh_one(
        &self,
        registry: &ProviderRegistry,
        database_path: &Path,
        provider_id: &str,
    ) -> RefreshResponse {
        let Some(_operation) = self.operation.try_lock().ok() else {
            return self.cached_response(registry, true).await;
        };
        let ids = registry
            .identities()
            .into_iter()
            .filter(|(id, _)| id == provider_id)
            .collect::<Vec<_>>();
        if ids.is_empty() {
            return self.cached_response(registry, false).await;
        }
        self.refresh_ids(registry, database_path, ids, true).await;
        let mut response = self.cached_response(registry, false).await;
        response.refreshed = true;
        response
    }

    async fn refresh_ids(
        &self,
        registry: &ProviderRegistry,
        database_path: &Path,
        ids: Vec<(String, String)>,
        force: bool,
    ) -> Vec<ProviderOverview> {
        let now = Utc::now();
        let mut fetched = Vec::with_capacity(ids.len());
        let mut skipped = BTreeSet::new();
        for (provider_id, display_name) in ids {
            if !force && !self.is_due(&provider_id, now).await {
                skipped.insert(provider_id.clone());
                if let Some(cached) = self.cache.read().await.get(&provider_id).cloned() {
                    fetched.push(cached);
                } else {
                    fetched.push(empty_overview(provider_id, display_name));
                }
                continue;
            }
            let mut overview = registry
                .refresh_one(&provider_id)
                .await
                .unwrap_or_else(|error| failed_overview(provider_id.clone(), display_name, error));
            overview.last_attempt_at = Some(now);
            fetched.push(overview);
        }

        attach_spend_estimates(database_path, &mut fetched).await;
        persist_successes(database_path, &fetched).await;

        let mut merged = Vec::with_capacity(fetched.len());
        for overview in fetched {
            if skipped.contains(&overview.provider_id) {
                merged.push(overview);
            } else {
                merged.push(self.merge_result(database_path, overview).await);
            }
        }
        merged
    }

    async fn merge_result(
        &self,
        database_path: &Path,
        mut current: ProviderOverview,
    ) -> ProviderOverview {
        if current.error.is_none() && current.snapshot.is_some() {
            current.stale = false;
            current.last_successful_refresh = current
                .snapshot
                .as_ref()
                .map(|snapshot| snapshot.fetched_at);
            current.next_retry_at = None;
            self.backoff.lock().await.remove(&current.provider_id);
            self.cache
                .write()
                .await
                .insert(current.provider_id.clone(), current.clone());
            return current;
        }

        let cached = self.cache.read().await.get(&current.provider_id).cloned();
        if let Some(cached) = cached.as_ref().filter(|result| result.snapshot.is_some()) {
            current.snapshot = cached.snapshot.clone();
            current.last_successful_refresh = cached
                .last_successful_refresh
                .or_else(|| cached.snapshot.as_ref().map(|snapshot| snapshot.fetched_at));
            current.stale = true;
        } else if current.snapshot.is_none() {
            if let Ok(Some(snapshot)) = load_latest(database_path, &current.provider_id).await {
                current.last_successful_refresh = Some(snapshot.fetched_at);
                current.snapshot = Some(snapshot);
                current.stale = true;
            }
        }

        let next_retry_at = self.record_failure(&current.provider_id).await;
        current.next_retry_at = next_retry_at;
        self.cache
            .write()
            .await
            .insert(current.provider_id.clone(), current.clone());
        current
    }

    async fn is_due(&self, provider_id: &str, now: DateTime<Utc>) -> bool {
        self.backoff
            .lock()
            .await
            .get(provider_id)
            .and_then(|failure| failure.next_retry_at)
            .is_none_or(|next| next <= now)
    }

    async fn record_failure(&self, provider_id: &str) -> Option<DateTime<Utc>> {
        let mut backoff = self.backoff.lock().await;
        let failure = backoff.entry(provider_id.to_owned()).or_default();
        failure.failures = failure.failures.saturating_add(1);
        let delay = retry_delay(failure.failures);
        let next = Utc::now()
            + chrono::Duration::from_std(delay).unwrap_or_else(|_| chrono::Duration::minutes(30));
        failure.next_retry_at = Some(next);
        failure.next_retry_at
    }

    async fn response(&self, results: Vec<ProviderOverview>, busy: bool) -> RefreshResponse {
        RefreshResponse {
            providers: results,
            refreshed: !busy,
            busy,
        }
    }

    pub(crate) async fn cached_response(
        &self,
        registry: &ProviderRegistry,
        busy: bool,
    ) -> RefreshResponse {
        let cache = self.cache.read().await;
        let providers = registry
            .identities()
            .into_iter()
            .map(|(id, display_name)| {
                cache
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| empty_overview(id, display_name))
            })
            .collect();
        RefreshResponse {
            providers,
            refreshed: false,
            busy,
        }
    }
}

fn empty_overview(provider_id: String, display_name: String) -> ProviderOverview {
    ProviderOverview {
        provider_id,
        display_name,
        snapshot: None,
        error: None,
        stale: false,
        last_successful_refresh: None,
        last_attempt_at: None,
        next_retry_at: None,
    }
}

fn failed_overview(
    provider_id: String,
    display_name: String,
    error: ProviderError,
) -> ProviderOverview {
    ProviderOverview {
        provider_id,
        display_name,
        snapshot: None,
        error: Some(error),
        stale: false,
        last_successful_refresh: None,
        last_attempt_at: None,
        next_retry_at: None,
    }
}

async fn load_latest(
    path: &Path,
    provider_id: &str,
) -> Result<Option<UsageSnapshot>, crate::error::AppError> {
    let path = path.to_path_buf();
    let provider_id = provider_id.to_owned();
    tokio::task::spawn_blocking(move || history::latest_snapshot(&path, &provider_id))
        .await
        .map_err(|_| crate::error::AppError::Background)?
}

async fn persist_successes(path: &Path, overviews: &[ProviderOverview]) {
    let path = path.to_path_buf();
    let snapshots: Vec<UsageSnapshot> = overviews
        .iter()
        .filter(|overview| overview.error.is_none())
        .filter_map(|overview| overview.snapshot.clone())
        .collect();
    let _ = tokio::time::timeout(
        history::HISTORY_CLEANUP_TIMEOUT,
        tokio::task::spawn_blocking(move || {
            for snapshot in snapshots {
                if let Err(error) = history::insert_snapshot(&path, &snapshot) {
                    tracing::warn!(event = "history_insert_failed", provider = ?snapshot.provider_id, error = ?error);
                }
            }
        }),
    )
    .await;
}

async fn attach_spend_estimates(path: &Path, overviews: &mut [ProviderOverview]) {
    let path = path.to_path_buf();
    let candidates: Vec<(String, f64, String)> = overviews
        .iter()
        .filter(|overview| overview.error.is_none())
        .filter_map(|overview| {
            let snapshot = overview.snapshot.as_ref()?;
            Some((
                snapshot.provider_id.clone(),
                snapshot.balance?,
                snapshot.balance_currency.clone()?,
            ))
        })
        .collect();
    let estimates = tokio::task::spawn_blocking(move || {
        let mut estimates = BTreeMap::new();
        for (provider_id, balance, currency) in candidates {
            if let Ok(Some(estimate)) = history::spend_estimate(
                &path,
                &provider_id,
                balance,
                &currency,
                history::SPEND_ESTIMATE_WINDOW,
            ) {
                estimates.insert(provider_id, estimate);
            }
        }
        estimates
    })
    .await
    .unwrap_or_default();
    for overview in overviews {
        if let Some(snapshot) = overview.snapshot.as_mut() {
            snapshot.spend_estimate = estimates.get(&snapshot.provider_id).cloned();
        }
    }
}

fn retry_delay(failures: u32) -> Duration {
    let exponent = failures.saturating_sub(1).min(10);
    RETRY_BASE
        .checked_mul(1_u32 << exponent)
        .unwrap_or(RETRY_MAX)
        .min(RETRY_MAX)
}

/// Starts the periodic poller. The first tick waits for the normal interval;
/// startup/bootstrap performs the initial refresh.
pub fn spawn_poller(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(POLL_INTERVAL).await;
            crate::commands::refresh_all_from_app(&app, false).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{
        AuthState, DataKind, DetectionResult, MetricSource, ProviderCapabilities, UsageProvider,
        UsageWindow,
    };
    use async_trait::async_trait;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tokio::sync::Notify;

    struct TestProvider {
        calls: Arc<AtomicUsize>,
        fail_after: Option<usize>,
        started: Option<Arc<Notify>>,
        release: Option<Arc<Notify>>,
    }

    fn snapshot() -> UsageSnapshot {
        UsageSnapshot {
            provider_id: "test-refresh".into(),
            display_name: "Refresh Test".into(),
            account_label: None,
            plan: None,
            has_subscription: None,
            capabilities: ProviderCapabilities {
                quota_windows: true,
                ..ProviderCapabilities::default()
            },
            auth_state: AuthState::Authenticated,
            data_kind: DataKind::Live,
            windows: vec![UsageWindow {
                id: "test".into(),
                label: "Test".into(),
                used_percent: Some(10.0),
                remaining_percent: Some(90.0),
                used_value: None,
                remaining_value: None,
                limit_value: None,
                unit: None,
                starts_at: None,
                reset_at: None,
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

    #[async_trait]
    impl UsageProvider for TestProvider {
        fn id(&self) -> &'static str {
            "test-refresh"
        }
        fn display_name(&self) -> &'static str {
            "Refresh Test"
        }
        fn capabilities(&self) -> ProviderCapabilities {
            snapshot().capabilities
        }
        async fn detect(&self) -> Result<DetectionResult, ProviderError> {
            Ok(DetectionResult {
                auth_state: AuthState::Authenticated,
                detail: None,
            })
        }
        async fn authenticate(&self) -> Result<AuthState, ProviderError> {
            Ok(AuthState::Authenticated)
        }
        async fn fetch_usage(&self) -> Result<UsageSnapshot, ProviderError> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            if let Some(started) = &self.started {
                started.notify_one();
            }
            if let Some(release) = &self.release {
                release.notified().await;
            }
            if self.fail_after.is_some_and(|limit| call > limit) {
                Err(ProviderError::Unavailable)
            } else {
                Ok(snapshot())
            }
        }
    }

    fn test_path() -> tempfile::TempDir {
        tempfile::tempdir().expect("temp dir")
    }

    #[test]
    fn retry_delay_is_bounded() {
        assert_eq!(retry_delay(1), RETRY_BASE);
        assert_eq!(retry_delay(2), RETRY_BASE * 2);
        assert_eq!(retry_delay(20), RETRY_MAX);
    }

    #[tokio::test]
    async fn preserves_last_successful_snapshot_after_failure() {
        let temp = test_path();
        crate::storage::initialize(&temp.path().join("ellie.sqlite3")).expect("database");
        let calls = Arc::new(AtomicUsize::new(0));
        let provider = Arc::new(TestProvider {
            calls: calls.clone(),
            fail_after: Some(1),
            started: None,
            release: None,
        });
        let mut registry = ProviderRegistry::default();
        registry.register(provider);
        let coordinator = RefreshCoordinator::default();
        let path = temp.path().join("ellie.sqlite3");
        let first = coordinator.refresh_all(&registry, &path, true).await;
        assert!(!first.providers[0].stale);
        let second = coordinator.refresh_all(&registry, &path, true).await;
        let result = &second.providers[0];
        assert!(result.stale);
        assert_eq!(result.error, Some(ProviderError::Unavailable));
        assert!(result.snapshot.is_some());
        assert!(result.last_successful_refresh.is_some());
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn skips_backed_off_provider_until_retry_time() {
        let temp = test_path();
        crate::storage::initialize(&temp.path().join("ellie.sqlite3")).expect("database");
        let calls = Arc::new(AtomicUsize::new(0));
        let provider = Arc::new(TestProvider {
            calls: calls.clone(),
            fail_after: Some(0),
            started: None,
            release: None,
        });
        let mut registry = ProviderRegistry::default();
        registry.register(provider);
        let coordinator = RefreshCoordinator::default();
        let path = temp.path().join("ellie.sqlite3");
        let first = coordinator.refresh_all(&registry, &path, true).await;
        assert!(first.providers[0].next_retry_at.is_some());
        let second = coordinator.refresh_all(&registry, &path, false).await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            second.providers[0].next_retry_at,
            first.providers[0].next_retry_at
        );
    }

    #[tokio::test]
    async fn rejects_overlapping_refresh_and_allows_the_first_to_finish() {
        let temp = test_path();
        crate::storage::initialize(&temp.path().join("ellie.sqlite3")).expect("database");
        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let provider = Arc::new(TestProvider {
            calls: Arc::new(AtomicUsize::new(0)),
            fail_after: None,
            started: Some(started.clone()),
            release: Some(release.clone()),
        });
        let mut registry = ProviderRegistry::default();
        registry.register(provider);
        let coordinator = Arc::new(RefreshCoordinator::default());
        let path = temp.path().join("ellie.sqlite3");
        let first_coordinator = coordinator.clone();
        let first_path = path.clone();
        let first = tokio::spawn(async move {
            first_coordinator
                .refresh_all(&registry, &first_path, true)
                .await
        });
        started.notified().await;
        let empty_registry = ProviderRegistry::default();
        let busy = coordinator.refresh_all(&empty_registry, &path, true).await;
        assert!(busy.busy);
        release.notify_one();
        let result = first.await.expect("first refresh");
        assert!(result.refreshed);
    }
}
