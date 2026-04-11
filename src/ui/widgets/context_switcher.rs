use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::app::AppState;
use crate::domain::models::{Subscription, SubscriptionState, Tenant};
use crate::ui::theme::Theme;
use crate::ui::widgets::SPINNER_CHARS;

/* ============================================================================================== */
/// A flat item in the rendered tenant/subscription list.
enum ContextItem<'a> {
    TenantHeader(&'a Tenant),
    SubscriptionRow {
        tenant: &'a Tenant,
        subscription: &'a Subscription,
        /// Index among all subscription rows (used for cursor mapping).
        sub_index: usize,
    },
}

/* ============================================================================================== */
/// Renders the main context switcher view.
/// 
/// Tenants are displayed as section headers; subscriptions are indented beneath.
/// The active context is marked; disabled subscriptions are dimmed.
pub fn render(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // search bar
            Constraint::Min(1),    // list
        ])
        .split(area);

    // Search input
    crate::ui::widgets::search_input::render(frame, layout[0], state, theme);

    // Build flat list
    let items = build_flat_list(state);
    let total_subs: usize = items
        .iter()
        .filter(|i| matches!(i, ContextItem::SubscriptionRow { .. }))
        .count();

    // Map subscription cursor to flat list index
    let flat_cursor = sub_index_to_flat(&items, state.context_list_cursor.min(total_subs.saturating_sub(1)));

    let border_style = if state.search_focused {
        theme.content_border_style()
    } else {
        theme.content_focused_style()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .style(theme.surface_style());

    let inner = block.inner(layout[1]);

    // Render items as a list
    let list_items: Vec<ListItem> = items
        .iter()
        .enumerate()
        .map(|(idx, item)| render_item(item, idx, flat_cursor, state, theme))
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(flat_cursor));

    let list = List::new(list_items)
        .style(theme.surface_style())
        .highlight_style(theme.selected_style());

    frame.render_widget(block, layout[1]);
    frame.render_stateful_widget(list, inner, &mut list_state);
}

/* ============================================================================================== */
/// Renders the loading state when no tenants have been loaded yet.
pub fn render_loading(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let spinner_chars = SPINNER_CHARS;
    let spinner = spinner_chars[state.spinner_frame as usize % spinner_chars.len()];

    let loading_line = Line::from(vec![
        Span::styled(format!("{} ", spinner), theme.spinner_style()),
        Span::styled("Loading tenants and subscriptions…", theme.spinner_style()),
    ]);
    let cmd_line = Line::from(vec![Span::styled(
        "  Running: az account list --all",
        theme.hint_style(),
    )]);

    let para = Paragraph::new(vec![
        Line::from(""),
        Line::from(""),
        loading_line,
        Line::from(""),
        cmd_line,
    ])
    .style(theme.base_style())
    .alignment(ratatui::layout::Alignment::Center);

    frame.render_widget(para, area);
}

/* ============================================================================================== */
/*                                    Public navigation helpers                                   */
/* ============================================================================================== */

/// Returns the total number of selectable subscription rows in the current state.
pub fn total_selectable(state: &AppState) -> usize {
    if state.search_query.is_empty() {
        state
            .subscriptions_by_tenant
            .values()
            .map(|v| v.len())
            .sum()
    } else {
        let q = state.search_query.to_lowercase();
        state
            .subscriptions_by_tenant
            .values()
            .flatten()
            .filter(|s| matches_query(s, &q))
            .count()
    }
}

/* ============================================================================================== */
/// Returns the [`AzureContext`] for the currently selected subscription row, if any.
pub fn selected_context(state: &AppState) -> Option<crate::domain::models::AzureContext> {
    let items = build_flat_list(state);
    let total_subs = items
        .iter()
        .filter(|i| matches!(i, ContextItem::SubscriptionRow { .. }))
        .count();
    let cursor = state.context_list_cursor.min(total_subs.saturating_sub(1));

    items.into_iter().find_map(|item| match item {
        ContextItem::SubscriptionRow {
            tenant,
            subscription,
            sub_index,
        } if sub_index == cursor => Some(crate::domain::models::AzureContext {
            tenant: tenant.clone(),
            subscription: subscription.clone(),
        }),
        _ => None,
    })
}


/* ============================================================================================== */
/*                                         Private helpers                                        */
/* ============================================================================================== */

fn build_flat_list<'a>(state: &'a AppState) -> Vec<ContextItem<'a>> {
    let mut items = Vec::new();
    let query = if state.search_query.is_empty() {
        None
    } else {
        Some(state.search_query.to_lowercase())
    };
    
    let mut sub_index = 0usize;

    for tenant in &state.tenants {
        let subs = match state.subscriptions_by_tenant.get(&tenant.id) {
            Some(s) => s,
            None => continue,
        };

        let filtered_subs: Vec<&Subscription> = subs
            .iter()
            .filter(|s| {
                query
                    .as_ref()
                    .map_or(true, |q| matches_query(s, q))
            })
            .collect();

        if filtered_subs.is_empty() {
            continue;
        }

        items.push(ContextItem::TenantHeader(tenant));

        for sub in filtered_subs {
            items.push(ContextItem::SubscriptionRow { 
                tenant, 
                subscription: sub, 
                sub_index 
            });
            sub_index += 1;
        }
    }

    items
}

/* ============================================================================================== */
fn matches_query(sub: &Subscription, query: &str) -> bool {
    sub.name.to_lowercase().contains(query) || sub.id.to_lowercase().contains(query)
}

/* ============================================================================================== */
fn sub_index_to_flat(items: &[ContextItem], sub_idx: usize) -> usize {
    items
        .iter()
        .enumerate()
        .find_map(|(i, item)| match item {
            ContextItem::SubscriptionRow { sub_index, .. } if *sub_index == sub_idx => Some(i),
            _ => None,
        })
        .unwrap_or(0)
}

/* ============================================================================================== */
fn render_item<'a>(
    item: &'a ContextItem<'a>,
    flat_idx: usize,
    cursor_flat: usize,
    state: &AppState,
    theme: &Theme,
) -> ListItem<'a> {
    match item {
        ContextItem::TenantHeader(tenant) => {
            let label = if tenant.default_domain.is_empty() {
                format!("  ▸ {}", tenant.display_name)
            } else {
                format!("  ▸ {}  ({})", tenant.display_name, tenant.default_domain)
            };
            ListItem::new(Line::from(Span::styled(label, theme.tenant_header_style())))
        }

        ContextItem::SubscriptionRow {
            tenant,
            subscription: sub,
            sub_index,
        } => {
            let is_selected = flat_idx == cursor_flat;
            let is_active = state
                .active_context
                .as_ref()
                .map_or(false, |ctx| ctx.subscription.id == sub.id);
            let is_enabled = sub.state.is_active();

            let prefix = if is_selected { "    [» " } else { "       " };
            let suffix = if is_selected && is_enabled { " ]" } else { "  " };

            let state_label = format!("  {}", sub.state);

            let active_marker = if is_active { " ● active" } else { "" };

            let name_span = if is_enabled {
                Span::styled(
                    format!("{}{}{}", prefix, sub.name, suffix),
                    if is_active {
                        theme.active_context_style()
                    } else if is_selected {
                        theme.selected_style()
                    } else {
                        theme.surface_style().fg(theme.text)
                    },
                )
            } else {
                Span::styled(
                    format!("{}{}{}", prefix, sub.name, suffix),
                    theme.dimmed_style(),
                )
            };

            let state_span = Span::styled(
                format!("{}{}", active_marker, state_label),
                if is_active {
                    theme.active_context_style()
                } else if is_enabled {
                    theme.surface_style().fg(theme.subtle)
                } else {
                    theme.dimmed_style()
                },
            );

            ListItem::new(Line::from(vec![name_span, state_span]))
        }
    }
}