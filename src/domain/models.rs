use std::collections::HashMap;
use chrono::Datelike;
use serde::{Deserialize, Serialize};

/// A normalized Azure tenant.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Tenant {
    pub id: String,
    pub tenant_display_name: String,
    pub tenant_default_domain: String,
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

/// A resource discovered via Azure Resource Graph, spanning all visible
/// subscriptions. Unlike [`Resource`], it carries its own `subscription_id`
/// because global search crosses subscriptions. The subscription *display name*
/// is resolved at render time, not stored here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalResource {
    pub id: String,              // full ARM resource ID
    pub name: String,
    pub resource_type: String,   // ARG returns this lowercased, e.g. "microsoft.compute/virtualmachines"
    pub resource_group: String,
    pub subscription_id: String,
    pub location: String,
}

/* ============================================================================================== */

/// Result of an `az vm run-command invoke` call. Run-command returns stdout and
/// stderr as separate status entries; the script's exit code is not reliably
/// surfaced, so `succeeded` is derived from the provisioning `display_status`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunCommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub display_status: String,
    pub succeeded: bool,
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
        format!("{} / {}", self.tenant.tenant_display_name, self.subscription.name)
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

impl CostPeriod {
    /// Returns a period for the current month (1st to today).
    pub fn current_month() -> Self {
        let today = chrono::Local::now().date_naive();
        let first = today.with_day(1).unwrap_or(today);
        Self {
            from: first.format("%Y-%m-%d").to_string(),
            to: today.format("%Y-%m-%d").to_string(),
        }
    }

    /* ========================================================================================== */
    /// Returns the previous month's period relative to this one.
    pub fn previous_month(&self) -> Self {
        if let Ok(from_date) = chrono::NaiveDate::parse_from_str(&self.from, "%Y-%m-%d") {
            let prev = if from_date.month() == 1 {
                chrono::NaiveDate::from_ymd_opt(from_date.year() - 1, 12, 1)
            } else {
                chrono::NaiveDate::from_ymd_opt(from_date.year(), from_date.month() - 1, 1)
            };
            if let Some(prev_first) = prev {
                let prev_last = if prev_first.month() == 12 {
                    chrono::NaiveDate::from_ymd_opt(prev_first.year() + 1, 1, 1)
                } else {
                    chrono::NaiveDate::from_ymd_opt(prev_first.year(), prev_first.month() + 1, 1)
                }
                .map(|d| d.pred_opt().unwrap_or(d))
                .unwrap_or(prev_first);

                return Self {
                    from: prev_first.format("%Y-%m-%d").to_string(),
                    to: prev_last.format("%Y-%m-%d").to_string(),
                };
            }
        }
        self.clone()
    }

    /* ========================================================================================== */
    /// Returns the next month's period, or `None` if already at the current month.
    pub fn next_month(&self) -> Option<Self> {
        let today = chrono::Local::now().date_naive();
        let current_month_first = today.with_day(1).unwrap_or(today);

        if let Ok(from_date) = chrono::NaiveDate::parse_from_str(&self.from, "%Y-%m-%d") {
            if from_date >= current_month_first {
                return None; // Already at current month.
            }

            let next = if from_date.month() == 12 {
                chrono::NaiveDate::from_ymd_opt(from_date.year() + 1, 1, 1)
            } else {
                chrono::NaiveDate::from_ymd_opt(from_date.year(), from_date.month() + 1, 1)
            };

            if let Some(next_first) = next {
                // If next month is the current month, cap at today.
                if next_first.year() == today.year() && next_first.month() == today.month() {
                    return Some(Self {
                        from: next_first.format("%Y-%m-%d").to_string(),
                        to: today.format("%Y-%m-%d").to_string(),
                    });
                }

                let next_last = if next_first.month() == 12 {
                    chrono::NaiveDate::from_ymd_opt(next_first.year() + 1, 1, 1)
                } else {
                    chrono::NaiveDate::from_ymd_opt(next_first.year(), next_first.month() + 1, 1)
                }
                .map(|d| d.pred_opt().unwrap_or(d))
                .unwrap_or(next_first);

                return Some(Self {
                    from: next_first.format("%Y-%m-%d").to_string(),
                    to: next_last.format("%Y-%m-%d").to_string(),
                });
            }
        }
        None
    }

    /* ========================================================================================== */
    /// Returns a display label like "Mar 2026".
    pub fn label(&self) -> String {
        if let Ok(from_date) = chrono::NaiveDate::parse_from_str(&self.from, "%Y-%m-%d") {
            from_date.format("%b %Y").to_string()
        } else {
            format!("{} → {}", self.from, self.to)
        }
    }
}

#[derive(Debug, Clone)]
pub struct CostLineItem {
    pub label: String,
    pub amount: f64,
}

/* ============================================================================================== */

/// A single Azure Activity Log entry, normalised for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityLogEntry {
    pub timestamp: String,             // eventTimestamp (ISO 8601)
    pub operation: String,             // operationName.localizedValue (fallback: value)
    pub status: String,                // status.value: Succeeded | Failed | Started
    pub level: String,                 // Critical | Error | Warning | Informational
    pub caller: Option<String>,
    pub resource_id: String,
    pub resource_name: String,         // derived via resource_name_from_id
    pub resource_group: Option<String>,
    pub correlation_id: Option<String>,
    pub detail: Option<String>,        // statusMessage / description / subStatus
}

impl ActivityLogEntry {
    /// True if this entry represents a failure (status Failed or error-level).
    pub fn is_failure(&self) -> bool {
        self.status.eq_ignore_ascii_case("Failed")
            || self.level.eq_ignore_ascii_case("Error")
            || self.level.eq_ignore_ascii_case("Critical")
    }
}


/* ============================================================================================== */
/*                                              Tests                                             */
/* ============================================================================================== */

#[cfg(test)]
mod global_resource_tests {
    use super::*;

    #[test]
    fn global_resource_constructs_with_all_fields() {
        let r = GlobalResource {
            id: "/subscriptions/s/resourceGroups/rg/providers/x/web-01".into(),
            name: "web-01".into(),
            resource_type: "microsoft.compute/virtualmachines".into(),
            resource_group: "rg".into(),
            subscription_id: "s".into(),
            location: "westeurope".into(),
        };
        assert_eq!(r.name, "web-01");
        assert_eq!(r.subscription_id, "s");
    }
}