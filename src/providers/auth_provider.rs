use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::az::commands;
use crate::az::executor::AzCliExecutor;
use crate::az::parser;
use crate::cache::store::{CacheKey, CacheStore};
use crate::config::CacheConfig;
use crate::domain::auth::AuthProvider;
use crate::domain::models::{AzureContext, Subscription, Tenant};
use crate::errors::{AppError, ErrorKind};

const TENANTS_CACHE_KIND: &str = "tenants";
const CONTEXT_LIST_CACHE_KIND: &str = "context_list";

/* ============================================================================================== */

/// Concrete AuthProvider that delegates to the AZ CLI.
pub struct AzAuthProvider {
    executor: Arc<dyn AzCliExecutor>,
    cache: Arc<RwLock<CacheStore>>,
    cache_config: CacheConfig,
}

impl AzAuthProvider {
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
impl AuthProvider for AzAuthProvider {
    async fn login(&self) -> Result<Vec<Tenant>, AppError> {
        let args = commands::login();
        self.executor
            .execute(&args.iter().map(|s| s.as_ref()).collect::<Vec<_>>(), self.cache_config.context_hard_ttl)
            .await?;

        let (tenants, _) = self.fetch_and_cache_context_list().await?;
        Ok(tenants)
    }

    /* ========================================================================================== */
    async fn login_to_tenant(&self, tenant_id: &str) -> Result<Vec<Subscription>, AppError> {
        let args = commands::login_tenant(tenant_id);
        self.executor
            .execute(
                &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                self.cache_config.context_hard_ttl,
            )
            .await?;

        // Invalidate tenant-scoped cache on re-login.
        {
            let mut cache = self.cache.write().await;
            cache.invalidate_scope(&crate::cache::CacheScope::Tenant(tenant_id.to_string()));
        }

        let (_, by_tenant) = self.fetch_and_cache_context_list().await?;
        Ok(by_tenant.get(tenant_id).cloned().unwrap_or_default())
    }

    /* ========================================================================================== */
    async fn list_contexts(
        &self,
    ) -> Result<(Vec<Tenant>, HashMap<String, Vec<Subscription>>), AppError> {
        let key = CacheKey::global(CONTEXT_LIST_CACHE_KIND);

        // Check cache freshness before hitting the CLI.
        {
            let cache = self.cache.read().await;
            if let Some(entry) =
                cache.get::<(Vec<Tenant>, HashMap<String, Vec<Subscription>>)>(&key)
            {
                if entry.is_fresh() || entry.is_stale() {
                    return Ok(entry.value.clone());
                }
            }
        }

        // Expired or missing — synchronous refresh.
        self.fetch_and_cache_context_list().await
    }

    /* ========================================================================================== */
    async fn set_subscription(&self, subscription_id: &str) -> Result<(), AppError> {
        let args = commands::account_set(subscription_id);
        self.executor
            .execute(
                &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                self.cache_config.context_soft_ttl,
            )
            .await?;
        Ok(())
    }

    /* ========================================================================================== */
    async fn get_active_context(&self) -> Result<Option<AzureContext>, AppError> {
        let args = commands::account_show();
        match self.executor.execute(&args, self.cache_config.context_soft_ttl).await {
            Ok(json) => {
                let ctx = parser::parse_account_show(&json)?;
                Ok(Some(ctx))
            }
            Err(e) if matches!(e.kind, ErrorKind::AuthExpired | ErrorKind::AuthFailed) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

/* ============================================================================================== */
/*                                         Private helpers                                        */
/* ============================================================================================== */

impl AzAuthProvider {
    async fn fetch_and_cache_context_list(
        &self,
    ) -> Result<(Vec<Tenant>, HashMap<String, Vec<Subscription>>), AppError> {
        let account_args = commands::account_list_all();
        let account_json = self
            .executor
            .execute(&account_args, self.cache_config.context_hard_ttl)
            .await?;

        // Try to fetch tenant display name; fall back gracefully if unsupported.
        let tenant_json = match self
            .executor
            .execute(
                &commands::account_tenant_list(),
                Duration::from_secs(10),
            )
            .await
        {
            Ok(json) => Some(json),
            Err(_) => None,
        };

        let result = parser::parse_account_list(&account_json, tenant_json.as_deref())?;

        let key = CacheKey::global(CONTEXT_LIST_CACHE_KIND);
        let mut cache = self.cache.write().await;
        cache.put(
            key,
            result.clone(),
            self.cache_config.context_soft_ttl,
            self.cache_config.context_hard_ttl,
        );

        Ok(result)
    }
}