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

/* ============================================================================================== */
/*                                              Cost                                              */
/* ============================================================================================== */

/// Returns args for `az rest` calling the Cost Management Query API,
/// aggregated by service name for a subscription.
pub fn cost_query_by_service(subscription_id: &str, from: &str, to: &str) -> Vec<String> {
    let uri = format!(
        "https://management.azure.com/subscriptions/{}/providers/Microsoft.CostManagement/query?api-version=2023-11-01",
        subscription_id
    );
    let body = format!(
        r#"{{"type":"Usage","timeframe":"Custom","timePeriod":{{"from":"{}T00:00:00Z","to":"{}T23:59:59Z"}},"dataset":{{"granularity":"None","aggregation":{{"totalCost":{{"name":"PreTaxCost","function":"Sum"}}}},"grouping":[{{"type":"Dimension","name":"ServiceName"}}]}}}}"#,
        from, to
    );
    vec![
        "rest".to_string(),
        "--method".to_string(),
        "POST".to_string(),
        "--uri".to_string(),
        uri,
        "--body".to_string(),
        body,
        "--output".to_string(),
        "json".to_string(),
    ]
}

/* ============================================================================================== */
/// Returns args for `az rest` calling the Cost Management Query API,
/// scoped to a resource group.
pub fn cost_query_by_resource_group(
    subscription_id: &str,
    resource_group: &str,
    from: &str,
    to: &str,
) -> Vec<String> {
    let uri = format!(
        "https://management.azure.com/subscriptions/{}/resourceGroups/{}/providers/Microsoft.CostManagement/query?api-version=2023-11-01",
        subscription_id, resource_group
    );
    let body = format!(
        r#"{{"type":"Usage","timeframe":"Custom","timePeriod":{{"from":"{}T00:00:00Z","to":"{}T23:59:59Z"}},"dataset":{{"granularity":"None","aggregation":{{"totalCost":{{"name":"PreTaxCost","function":"Sum"}}}},"grouping":[{{"type":"Dimension","name":"ServiceName"}}]}}}}"#,
        from, to
    );
    vec![
        "rest".to_string(),
        "--method".to_string(),
        "POST".to_string(),
        "--uri".to_string(),
        uri,
        "--body".to_string(),
        body,
        "--output".to_string(),
        "json".to_string(),
    ]
}

/* ============================================================================================== */
/// Returns args for `az rest` calling the Cost Management Query API for a
/// subscription, aggregated by resource group rather than by service.
pub fn cost_query_grouped_by_resource_group(
    subscription_id: &str,
    from: &str,
    to: &str,
) -> Vec<String> {
    let uri = format!(
        "https://management.azure.com/subscriptions/{}/providers/Microsoft.CostManagement/query?api-version=2023-11-01",
        subscription_id
    );
    let body = format!(
        r#"{{"type":"Usage","timeframe":"Custom","timePeriod":{{"from":"{}T00:00:00Z","to":"{}T23:59:59Z"}},"dataset":{{"granularity":"None","aggregation":{{"totalCost":{{"name":"PreTaxCost","function":"Sum"}}}},"grouping":[{{"type":"Dimension","name":"ResourceGroupName"}}]}}}}"#,
        from, to
    );
    vec![
        "rest".to_string(),
        "--method".to_string(),
        "POST".to_string(),
        "--uri".to_string(),
        uri,
        "--body".to_string(),
        body,
        "--output".to_string(),
        "json".to_string(),
    ]
}

/* ============================================================================================== */
/*                                       VM run-command                                           */
/* ============================================================================================== */

/// Returns args for
/// `az vm run-command invoke --subscription <sub> --resource-group <rg>
///  --name <vm> --command-id RunPowerShellScript --scripts <script> --output json`.
///
/// `--subscription` is passed explicitly so the call cannot target the wrong
/// active context.
pub fn vm_run_command_powershell(
    subscription_id: &str,
    resource_group: &str,
    vm_name: &str,
    script: &str,
) -> Vec<String> {
    vec![
        "vm".to_string(),
        "run-command".to_string(),
        "invoke".to_string(),
        "--subscription".to_string(),
        subscription_id.to_string(),
        "--resource-group".to_string(),
        resource_group.to_string(),
        "--name".to_string(),
        vm_name.to_string(),
        "--command-id".to_string(),
        "RunPowerShellScript".to_string(),
        "--scripts".to_string(),
        script.to_string(),
        "--output".to_string(),
        "json".to_string(),
    ]
}

/* ============================================================================================== */
/*                                          Activity log                                          */
/* ============================================================================================== */

use crate::domain::activity::{ActivityScope, ActivityWindow};

/// Returns args for `az monitor activity-log list` for the given scope and
/// window. The subscription is always passed explicitly; resource and
/// resource-group scopes add the matching narrowing flag.
pub fn activity_log_list(scope: &ActivityScope, window: ActivityWindow) -> Vec<String> {
    let mut args = vec![
        "monitor".to_string(),
        "activity-log".to_string(),
        "list".to_string(),
        "--subscription".to_string(),
        scope.subscription_id().to_string(),
        "--offset".to_string(),
        window.offset().to_string(),
        "--max-events".to_string(),
        "200".to_string(),
    ];

    match scope {
        ActivityScope::Subscription { .. } => {}
        ActivityScope::ResourceGroup { resource_group, .. } => {
            args.push("--resource-group".to_string());
            args.push(resource_group.clone());
        }
        ActivityScope::Resource { resource_id, .. } => {
            args.push("--resource-id".to_string());
            args.push(resource_id.clone());
        }
    }

    args.push("--output".to_string());
    args.push("json".to_string());
    args
}

/* ============================================================================================== */
/*                                       Resource Graph                                           */
/* ============================================================================================== */

/// Returns args for `az graph query -q "<kql>" --first <n> [--skip-token <tok>]
/// --output json`. Requires the `resource-graph` CLI extension; a missing
/// extension surfaces as [`crate::errors::ErrorKind::CliExtensionMissing`] from
/// the executor.
pub fn graph_query(kql: &str, first: u32, skip_token: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "graph".to_string(),
        "query".to_string(),
        "-q".to_string(),
        kql.to_string(),
        "--first".to_string(),
        first.to_string(),
    ];
    if let Some(tok) = skip_token {
        args.push("--skip-token".to_string());
        args.push(tok.to_string());
    }
    args.push("--output".to_string());
    args.push("json".to_string());
    args
}

/* ============================================================================================== */
/// Returns args for `az extension add --name resource-graph --output json`.
pub fn extension_add(name: &str) -> Vec<String> {
    vec![
        "extension".to_string(),
        "add".to_string(),
        "--name".to_string(),
        name.to_string(),
        "--output".to_string(),
        "json".to_string(),
    ]
}

/* ============================================================================================== */
/*                                              Tests                                             */
/* ============================================================================================== */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vm_run_command_builds_expected_args() {
        let args = vm_run_command_powershell("sub-1", "rg-web", "web-01", "Get-Date");
        assert_eq!(
            args,
            vec![
                "vm", "run-command", "invoke",
                "--subscription", "sub-1",
                "--resource-group", "rg-web",
                "--name", "web-01",
                "--command-id", "RunPowerShellScript",
                "--scripts", "Get-Date",
                "--output", "json",
            ]
        );
    }

    #[test]
    fn activity_log_subscription_scope() {
        use crate::domain::activity::{ActivityScope, ActivityWindow};
        let scope = ActivityScope::Subscription { subscription_id: "sub-1".into() };
        let args = activity_log_list(&scope, ActivityWindow::Day);
        assert_eq!(
            args,
            vec![
                "monitor", "activity-log", "list",
                "--subscription", "sub-1",
                "--offset", "24h",
                "--max-events", "200",
                "--output", "json",
            ]
        );
    }

    #[test]
    fn activity_log_resource_scope_adds_resource_id() {
        use crate::domain::activity::{ActivityScope, ActivityWindow};
        let scope = ActivityScope::Resource {
            subscription_id: "sub-1".into(),
            resource_group: "rg".into(),
            resource_id: "/subscriptions/sub-1/.../web-01".into(),
            resource_name: "web-01".into(),
        };
        let args = activity_log_list(&scope, ActivityWindow::Week);
        assert!(args.contains(&"--resource-id".to_string()));
        assert!(args.contains(&"/subscriptions/sub-1/.../web-01".to_string()));
        assert!(args.contains(&"7d".to_string()));
        assert!(!args.contains(&"--resource-group".to_string()));
    }

    #[test]
    fn activity_log_rg_scope_adds_resource_group() {
        use crate::domain::activity::{ActivityScope, ActivityWindow};
        let scope = ActivityScope::ResourceGroup {
            subscription_id: "sub-1".into(),
            resource_group: "rg-web".into(),
        };
        let args = activity_log_list(&scope, ActivityWindow::Day);
        assert!(args.contains(&"--resource-group".to_string()));
        assert!(args.contains(&"rg-web".to_string()));
        assert!(!args.contains(&"--resource-id".to_string()));
    }

    #[test]
    fn cost_query_grouped_by_rg_builds_expected_args() {
        let args = cost_query_grouped_by_resource_group("sub-1", "2026-03-01", "2026-03-31");
        assert_eq!(args[0], "rest");
        assert!(args.contains(&"POST".to_string()));
        // URI is the subscription-scoped Cost Management query endpoint.
        assert!(args.iter().any(|a| a.contains("/subscriptions/sub-1/providers/Microsoft.CostManagement/query")));
        // Body groups on the resource-group dimension.
        assert!(args.iter().any(|a| a.contains("ResourceGroupName")));
        assert!(args.iter().any(|a| a.contains("2026-03-01")));
    }

    #[test]
    fn graph_query_builds_args_without_skip_token() {
        let args = graph_query("Resources | project name", 1000, None);
        assert_eq!(
            args,
            vec![
                "graph", "query",
                "-q", "Resources | project name",
                "--first", "1000",
                "--output", "json",
            ]
        );
    }

    #[test]
    fn graph_query_includes_skip_token_when_present() {
        let args = graph_query("Resources", 500, Some("tok-abc"));
        assert!(args.contains(&"--skip-token".to_string()));
        assert!(args.contains(&"tok-abc".to_string()));
        assert!(args.contains(&"500".to_string()));
    }
}