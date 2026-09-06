use async_trait::async_trait;
use chrono::{Duration, Utc};

use super::{
    AuthState, DataKind, DetectionResult, MetricSource, ProviderCapabilities, ProviderError,
    TokenUsage, UsageProvider, UsageSnapshot, UsageWindow,
};

/// Illustrative data only. This provider performs no I/O and has no credentials.
pub struct MockProvider;

#[async_trait]
impl UsageProvider for MockProvider {
    fn id(&self) -> &'static str {
        "ellie-demo"
    }
    fn display_name(&self) -> &'static str {
        "Ellie Demo"
    }
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            quota_windows: true,
            token_usage: true,
            account_balance: false,
            credits: false,
            cost_tracking: true,
            local_history: false,
        }
    }
    async fn detect(&self) -> Result<DetectionResult, ProviderError> {
        Ok(DetectionResult {
            auth_state: AuthState::Unsupported,
            detail: Some("Illustrative provider; no account connection.".into()),
        })
    }
    async fn authenticate(&self) -> Result<AuthState, ProviderError> {
        Ok(AuthState::Unsupported)
    }
    async fn fetch_usage(&self) -> Result<UsageSnapshot, ProviderError> {
        let now = Utc::now();
        let snapshot = UsageSnapshot {
            provider_id: self.id().into(),
            display_name: self.display_name().into(),
            account_label: Some("Illustrative account".into()),
            plan: Some("Demo".into()),
            capabilities: self.capabilities(),
            auth_state: AuthState::Unsupported,
            data_kind: DataKind::Mock,
            windows: vec![UsageWindow {
                id: "sample-window".into(),
                label: "Sample allowance".into(),
                used_percent: Some(41.0),
                remaining_percent: Some(59.0),
                used_value: None,
                remaining_value: None,
                limit_value: None,
                unit: None,
                starts_at: Some(now - Duration::hours(1)),
                reset_at: Some(now + Duration::hours(4)),
                source: MetricSource::ProviderReported,
            }],
            credits: None,
            balance: None,
            token_usage: Some(TokenUsage {
                total_tokens: Some(145_000),
                request_count: Some(28),
                estimated_cost_usd: Some(0.42),
                source: MetricSource::LocallyCalculated,
                ..TokenUsage::default()
            }),
            fetched_at: now,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_data_is_explicitly_marked_and_has_no_live_authentication() {
        let provider = MockProvider;
        let snapshot = provider.fetch_usage().await.expect("mock snapshot");
        assert_eq!(snapshot.data_kind, DataKind::Mock);
        assert_eq!(snapshot.auth_state, AuthState::Unsupported);
        assert!(snapshot.capabilities.quota_windows);
        assert!(snapshot.capabilities.token_usage);
        assert!(!snapshot.capabilities.account_balance);
        assert_eq!(snapshot.windows[0].source, MetricSource::ProviderReported);
        assert_eq!(
            snapshot.token_usage.expect("sample token data").source,
            MetricSource::LocallyCalculated
        );
    }
}
