use std::collections::HashMap;

use serde::Deserialize;

use crate::domain::{ActivityLogEntry, CostLineItem, CostPeriod, CostScope, CostSummary, Resource, ResourceGroup, RunCommandOutput};
use crate::domain::models::{AzureContext, GlobalResource, Subscription, SubscriptionState, Tenant};
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
    #[allow(dead_code)]
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
struct RawRunCommandResponse {
    #[serde(default)]
    value: Vec<RawRunCommandStatus>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRunCommandStatus {
    #[serde(default)]
    code: String,
    #[serde(default)]
    display_status: String,
    #[serde(default)]
    message: String,
}

/* ============================================================================================== */
#[derive(Debug, Deserialize)]
struct RawCostQueryResponse {
    #[serde(default)]
    properties: Option<RawCostQueryProperties>,
    // Fallback: rows/columns at top level (for flexibility).
    #[serde(default)]
    rows: Vec<Vec<serde_json::Value>>,
    #[serde(default)]
    #[allow(dead_code)]
    columns: Vec<RawCostColumn>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawCostQueryProperties {
    #[serde(default)]
    rows: Vec<Vec<serde_json::Value>>,
    #[serde(default)]
    #[allow(dead_code)]
    columns: Vec<RawCostColumn>,
    #[allow(dead_code)]
    next_link: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawCostColumn {
    #[allow(dead_code)]
    name: String,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    column_type: String,
}

/* ============================================================================================== */
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RawLocalized {
    #[serde(default)]
    value: String,
    #[serde(default)]
    localized_value: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawActivityEntry {
    #[serde(default)]
    event_timestamp: String,
    #[serde(default)]
    operation_name: RawLocalized,
    #[serde(default)]
    status: RawLocalized,
    #[serde(default)]
    sub_status: RawLocalized,
    #[serde(default)]
    level: String,
    #[serde(default)]
    caller: Option<String>,
    #[serde(default)]
    resource_id: String,
    #[serde(default)]
    resource_group_name: Option<String>,
    #[serde(default)]
    correlation_id: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    properties: Option<std::collections::HashMap<String, serde_json::Value>>,
}

/* ============================================================================================== */
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawGraphResource {
    id: String,
    name: String,
    #[serde(rename = "type")]
    resource_type: String,
    #[serde(default)]
    resource_group: String,
    subscription_id: String,
    #[serde(default)]
    location: String,
}

#[derive(Debug, Deserialize)]
struct RawGraphResponse {
    #[serde(default)]
    data: Vec<RawGraphResource>,
    #[serde(rename = "skipToken", default)]
    skip_token: Option<String>,
    #[serde(rename = "$skipToken", default)]
    skip_token_alt: Option<String>,
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
/// Parses one page of `az graph query` output into [`GlobalResource`] rows plus
/// the next skip token (or `None` when the result set is exhausted). Azure
/// Resource Graph spells the continuation token either `skipToken` or
/// `$skipToken` depending on `az` version; both are accepted.
///
/// # Errors
/// Returns [`AppError`] with [`ErrorKind::CliParseError`] if the JSON does not
/// match the expected Resource Graph shape.
pub fn parse_graph_rows(json: &str) -> Result<(Vec<GlobalResource>, Option<String>), AppError> {
    let resp: RawGraphResponse = serde_json::from_str(json)
        .map_err(|e| AppError::cli_parse_error(format!("graph query: {}", e)))?;

    let rows = resp
        .data
        .into_iter()
        .map(|r| GlobalResource {
            id: r.id,
            name: r.name,
            resource_type: r.resource_type,
            resource_group: r.resource_group,
            subscription_id: r.subscription_id,
            location: r.location,
        })
        .collect();

    let token = resp
        .skip_token
        .or(resp.skip_token_alt)
        .filter(|t| !t.is_empty());

    Ok((rows, token))
}

/* ========================================== Activity ========================================== */
/// Returns the last `/`-segment of an ARM resource ID (its short name), or an
/// empty string if the ID is empty.
pub fn resource_name_from_id(resource_id: &str) -> String {
    resource_id.rsplit('/').next().unwrap_or("").to_string()
}

/// Parses the output of `az monitor activity-log list` into normalised entries.
///
/// `detail` prefers `properties.statusMessage`, then `description`, then the
/// localized `subStatus` — whichever first yields non-empty text — so failures
/// surface their error message.
///
/// # Errors
/// Returns [`AppError`] with [`ErrorKind::CliParseError`] on JSON failures.
pub fn parse_activity_log(json: &str) -> Result<Vec<ActivityLogEntry>, AppError> {
    let raw: Vec<RawActivityEntry> = serde_json::from_str(json)
        .map_err(|e| AppError::cli_parse_error(format!("activity log: {}", e)))?;

    let entries = raw
        .into_iter()
        .map(|r| {
            let operation = first_non_empty(&[
                &r.operation_name.localized_value,
                &r.operation_name.value,
            ]);
            let status = first_non_empty(&[&r.status.value, &r.status.localized_value]);

            let status_message = r
                .properties
                .as_ref()
                .and_then(|p| p.get("statusMessage"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let detail = status_message
                .filter(|s| !s.is_empty())
                .or_else(|| r.description.clone().filter(|s| !s.is_empty()))
                .or_else(|| {
                    let ss = &r.sub_status.localized_value;
                    if ss.is_empty() { None } else { Some(ss.clone()) }
                });

            ActivityLogEntry {
                resource_name: resource_name_from_id(&r.resource_id),
                timestamp: r.event_timestamp,
                operation,
                status,
                level: r.level,
                caller: r.caller,
                resource_id: r.resource_id,
                resource_group: r.resource_group_name,
                correlation_id: r.correlation_id,
                detail,
            }
        })
        .collect();

    Ok(entries)
}

/// Returns the first non-empty string from `candidates`, or an empty string.
fn first_non_empty(candidates: &[&str]) -> String {
    candidates
        .iter()
        .find(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_default()
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

/* ========================================= Run command ======================================== */
/// Parses the output of `az vm run-command invoke` into a [`RunCommandOutput`].
///
/// The response has a `value` array whose entries carry `code`
/// (`ComponentStatus/StdOut/...` or `.../StdErr/...`), a `displayStatus`, and a
/// `message`. Success is derived from `displayStatus` containing "succeeded".
///
/// # Errors
/// Returns [`AppError`] with [`ErrorKind::CliParseError`] on JSON failures.
pub fn parse_run_command_output(json: &str) -> Result<RunCommandOutput, AppError> {
    let raw: RawRunCommandResponse = serde_json::from_str(json)
        .map_err(|e| AppError::cli_parse_error(format!("run-command: {}", e)))?;

    let mut out = RunCommandOutput::default();
    for status in raw.value {
        if status.code.contains("StdOut") {
            out.stdout = status.message;
        } else if status.code.contains("StdErr") {
            out.stderr = status.message;
        }
        if !status.display_status.is_empty() {
            out.display_status = status.display_status;
        }
    }
    out.succeeded = out.display_status.to_lowercase().contains("succeeded");
    Ok(out)
}

/* ============================================ Cost ============================================ */
/// Parses the output of the Cost Management Query REST API into a [`CostSummary`].
///
/// The response wraps `columns` and `rows` inside a `properties` object.
/// Each row is `[cost, service_name, currency]`. The `scope` and `period`
/// are injected by the caller since they are not in the response body.
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

    // Prefer properties.rows; fall back to top-level rows.
    let rows = if let Some(ref props) = raw.properties {
        &props.rows
    } else {
        &raw.rows
    };

    let mut currency = String::from("USD");
    let mut breakdown: Vec<CostLineItem> = Vec::new();
    let mut total = 0.0;

    for row in rows {
        // Each row: [cost, service_name, currency].
        if row.len() < 3 {
            continue;
        }

        let amount = match &row[0] {
            serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0),
            _ => 0.0,
        };

        let label = match &row[1] {
            serde_json::Value::String(s) => s.clone(),
            _ => "Unknown".to_string(),
        };

        if let serde_json::Value::String(c) = &row[2] {
            currency = c.clone();
        }

        total += amount;
        breakdown.push(CostLineItem {
            label,
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
        "properties": {
            "columns": [
                {"name": "PreTaxCost", "type": "Number"},
                {"name": "ServiceName", "type": "String"},
                {"name": "Currency", "type": "String"}
            ],
            "rows": [
                [612.40, "Virtual Machines", "EUR"],
                [284.15, "Azure SQL Database", "EUR"],
                [156.22, "Storage Accounts", "EUR"],
                [98.50, "Azure Kubernetes Service", "EUR"],
                [42.30, "Key Vault", "EUR"]
            ],
            "nextLink": null
        }
    }"#;

    const RUN_COMMAND_JSON: &str = r#"{
        "value": [
            {
                "code": "ComponentStatus/StdOut/succeeded",
                "displayStatus": "Provisioning succeeded",
                "level": "Info",
                "message": "Tuesday, 16 June 2026 10:00:00",
                "time": null
            },
            {
                "code": "ComponentStatus/StdErr/succeeded",
                "displayStatus": "Provisioning succeeded",
                "level": "Info",
                "message": "",
                "time": null
            }
        ]
    }"#;

    const ACTIVITY_LOG_JSON: &str = r#"[
        {
            "eventTimestamp": "2026-06-17T10:42:00Z",
            "operationName": { "value": "Microsoft.Compute/virtualMachines/restart/action", "localizedValue": "Restart Virtual Machine" },
            "status": { "value": "Succeeded", "localizedValue": "Succeeded" },
            "subStatus": { "value": "OK", "localizedValue": "OK" },
            "level": "Informational",
            "caller": "ops@contoso.com",
            "resourceId": "/subscriptions/s/resourceGroups/rg-web/providers/Microsoft.Compute/virtualMachines/web-01",
            "resourceGroupName": "rg-web",
            "correlationId": "corr-1",
            "description": ""
        },
        {
            "eventTimestamp": "2026-06-17T09:58:00Z",
            "operationName": { "value": "Microsoft.Resources/deployments/write", "localizedValue": "Create Deployment" },
            "status": { "value": "Failed", "localizedValue": "Failed" },
            "subStatus": { "value": "Conflict", "localizedValue": "Conflict" },
            "level": "Error",
            "caller": "ci@contoso.com",
            "resourceId": "/subscriptions/s/resourceGroups/rg-web/providers/Microsoft.Resources/deployments/main",
            "resourceGroupName": "rg-web",
            "correlationId": "corr-2",
            "properties": { "statusMessage": "Deployment failed: resource already exists" }
        }
    ]"#;

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
        assert_eq!(summary.breakdown[0].label, "Virtual Machines");
        assert_eq!(summary.breakdown[4].label, "Key Vault");
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

    #[test]
    fn parse_run_command_splits_stdout_stderr() {
        let out = parse_run_command_output(RUN_COMMAND_JSON).unwrap();
        assert_eq!(out.stdout, "Tuesday, 16 June 2026 10:00:00");
        assert_eq!(out.stderr, "");
        assert!(out.succeeded);
    }

    #[test]
    fn parse_run_command_marks_failure_on_stderr() {
        let json = r#"{"value":[
            {"code":"ComponentStatus/StdOut/succeeded","displayStatus":"Provisioning failed","message":""},
            {"code":"ComponentStatus/StdErr/succeeded","displayStatus":"Provisioning failed","message":"boom"}
        ]}"#;
        let out = parse_run_command_output(json).unwrap();
        assert_eq!(out.stderr, "boom");
        assert!(!out.succeeded);
    }

    #[test]
    fn resource_name_from_id_takes_last_segment() {
        let id = "/subscriptions/s/resourceGroups/rg/providers/Microsoft.Compute/virtualMachines/web-01";
        assert_eq!(resource_name_from_id(id), "web-01");
    }

    #[test]
    fn resource_name_from_id_handles_empty() {
        assert_eq!(resource_name_from_id(""), "");
    }

    #[test]
    fn parse_activity_log_basic() {
        let entries = parse_activity_log(ACTIVITY_LOG_JSON).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].operation, "Restart Virtual Machine");
        assert_eq!(entries[0].status, "Succeeded");
        assert_eq!(entries[0].resource_name, "web-01");
        assert_eq!(entries[0].caller.as_deref(), Some("ops@contoso.com"));
    }

    #[test]
    fn parse_activity_log_failure_carries_detail() {
        let entries = parse_activity_log(ACTIVITY_LOG_JSON).unwrap();
        let failed = &entries[1];
        assert!(failed.is_failure());
        assert_eq!(
            failed.detail.as_deref(),
            Some("Deployment failed: resource already exists")
        );
    }

    #[test]
    fn parse_cost_query_handles_rg_grouping() {
        let json = r#"{
            "properties": {
                "columns": [
                    {"name": "PreTaxCost", "type": "Number"},
                    {"name": "ResourceGroupName", "type": "String"},
                    {"name": "Currency", "type": "String"}
                ],
                "rows": [
                    [820.00, "rg-prod-web", "EUR"],
                    [410.55, "rg-prod-data", "EUR"]
                ]
            }
        }"#;
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

        assert_eq!(summary.breakdown.len(), 2);
        // Sorted by amount descending; the RG name lands in `label`.
        assert_eq!(summary.breakdown[0].label, "rg-prod-web");
        assert!((summary.total - 1230.55).abs() < 0.01);
    }

    #[test]
    fn parses_rows_and_skip_token() {
        let json = r#"{
            "data": [
                {"id":"/subscriptions/s/resourceGroups/rg/providers/Microsoft.Compute/virtualMachines/web-01",
                 "name":"web-01","type":"microsoft.compute/virtualmachines",
                 "resourceGroup":"rg","subscriptionId":"s","location":"westeurope"}
            ],
            "skipToken": "next-page-token",
            "count": 1
        }"#;
        let (rows, token) = parse_graph_rows(json).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "web-01");
        assert_eq!(rows[0].subscription_id, "s");
        assert_eq!(token.as_deref(), Some("next-page-token"));
    }

    #[test]
    fn final_page_has_no_skip_token() {
        let json = r#"{"data":[],"count":0}"#;
        let (rows, token) = parse_graph_rows(json).unwrap();
        assert!(rows.is_empty());
        assert!(token.is_none());
    }

    #[test]
    fn accepts_dollar_skip_token_spelling() {
        let json = r#"{"data":[],"$skipToken":"abc"}"#;
        let (_rows, token) = parse_graph_rows(json).unwrap();
        assert_eq!(token.as_deref(), Some("abc"));
    }

    #[test]
    fn null_skip_token_is_none() {
        let json = r#"{"data":[],"skipToken":null}"#;
        let (_rows, token) = parse_graph_rows(json).unwrap();
        assert!(token.is_none());
    }
}