use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricSource {
    ProviderReported,
    #[default]
    LocallyCalculated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DataKind {
    Live,
    Mock,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthState {
    Authenticated,
    AuthenticationDetected,
    AuthenticationRequired,
    Expired,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectionResult {
    pub auth_state: AuthState,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilities {
    pub quota_windows: bool,
    pub token_usage: bool,
    pub account_balance: bool,
    pub credits: bool,
    pub cost_tracking: bool,
    pub local_history: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    pub id: String,
    pub label: String,
    pub used_percent: Option<f64>,
    pub remaining_percent: Option<f64>,
    pub used_value: Option<f64>,
    pub remaining_value: Option<f64>,
    pub limit_value: Option<f64>,
    pub unit: Option<String>,
    pub starts_at: Option<DateTime<Utc>>,
    pub reset_at: Option<DateTime<Utc>>,
    pub source: MetricSource,
}

impl UsageWindow {
    pub fn validate(&self) -> Result<(), ProviderError> {
        for value in [self.used_percent, self.remaining_percent]
            .into_iter()
            .flatten()
        {
            if !(0.0..=100.0).contains(&value) {
                return Err(ProviderError::InvalidSnapshot);
            }
        }
        if let (Some(used), Some(remaining)) = (self.used_percent, self.remaining_percent) {
            if (used + remaining - 100.0).abs() > f64::EPSILON {
                return Err(ProviderError::InvalidSnapshot);
            }
        }
        for value in [self.used_value, self.remaining_value, self.limit_value]
            .into_iter()
            .flatten()
        {
            if value.is_sign_negative() || !value.is_finite() {
                return Err(ProviderError::InvalidSnapshot);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
    pub cached_output_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub request_count: Option<u64>,
    pub estimated_cost_usd: Option<f64>,
    pub source: MetricSource,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSnapshot {
    pub provider_id: String,
    pub display_name: String,
    pub account_label: Option<String>,
    pub plan: Option<String>,
    /// Whether the account has an active subscription/entitlement. `None` means
    /// unknown or not subscription-based (for example API-billed accounts);
    /// `Some(false)` marks an unsubscribed account whose card should be hidden.
    pub has_subscription: Option<bool>,
    pub capabilities: ProviderCapabilities,
    pub auth_state: AuthState,
    pub data_kind: DataKind,
    pub windows: Vec<UsageWindow>,
    pub credits: Option<f64>,
    pub balance: Option<f64>,
    /// ISO-4217 currency code for `balance` (for example `USD`, `CNY`).
    pub balance_currency: Option<String>,
    pub token_usage: Option<TokenUsage>,
    pub fetched_at: DateTime<Utc>,
}

impl UsageSnapshot {
    pub fn validate(&self) -> Result<(), ProviderError> {
        if self.provider_id.trim().is_empty() || self.display_name.trim().is_empty() {
            return Err(ProviderError::InvalidSnapshot);
        }
        if self
            .balance
            .is_some_and(|value| value.is_sign_negative() || !value.is_finite())
            || self
                .credits
                .is_some_and(|value| value.is_sign_negative() || !value.is_finite())
        {
            return Err(ProviderError::InvalidSnapshot);
        }
        self.windows.iter().try_for_each(UsageWindow::validate)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderOverview {
    pub snapshot: Option<UsageSnapshot>,
    pub error: Option<ProviderError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum ProviderError {
    #[error("provider data was invalid")]
    InvalidSnapshot,
    #[error("provider access is not configured")]
    AuthenticationRequired,
    #[error("provider authentication has expired")]
    AuthenticationExpired,
    #[error("provider data is unavailable")]
    Unavailable,
}

#[async_trait]
pub trait UsageProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn capabilities(&self) -> ProviderCapabilities;
    async fn detect(&self) -> Result<DetectionResult, ProviderError>;
    async fn authenticate(&self) -> Result<AuthState, ProviderError>;
    async fn fetch_usage(&self) -> Result<UsageSnapshot, ProviderError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_percentages_and_inconsistent_complements() {
        let base = UsageWindow {
            id: "test".into(),
            label: "Test".into(),
            used_percent: Some(50.0),
            remaining_percent: Some(50.0),
            used_value: None,
            remaining_value: None,
            limit_value: None,
            unit: None,
            starts_at: None,
            reset_at: None,
            source: MetricSource::ProviderReported,
        };
        assert!(base.validate().is_ok());
        let over_limit = UsageWindow {
            used_percent: Some(101.0),
            ..base.clone()
        };
        assert_eq!(over_limit.validate(), Err(ProviderError::InvalidSnapshot));
        let inconsistent = UsageWindow {
            remaining_percent: Some(49.0),
            ..base
        };
        assert_eq!(inconsistent.validate(), Err(ProviderError::InvalidSnapshot));
    }

    #[test]
    fn rejects_negative_balances() {
        let snapshot = UsageSnapshot {
            provider_id: "demo".into(),
            display_name: "Demo".into(),
            account_label: None,
            plan: None,
            capabilities: ProviderCapabilities::default(),
            auth_state: AuthState::Unsupported,
            has_subscription: None,
            data_kind: DataKind::Mock,
            windows: vec![],
            credits: None,
            balance: Some(-0.01),
            balance_currency: None,
            token_usage: None,
            fetched_at: Utc::now(),
        };
        assert_eq!(snapshot.validate(), Err(ProviderError::InvalidSnapshot));
    }
}
