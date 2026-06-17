use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::ui::theme::Theme;

/* ============================================================================================== */
/// Renders a single-row contextual hint bar. Each `(key, label)` pair shows the
/// key in the accent colour followed by its label in subtle text, pairs
/// separated by a dim middle dot.
pub fn render(frame: &mut Frame, area: Rect, hints: &[(&str, &str)], theme: &Theme) {
    let mut spans: Vec<Span> = vec![Span::styled(" ", theme.surface_style())];

    for (i, (key, label)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ·  ", theme.surface_style().fg(theme.muted)));
        }
        spans.push(Span::styled((*key).to_string(), theme.surface_style().fg(theme.azure_light)));
        spans.push(Span::styled(" ", theme.surface_style()));
        spans.push(Span::styled((*label).to_string(), theme.surface_style().fg(theme.subtle)));
    }

    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(theme.surface_style()),
        area,
    );
}