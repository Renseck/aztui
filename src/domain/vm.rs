//! VM operations capability.

use async_trait::async_trait;

use crate::domain::models::RunCommandOutput;
use crate::errors::AppError;

/* ============================================================================================== */
/*                                          VmProvider                                            */
/* ============================================================================================== */

/// Provides VM operational capabilities (currently: run a PowerShell script).
#[async_trait]
pub trait VmProvider: Send + Sync {
    /// Runs a PowerShell script on a VM via `az vm run-command invoke` and
    /// returns its captured output.
    async fn run_powershell(
        &self,
        subscription_id: &str,
        resource_group: &str,
        vm_name: &str,
        script: &str,
    ) -> Result<RunCommandOutput, AppError>;
}