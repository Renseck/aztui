use async_trait::async_trait;

use crate::domain::models::GlobalResource;
use crate::errors::AppError;

/* ============================================================================================== */
/*                                        GraphProvider trait                                     */
/* ============================================================================================== */

/// Capability for querying Azure Resource Graph across every subscription the
/// signed-in identity can see.
#[async_trait]
pub trait GraphProvider: Send + Sync {
    /// Pulls the full cross-subscription resource inventory, paginating
    /// internally until the Resource Graph result set is exhausted.
    async fn list_all_resources(&self) -> Result<Vec<GlobalResource>, AppError>;

    /// Installs the `resource-graph` CLI extension (`az extension add`). Returns
    /// once the extension is installed or an error if the install failed.
    async fn install_resource_graph(&self) -> Result<(), AppError>;
}