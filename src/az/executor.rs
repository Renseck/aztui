use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use tokio::process::Command as TokioCommand;
use tokio::time::timeout;

use crate::errors::{AppError, ErrorKind};

/* ============================================================================================== */
/*                                       AzCliExecutor trait                                      */
/* ============================================================================================== */

/// The contract for executing AZ CLI commands.
/// Concrete implementation calls `az` as a subprocess.
/// A mock implementation can return canned responses for testing.
#[async_trait]
pub trait AzCliExecutor: Send + Sync {
    /// Executes an `az` command and returns the raw stdout as a String.
    /// 
    /// # Arguments
    /// * `args` - CLI arguments, e.g. `["account", "list", "--all"]`
    /// * `dur` - maximum time to wait for the command
    /// 
    /// # Returns
    /// Raw stdout (typicall JSON) on success, or an [`AppError`] on failure.
    async fn execute(&self, args: &[&str], dur: Duration) -> Result<String, AppError>;
}

/* ============================================================================================== */
/*                                      SubprocessCliExecutor                                     */
/* ============================================================================================== */

/// Executes `az` as a real subprocess via Tokio.
pub struct SubprocessCliExecutor {
    /// Resolved path to the `az` binary.
    az_path: PathBuf,
}

impl SubprocessCliExecutor {

    pub fn new(az_path: Option<PathBuf>) -> Result<Self, AppError> {
        let path = match az_path {
            Some(p) => p,
            None => {
                // Try to resolve `az` via PATH by running `which az` / checking environment.
                // On Windows, `az` may be `az.cmd`.
                let candidates = if cfg!(windows) {
                    vec!["az.cmd", "az"]
                } else {
                    vec!["az"]
                };

                candidates
                    .iter()
                    .find_map(|name| which_az(name))
                    .ok_or_else(AppError::cli_not_found)?
            }
        };

        Ok(Self { az_path: path })
    }
}

/* ============================================================================================== */

#[async_trait]
impl AzCliExecutor for SubprocessCliExecutor {
    async fn execute(&self, args: &[&str], dur: Duration) -> Result<String, AppError> {
        let mut cmd = TokioCommand::new(&self.az_path);
        cmd.args(args);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let result = timeout(dur, async {
            let output = cmd.output().await.map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    AppError::cli_not_found()
                } else {
                    AppError::new(
                        ErrorKind::CliExecutionFailed,
                        format!("Failed to spawn az process: {}", e),
                    )
                }
            })?;

            if output.status.success() {
                String::from_utf8(output.stdout).map_err(|e| {
                    AppError::cli_parse_error(format!("az output is not valid UTF-8: {}", e))
                })
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                Err(crate::errors::error_from_cli_stderr(&stderr))
            }
        })
        .await;

        match result {
            Ok(inner) => inner,
            Err(_elapsed) => Err(AppError::cli_timeout()),
        }
    }
}

/* ============================================================================================== */
/*                                         Private helpers                                        */
/* ============================================================================================== */

fn which_az(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|path_var| {
        std::env::split_paths(&path_var).find_map(|dir| {
            let candidate = dir.join(name);
            if candidate.is_file() {
                Some(candidate)
            } else {
                None
            }
        })
    })
}