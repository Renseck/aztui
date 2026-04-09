// Phase 3 - resource browsing capability.

use async_trait::async_trait;

use crate::domain::models::{Resource, ResourceGroup};
use crate::errors::AppError;

/* ============================================================================================== */
/*                                        ResourceProvider                                        */
/* ============================================================================================== */

/// Provides resource browsing capabilites. (Phase 3)
#[async_trait]
pub trait ResourceProvider: Send + Sync {
    async fn list_resource_groups(
        &self,
        subscription_id: &str,
    ) -> Result<Vec<ResourceGroup>, AppError>;

    async fn list_resources(
        &self,
        subscription_id: &str,
    ) -> Result<Vec<Resource>, AppError>;
}