use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{AppState, Modal};
use crate::ui::theme::Theme;

/* ============================================================================================== */
/// Controls where a modal is positioned on screen.
pub enum ModalPosition {
    Center,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    /// Place the modal's top-left corner at a specific coordinate.
    At { x: u16, y: u16 },
}

/* ============================================================================================== */
/// Renders a centered modal box on top of the existing content.
/// 
/// The caller is responsible for rendering the specific modal content within
/// the inner area returned by this function.
pub fn render_modal_frame(
    frame: &mut Frame,
    title: &str,
    footer: Option<&str>,
    position: ModalPosition,
    width_pct: u16,
    height: u16,
    theme: &Theme,
    border_style: ratatui::style::Style,
) -> Rect {
    let area = frame.area();

    let modal_width = (area.width as u32 * width_pct as u32 / 100).min(area.width as u32) as u16;
    let modal_height = height.min(area.height - 4);

    let (x, y) = match position {
        ModalPosition::Center => (
            area.x + (area.width.saturating_sub(modal_width)) / 2,
            area.y + (area.width.saturating_sub(modal_height)) / 2,
        ),
        ModalPosition::TopLeft => (
            area.x + 1,
            area.y + 1,
        ),
        ModalPosition::TopRight => (
            area.x + area.width.saturating_sub(modal_width + 1),
            area.y + 1,
        ),
        ModalPosition::BottomLeft => (
            area.x + 1,
            area.y + (area.width.saturating_sub(modal_height + 1)),
        ),
        ModalPosition::BottomRight => (
            area.x + area.width.saturating_sub(modal_width + 1),
            area.y + (area.width.saturating_sub(modal_height + 1)),
        ),
        ModalPosition::At { x, y } => (x, y),
    };

    let modal_area = Rect::new(x, y, modal_width, modal_height);

    // Clear the background behind the modal.
    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .style(theme.surface_style());

    let inner = block.inner(modal_area);

    if let Some(footer_text) = footer {
        // Reserve one line at the bottom for footer.
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(inner);

        let footer_para = Paragraph::new(footer_text)
            .style(theme.hint_style())
            .alignment(Alignment::Center);
        frame.render_widget(footer_para, layout[1]);

        frame.render_widget(block, modal_area);
        layout[0]
    } else {
        frame.render_widget(block, modal_area);
        inner
    }
}

/* ============================================================================================== */
/// Renders the generic error detail modal.
pub fn render_error_detail(frame: &mut Frame, state: &AppState, theme: &Theme) {
    let error = match &state.modal {
        Some(Modal::ErrorDetail(e)) => e.clone(),
        _ => return,
    };

    let inner = render_modal_frame(
        frame,
        &format!("⚠  {}", error.kind_label()),
        Some("Esc: close"),
        ModalPosition::Center,
        65,
        14,
        theme,
        theme.error_border_style(),
    );

    let mut lines: Vec<Line> = vec![
        Line::from(""),
        Line::from(Span::styled(&error.message, theme.surface_style().fg(theme.text))),
        Line::from(""),
    ];

    if let Some(detail) = &error.source_detail {
        lines.push(Line::from(Span::styled("Detail:", theme.heading_style())));
        for detail_line in detail.lines().take(4) {
            lines.push(Line::from(Span::styled(
                detail_line.to_string(),
                theme.surface_style().fg(theme.subtle),
            )));
        }
        lines.push(Line::from(""));
    }

    if let Some(recovery) = &error.recovery {
        let hint = match recovery {
            crate::errors::RecoveryAction::ReLogin => "Action: Re-login".to_string(),
            crate::errors::RecoveryAction::LoginToTenant(tid) => {
                format!("Action: Login to tenant {}", tid)
            }
            crate::errors::RecoveryAction::Retry(_) => "Action: Retry the operation".to_string(),
            crate::errors::RecoveryAction::OpenSettings => "Action: Open settings".to_string(),
            crate::errors::RecoveryAction::Manual(hint) => hint.clone(),
        };
        lines.push(Line::from(Span::styled(
            format!("Suggested: {}", hint),
            theme.surface_style().fg(theme.amber),
        )));
    }

    let para = Paragraph::new(lines)
        .style(theme.surface_style())
        .wrap(ratatui::widgets::Wrap { trim: true });

    frame.render_widget(para, inner);
}

/* ============================================================================================== */
/// Renders the generic confirm dialog.
pub fn render_confirm(frame: &mut Frame, state: &AppState, theme: &Theme) {
    let (message, _on_confirm) = match &state.modal {
        Some(Modal::Confirm { message, on_confirm }) => (message.clone(), on_confirm.clone()),
        _ => return,
    };

    let inner = render_modal_frame(
        frame,
        "Confirm",
        Some("Enter: confirm   Esc: cancel"),
        ModalPosition::Center,
        50,
        10,
        theme,
        theme.confirm_border_style(),
    );

    let lines: Vec<Line> = vec![
        Line::from(""),
        Line::from(Span::styled(&message, theme.surface_style().fg(theme.text))),
        Line::from(""),
        Line::from(Span::styled(
            "This will run az login --tenant and update your active CLI context.",
            theme.surface_style().fg(theme.subtle),
        )),
    ];

    let para = Paragraph::new(lines)
        .style(theme.surface_style())
        .wrap(ratatui::widgets::Wrap { trim: true });

    frame.render_widget(para, inner);
}