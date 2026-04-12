use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::app::AppState;
use crate::domain::models::CostSummary;
use crate::ui::theme::Theme;
use crate::ui::widgets::SPINNER_CHARS;

/* ============================================================================================== */
/// Renders the cost explorer view.
pub fn render(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    match &state.cost_summary {
        Some(summary) => render_summary(frame, area, state, summary, theme),
        None => {
            if state.active_context.is_none() {
                render_no_subscription(frame, area, theme);
            } else {
                render_loading(frame, area, state, theme);
            }
        }
    }
}

/* ============================================================================================== */
/*                                        Private renderers                                       */
/* ============================================================================================== */

fn render_summary(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    summary: &CostSummary,
    theme: &Theme,
) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // header (subscription, period, total)
            Constraint::Min(1),    // breakdown table
        ])
        .split(area);

    render_header(frame, layout[0], state, summary, theme);
    render_breakdown(frame, layout[1], state, summary, theme);
}

/* ============================================================================================== */
fn render_header(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    summary: &CostSummary,
    theme: &Theme,
) {
    let sub_name = state
        .active_context
        .as_ref()
        .map(|ctx| ctx.subscription.name.as_str())
        .unwrap_or("Unknown");

    let period_label = state.cost_period.label();
    let can_go_next = state.cost_period.next_month().is_some();

    let period_nav = format!(
        "  [ ◂ prev ]  {}  {}",
        period_label,
        if can_go_next { "[ next ▸ ]" } else { "" }
    );

    let total_line = format!(
        "  Total: {}",
        format_cost(summary.total, &summary.currency)
    );

    let lines = vec![
        Line::from(vec![
            Span::styled("  Subscription: ", theme.hint_style()),
            Span::styled(sub_name, theme.surface_style().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("  Period: ", theme.hint_style()),
            Span::styled(period_nav, theme.surface_style().fg(theme.azure_light)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            total_line,
            theme.heading_style(),
        )]),
    ];

    let para = Paragraph::new(lines).style(theme.base_style());
    frame.render_widget(para, area);
}

/* ============================================================================================== */
fn render_breakdown(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    summary: &CostSummary,
    theme: &Theme,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.content_focused_style())
        .style(theme.surface_style());

    let inner = block.inner(area);

    if summary.breakdown.is_empty() {
        let para = Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "  No cost data for this period.",
                theme.hint_style(),
            )]),
        ])
        .style(theme.surface_style());
        frame.render_widget(block, area);
        frame.render_widget(para, inner);
        return;
    }

    // Determine how many rows fit and whether we need an "Other" row.
    let available_rows = inner.height.saturating_sub(2) as usize; // header + bottom margin
    let (visible, other_count, other_amount) = if summary.breakdown.len() > available_rows && available_rows > 1 {
        let shown = available_rows - 1; // reserve one row for "Other"
        let other: Vec<_> = summary.breakdown[shown..].to_vec();
        let other_sum: f64 = other.iter().map(|item| item.amount).sum();
        (&summary.breakdown[..shown], other.len(), other_sum)
    } else {
        (summary.breakdown.as_slice(), 0, 0.0)
    };

    // Compute max service name length for alignment.
    let max_name_len = visible
        .iter()
        .map(|item| item.service_name.len())
        .max()
        .unwrap_or(0)
        .max(if other_count > 0 {
            format!("Other ({} services)", other_count).len()
        } else {
            0
        });

    // Build list items.
    let mut list_items: Vec<ListItem> = Vec::new();

    // Header row.
    let header_line = format!(
        "  {:<width$}  {:>12}  {:>10}  {:>6}",
        "Service",
        "Cost",
        "",
        "%",
        width = max_name_len,
    );
    list_items.push(ListItem::new(Line::from(Span::styled(
        header_line,
        theme.hint_style().add_modifier(Modifier::BOLD),
    ))));

    // Separator.
    let sep_width = max_name_len + 12 + 10 + 6 + 10;
    list_items.push(ListItem::new(Line::from(Span::styled(
        format!("  {}", "─".repeat(sep_width.min(inner.width as usize - 2))),
        theme.hint_style(),
    ))));

    for (idx, item) in visible.iter().enumerate() {
        let is_selected = idx == state.cost_selected_index;
        let line = build_cost_row(
            &item.service_name,
            item.amount,
            summary.total,
            &summary.currency,
            max_name_len,
            is_selected,
            theme,
        );
        list_items.push(ListItem::new(line));
    }

    // "Other" row.
    if other_count > 0 {
        let label = format!("Other ({} services)", other_count);
        let is_selected = visible.len() == state.cost_selected_index;
        let line = build_cost_row(
            &label,
            other_amount,
            summary.total,
            &summary.currency,
            max_name_len,
            is_selected,
            theme,
        );
        list_items.push(ListItem::new(line));
    }

    let mut list_state = ListState::default();
    // Offset by 2 for header + separator.
    list_state.select(Some(state.cost_selected_index + 2));

    let list = List::new(list_items)
        .style(theme.surface_style())
        .highlight_style(theme.selected_style());

    frame.render_widget(block, area);
    frame.render_stateful_widget(list, inner, &mut list_state);
}

/* ============================================================================================== */
fn build_cost_row<'a>(
    service_name: &str,
    amount: f64,
    total: f64,
    currency: &str,
    max_name_len: usize,
    is_selected: bool,
    theme: &Theme,
) -> Line<'a> {
    let prefix = if is_selected { "  » " } else { "    " };
    let pct = if total > 0.0 {
        (amount / total) * 100.0
    } else {
        0.0
    };
    let bar = render_bar(if total > 0.0 { amount / total } else { 0.0 }, 10);
    let cost_str = format_cost(amount, currency);
    let pct_str = format!("{:>5.1}%", pct);

    let row = format!(
        "{}{:<width$}  {:>12}  {}  {}",
        prefix,
        service_name,
        cost_str,
        bar,
        pct_str,
        width = max_name_len,
    );

    let style = if is_selected {
        theme.selected_style()
    } else {
        theme.surface_style().fg(theme.text)
    };

    Line::from(Span::styled(row, style))
}

/* ============================================================================================== */
fn render_loading(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let spinner_chars = SPINNER_CHARS;
    let spinner = spinner_chars[state.spinner_frame as usize % spinner_chars.len()];

    let loading_line = Line::from(vec![
        Span::styled(format!("{} ", spinner), theme.spinner_style()),
        Span::styled("Loading cost data…", theme.spinner_style()),
    ]);
    let cmd_line = Line::from(vec![Span::styled(
        "  Running: az rest (Cost Management Query API)",
        theme.hint_style(),
    )]);

    let para = Paragraph::new(vec![
        Line::from(""),
        Line::from(""),
        loading_line,
        Line::from(""),
        cmd_line,
    ])
    .style(theme.base_style())
    .alignment(ratatui::layout::Alignment::Center);

    frame.render_widget(para, area);
}

/* ============================================================================================== */
fn render_no_subscription(frame: &mut Frame, area: Rect, theme: &Theme) {
    let para = Paragraph::new(vec![
        Line::from(""),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Select a subscription first (press 1 to go to context switcher)",
            theme.hint_style(),
        )]),
    ])
    .style(theme.base_style())
    .alignment(ratatui::layout::Alignment::Center);

    frame.render_widget(para, area);
}

/* ============================================================================================== */
/*                                      Formatting utilities                                      */
/* ============================================================================================== */

/// Formats a cost amount with currency symbol and thousand separators.
fn format_cost(amount: f64, currency: &str) -> String {
    let symbol = match currency {
        "EUR" => "€",
        "USD" => "$",
        "GBP" => "£",
        "JPY" => "¥",
        "CHF" => "CHF ",
        other => other,
    };

    let formatted = format_with_thousands(amount);
    format!("{}{}", symbol, formatted)
}

/* ============================================================================================== */
/// Adds thousand separators to a float formatted to 2 decimal places.
fn format_with_thousands(amount: f64) -> String {
    let integer = amount.trunc() as i64;
    let decimal = ((amount.fract() * 100.0).round() as i64).abs();

    let int_str = integer.abs().to_string();
    let mut result = String::new();
    for (i, ch) in int_str.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    let int_formatted: String = result.chars().rev().collect();

    if integer < 0 {
        format!("-{}.{:02}", int_formatted, decimal)
    } else {
        format!("{}.{:02}", int_formatted, decimal)
    }
}

/* ============================================================================================== */
/// Renders a proportional bar chart: `"████████░░"`.
fn render_bar(fraction: f64, width: usize) -> String {
    let filled = (fraction * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

/* ============================================================================================== */
/// Returns the total number of selectable rows in the cost breakdown.
pub fn total_selectable(state: &AppState) -> usize {
    match &state.cost_summary {
        Some(summary) => {
            if summary.breakdown.is_empty() {
                0
            } else {
                summary.breakdown.len() // Will be clamped by the "Other" grouping in render
            }
        }
        None => 0,
    }
}
