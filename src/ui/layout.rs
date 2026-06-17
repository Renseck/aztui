use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::app::{AppState, Modal, View};
use crate::ui::theme::{Theme};
use crate::ui::widgets::{context_switcher, modal, quick_switch, status_bar};

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
            Modal::QuickSwitch { .. } => quick_switch::render(frame, state, theme),
            Modal::ErrorDetail(_) => modal::render_error_detail(frame, state, theme),
            Modal::Confirm { .. } => modal::render_confirm(frame, state, theme),
            Modal::PasswordPrompt { .. } => modal::render_password_prompt(frame, state, theme),
            Modal::ActivityDetail {.. } => {},
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
        View::Help => " > Help",
    };

    let title_line = Line::from(vec![
        Span::styled(
            "  aztui", 
            theme.heading_style().add_modifier(Modifier::BOLD),
        ),
        Span::styled(view_label, theme.surface_style().fg(theme.subtle)),
    ]);

    let hint_line = Span::styled("? for help ", theme.hint_style());

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(14)])
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
        View::ActivityLog => {}
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
    let lines = vec![
        Line::from(""),
        Line::from(vec![Span::styled("  Navigation", theme.heading_style())]),
        Line::from(vec![Span::styled(
            "  ─────────────────────────────────────────",
            theme.hint_style(),
        )]),
        Line::from(vec![
            Span::styled("  ↑/↓  or  j/k   ", theme.surface_style().fg(theme.azure_light)),
            Span::styled("Navigate list", theme.surface_style().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("  Tab / ← / →     ", theme.surface_style().fg(theme.azure_light)),
            Span::styled("Switch pane (resource browser)", theme.surface_style().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("  Enter           ", theme.surface_style().fg(theme.azure_light)),
            Span::styled("Select / confirm", theme.surface_style().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("  F5              ", theme.surface_style().fg(theme.azure_light)),
            Span::styled("Run script on VM (run-command view)", theme.surface_style().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("  Tab             ", theme.surface_style().fg(theme.azure_light)),
            Span::styled("Switch editor/output (run-command view)", theme.surface_style().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("  Esc             ", theme.surface_style().fg(theme.azure_light)),
            Span::styled("Clear search / close modal", theme.surface_style().fg(theme.text)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled("  Actions", theme.heading_style())]),
        Line::from(vec![Span::styled(
            "  ─────────────────────────────────────────",
            theme.hint_style(),
        )]),
        Line::from(vec![
            Span::styled("  /               ", theme.surface_style().fg(theme.azure_light)),
            Span::styled("Focus search", theme.surface_style().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+G          ", theme.surface_style().fg(theme.azure_light)),
            Span::styled("Quick switch context", theme.surface_style().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("  r               ", theme.surface_style().fg(theme.azure_light)),
            Span::styled("Refresh current view", theme.surface_style().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("  [ / ]  or  h/l  ", theme.surface_style().fg(theme.azure_light)),
            Span::styled("Period navigation (cost explorer)", theme.surface_style().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("  1               ", theme.surface_style().fg(theme.azure_light)),
            Span::styled("Context switcher", theme.surface_style().fg(theme.text)),
        ]),
         Line::from(vec![
            Span::styled("  2               ", theme.surface_style().fg(theme.azure_light)),
            Span::styled("Resource browser", theme.surface_style().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("  3               ", theme.surface_style().fg(theme.azure_light)),
            Span::styled("Cost explorer", theme.surface_style().fg(theme.text)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled("  System", theme.heading_style())]),
        Line::from(vec![Span::styled(
            "  ─────────────────────────────────────────",
            theme.hint_style(),
        )]),
        Line::from(vec![
            Span::styled("  ?               ", theme.surface_style().fg(theme.azure_light)),
            Span::styled("Toggle this help screen", theme.surface_style().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("  q               ", theme.surface_style().fg(theme.azure_light)),
            Span::styled("Quit", theme.surface_style().fg(theme.text)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            format!("  aztui v{}", env!("CARGO_PKG_VERSION")),
            theme.hint_style(),
        )]),
    ];

    let para = Paragraph::new(lines).style(theme.base_style());
    frame.render_widget(para, area);
}