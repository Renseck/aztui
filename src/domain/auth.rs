use std::collections::HashMap;

use async_trait::async_trait;

use crate::domain::models::{AzureContext, Subscription, Tenant};
use crate::errors::AppError;

/* ============================================================================================== */
/*                                          AuthProvider                                          */
/* ============================================================================================== */

/// Provides authentication and context management.
/// This is the core capability for Phase 1.
#[async_trait]
pub trait AuthProvider: Send + Sync {
    /// Perform interactive login (opens browser).
    /// Returns the list of accessible tenants.
    async fn login(&self) -> Result<Vec<Tenant>, AppError>;

    /* ========================================================================================== */

    /// Login to a specific tenant. Returns subscriptions accessible under it.
    async fn login_to_tenant(&self, tenant_id: &str) -> Result<Vec<Subscription>, AppError>;

    /* ========================================================================================== */

    /// Get all known tenants and subscriptions (from cache or CLI).
    async fn list_contexts(
        &self,
    ) -> Result<(Vec<Tenant>, HashMap<String, Vec<Subscription>>), AppError>;

    /* ========================================================================================== */

    /// Set the active subscription in AZ CLI.
    async fn set_subscription(&self, subscription_id: &str) -> Result<(), AppError>;

    /* ========================================================================================== */

    /// Get the currently active context from AZ CLI state.
    async fn get_active_context(&self) -> Result<Option<AzureContext>, AppError>;
}