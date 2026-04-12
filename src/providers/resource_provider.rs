use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::az::commands;
use crate::az::executor::AzCliExecutor;
use crate::az::parser;
use crate::cache::store::{CacheKey, CacheStore};
use crate::config::CacheConfig;
use crate::domain::models::{Resource, ResourceGroup};
use crate::domain::resources::ResourceProvider;
use crate::errors::AppError;

/* ============================================================================================== */
/*                                       AzResourceProvider                                       */
/* ============================================================================================== */

/// Concrete ResourceProvider that delegates to the AZ CLI.
pub struct AzResourceProvider {
    executor: Arc<dyn AzCliExecutor>,
    cache: Arc<RwLock<CacheStore>>,
    cache_config: CacheConfig,
}

impl AzResourceProvider {
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
impl ResourceProvider for AzResourceProvider {
    async fn list_resource_groups(
        &self,
        subscription_id: &str,
    ) -> Result<Vec<ResourceGroup>, AppError> {
        let key = CacheKey::subscription(subscription_id, "resource_groups");

        // Check cahe.
        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.get::<Vec<ResourceGroup>>(&key) {
                if entry.is_fresh() || entry.is_stale() {
                    return Ok(entry.value.clone());
                }
            }
        }

        // Expired or missing - synchronous refresh.
        self.fetch_and_cache_resource_groups(subscription_id).await
    }

    /* ========================================================================================== */
    async fn list_resources(
        &self,
        subscription_id: &str,
        resource_group: &str,
    ) -> Result<Vec<Resource>, AppError> {
        let key = CacheKey::subscription(
            subscription_id, 
            format!("resources:{}", resource_group)
        );

        // Check cache.
        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.get::<Vec<Resource>>(&key) {
                if entry.is_fresh() || entry.is_stale() {
                    return Ok(entry.value.clone());
                }
            }
        }

        // Expired or missing - synchronous refresh.
        self.fetch_and_cache_resources(subscription_id, resource_group).await
    }
}

/* ============================================================================================== */
/*                                         Private helpers                                        */
/* ============================================================================================== */

impl AzResourceProvider {
    async fn fetch_and_cache_resource_groups(
        &self,
        subscription_id: &str,
    ) -> Result<Vec<ResourceGroup>, AppError> {
        let args = commands::resource_group_list(subscription_id);
        let json = self
            .executor
            .execute(
                &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                self.cache_config.resource_hard_ttl,
            )
            .await?;

        let groups = parser::parse_resource_group_list(&json, subscription_id)?;

        let key = CacheKey::subscription(subscription_id, "resource_group");
        let mut cache = self.cache.write().await;
        cache.put(
            key,
            groups.clone(),
            self.cache_config.resource_soft_ttl,
            self.cache_config.context_hard_ttl,
        );

        Ok(groups)
    }

    /* ========================================================================================== */
    async fn fetch_and_cache_resources(
        &self,
        subscription_id: &str,
        resource_group: &str,
    ) -> Result<Vec<Resource>, AppError> {
        let args = commands::resource_list(subscription_id, resource_group);
        let json = self
            .executor
            .execute(
                &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(), 
            self.cache_config.resource_hard_ttl
            )
            .await?;

        let resources = parser::parse_resource_list(&json)?;

        let key = CacheKey::subscription(
            subscription_id, 
            format!("resources:{}", resource_group)
        );
        let mut cache = self.cache.write().await;
        cache.put(
            key,
            resources.clone(),
            self.cache_config.resource_soft_ttl,
            self.cache_config.context_hard_ttl,
        );
        
        Ok(resources)
    }
}