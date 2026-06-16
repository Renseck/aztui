use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{AppState, RunCommandSession, RunPane, RunStatus};
use crate::ui::theme::Theme;
use crate::ui::widgets::SPINNER_CHARS;

/* ============================================================================================== */
/// Renders the VM run-command view: a script editor, an output pane, and a status line.
pub fn render(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let session = match &state.run_command {
        Some(s) => s,
        None => return,
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(45), // editor
            Constraint::Min(3),         // output
            Constraint::Length(1),      // status line
        ])
        .split(area);

    render_editor(frame, chunks[0], session, theme);
    render_output(frame, chunks[1], state, session, theme);
    render_status(frame, chunks[2], session, theme);
}

/* ============================================================================================== */
fn render_editor(frame: &mut Frame, area: Rect, session: &RunCommandSession, theme: &Theme) {
    let focused = session.focus == RunPane::Editor;
    let border_style = if focused {
        theme.content_focused_style()
    } else {
        theme.content_border_style()
    };

    let block = Block::default()
        .title(format!(" Script — {} ", session.vm_name))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .style(theme.surface_style());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Render the script text ourselves: tui-textarea's `widget()` targets a
    // different ratatui version than this Frame, so we read its buffer instead.
    let lines: Vec<Line> = session
        .editor
        .lines()
        .iter()
        .map(|l| Line::from(Span::styled(l.clone(), theme.surface_style().fg(theme.text))))
        .collect();

    let para = Paragraph::new(lines).style(theme.surface_style());
    frame.render_widget(para, inner);

    // Show the real terminal cursor at the editor's cursor position when focused.
    if focused {
        let (row, col) = session.editor.cursor(); // (row, column)
        let cx = inner.x + (col as u16).min(inner.width.saturating_sub(1));
        let cy = inner.y + (row as u16).min(inner.height.saturating_sub(1));
        frame.set_cursor_position((cx, cy));
    }
}


/* ============================================================================================== */
fn render_output(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    session: &RunCommandSession,
    theme: &Theme,
) {
    let focused = session.focus == RunPane::Output;
    let border_style = if focused {
        theme.content_focused_style()
    } else {
        theme.content_border_style()
    };

    let block = Block::default()
        .title(" Output ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .style(theme.surface_style());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Spinner while running.
    if session.status == RunStatus::Running {
        let spinner = SPINNER_CHARS[state.spinner_frame as usize % SPINNER_CHARS.len()];
        let para = Paragraph::new(Line::from(vec![
            Span::styled(format!(" {} ", spinner), theme.spinner_style()),
            Span::styled(format!("Running on {}...", session.vm_name), theme.spinner_style()),
        ]))
        .style(theme.surface_style());
        frame.render_widget(para, inner);
        return;
    }

    let lines: Vec<Line> = match &session.output {
        Some(out) => {
            let mut v: Vec<Line> = Vec::new();
            for l in out.stdout.lines() {
                v.push(Line::from(Span::styled(
                    l.to_string(),
                    theme.surface_style().fg(theme.text),
                )));
            }
            if !out.stderr.trim().is_empty() {
                v.push(Line::from(""));
                v.push(Line::from(Span::styled("stderr:", theme.error_style())));
                for l in out.stderr.lines() {
                    v.push(Line::from(Span::styled(l.to_string(), theme.error_style())));
                }
            }
            if v.is_empty() {
                v.push(Line::from(Span::styled("(no output)", theme.hint_style())));
            }
            v
        }
        None => vec![Line::from(Span::styled(
            "Press F5 to run the script.",
            theme.hint_style(),
        ))],
    };

    let para = Paragraph::new(lines)
        .style(theme.surface_style())
        .wrap(Wrap { trim: false })
        .scroll((session.output_scroll, 0));
    frame.render_widget(para, inner);
}

/* ============================================================================================== */
fn render_status(frame: &mut Frame, area: Rect, session: &RunCommandSession, theme: &Theme) {
    let status_span = match session.status {
        RunStatus::Idle => Span::styled(
            " F5 run · Tab switch pane · Esc back",
            theme.hint_style(),
        ),
        RunStatus::Running => Span::styled(" Running…", theme.spinner_style()),
        RunStatus::Completed => Span::styled(" ✓ Completed", theme.active_context_indicator_style()),
        RunStatus::Failed => Span::styled(" ✗ Failed", theme.error_style()),
    };

    frame.render_widget(
        Paragraph::new(Line::from(status_span)).style(theme.status_bar_style()),
        area,
    );
}