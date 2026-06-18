use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::az::commands;
use crate::az::executor::AzCliExecutor;
use crate::az::parser;
use crate::cache::store::{CacheKey, CacheStore};
use crate::config::CacheConfig;
use crate::domain::cost::CostProvider;
use crate::domain::models::{CostPeriod, CostScope, CostSummary};
use crate::errors::AppError;

/* ============================================================================================== */
/*                                         AzCostProvider                                         */
/* ============================================================================================== */

/// Concrete CostProvider that delegates to the AZ CLI.
pub struct AzCostProvider {
    executor: Arc<dyn AzCliExecutor>,
    cache: Arc<RwLock<CacheStore>>,
    cache_config: CacheConfig,
}

impl AzCostProvider {
    pub fn new(
        executor: Arc<dyn AzCliExecutor>,
        cache: Arc<RwLock<CacheStore>>,
        cache_config: CacheConfig,
    ) -> Self {
        Self {
            executor,
            cache,
            cache_config,
        }
    }
}

/* ============================================================================================== */

#[async_trait]
impl CostProvider for AzCostProvider {
    async fn get_cost_summary(
        &self,
        subscription_id: &str,
        period: &CostPeriod,
    ) -> Result<CostSummary, AppError> {
        let key = CacheKey::subscription(
            subscription_id,
            format!("cost:{}:{}", period.from, period.to),
        );

        // Check cache.
        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.get::<CostSummary>(&key) {
                if entry.is_fresh() || entry.is_stale() {
                    return Ok(entry.value.clone());
                }
            }
        }

        // Expired or missing — synchronous refresh.
        self.fetch_and_cache_cost(subscription_id, period).await
    }

    /* ========================================================================================== */
    async fn get_resource_group_cost(
        &self,
        subscription_id: &str,
        resource_group: &str,
        period: &CostPeriod,
    ) -> Result<CostSummary, AppError> {
        let key = CacheKey::subscription(
            subscription_id,
            format!("cost:{}:{}:{}", resource_group, period.from, period.to),
        );

        // Check cache.
        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.get::<CostSummary>(&key) {
                if entry.is_fresh() || entry.is_stale() {
                    return Ok(entry.value.clone());
                }
            }
        }

        // Expired or missing — synchronous refresh.
        self.fetch_and_cache_rg_cost(subscription_id, resource_group, period)
            .await
    }

    /* ========================================================================================== */
    async fn get_subscription_cost_grouped_by_rg(
        &self,
        subscription_id: &str,
        period: &CostPeriod,
    ) -> Result<CostSummary, AppError> {
        let key = CacheKey::subscription(
            subscription_id,
            format!("cost_by_rg:{}:{}", period.from, period.to),
        );

        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.get::<CostSummary>(&key) {
                if entry.is_fresh() || entry.is_stale() {
                    return Ok(entry.value.clone());
                }
            }
        }

        self.fetch_and_cache_cost_by_rg(subscription_id, period).await
    }
}

/* ============================================================================================== */
/*                                         Private helpers                                        */
/* ============================================================================================== */

impl AzCostProvider {
    async fn fetch_and_cache_cost(
        &self,
        subscription_id: &str,
        period: &CostPeriod,
    ) -> Result<CostSummary, AppError> {
        let args = commands::cost_query_by_service(
            subscription_id,
            &period.from,
            &period.to,
        );
        let json = self
            .executor
            .execute(
                &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                self.cache_config.cost_hard_ttl,
            )
            .await?;

        let scope = CostScope::Subscription(subscription_id.to_string());
        let summary = parser::parse_cost_query(&json, scope, period.clone())?;

        let key = CacheKey::subscription(
            subscription_id,
            format!("cost:{}:{}", period.from, period.to),
        );
        let mut cache = self.cache.write().await;
        cache.put(
            key,
            summary.clone(),
            self.cache_config.cost_soft_ttl,
            self.cache_config.cost_hard_ttl,
        );

        Ok(summary)
    }

    /* ========================================================================================== */
    async fn fetch_and_cache_rg_cost(
        &self,
        subscription_id: &str,
        resource_group: &str,
        period: &CostPeriod,
    ) -> Result<CostSummary, AppError> {
        let args = commands::cost_query_by_resource_group(
            subscription_id,
            resource_group,
            &period.from,
            &period.to,
        );
        let json = self
            .executor
            .execute(
                &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                self.cache_config.cost_hard_ttl,
            )
            .await?;

        let scope = CostScope::ResourceGroup {
            subscription_id: subscription_id.to_string(),
            name: resource_group.to_string(),
        };
        let summary = parser::parse_cost_query(&json, scope, period.clone())?;

        let key = CacheKey::subscription(
            subscription_id,
            format!("cost:{}:{}:{}", resource_group, period.from, period.to),
        );
        let mut cache = self.cache.write().await;
        cache.put(
            key,
            summary.clone(),
            self.cache_config.cost_soft_ttl,
            self.cache_config.cost_hard_ttl,
        );

        Ok(summary)
    }

    /* ========================================================================================== */
    async fn fetch_and_cache_cost_by_rg(
        &self,
        subscription_id: &str,
        period: &CostPeriod,
    ) -> Result<CostSummary, AppError> {
        let args = commands::cost_query_grouped_by_resource_group(
            subscription_id,
            &period.from,
            &period.to,
        );
        let json = self
            .executor
            .execute(
                &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                self.cache_config.cost_hard_ttl,
            )
            .await?;

        let scope = CostScope::Subscription(subscription_id.to_string());
        let summary = parser::parse_cost_query(&json, scope, period.clone())?;

        let key = CacheKey::subscription(
            subscription_id,
            format!("cost_by_rg:{}:{}", period.from, period.to),
        );
        let mut cache = self.cache.write().await;
        cache.put(
            key,
            summary.clone(),
            self.cache_config.cost_soft_ttl,
            self.cache_config.cost_hard_ttl,
        );

        Ok(summary)
    }
}

/* ============================================================================================== */
/*                                              Tests                                             */
/* ============================================================================================== */

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use crate::az::executor::AzCliExecutor;

    struct MockExecutor {
        response: String,
    }

    #[async_trait]
    impl AzCliExecutor for MockExecutor {
        async fn execute(&self, _args: &[&str], _dur: Duration) -> Result<String, AppError> {
            Ok(self.response.clone())
        }
    }

    fn provider_with(response: &str) -> AzCostProvider {
        AzCostProvider::new(
            Arc::new(MockExecutor { response: response.to_string() }),
            Arc::new(RwLock::new(CacheStore::new())),
            CacheConfig::default(),
        )
    }

    #[tokio::test]
    async fn grouped_by_rg_parses_resource_group_rows() {
        let json = r#"{"properties":{"columns":[
            {"name":"PreTaxCost","type":"Number"},
            {"name":"ResourceGroupName","type":"String"},
            {"name":"Currency","type":"String"}],
            "rows":[[820.0,"rg-prod-web","EUR"],[410.0,"rg-data","EUR"]]}}"#;
        let provider = provider_with(json);
        let period = CostPeriod { from: "2026-03-01".into(), to: "2026-03-31".into() };

        let summary = provider
            .get_subscription_cost_grouped_by_rg("sub-1", &period)
            .await
            .unwrap();

        assert_eq!(summary.breakdown.len(), 2);
        assert_eq!(summary.breakdown[0].label, "rg-prod-web");
    }
}