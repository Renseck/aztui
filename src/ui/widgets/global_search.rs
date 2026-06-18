//! Global resource search view: cross-subscription fuzzy finder backed by Azure
//! Resource Graph.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::AppState;
use crate::domain::models::GlobalResource;
use crate::ui::fuzzy::{fuzzy_match, highlight};
use crate::ui::theme::Theme;
use crate::ui::widgets::resource_browser::{abbreviate_resource_type, is_vm};

/// Column widths (in characters) for the result rows. The subscription column
/// takes whatever horizontal space remains.
const NAME_W: usize = 32;
const TYPE_W: usize = 14;
const RG_W: usize = 26;

/* ============================================================================================== */
/// Renders the global search view: a search line on top and a results list
/// (Name · Type · Resource group · Subscription) below. VM rows render the name
/// in the theme's VM/actionable colour to signal that Enter opens the
/// run-command view.
pub fn render(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(area);

    // Search line.
    let search_border = if state.search_focused {
        theme.search_focused_style()
    } else {
        theme.content_border_style()
    };
    let search_block = Block::default()
        .title(" Search (/) ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(search_border);
    let query = Paragraph::new(state.global_search_query.as_str())
        .style(theme.base_style())
        .block(search_block);
    frame.render_widget(query, chunks[0]);

    // Results list.
    let rows = filtered_global_resources(state);
    let cursor = state.global_search_cursor.min(rows.len().saturating_sub(1));

    let items: Vec<ListItem> = rows
        .iter()
        .enumerate()
        .map(|(i, (r, name_idx))| {
            let is_selected = i == cursor;
            let name_style = if is_selected {
                theme.selected_style()
            } else if is_vm(&r.resource_type) {
                theme.vm_type_style()
            } else {
                theme.surface_style().fg(theme.text)
            };

            let prefix = if is_selected { " > " } else { "   " };
            let mut spans = vec![Span::styled(prefix.to_string(), name_style)];
            spans.extend(highlight(&r.name, name_idx, name_style, theme.match_style()));

            // Pad the name column so the trailing columns line up.
            let name_len = r.name.chars().count();
            if name_len < NAME_W {
                spans.push(Span::styled(" ".repeat(NAME_W - name_len), name_style));
            }

            let sub_name = state
                .subscriptions_by_tenant
                .values()
                .flatten()
                .find(|s| s.id == r.subscription_id)
                .map(|s| s.name.clone())
                .unwrap_or_else(|| r.subscription_id.clone());

            spans.push(Span::styled(
                pad(abbreviate_resource_type(&r.resource_type), TYPE_W),
                theme.surface_style().fg(theme.azure_light),
            ));
            spans.push(Span::styled(
                pad(&r.resource_group, RG_W),
                theme.surface_style().fg(theme.text),
            ));
            spans.push(Span::styled(sub_name, theme.surface_style().fg(theme.subtle)));

            ListItem::new(Line::from(spans))
        })
        .collect();

    let title = format!(" Global resources ({}) ", rows.len());
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.content_border_style())
        .style(theme.surface_style());
    let inner = block.inner(chunks[1]);

    let list = List::new(items)
        .style(theme.surface_style())
        .highlight_style(theme.selected_style())
        .scroll_padding(state.config.ui.scroll_off);

    let mut list_state = state.scroll.global_search.borrow_mut();
    if rows.is_empty() {
        list_state.select(None);
    } else {
        list_state.select(Some(cursor));
    }

    frame.render_widget(block, chunks[1]);
    frame.render_stateful_widget(list, inner, &mut list_state);
}

/* ============================================================================================== */
/// Pads (or truncates) `s` to exactly `width` characters so list columns align.
fn pad(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        s.chars().take(width.saturating_sub(1)).chain(std::iter::once(' ')).collect()
    } else {
        let mut out = String::from(s);
        out.extend(std::iter::repeat(' ').take(width - len));
        out
    }
}

/* ============================================================================================== */
/// Returns the global resources matching the current fuzzy query, each paired
/// with the matched character indices (for highlighting the name), sorted by
/// descending fuzzy score. An empty query returns every resource in input order.
pub fn filtered_global_resources(state: &AppState) -> Vec<(&GlobalResource, Vec<usize>)> {
    let needle = state.global_search_query.as_str();
    let mut scored: Vec<(i64, &GlobalResource, Vec<usize>)> = state
        .global_resources
        .iter()
        .filter_map(|r| {
            let haystack = format!("{} {} {} {}", r.name, r.resource_type, r.resource_group, r.subscription_id);
            fuzzy_match(&haystack, needle).map(|(score, _idx)| {
                // Re-run against the name alone for highlight indices on the name column.
                let name_idx = fuzzy_match(&r.name, needle).map(|(_, i)| i).unwrap_or_default();
                (score, r, name_idx)
            })
        })
        .collect();

    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.into_iter().map(|(_, r, idx)| (r, idx)).collect()
}

/* ============================================================================================== */
/// Returns the currently selected global resource, if any.
pub fn selected_global_resource(state: &AppState) -> Option<&GlobalResource> {
    filtered_global_resources(state)
        .get(state.global_search_cursor)
        .map(|(r, _)| *r)
}

/* ============================================================================================== */
/*                                              Tests                                             */
/* ============================================================================================== */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::security::SecurityManager;

    fn state_with(rows: Vec<GlobalResource>) -> AppState {
        let mut s = AppState::new(AppConfig::default(), SecurityManager::disabled());
        s.global_resources = rows;
        s
    }

    fn row(name: &str) -> GlobalResource {
        GlobalResource {
            id: format!("/x/{name}"),
            name: name.into(),
            resource_type: "microsoft.storage/storageaccounts".into(),
            resource_group: "rg".into(),
            subscription_id: "s".into(),
            location: "westeurope".into(),
        }
    }

    #[test]
    fn empty_query_returns_all() {
        let s = state_with(vec![row("alpha"), row("beta")]);
        assert_eq!(filtered_global_resources(&s).len(), 2);
    }

    #[test]
    fn query_filters_by_fuzzy_subsequence() {
        let mut s = state_with(vec![row("storage-prod"), row("vault-dev")]);
        s.global_search_query = "stgprd".into();
        let out = filtered_global_resources(&s);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0.name, "storage-prod");
    }

    #[test]
    fn selected_respects_cursor() {
        let mut s = state_with(vec![row("a"), row("b")]);
        s.global_search_cursor = 1;
        assert_eq!(selected_global_resource(&s).unwrap().name, "b");
    }
}