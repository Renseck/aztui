use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use crate::az::commands;
use crate::az::executor::AzCliExecutor;
use crate::az::parser;
use crate::domain::models::RunCommandOutput;
use crate::domain::vm::VmProvider;
use crate::errors::AppError;

/* ============================================================================================== */
/*                                         AzVmProvider                                           */
/* ============================================================================================== */

/// Concrete [`VmProvider`] that delegates to the AZ CLI. Run-command calls can be
/// slow (tens of seconds), so it carries its own generous timeout rather than
/// the default CLI timeout.
pub struct AzVmProvider {
    executor: Arc<dyn AzCliExecutor>,
    timeout: Duration,
}

impl AzVmProvider {
    pub fn new(executor: Arc<dyn AzCliExecutor>, timeout: Duration) -> Self {
        Self { executor, timeout }
    }
}

/* ============================================================================================== */

#[async_trait]
impl VmProvider for AzVmProvider {
    async fn run_powershell(
        &self,
        subscription_id: &str,
        resource_group: &str,
        vm_name: &str,
        script: &str,
    ) -> Result<RunCommandOutput, AppError> {
        let args = commands::vm_run_command_powershell(
            subscription_id,
            resource_group,
            vm_name,
            script,
        );
        let json = self
            .executor
            .execute(
                &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                self.timeout,
            )
            .await?;

        parser::parse_run_command_output(&json)
    }
}

/* ============================================================================================== */
/*                                              Tests                                             */
/* ============================================================================================== */

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal executor returning a canned response, for testing the provider.
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
    async fn run_powershell_parses_output() {
        let json = r#"{"value":[
            {"code":"ComponentStatus/StdOut/succeeded","displayStatus":"Provisioning succeeded","message":"hello"},
            {"code":"ComponentStatus/StdErr/succeeded","displayStatus":"Provisioning succeeded","message":""}
        ]}"#;
        let provider = AzVmProvider::new(
            Arc::new(MockExecutor { response: json.to_string() }),
            Duration::from_secs(5),
        );

        let out = provider
            .run_powershell("sub", "rg", "vm", "Get-Date")
            .await
            .unwrap();

        assert_eq!(out.stdout, "hello");
        assert!(out.succeeded);
    }
}