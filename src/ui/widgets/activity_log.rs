use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::app::AppState;
use crate::domain::models::ActivityLogEntry;
use crate::ui::theme::Theme;
use crate::ui::widgets::SPINNER_CHARS;

/* ============================================================================================== */
/*                                          Filter helpers                                        */
/* ============================================================================================== */

/// Returns the entries visible under the current failed-only + search filters.
pub fn filtered_entries(state: &AppState) -> Vec<&ActivityLogEntry> {
    let activity = match &state.activity {
        Some(a) => a,
        None => return Vec::new(),
    };
    let query = activity.search.to_lowercase();

    activity
        .entries
        .iter()
        .filter(|e| !activity.failed_only || e.is_failure())
        .filter(|e| {
            query.is_empty()
                || e.operation.to_lowercase().contains(&query)
                || e.resource_name.to_lowercase().contains(&query)
                || e.caller
                    .as_ref()
                    .map_or(false, |c| c.to_lowercase().contains(&query))
        })
        .collect()
}

/// Count of currently-visible entries (used for cursor clamping).
pub fn filtered_len(state: &AppState) -> usize {
    filtered_entries(state).len()
}

/// The entry under the cursor, if any.
pub fn selected_entry(state: &AppState) -> Option<ActivityLogEntry> {
    let activity = state.activity.as_ref()?;
    let entries = filtered_entries(state);
    let cursor = activity.cursor.min(entries.len().saturating_sub(1));
    entries.get(cursor).map(|e| (*e).clone())
}

/* ============================================================================================== */
/*                                          Public renderer                                        */
/* ============================================================================================== */

/// Renders the activity log view.
pub fn render(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    if state.active_context.is_none() {
        let para = Paragraph::new(Line::from(Span::styled(
            "  Select a subscription first (press 1 for context switcher)",
            theme.hint_style(),
        )))
        .style(theme.base_style());
        frame.render_widget(para, area);
        return;
    }

    let activity = match &state.activity {
        Some(a) => a,
        None => return,
    };

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // search bar
            Constraint::Min(1),    // list
        ])
        .split(area);

    crate::ui::widgets::search_input::render(
        frame,
        layout[0],
        &activity.search,
        activity.search_focused,
        theme,
    );

    // Title: scope + window + failed-only marker.
    let sub_name = state
        .active_context
        .as_ref()
        .map(|c| c.subscription.name.clone())
        .unwrap_or_default();
    let failed_marker = if activity.failed_only { "  [failed only]" } else { "" };
    let title = format!(
        " Activity — {} / {} ({}){} ",
        sub_name,
        activity.scope.label(),
        activity.window.label(),
        failed_marker,
    );

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.content_focused_style())
        .style(theme.surface_style());

    let inner = block.inner(layout[1]);
    frame.render_widget(block, layout[1]);

    // Loading state (matched by op description, like the resource browser).
    if state
        .pending_operations
        .values()
        .any(|op| op.description.starts_with("Loading activity log"))
    {
        let spinner = SPINNER_CHARS[state.spinner_frame as usize % SPINNER_CHARS.len()];
        let para = Paragraph::new(Line::from(vec![
            Span::styled(format!(" {} ", spinner), theme.spinner_style()),
            Span::styled("Loading activity log...", theme.spinner_style()),
        ]))
        .style(theme.surface_style());
        frame.render_widget(para, inner);
        return;
    }

    let entries = filtered_entries(state);
    if entries.is_empty() {
        let para = Paragraph::new(Line::from(Span::styled(
            "  No activity in this scope/window.",
            theme.hint_style(),
        )))
        .style(theme.surface_style());
        frame.render_widget(para, inner);
        return;
    }

    let cursor = activity.cursor.min(entries.len().saturating_sub(1));

    let items: Vec<ListItem> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let is_selected = i == cursor;
            let prefix = if is_selected { " > " } else { "   " };

            let status_style = match e.status.as_str() {
                s if s.eq_ignore_ascii_case("Failed") => theme.error_style(),
                s if s.eq_ignore_ascii_case("Started") => theme.spinner_style(),
                _ => theme.surface_style().fg(theme.green),
            };
            let name_style = if is_selected {
                theme.selected_style()
            } else {
                theme.surface_style().fg(theme.text)
            };

            ListItem::new(Line::from(vec![
                Span::styled(format!("{}{:<16}", prefix, short_time(&e.timestamp)), name_style),
                Span::styled(format!("  {:<32}", truncate(&e.operation, 32)), name_style),
                Span::styled(format!("  {:<20}", truncate(&e.resource_name, 20)), theme.surface_style().fg(theme.azure_light)),
                Span::styled(format!("  {:<10}", e.status), status_style),
                Span::styled(format!("  {}", e.caller.clone().unwrap_or_default()), theme.surface_style().fg(theme.subtle)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .style(theme.surface_style())
        .highlight_style(theme.selected_style());

    let mut list_state = ListState::default();
    list_state.select(Some(cursor));
    frame.render_stateful_widget(list, inner, &mut list_state);
}

/* ============================================================================================== */
/*                                         Private helpers                                         */
/* ============================================================================================== */

/// Trims an ISO timestamp to `MM-DD HH:MM` for compact display; falls back to the
/// raw string if it doesn't parse.
fn short_time(ts: &str) -> String {
    // "2026-06-17T10:42:00Z" -> "06-17 10:42"
    if ts.len() >= 16 && ts.as_bytes().get(10) == Some(&b'T') {
        format!("{} {}", &ts[5..10], &ts[11..16])
    } else {
        ts.to_string()
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}