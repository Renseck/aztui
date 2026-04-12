/* ============================================================================================== */
/*                                         Auth & context                                         */
/* ============================================================================================== */

/// Returns args for `az account list --all --output json`.
pub fn account_list_all() -> Vec<&'static str> {
    vec!["account", "list", "--all", "--output", "json"]
}

/* ============================================================================================== */
/// Returns args for `az account show --output json`.
pub fn account_show() -> Vec<&'static str> {
    vec!["account", "show", "--output", "json"]
}

/* ============================================================================================== */
/// Returns args for `az account tenant list --output json`.
pub fn account_tenant_list() -> Vec<&'static str> {
    vec!["account", "tenant", "list", "--output", "json"]
}

/* ============================================================================================== */
/// Returns args for `az login --output json`.
pub fn login() -> Vec<&'static str> {
    vec!["login", "--output", "json"]
}

/* ============================================================================================== */
/// Returns args for `az login --tenant <tenant_id> --output json`.
pub fn login_tenant(tenant_id: &str) -> Vec<String> {
    vec![
        "login".to_string(),
        "--tenant".to_string(),
        tenant_id.to_string(),
        "--output".to_string(),
        "json".to_string(),
    ]
}
/* ============================================================================================== */
/// Returns args for `az account set --subscription <id> --output json`.
pub fn account_set(subscription_id: &str) -> Vec<String> {
    vec![
        "account".to_string(),
        "set".to_string(),
        "--subscription".to_string(),
        subscription_id.to_string(),
    ]
}


/* ============================================================================================== */
/*                                            Resources                                           */
/* ============================================================================================== */

/// Returns args for `az group list --subscription <id> --output json`.
pub fn resource_group_list(subscription_id: &str) -> Vec<String> {
    vec![
        "group".to_string(),
        "list".to_string(),
        "--subscription".to_string(),
        subscription_id.to_string(),
        "--output".to_string(),
        "json".to_string(),
    ]
}

/* ============================================================================================== */
/// Returns args for `az resource list --subscription <id> --resource-group <name> --output json`.
pub fn resource_list(subscription_id: &str, resource_group: &str) -> Vec<String> {
    vec![
        "resource".to_string(),
        "list".to_string(),
        "--subscription".to_string(),
        subscription_id.to_string(),
        "--resource-group".to_string(),
        resource_group.to_string(),
        "--output".to_string(),
        "json".to_string(),
    ]
}