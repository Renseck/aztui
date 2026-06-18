use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::{AppState, Modal};
use crate::domain::models::AzureContext;
use crate::ui::theme::Theme;
use crate::ui::widgets::modal::{render_modal_frame, ModalPosition};

/* ============================================================================================== */
/// Renders the Ctrl+P quick switch modal overlay.
pub fn render(frame: &mut Frame, state: &AppState, theme: &Theme) {
    let (query, filtered, cursor) = match &state.modal {
        Some(Modal::QuickSwitch { query, filtered, cursor }) => {
            (query.clone(), filtered.clone(), *cursor)
        }
        _ => return,
    };

     let modal_h = (filtered.len() as u16 + 8).min(frame.area().height - 4).max(10);

    let inner = render_modal_frame(
        frame,
        "Switch context",
        Some("Enter: switch   Esc: cancel"),
        ModalPosition::Center,
        65,
        modal_h,
        theme,
        theme.modal_border_style(),
        
    );

    // Split inner: search input, list, footer hint
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // search
            Constraint::Length(1), // spacer
            Constraint::Min(1),    // list
            Constraint::Length(1), // hints
        ])
        .split(inner);

    // Search input
    let cursor_char = "_";
    let search_line = Line::from(vec![
        Span::styled("  ", theme.hint_style()),
        Span::styled(&query, theme.surface_style().fg(theme.bright).add_modifier(Modifier::BOLD)),
        Span::styled(cursor_char, theme.surface_style().fg(theme.azure_light)),
    ]);
    frame.render_widget(Paragraph::new(search_line).style(theme.surface_style()), layout[0]);

    // Divider
    let divider = "─".repeat(inner.width as usize);
    frame.render_widget(
        Paragraph::new(divider).style(theme.surface_style().fg(theme.muted)),
        layout[1],
    );

    // Filtered list
    let recent_ids: Vec<String> = state
        .recent_contexts
        .iter()
        .map(|c| c.subscription.id.clone())
        .collect();

    let recent: Vec<&AzureContext> = filtered
        .iter()
        .filter(|c| recent_ids.contains(&c.subscription.id))
        .collect();

    let all_non_recent: Vec<&AzureContext> = filtered
        .iter()
        .filter(|c| !recent_ids.contains(&c.subscription.id))
        .collect();

    let mut list_items: Vec<ListItem> = Vec::new();
    let mut item_idx = 0usize;

    if !recent.is_empty() {
        list_items.push(ListItem::new(Line::from(Span::styled(
            "  RECENT",
            theme.hint_style().add_modifier(Modifier::BOLD),
        ))));
        for ctx in &recent {
            let style = if item_idx == cursor {
                theme.selected_style()
            } else {
                theme.surface_style().fg(theme.text)
            };
            let base = style;
            let indices = crate::ui::fuzzy::fuzzy_match(&ctx.label(), &query)
                .map(|(_, idx)| idx)
                .unwrap_or_default();
            let mut spans = vec![Span::styled("  ", base)];
            spans.extend(crate::ui::fuzzy::highlight(&ctx.label(), &indices, base, theme.match_style()));
            list_items.push(ListItem::new(Line::from(spans)));
            item_idx += 1;
        }
        list_items.push(ListItem::new(Line::from("")));
    }

    if !all_non_recent.is_empty() {
        list_items.push(ListItem::new(Line::from(Span::styled(
            "  ALL MATCHES",
            theme.hint_style().add_modifier(Modifier::BOLD),
        ))));
        for ctx in &all_non_recent {
            let style = if item_idx == cursor {
                theme.selected_style()
            } else {
                theme.surface_style().fg(theme.text)
            };
            let base = style;
            let indices = crate::ui::fuzzy::fuzzy_match(&ctx.label(), &query)
                .map(|(_, idx)| idx)
                .unwrap_or_default();
            let mut spans = vec![Span::styled("  ", base)];
            spans.extend(crate::ui::fuzzy::highlight(&ctx.label(), &indices, base, theme.match_style()));
            list_items.push(ListItem::new(Line::from(spans)));
            item_idx += 1;
        }
    }

    let list = List::new(list_items).style(theme.surface_style());
    frame.render_widget(list, layout[2]);

    // Footer hints
    let hints = Line::from(vec![Span::styled(
        "  Enter: switch   Esc: cancel",
        theme.hint_style(),
    )]);
    frame.render_widget(Paragraph::new(hints).style(theme.surface_style()), layout[3]);
}

/* ============================================================================================== */
/// Builds the filtered context list for the quick switch modal: fuzzy-matched
/// against each context's label and sorted by match score (best first). An
/// empty query returns every context in tenant order.
pub fn build_filtered(state: &AppState, query: &str) -> Vec<AzureContext> {
    let mut scored: Vec<(i64, AzureContext)> = state
        .tenants
        .iter()
        .flat_map(|tenant| {
            state
                .subscriptions_by_tenant
                .get(&tenant.id)
                .into_iter()
                .flatten()
                .filter_map(move |sub| {
                    let ctx = AzureContext {
                        tenant: tenant.clone(),
                        subscription: sub.clone(),
                    };
                    crate::ui::fuzzy::fuzzy_match(&ctx.label(), query).map(|(score, _)| (score, ctx))
                })
        })
        .collect();

    if !query.is_empty() {
        scored.sort_by(|a, b| b.0.cmp(&a.0));
    }

    scored.into_iter().map(|(_, ctx)| ctx).collect()
}

/* ============================================================================================== */
/// Returns the [`AzureContext`] at the quick switch cursor position, if any.
/// Skips section header rows (RECENT / ALL MATCHES).
pub fn selected_context(state: &AppState) -> Option<AzureContext> {
    let (filtered, cursor) = match &state.modal {
        Some(Modal::QuickSwitch { filtered, cursor, .. }) => (filtered.clone(), *cursor),
        _ => return None,
    };
    filtered.into_iter().nth(cursor)
}
