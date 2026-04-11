use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::app::AppState;
use crate::ui::theme::Theme;

/* ============================================================================================== */
/// Renders the inline search input bar.
/// 
/// Shown when `state.search_focused` is true or `state.search_query` is non-empty.
pub fn render(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let border_style = if state.search_focused {
        theme.search_focused_style()
    } else {
        theme.content_border_style()
    };

    let cursor = if state.search_focused { "_" } else { "" };
    let query_display = format!("{}{}", state.search_query, cursor);

    let content = Line::from(vec![
        Span::styled("Search: ", theme.hint_style()),
        Span::styled(query_display, theme.surface_style().fg(theme.bright).add_modifier(Modifier::BOLD)),
    ]);

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_type(BorderType::Plain)
        .border_style(border_style);

    let widget = Paragraph::new(content)
        .style(theme.surface_style())
        .block(block);

    frame.render_widget(widget, area);
}