use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::{AppState, Pane};
use crate::domain::models::{Resource, ResourceGroup};
use crate::ui::theme::Theme;
use crate::ui::widgets::SPINNER_CHARS;

/* ============================================================================================== */
/*                                        VM identification                                       */
/* ============================================================================================== */

/// ARM resource type for a virtual machine.
pub const VM_RESOURCE_TYPE: &str = "Microsoft.Compute/virtualMachines";

/// Returns true if the given ARM resource type is a virtual machine.
pub fn is_vm(resource_type: &str) -> bool {
    resource_type == VM_RESOURCE_TYPE
}

/* ============================================================================================== */
/*                                   Resource type abbreviations                                  */
/* ============================================================================================== */

/// Returns a short display name for a full ARM resource type string.
pub fn abbreviate_resource_type(full_type: &str) -> &str {
    match full_type {
        "Microsoft.Compute/virtualMachines" => "VM",
        "Microsoft.Compute/disks" => "Disk",
        "Microsoft.Compute/availabilitySets" => "Avail Set",
        "Microsoft.Storage/storageAccounts" => "Storage",
        "Microsoft.KeyVault/vaults" => "KeyVault",
        "Microsoft.Sql/servers" => "SQL Server",
        "Microsoft.Sql/servers/databases" => "SQL DB",
        "Microsoft.Network/virtualNetworks" => "VNet",
        "Microsoft.Network/networkInterfaces" => "NIC",
        "Microsoft.Network/networkSecurityGroups" => "NSG",
        "Microsoft.Network/publicIPAddresses" => "Public IP",
        "Microsoft.Network/loadBalancers" => "LB",
        "Microsoft.Network/applicationGateways" => "App GW",
        "Microsoft.Network/privateDnsZones" => "Private DNS",
        "Microsoft.ContainerService/managedClusters" => "AKS",
        "Microsoft.ContainerRegistry/registries" => "ACR",
        "Microsoft.Web/sites" => "App Service",
        "Microsoft.Web/serverFarms" => "App Plan",
        "Microsoft.Insights/components" => "App Insights",
        "Microsoft.OperationalInsights/workspaces" => "Log Analytics",
        "Microsoft.ManagedIdentity/userAssignedIdentities" => "Managed ID",
        "Microsoft.Authorization/roleAssignments" => "Role Assign",
        "Microsoft.Cache/Redis" => "Redis",
        "Microsoft.DocumentDB/databaseAccounts" => "Cosmos DB",
        "Microsoft.EventHub/namespaces" => "Event Hub",
        "Microsoft.ServiceBus/namespaces" => "Service Bus",
        _ => {
            // Fallback: last segment of the type string.
            full_type.rsplit('/').next().unwrap_or(full_type)
        }
    }
}

/* ============================================================================================== */
/// Abbreviates common Azure region names for compact display.
fn abbreviate_location(location: &str) -> &str {
    match location {
        "westeurope" => "WEU",
        "northeurope" => "NEU",
        "eastus" => "EUS",
        "eastus2" => "EUS2",
        "westus" => "WUS",
        "westus2" => "WUS2",
        "westus3" => "WUS3",
        "centralus" => "CUS",
        "southcentralus" => "SCUS",
        "northcentralus" => "NCUS",
        "canadacentral" => "CAC",
        "canadaeast" => "CAE",
        "uksouth" => "UKS",
        "ukwest" => "UKW",
        "australiaeast" => "AUE",
        "southeastasia" => "SEA",
        "japaneast" => "JPE",
        "koreacentral" => "KRC",
        "brazilsouth" => "BRS",
        "germanywestcentral" => "GWC",
        "francecentral" => "FRC",
        "switzerlandnorth" => "CHN",
        "norwayeast" => "NOE",
        "swedencentral" => "SEC",
        _ => location,
    }
}

/* ============================================================================================== */
/*                                         Public renderer                                        */
/* ============================================================================================== */

/// Renders the two-pane resource browser view.
pub fn render(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    if state.active_context.is_none() {
        render_no_subscription(frame, area, theme);
        return;
    }

    // If resource groups are loading and we have none yet, show loading state.
    if state.resource_groups.is_empty() && !state.pending_operations.is_empty() {
        render_loading(frame, area, state, theme);
        return;
    }

    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(35),
            Constraint::Percentage(65),
        ])
        .split(area);

    render_left_pane(frame, panes[0], state, theme);
    render_right_pane(frame, panes[1], state, theme);
}

/* ============================================================================================== */
/*                                        Private renderers                                       */
/* ============================================================================================== */

fn render_no_subscription(frame: &mut Frame, area: Rect, theme: &Theme) {
    let lines = vec![
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "  Select a subscription first (press 1 for context switcher)", 
            theme.hint_style(),
        )),
    ];
    let para = Paragraph::new(lines).style(theme.base_style());
    frame.render_widget(para, area);
}

/* ============================================================================================== */
fn render_loading(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let spinner_chars = SPINNER_CHARS;
    let spinner = spinner_chars[state.spinner_frame as usize % spinner_chars.len()];

    let lines = vec![
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::styled(format!("{} ", spinner), theme.spinner_style()),
            Span::styled("Loading resource groups...", theme.spinner_style()),
        ]),
    ];
    let para = Paragraph::new(lines)
        .style(theme.base_style())
        .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(para, area);
}

/* ============================================================================================== */
/*                                            Left pane                                           */
/* ============================================================================================== */

fn render_left_pane(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let is_focused = state.resource_browser_focus == Pane::Left;

    let border_style = if is_focused {
        theme.content_focused_style()
    } else {
        theme.content_border_style()
    };

    let count = filtered_resource_groups(state).len();
    let title = format!(" Resource groups ({}) ", count);
    
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .style(theme.surface_style());

    let inner = block.inner(area);

    let filtered = filtered_resource_groups(state);
    let cursor = state.resource_group_cursor.min(filtered.len().saturating_sub(1));

    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .map(|(i, rg)| {
            let is_selected = i == cursor && is_focused;
            let prefix = if is_selected { " > " } else { "   " };
            let style = if is_selected {
                theme.selected_style()
            } else {
                theme.surface_style().fg(theme.text)
            };
            let location = abbreviate_location(&rg.location);
            ListItem::new(Line::from(vec![
                Span::styled(format!("{}{}", prefix, rg.name), style),
                Span::styled(format!("  {}", location), theme.surface_style().fg(theme.subtle)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .style(theme.surface_style())
        .highlight_style(theme.selected_style())
        .scroll_padding(state.config.ui.scroll_off);

    let mut list_state = state.scroll.resource_groups.borrow_mut();
    if is_focused && !filtered.is_empty() {
        list_state.select(Some(cursor));
    } else {
        list_state.select(None);
    }

    frame.render_widget(block, area);
    frame.render_stateful_widget(list, inner, &mut list_state);
}

/* ============================================================================================== */
/*                                           Right Pane                                           */
/* ============================================================================================== */

fn render_right_pane(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let is_focused = state.resource_browser_focus == Pane::Right;

    let border_style = if is_focused {
        theme.content_focused_style()
    } else {
        theme.content_border_style()
    };

    let rg_name = selected_resource_group_name(state).unwrap_or_default();
    let count = filtered_resources(state).len();
    let title = if rg_name.is_empty() {
        " Resources ".to_string()
    } else {
        format!(" {} ({}) ", rg_name, count)
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .style(theme.surface_style());

    let inner = block.inner(area);

    // Show loading if resources are being fetched.
    if state.pending_operations.values().any(|op| op.description.starts_with("Loading resources")) {
        let spinner_chars = SPINNER_CHARS;
        let spinner = spinner_chars[state.spinner_frame as usize % spinner_chars.len()];
        let loading = Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(format!(" {} ", spinner), theme.spinner_style()),
                Span::styled("Loading...", theme.spinner_style()),
            ]),
        ])
        .style(theme.surface_style());
        frame.render_widget(block, area);
        frame.render_widget(loading, inner);
        return;
    }

    let filtered = filtered_resources(state);

    if filtered.is_empty() && state.resource_groups.is_empty() {
        frame.render_widget(block, area);
        return;
    }

    let cursor = state.resource_cursor.min(filtered.len().saturating_sub(1));

    // Calculate column widths for compact display.
    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .map(|(i, res)| {
            let is_selected = i == cursor && is_focused;
            let prefix = if is_selected { " > " } else { "   " };
            let abbrev_type = abbreviate_resource_type(&res.resource_type);
            let location = abbreviate_location(&res.location);

            let name_style = if is_selected {
                theme.selected_style()
            } else {
                theme.surface_style().fg(theme.text)
            };

            let type_style = if is_vm(&res.resource_type) {
                theme.vm_type_style()
            } else {
                theme.surface_style().fg(theme.azure_light)
            };

            ListItem::new(Line::from(vec![
                Span::styled(format!("{}{}", prefix, res.name), name_style),
                Span::styled(format!("  {}", abbrev_type), type_style),
                Span::styled(format!("  {}", location), theme.surface_style().fg(theme.subtle)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .style(theme.surface_style())
        .highlight_style(theme.selected_style())
        .scroll_padding(state.config.ui.scroll_off);

    let mut list_state = state.scroll.resources.borrow_mut();
    if is_focused && !filtered.is_empty() {
        list_state.select(Some(cursor));
    } else {
        list_state.select(None);
    }

    frame.render_widget(block, area);
    frame.render_stateful_widget(list, inner, &mut list_state);
}


/* ============================================================================================== */
/*                                         Public helpers                                         */
/* ============================================================================================== */

/// Returns the name of the currently selected resource group, if any.
pub fn selected_resource_group_name(state: &AppState) -> Option<String> {
    let filtered = filtered_resource_groups(state);
    let cursor = state.resource_group_cursor.min(filtered.len().saturating_sub(1));
    filtered.get(cursor).map(|rg| rg.name.clone())
}

/* ============================================================================================== */
/// Returns filtered resource groups based on the search query (when left pane is focused).
pub fn filtered_resource_groups(state: &AppState) -> Vec<&ResourceGroup> {
    let query = if state.resource_browser_focus == Pane::Left && !state.resource_search_query.is_empty() {
        Some(state.resource_search_query.to_lowercase())
    } else {
        None
    };

    state
        .resource_groups
        .iter()
        .filter(|rg| {
            query
                .as_ref()
                .map_or(true, |q| rg.name.to_lowercase().contains(q))
        })
        .collect()
}

/* ============================================================================================== */

/// A selected VM's coordinates, for opening the run-command view.
pub struct VmTarget {
    pub subscription_id: String,
    pub resource_group: String,
    pub vm_name: String,
}

/// Returns the [`VmTarget`] for the right-pane selection if it is a VM, else `None`.
pub fn selected_vm_target(state: &AppState) -> Option<VmTarget> {
    let filtered = filtered_resources(state);
    let cursor = state.resource_cursor.min(filtered.len().saturating_sub(1));
    let res = filtered.get(cursor)?;
    if !is_vm(&res.resource_type) {
        return None;
    }
    let subscription_id = state.active_context.as_ref()?.subscription.id.clone();
    Some(VmTarget {
        subscription_id,
        resource_group: res.resource_group.clone(),
        vm_name: res.name.clone(),
    })
}

/* ============================================================================================== */

/// Returns filtered resources based on the search query (when right pane is focused).
pub fn filtered_resources(state: &AppState) -> Vec<&Resource> {
    let query = if state.resource_browser_focus == Pane::Right && !state.resource_search_query.is_empty() {
        Some(state.resource_search_query.to_lowercase())
    } else {
        None
    };

    state
        .resources
        .iter()
        .filter(|r| {
            query.as_ref().map_or(true, |q| {
                r.name.to_lowercase().contains(q)
                    || r.resource_type.to_lowercase().contains(q)
            })
        })
        .collect()
}

/* ============================================================================================== */
/*                                              Tests                                             */
/* ============================================================================================== */

#[cfg(test)]
mod tests {
    use super::is_vm;

    #[test]
    fn is_vm_true_for_virtual_machine_type() {
        assert!(is_vm("Microsoft.Compute/virtualMachines"));
    }

    #[test]
    fn is_vm_false_for_other_types() {
        assert!(!is_vm("Microsoft.Storage/storageAccounts"));
        assert!(!is_vm("Microsoft.Compute/disks"));
        assert!(!is_vm(""));
    }
}