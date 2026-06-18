// Phase 4 - cost and usage data capability.

use async_trait::async_trait;

use crate::domain::models::{CostPeriod, CostSummary};
use crate::errors::AppError;

/* ============================================================================================== */
/*                                          CostProvider                                          */
/* ============================================================================================== */

/// Provides cost and usage data. (Phase 4)
#[async_trait]
pub trait CostProvider: Send + Sync {
    async fn get_cost_summary(
        &self,
        subscription_id: &str,
        period: &CostPeriod,
    ) -> Result<CostSummary, AppError>;

    /* ========================================================================================== */
    async fn get_resource_group_cost(
        &self,
        subscription_id: &str,
        resource_group: &str,
        period: &CostPeriod,
    ) -> Result<CostSummary, AppError>;

    /* ========================================================================================== */
    /// Whole-subscription cost aggregated by resource group.
    async fn get_subscription_cost_grouped_by_rg(
        &self,
        subscription_id: &str,
        period: &CostPeriod,
    ) -> Result<CostSummary, AppError>;
}