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
}
