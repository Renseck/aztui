use std::collections::HashMap;
use serde::{Deserialize, Serialize}

/// A normalized Azure tenant.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Tenant {
    pub id: String,
    pub display_name: String,
    pub default_domain: String,
}

/* ============================================================================================== */

/// A normalized Azure subscription.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Subscription {
    pub id: String,
    pub name: String,
    pub tenant_id: String,
    pub state: SubscriptionState,
}

/* ============================================================================================== */

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubscriptionState {
    Enabled,
    Disabled,
    Warned,
    PastDue,
    Unknown(String),
}

impl SubscriptionState {
    pub fn is_active(&self) -> bool {
        matches!(self, SubscriptionState::Enabled)
    }
}

impl std::fmt::Display for SubscriptionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubscriptionState::Enabled => write!(f, "Enabled"),
            SubscriptionState::Disabled => write!(f, "Disabled"),
            SubscriptionState::Warned => write!(f, "Warned"),
            SubscriptionState::PastDue => write!(f, "PastDue"),
            SubscriptionState::Unknown(s) => write!(f, "{}", s),
        }
    }
}

/* ============================================================================================== */

/// An Azure resource group. (Phase 3)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceGroup {
    pub name: String,
    pub subscription_id: String,
    pub location: String,
    pub tags: HashMap<String, String>,
}

/* ============================================================================================== */

/// A single Azure resource, generic enough for browsing. (Phase 3)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    pub id: String,
    pub name: String,
    pub resource_type: String,
    pub resource_group: String,
    pub location: String,
    pub tags: HashMap<String, String>,
}

/* ============================================================================================== */

/// Represents the user's current working context in Azure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AzureContext {
    pub tenant: Tenant,
    pub subscription: Subscription,
}

impl AzureContext {
    /// Returns a human-readable label: "Tenant Name / Subscription Name".
    pub fn label(&self) -> String {
        format!("{} / {}", self.tenant.display_name, self.subscription.name)
    }
}

/* ============================================================================================== */
/*                                      Phase 4 - Cost types                                      */
/* ============================================================================================== */

/// Cost summary for a scope. (Phase 4)
#[derive(Debug, Clone)]
pub struct CostSummary {
    pub scope: CostScope,
    pub currency: String,
    pub total: f64,
    pub period: CostPeriod,
    pub breakdown: Vec<CostLineItem>,
}

#[derive(Debug, Clone)]
pub enum CostScope {
    Subscription(String),
    ResourceGroup { subscription_id: String, name: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostPeriod {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone)]
pub struct CostLineItem {
    pub service_name: String,
    pub amount: f64,
}