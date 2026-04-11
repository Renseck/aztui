use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::AppState;
use crate::ui::theme::Theme;

const SPINNER_CHARS: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/* ============================================================================================== */
/// Renders the single-row status bar at the bottom (or top) of the screen.
/// 
/// Layout: [active context] ... [pending op spinner] ... [error / cache age]
pub fn render(frame: &mut Frame, area: ratatui::layout::Rect, state: &AppState, theme: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(30),
            Constraint::Percentage(20),
        ])
        .split(area);
    
    // Left: active context.
    let context_text = if state.locked {
        Line::from(vec![Span::styled(
            " Locked - enter master password to continue",
            theme.error_style(),
        )])
    } else {
        match &state.active_context {
            Some(ctx) => Line::from(vec![
                Span::styled(" ● ", theme.active_context_indicator_style()),
                Span::styled(ctx.label(), theme.active_context_indicator_style()),
            ]),
            None => Line::from(vec![Span::styled(" No active context", theme.hint_style())]),
        }
    };

    let context_widget = Paragraph::new(context_text).style(theme.status_bar_style());
    frame.render_widget(context_widget, chunks[0]);

    // Center: pending operation spinner
    let pending_text = if let Some(op) = state.pending_operations.values().next() {
        let spinner = SPINNER_CHARS[state.spinner_frame as usize % SPINNER_CHARS.len()];
        Line::from(vec![
            Span::styled(format!("{} ", spinner), theme.spinner_style()),
            Span::styled(&op.description, theme.spinner_style()),
        ])
    } else {
        Line::from("")
    };

    let pending_widget = Paragraph::new(pending_text).style(theme.status_bar_style());
    frame.render_widget(pending_widget, chunks[0]);

    // Right: error summary or hints.
    let right_text = if let Some(err) = &state.last_error {
        Line::from(vec![Span::styled(
            format!("⚠ {} ", err.kind_label()),
            theme.error_style(),
        )])
    } else {
        Line::from(vec![Span::styled("? help  q quit ", theme.hint_style())])
    };

    let right_widget = Paragraph::new(right_text)
        .style(theme.status_bar_style())
        .alignment(ratatui::layout::Alignment::Right);
    frame.render_widget(right_widget, chunks[2]);
}