use std::collections::HashMap;

use serde::Deserialize;

use crate::domain::{CostLineItem, CostPeriod, CostScope, CostSummary, Resource, ResourceGroup};
use crate::domain::models::{AzureContext, Subscription, SubscriptionState, Tenant};
use crate::errors::AppError;

/* ============================================================================================== */
/*                                   Raw JSON shapes from az CLI                                  */
/* ============================================================================================== */

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAccount {
    id: String,
    name: String,
    tenant_id: String,
    home_tenant_id: Option<String>,
    state: String,
    #[serde(default)]
    tenant_display_name: String,
    #[serde(default)]
    tenant_default_domain: String,
}

/* ============================================================================================== */
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawResource {
    id: String,
    name: String,
    #[serde(rename = "type")]
    resource_type: String,
    resource_group: String,
    location: String,
    #[serde(default)]
    tags: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawResourceGroup {
    name: String,
    location: String,
    #[serde(default)]
    tags: Option<HashMap<String, String>>,
}

/* ============================================================================================== */
#[derive(Debug, Deserialize)]
struct RawCostQueryResponse {
    #[serde(default)]
    rows: Vec<Vec<serde_json::Value>>,
    #[serde(default)]
    columns: Vec<RawCostColumn>,
}

#[derive(Debug, Deserialize)]
struct RawCostColumn {
    name: String,
    #[serde(rename = "type")]
    column_type: String,
}

/* ============================================================================================== */
/*                                    Public parsing functions                                    */
/* ============================================================================================== */

/* ======================================= Auth & context ======================================= */
/// Parses the output of `az account list --all` into a tenant map and
/// grouped subscriptions.
///
/// Tenants are deduplicated by `tenant_id`. Display names are derived from
/// a supplemental tenant list if provided; otherwise the GUID is used.
///
/// # Errors
/// Returns [`AppError`] with [`ErrorKind::CliParseError`] on JSON failures.
pub fn parse_account_list(
    account_json: &str,
) -> Result<(Vec<Tenant>, HashMap<String, Vec<Subscription>>), AppError> {
    let raw_accounts: Vec<RawAccount> = serde_json::from_str(account_json)
        .map_err(|e| AppError::cli_parse_error(format!("account list: {}", e)))?;

    let mut tenants_map: HashMap<String, Tenant> = HashMap::new();
    let mut subscriptions_by_tenant: HashMap<String, Vec<Subscription>> = HashMap::new();

    for raw in raw_accounts {
        let tid = raw.tenant_id.clone();

        // Create or reuse tenant entry.
        if !tenants_map.contains_key(&tid) {
            let display_name = if raw.tenant_display_name.is_empty() {
                tid.clone()
            } else {
                raw.tenant_display_name.clone()
            };

            tenants_map.insert(
                tid.clone(),
                Tenant {
                    id: tid.clone(),
                    tenant_display_name: display_name,
                    tenant_default_domain: raw.tenant_default_domain.clone(),
                },
            );
        }

        let subscription = Subscription {
            id: raw.id,
            name: raw.name,
            tenant_id: tid.clone(),
            state: parse_subscription_state(&raw.state),
        };

        subscriptions_by_tenant
            .entry(tid)
            .or_default()
            .push(subscription);
    }

    // Sort tenants by display name for stable ordering.
    let mut tenants: Vec<Tenant> = tenants_map.into_values().collect();
    tenants.sort_by(|a, b| a.tenant_display_name.cmp(&b.tenant_display_name));

    // Sort subscriptions within each tenant alphabetically.
    for subs in subscriptions_by_tenant.values_mut() {
        subs.sort_by(|a, b| a.name.cmp(&b.name));
    }

    Ok((tenants, subscriptions_by_tenant))
}

/* ============================================================================================== */
/// Parses the output of `az account show` into an [`AzureContext`].
///
/// # Errors
/// Returns [`AppError`] with [`ErrorKind::CliParseError`] on JSON failures.
pub fn parse_account_show(json: &str) -> Result<AzureContext, AppError> {
    let raw: RawAccount = serde_json::from_str(json)
        .map_err(|e| AppError::cli_parse_error(format!("account show: {}", e)))?;

    let tenant = Tenant {
        id: raw.tenant_id.clone(),
        tenant_display_name: if raw.tenant_display_name.is_empty() {
            raw.tenant_id.clone()
        } else {
            raw.tenant_display_name
        },
        tenant_default_domain: raw.tenant_default_domain,
    };

    let subscription = Subscription {
        id: raw.id,
        name: raw.name,
        tenant_id: raw.tenant_id,
        state: parse_subscription_state(&raw.state),
    };

    Ok(AzureContext { tenant, subscription })
}

/* ========================================== Resources ========================================= */
/// Parses the output of `az group list --subscription <id>` into a list of
/// [`ResourceGroup`]s.
///
/// The `subscription_id` is injected by the caller since the CLI output does
/// not include it.
///
/// # Errors
/// Returns [`AppError`] with [`ErrorKind::CliParseError`] on JSON failures.
pub fn parse_resource_group_list(
    json: &str,
    subscription_id: &str
) -> Result<Vec<ResourceGroup>, AppError> {
    let raw_groups: Vec<RawResourceGroup> = serde_json::from_str(json)
        .map_err(|e| AppError::cli_parse_error(format!("resource group list: {}", e)))?;

    let mut groups: Vec<ResourceGroup> = raw_groups
        .into_iter()
        .map(|rg| ResourceGroup {
            name: rg.name,
            subscription_id: subscription_id.to_string(),
            location: rg.location,
            tags: rg.tags.unwrap_or_default(),
        })
        .collect();

    groups.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(groups)
}

/* ============================================================================================== */
/// Parses the output of `az resource list --resource-group <name>` into a list
/// of [`Resource`]s.
///
/// # Errors
/// Returns [`AppError`] with [`ErrorKind::CliParseError`] on JSON failures.
pub fn parse_resource_list(json: &str) -> Result<Vec<Resource>, AppError> {
    let raw_resources: Vec<RawResource> = serde_json::from_str(json)
        .map_err(|e| AppError::cli_parse_error(format!("resource list: {}", e)))?;

    let mut resources: Vec<Resource> = raw_resources
        .into_iter()
        .map(|r| Resource {
            id: r.id,
            name: r.name,
            resource_type: r.resource_type,
            resource_group: r.resource_group,
            location: r.location,
            tags: r.tags.unwrap_or_default(),
        })
        .collect();

    resources.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(resources)
}

/* ============================================ Cost ============================================ */
/// Parses the output of `az costmanagement query` into a [`CostSummary`].
///
/// The response contains `rows` with `[cost, service_name, currency]` tuples.
/// The `scope` and `period` are injected by the caller since they are not
/// present in the response body.
///
/// # Errors
/// Returns [`AppError`] with [`ErrorKind::CliParseError`] on JSON failures.
pub fn parse_cost_query(
    json: &str,
    scope: CostScope,
    period: CostPeriod,
) -> Result<CostSummary, AppError> {
    let raw: RawCostQueryResponse = serde_json::from_str(json)
        .map_err(|e| AppError::cli_parse_error(format!("cost query: {}", e)))?;

    let mut currency = String::from("USD");
    let mut breakdown: Vec<CostLineItem> = Vec::new();
    let mut total = 0.0;

    for row in &raw.rows {
        // Each row is an array: [cost, service_name, currency].
        if row.len() < 3 {
            continue;
        }

        let amount = match &row[0] {
            serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0),
            _ => 0.0,
        };

        let service_name = match &row[1] {
            serde_json::Value::String(s) => s.clone(),
            _ => "Unknown".to_string(),
        };

        if let serde_json::Value::String(c) = &row[2] {
            currency = c.clone();
        }

        total += amount;
        breakdown.push(CostLineItem {
            service_name,
            amount,
        });
    }

    // Sort by amount descending.
    breakdown.sort_by(|a, b| b.amount.partial_cmp(&a.amount).unwrap_or(std::cmp::Ordering::Equal));

    Ok(CostSummary {
        scope,
        currency,
        total,
        period,
        breakdown,
    })
}

/* ============================================================================================== */
/*                                         Private helpers                                        */
/* ============================================================================================== */

fn parse_subscription_state(s: &str) -> SubscriptionState {
    match s {
        "Enabled" => SubscriptionState::Enabled,
        "Disabled" => SubscriptionState::Disabled,
        "Warned" => SubscriptionState::Warned,
        "PastDue" => SubscriptionState::PastDue,
        other => SubscriptionState::Unknown(other.to_string()),
    }
}

/* ============================================================================================== */
/*                                              Tests                                             */
/* ============================================================================================== */

#[cfg(test)]
mod tests {
    use super::*;

    const ACCOUNT_LIST_JSON: &str = r#"[
        {
            "cloudName": "AzureCloud",
            "homeTenantId": "tenant-a-guid",
            "id": "sub-1-guid",
            "isDefault": true,
            "managedByTenants": [],
            "name": "contoso-prod",
            "state": "Enabled",
            "tenantId": "tenant-a-guid",
            "tenantDisplayName": "Contoso Ltd",
            "tenantDefaultDomain": "contoso.onmicrosoft.com"
        },
        {
            "cloudName": "AzureCloud",
            "homeTenantId": "tenant-a-guid",
            "id": "sub-2-guid",
            "isDefault": false,
            "managedByTenants": [],
            "name": "contoso-dev",
            "state": "Disabled",
            "tenantId": "tenant-a-guid",
            "tenantDisplayName": "Contoso Ltd",
            "tenantDefaultDomain": "contoso.onmicrosoft.com"
        },
        {
            "cloudName": "AzureCloud",
            "homeTenantId": "tenant-b-guid",
            "id": "sub-3-guid",
            "isDefault": false,
            "managedByTenants": [],
            "name": "fabrikam-prod",
            "state": "Enabled",
            "tenantId": "tenant-b-guid",
            "tenantDisplayName": "Fabrikam Inc",
            "tenantDefaultDomain": "fabrikam.onmicrosoft.com"
        }
    ]"#;

    const ACCOUNT_LIST_NO_TENANT_NAMES_JSON: &str = r#"[
        {
            "cloudName": "AzureCloud",
            "homeTenantId": "tenant-a-guid",
            "id": "sub-1-guid",
            "isDefault": true,
            "managedByTenants": [],
            "name": "contoso-prod",
            "state": "Enabled",
            "tenantId": "tenant-a-guid"
        }
    ]"#;

    const RESOURCE_GROUP_LIST_JSON: &str = r#"[
        {
            "id": "/subscriptions/sub-1-guid/resourceGroups/rg-web",
            "location": "westeurope",
            "managedBy": null,
            "name": "rg-web",
            "properties": { "provisioningState": "Succeeded" },
            "tags": { "env": "prod", "team": "platform" },
            "type": "Microsoft.Resources/resourceGroups"
        },
        {
            "id": "/subscriptions/sub-1-guid/resourceGroups/rg-data",
            "location": "northeurope",
            "managedBy": null,
            "name": "rg-data",
            "properties": { "provisioningState": "Succeeded" },
            "tags": {},
            "type": "Microsoft.Resources/resourceGroups"
        }
    ]"#;

    const RESOURCE_LIST_JSON: &str = r#"[
        {
            "id": "/subscriptions/sub-1/resourceGroups/rg-web/providers/Microsoft.Compute/virtualMachines/vm-1",
            "name": "vm-1",
            "type": "Microsoft.Compute/virtualMachines",
            "resourceGroup": "rg-web",
            "location": "westeurope",
            "tags": { "env": "prod" }
        },
        {
            "id": "/subscriptions/sub-1/resourceGroups/rg-web/providers/Microsoft.Storage/storageAccounts/store1",
            "name": "store1",
            "type": "Microsoft.Storage/storageAccounts",
            "resourceGroup": "rg-web",
            "location": "westeurope",
            "tags": null
        }
    ]"#;

    const COST_QUERY_JSON: &str = r#"{
        "columns": [
            {"name": "Cost", "type": "Number"},
            {"name": "ServiceName", "type": "String"},
            {"name": "Currency", "type": "String"}
        ],
        "rows": [
            [612.40, "Virtual Machines", "EUR"],
            [284.15, "Azure SQL Database", "EUR"],
            [156.22, "Storage Accounts", "EUR"],
            [98.50, "Azure Kubernetes Service", "EUR"],
            [42.30, "Key Vault", "EUR"]
        ]
    }"#;

    #[test]
    fn parse_account_list_groups_by_tenant() {
        let (tenants, by_tenant) =
            parse_account_list(ACCOUNT_LIST_JSON).unwrap();

        assert_eq!(tenants.len(), 2);
        assert_eq!(by_tenant["tenant-a-guid"].len(), 2);
        assert_eq!(by_tenant["tenant-b-guid"].len(), 1);
    }

    #[test]
    fn parse_account_list_resolves_tenant_names() {
        let (tenants, _) =
            parse_account_list(ACCOUNT_LIST_JSON).unwrap();

        let contoso = tenants.iter().find(|t| t.id == "tenant-a-guid").unwrap();
        assert_eq!(contoso.tenant_display_name, "Contoso Ltd");
        assert_eq!(contoso.tenant_default_domain, "contoso.onmicrosoft.com");
    }

    #[test]
    fn parse_account_list_falls_back_to_guid_without_display_name() {
        let (tenants, _) = parse_account_list(ACCOUNT_LIST_NO_TENANT_NAMES_JSON).unwrap();
        let t = tenants.iter().find(|t| t.id == "tenant-a-guid").unwrap();
        assert_eq!(t.tenant_display_name, "tenant-a-guid");
    }

    #[test]
    fn parse_subscription_state_variants() {
        assert_eq!(parse_subscription_state("Enabled"), SubscriptionState::Enabled);
        assert_eq!(parse_subscription_state("Disabled"), SubscriptionState::Disabled);
        assert!(matches!(parse_subscription_state("Expired"), SubscriptionState::Unknown(_)));
    }

    #[test]
    fn parse_resource_group_list_basic() {
        let groups = parse_resource_group_list(RESOURCE_GROUP_LIST_JSON, "sub-1-guid").unwrap();
        assert_eq!(groups.len(), 2);
        // Sorted alphabetically by name.
        assert_eq!(groups[0].name, "rg-data");
        assert_eq!(groups[1].name, "rg-web");
        assert_eq!(groups[1].subscription_id, "sub-1-guid");
        assert_eq!(groups[1].tags.get("env"), Some(&"prod".to_string()));
    }

    #[test]
    fn parse_resource_group_list_empty() {
        let groups = parse_resource_group_list("[]", "sub-1").unwrap();
        assert!(groups.is_empty());
    }

    #[test]
    fn parse_resource_list_basic() {
        let resources = parse_resource_list(RESOURCE_LIST_JSON).unwrap();
        assert_eq!(resources.len(), 2);
        // Sorted alphabetically by name.
        assert_eq!(resources[0].name, "store1");
        assert_eq!(resources[1].name, "vm-1");
        assert_eq!(resources[1].resource_type, "Microsoft.Compute/virtualMachines");
        assert!(resources[0].tags.is_empty()); // null tags → empty map
    }

    #[test]
    fn parse_cost_query_basic() {
        let period = CostPeriod {
            from: "2026-03-01".to_string(),
            to: "2026-03-31".to_string(),
        };
        let summary = parse_cost_query(
            COST_QUERY_JSON,
            CostScope::Subscription("sub-1".to_string()),
            period,
        )
        .unwrap();

        assert_eq!(summary.breakdown.len(), 5);
        assert_eq!(summary.currency, "EUR");
        // Total should be sum of all rows.
        assert!((summary.total - 1193.57).abs() < 0.01);
        // Sorted by amount descending.
        assert_eq!(summary.breakdown[0].service_name, "Virtual Machines");
        assert_eq!(summary.breakdown[4].service_name, "Key Vault");
    }

    #[test]
    fn parse_cost_query_empty() {
        let json = r#"{"columns": [], "rows": []}"#;
        let period = CostPeriod {
            from: "2026-03-01".to_string(),
            to: "2026-03-31".to_string(),
        };
        let summary = parse_cost_query(
            json,
            CostScope::Subscription("sub-1".to_string()),
            period,
        )
        .unwrap();

        assert_eq!(summary.total, 0.0);
        assert!(summary.breakdown.is_empty());
    }
}