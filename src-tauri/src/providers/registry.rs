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
                    snapshot: Some(snapshot),
                    error: None,
                },
                Err(error) => ProviderOverview {
                    snapshot: None,
                    error: Some(error),
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
                snapshot: Some(snapshot),
                error: None,
            },
            Err(error) => ProviderOverview {
                snapshot: None,
                error: Some(error),
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
        assert!(results
            .iter()
            .any(|result| result.error == Some(ProviderError::Unavailable)));
    }
}
