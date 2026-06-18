use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::az::commands;
use crate::az::executor::AzCliExecutor;
use crate::az::parser;
use crate::cache::store::{CacheKey, CacheStore};
use crate::config::CacheConfig;
use crate::domain::graph::GraphProvider;
use crate::domain::models::GlobalResource;
use crate::errors::AppError;

const GRAPH_INVENTORY_CACHE_KIND: &str = "graph_inventory";
const GRAPH_PAGE_SIZE: u32 = 1000;
const GLOBAL_RESOURCES_KQL: &str =
    "Resources | project id, name, type, resourceGroup, subscriptionId, location | order by name asc";

/* ============================================================================================== */
/*                                        AzGraphProvider                                         */
/* ============================================================================================== */

/// Concrete [`GraphProvider`] that delegates to `az graph query`.
pub struct AzGraphProvider {
    executor: Arc<dyn AzCliExecutor>,
    cache: Arc<RwLock<CacheStore>>,
    cache_config: CacheConfig,
}

impl AzGraphProvider {
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
impl GraphProvider for AzGraphProvider {
    async fn list_all_resources(&self) -> Result<Vec<GlobalResource>, AppError> {
        let key = CacheKey::global(GRAPH_INVENTORY_CACHE_KIND);

        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.get::<Vec<GlobalResource>>(&key) {
                if entry.is_fresh() || entry.is_stale() {
                    return Ok(entry.value.clone());
                }
            }
        }

        self.fetch_and_cache().await
    }
}

/* ============================================================================================== */
/*                                         Private helpers                                        */
/* ============================================================================================== */

impl AzGraphProvider {
    async fn fetch_and_cache(&self) -> Result<Vec<GlobalResource>, AppError> {
        let mut all = Vec::new();
        let mut skip_token: Option<String> = None;

        loop {
            let args = commands::graph_query(
                GLOBAL_RESOURCES_KQL,
                GRAPH_PAGE_SIZE,
                skip_token.as_deref(),
            );
            let json = self
                .executor
                .execute(
                    &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                    self.cache_config.graph_hard_ttl,
                )
                .await?;

            let (mut rows, next) = parser::parse_graph_rows(&json)?;
            all.append(&mut rows);

            match next {
                Some(tok) => skip_token = Some(tok),
                None => break,
            }
        }

        let key = CacheKey::global(GRAPH_INVENTORY_CACHE_KIND);
        let mut cache = self.cache.write().await;
        cache.put(
            key,
            all.clone(),
            self.cache_config.graph_soft_ttl,
            self.cache_config.graph_hard_ttl,
        );

        Ok(all)
    }
}

/* ============================================================================================== */
/*                                              Tests                                             */
/* ============================================================================================== */

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::Duration;

    /// Executor that returns a queued sequence of responses, one per call, so a
    /// multi-page pagination loop can be exercised deterministically.
    struct SequenceExecutor {
        responses: Mutex<Vec<Result<String, AppError>>>,
    }

    impl SequenceExecutor {
        fn new(responses: Vec<Result<String, AppError>>) -> Self {
            Self { responses: Mutex::new(responses) }
        }
    }

    #[async_trait]
    impl AzCliExecutor for SequenceExecutor {
        async fn execute(&self, _args: &[&str], _dur: Duration) -> Result<String, AppError> {
            let mut q = self.responses.lock().unwrap();
            if q.is_empty() {
                panic!("executor called more times than responses queued");
            }
            q.remove(0)
        }
    }

    fn provider(responses: Vec<Result<String, AppError>>) -> AzGraphProvider {
        AzGraphProvider::new(
            Arc::new(SequenceExecutor::new(responses)),
            Arc::new(RwLock::new(CacheStore::new())),
            CacheConfig::default(),
        )
    }

    fn page(name: &str, token: Option<&str>) -> String {
        let tok = match token {
            Some(t) => format!(r#","skipToken":"{}""#, t),
            None => String::new(),
        };
        format!(
            r#"{{"data":[{{"id":"/x/{name}","name":"{name}","type":"microsoft.storage/storageaccounts","resourceGroup":"rg","subscriptionId":"s","location":"westeurope"}}]{tok}}}"#
        )
    }

    #[tokio::test]
    async fn accumulates_rows_across_pages() {
        let p = provider(vec![
            Ok(page("a", Some("tok1"))),
            Ok(page("b", None)),
        ]);
        let rows = p.list_all_resources().await.unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "a");
        assert_eq!(rows[1].name, "b");
    }

    #[tokio::test]
    async fn missing_extension_error_propagates() {
        let p = provider(vec![Err(crate::errors::error_from_cli_stderr(
            "ERROR: The command requires the extension resource-graph.",
        ))]);
        let err = p.list_all_resources().await.unwrap_err();
        assert_eq!(err.kind, crate::errors::ErrorKind::CliExtensionMissing);
    }
}