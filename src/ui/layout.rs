use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::actions::HelpSection;
use crate::app::{AppState, Modal, View};
use crate::ui::theme::{Theme};
use crate::ui::widgets::{context_switcher, modal, status_bar};

/* ============================================================================================== */
/// Top-level render function. Draws the complete TUI frame from `state`.
///
/// Layout:
/// ```text
/// ┌──────────────────────────────────────────┐
/// │  Title bar  (row 0)                      │
/// ├──────────────────────────────────────────┤
/// │  Content area                            │
/// │  (active view or loading state)          │
/// ├──────────────────────────────────────────┤
/// │  Status bar (row h-1)                    │
/// └──────────────────────────────────────────┘
/// ```
pub fn render(frame: &mut Frame, state: &AppState, theme: &Theme) {
    let chunks = Layout::default()
        .constraints([
            Constraint::Length(1), // title bar
            Constraint::Min(1),    // content
            Constraint::Length(1), // status bar
        ])
        .split(frame.area());

    render_title_bar(frame, chunks[0], state, theme);
    render_content(frame, chunks[1], state, theme);
    status_bar::render(frame, chunks[2], state, theme);

    // Modal overlays rendered last (on top of everything else).
    if let Some(m) = &state.modal {
        match m {
            Modal::Palette(p) => crate::ui::widgets::palette::render(frame, state, p, theme),
            Modal::ErrorDetail(_) => modal::render_error_detail(frame, state, theme),
            Modal::Confirm { .. } => modal::render_confirm(frame, state, theme),
            Modal::PasswordPrompt { .. } => modal::render_password_prompt(frame, state, theme),
            Modal::ActivityDetail(_) => modal::render_activity_detail(frame, state, theme),
        }
    }
}

/* ============================================================================================== */
/*                                        Private renderers                                       */
/* ============================================================================================== */

fn render_title_bar(frame: &mut Frame, area: ratatui::layout::Rect, state: &AppState, theme: &Theme) {
    let view_label = match state.active_view {
        View::ContextSwitcher => "",
        View::ResourceBrowser => " > Resources",
        View::CostExplorer => " > Cost Explorer",
        View::RunCommand => " > Run Command",
        View::ActivityLog => " > Activity Log",
        View::GlobalSearch => " > Global Search",
        View::Help => " > Help",
    };

    let title_line = Line::from(vec![
        Span::styled(
            "  aztui", 
            theme.heading_style().add_modifier(Modifier::BOLD),
        ),
        Span::styled(view_label, theme.surface_style().fg(theme.subtle)),
    ]);

    let hint_line = Span::styled(": palette  ? help ", theme.hint_style());

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(20)])
        .split(area);

    frame.render_widget(
        Paragraph::new(title_line).style(theme.surface_style()), 
        layout[0],
    );
    frame.render_widget(
        Paragraph::new(Line::from(hint_line))
            .style(theme.surface_style())
            .alignment(ratatui::layout::Alignment::Right),
        layout[1],
    );
}

/* ============================================================================================== */
fn render_content(frame: &mut Frame, area: ratatui::layout::Rect, state: &AppState, theme: &Theme) {
    // Clear content area with base background.
    let bg = Block::default().style(theme.base_style());
    frame.render_widget(bg, area);

    match state.active_view {
        View::ContextSwitcher => {
            if state.tenants.is_empty() && !state.pending_operations.is_empty() {
                context_switcher::render_loading(frame, area, state, theme);
            } else {
                context_switcher::render(frame, area, state, theme);
            }
        }
        View::ResourceBrowser => {
            crate::ui::widgets::resource_browser::render(frame, area, state, theme);
        }
        View::CostExplorer => {
            crate::ui::widgets::cost_explorer::render(frame, area, state, theme);
        }
        View::RunCommand => {
            crate::ui::widgets::run_command::render(frame, area, state, theme);
        }
        View::ActivityLog => {
            crate::ui::widgets::activity_log::render(frame, area, state, theme);
        }
        View::GlobalSearch => {
            crate::ui::widgets::global_search::render(frame, area, state, theme);
        }
        View::Help => render_help(frame, area, theme),
    }
}

/* ============================================================================================== */
fn _render_phase_stub(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    name: &str,
    phase: &str,
    theme: &Theme,
) {
    let para = Paragraph::new(vec![
        Line::from(""),
        Line::from(""),
        Line::from(vec![Span::styled(
            format!("  {} - coming in {}", name, phase),
            theme.hint_style(),
        )]),
    ])
    .style(theme.base_style());
    frame.render_widget(para, area);
}

/* ============================================================================================== */
fn render_help(frame: &mut Frame, area: ratatui::layout::Rect, theme: &Theme) {
    // Two columns so the generated help fits an 80x24 terminal.
    let (left, right): (Vec<HelpSection>, Vec<HelpSection>) = crate::actions::help_sections()
        .into_iter()
        .partition(|s| s.title == "Global" || s.title == "Navigation");

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let mut left_lines = help_lines(&left, theme);
    left_lines.push(Line::from(Span::styled(
        format!("  aztui v{}", env!("CARGO_PKG_VERSION")),
        theme.hint_style(),
    )));

    frame.render_widget(Paragraph::new(left_lines).style(theme.base_style()), cols[0]);
    frame.render_widget(Paragraph::new(help_lines(&right, theme)).style(theme.base_style()), cols[1]);
}

/* ============================================================================================== */
fn help_lines(sections: &[HelpSection], theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from("")];
    for section in sections {
        lines.push(Line::from(Span::styled(format!("  {}", section.title), theme.heading_style())));
        for (key, label) in &section.entries {
            lines.push(Line::from(vec![
                Span::styled(format!("  {:<14}", key), theme.surface_style().fg(theme.azure_light)),
                Span::styled(label.to_string(), theme.surface_style().fg(theme.text)),
            ]));
        }
        lines.push(Line::from(""));
    }
    lines
}