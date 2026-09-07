use std::{collections::BTreeMap, sync::Arc};

use super::{ProviderCapabilities, ProviderError, ProviderOverview, UsageProvider};

#[derive(Default)]
pub struct ProviderRegistry {
    providers: BTreeMap<String, Arc<dyn UsageProvider>>,
}

impl ProviderRegistry {
    pub fn register(&mut self, provider: Arc<dyn UsageProvider>) {
        self.providers.insert(provider.id().to_owned(), provider);
    }

    pub fn identities(&self) -> Vec<(String, String)> {
        self.providers
            .values()
            .map(|provider| (provider.id().to_owned(), provider.display_name().to_owned()))
            .collect()
    }

    pub fn capabilities(&self, id: &str) -> Option<ProviderCapabilities> {
        self.providers
            .get(id)
            .map(|provider| provider.capabilities())
    }

    pub async fn refresh_all(&self) -> Vec<ProviderOverview> {
        let mut results = Vec::with_capacity(self.providers.len());
        for provider in self.providers.values() {
            let overview = match provider.fetch_usage().await {
                Ok(snapshot) => ProviderOverview {
                    provider_id: provider.id().to_owned(),
                    display_name: provider.display_name().to_owned(),
                    snapshot: Some(snapshot),
                    error: None,
                    stale: false,
                    last_successful_refresh: None,
                    last_attempt_at: None,
                    next_retry_at: None,
                },
                Err(error) => ProviderOverview {
                    provider_id: provider.id().to_owned(),
                    display_name: provider.display_name().to_owned(),
                    snapshot: None,
                    error: Some(error),
                    stale: false,
                    last_successful_refresh: None,
                    last_attempt_at: None,
                    next_retry_at: None,
                },
            };
            results.push(overview);
        }
        results
    }

    pub async fn refresh_one(&self, id: &str) -> Result<ProviderOverview, ProviderError> {
        let provider = self.providers.get(id).ok_or(ProviderError::Unavailable)?;
        Ok(match provider.fetch_usage().await {
            Ok(snapshot) => ProviderOverview {
                provider_id: provider.id().to_owned(),
                display_name: provider.display_name().to_owned(),
                snapshot: Some(snapshot),
                error: None,
                stale: false,
                last_successful_refresh: None,
                last_attempt_at: None,
                next_retry_at: None,
            },
            Err(error) => ProviderOverview {
                provider_id: provider.id().to_owned(),
                display_name: provider.display_name().to_owned(),
                snapshot: None,
                error: Some(error),
                stale: false,
                last_successful_refresh: None,
                last_attempt_at: None,
                next_retry_at: None,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{AuthState, DetectionResult, MockProvider, UsageSnapshot};
    use async_trait::async_trait;
    use std::sync::Arc;

    struct FailingProvider;
    #[async_trait]
    impl UsageProvider for FailingProvider {
        fn id(&self) -> &'static str {
            "failing"
        }
        fn display_name(&self) -> &'static str {
            "Failing"
        }
        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities::default()
        }
        async fn detect(&self) -> Result<DetectionResult, ProviderError> {
            Err(ProviderError::Unavailable)
        }
        async fn authenticate(&self) -> Result<AuthState, ProviderError> {
            Err(ProviderError::Unavailable)
        }
        async fn fetch_usage(&self) -> Result<UsageSnapshot, ProviderError> {
            Err(ProviderError::Unavailable)
        }
    }

    #[tokio::test]
    async fn isolates_one_provider_failure() {
        let mut registry = ProviderRegistry::default();
        registry.register(Arc::new(MockProvider));
        registry.register(Arc::new(FailingProvider));
        let results = registry.refresh_all().await;
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|result| result.snapshot.is_some()));
        let failed = results
            .iter()
            .find(|result| result.provider_id == "failing")
            .expect("failed provider identity");
        assert_eq!(failed.display_name, "Failing");
        assert_eq!(failed.error, Some(ProviderError::Unavailable));
        let single = registry
            .refresh_one("failing")
            .await
            .expect("registered provider");
        assert_eq!(*failed, single);
    }
}
