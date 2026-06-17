use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph};
use ratatui::Frame;

use crate::ui::theme::Theme;

/* ============================================================================================== */
/// Renders the inline search input bar on a single row.
///
/// `query` is the current text; `focused` controls the trailing cursor and is
/// the caller's responsibility (this widget is stateless). Rendered without a
/// bordered block so the text is visible even in a 1-row area — the content
/// widget below provides its own visual separation.
pub fn render(frame: &mut Frame, area: Rect, query: &str, focused: bool, theme: &Theme) {
    let cursor = if focused { "_" } else { "" };
    let query_display = format!("{}{}", query, cursor);

    let content = Line::from(vec![
        Span::styled("Search: ", theme.hint_style()),
        Span::styled(
            query_display,
            theme.surface_style().fg(theme.bright).add_modifier(Modifier::BOLD),
        ),
    ]);

    frame.render_widget(Paragraph::new(content).style(theme.surface_style()), area);
}