use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use crate::az::commands;
use crate::az::executor::AzCliExecutor;
use crate::az::parser;
use crate::domain::activity::{ActivityLogProvider, ActivityScope, ActivityWindow};
use crate::domain::models::ActivityLogEntry;
use crate::errors::AppError;

/* ============================================================================================== */
/*                                       AzActivityLogProvider                                     */
/* ============================================================================================== */

/// Concrete [`ActivityLogProvider`] that delegates to the AZ CLI.
pub struct AzActivityLogProvider {
    executor: Arc<dyn AzCliExecutor>,
    timeout: Duration,
}

impl AzActivityLogProvider {
    pub fn new(executor: Arc<dyn AzCliExecutor>, timeout: Duration) -> Self {
        Self { executor, timeout }
    }
}

/* ============================================================================================== */

#[async_trait]
impl ActivityLogProvider for AzActivityLogProvider {
    async fn list_activity(
        &self,
        scope: &ActivityScope,
        window: ActivityWindow,
    ) -> Result<Vec<ActivityLogEntry>, AppError> {
        let args = commands::activity_log_list(scope, window);
        let json = self
            .executor
            .execute(
                &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                self.timeout,
            )
            .await?;

        parser::parse_activity_log(&json)
    }
}

/* ============================================================================================== */
/*                                              Tests                                             */
/* ============================================================================================== */

#[cfg(test)]
mod tests {
    use super::*;

    struct MockExecutor {
        response: String,
    }

    #[async_trait]
    impl AzCliExecutor for MockExecutor {
        async fn execute(&self, _args: &[&str], _dur: Duration) -> Result<String, AppError> {
            Ok(self.response.clone())
        }
    }

    #[tokio::test]
    async fn list_activity_parses_entries() {
        let json = r#"[
            {
                "eventTimestamp": "2026-06-17T10:42:00Z",
                "operationName": { "localizedValue": "Restart Virtual Machine" },
                "status": { "value": "Succeeded" },
                "level": "Informational",
                "resourceId": "/subscriptions/s/resourceGroups/rg/providers/Microsoft.Compute/virtualMachines/web-01"
            }
        ]"#;
        let provider = AzActivityLogProvider::new(
            Arc::new(MockExecutor { response: json.to_string() }),
            Duration::from_secs(30),
        );

        let scope = ActivityScope::Subscription { subscription_id: "s".into() };
        let entries = provider.list_activity(&scope, ActivityWindow::Day).await.unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].resource_name, "web-01");
    }
}