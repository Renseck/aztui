use std::collections::HashMap;

use serde::Deserialize;

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
    home_tenant_id: Option<String>,
    state: String,
    #[serde(flatten)]
    tenant: RawTenant,
}

/* ============================================================================================== */
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTenant {
    tenant_id: String,
    #[serde(default)]
    tenant_display_name: String,
    #[serde(default)]
    tenant_default_domain: String,
}

/* ============================================================================================== */
/*                                    Public parsing functions                                    */
/* ============================================================================================== */

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
}