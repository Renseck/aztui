//! Command palette overlay. Renders a [`PaletteState`]; all row building
//! happens in `crate::palette`.

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, Paragraph};
use ratatui::Frame;

use crate::actions::{self, ActionId};
use crate::app::AppState;
use crate::palette::{action_label, subscription_name, PaletteMode, PaletteRow, PaletteState};
use crate::ui::fuzzy::{fuzzy_match, highlight};
use crate::ui::theme::Theme;
use crate::ui::widgets::global_search::pad;
use crate::ui::widgets::modal::{render_modal_frame, ModalPosition};
use crate::ui::widgets::resource_browser::{abbreviate_resource_type, is_vm};
use crate::ui::widgets::SPINNER_CHARS;

const NAME_W: usize = 28;
const TYPE_W: usize = 14;
const RG_W: usize = 22;

/* ============================================================================================== */
/// Renders the palette overlay: query line, divider, sectioned rows, footer.
pub fn render(frame: &mut Frame, state: &AppState, palette: &PaletteState, theme: &Theme) {
    let title = match &palette.mode {
        PaletteMode::All => "Command palette".to_string(),
        PaletteMode::ContextsOnly => "Switch context".to_string(),
        PaletteMode::TargetActions(t) => format!("{} ›", t.name()),
    };
    let footer = match palette.mode {
        PaletteMode::All => "↵ run · Tab actions · Esc close",
        PaletteMode::ContextsOnly => "↵ switch · Esc cancel",
        PaletteMode::TargetActions(_) => "↵ run · Esc back",
    };
    let max_h = frame.area().height.saturating_sub(4).max(1);
    let height = (palette.rows.len() as u16 + 6).max(10).min(max_h);

    let inner = render_modal_frame(
        frame,
        &title,
        Some(footer),
        ModalPosition::Center,
        70,
        height,
        theme,
        theme.modal_border_style(),
    );

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    let search_line = Line::from(vec![
        Span::styled("  > ", theme.hint_style()),
        Span::styled(
            palette.query.clone(),
            theme.surface_style().fg(theme.bright).add_modifier(Modifier::BOLD),
        ),
        Span::styled("_", theme.surface_style().fg(theme.azure_light)),
    ]);
    frame.render_widget(Paragraph::new(search_line).style(theme.surface_style()), layout[0]);
    frame.render_widget(
        Paragraph::new("─".repeat(inner.width as usize)).style(theme.surface_style().fg(theme.muted)),
        layout[1],
    );

    let width = layout[2].width as usize;
    let default = match &palette.mode {
        PaletteMode::TargetActions(t) => actions::default_action(t),
        _ => None,
    };

    let mut selectable = 0usize;
    let mut selected_flat = None;
    let items: Vec<ListItem> = palette
        .rows
        .iter()
        .enumerate()
        .map(|(flat, row)| {
            let is_selected = row.is_selectable() && selectable == palette.cursor;
            if row.is_selectable() {
                if is_selected {
                    selected_flat = Some(flat);
                }
                selectable += 1;
            }
            ListItem::new(render_row(row, is_selected, default, palette, state, width, theme))
        })
        .collect();

    let list = List::new(items).style(theme.surface_style()).scroll_padding(1);
    let mut list_state = state.scroll.palette.borrow_mut();
    list_state.select(selected_flat);
    frame.render_stateful_widget(list, layout[2], &mut list_state);
}

/* ============================================================================================== */
/*                                         Private helpers                                        */
/* ============================================================================================== */

fn render_row(
    row: &PaletteRow,
    is_selected: bool,
    default: Option<ActionId>,
    palette: &PaletteState,
    state: &AppState,
    width: usize,
    theme: &Theme,
) -> Line<'static> {
    let base = if is_selected { theme.selected_style() } else { theme.surface_style().fg(theme.text) };
    let prefix = if is_selected { " » " } else { "   " };

    match row {
        PaletteRow::Header(title) => Line::from(Span::styled(
            format!("  {}", title),
            theme.hint_style().add_modifier(Modifier::BOLD),
        )),
        PaletteRow::Info(text) => {
            let spinner = if text.starts_with("Loading") {
                format!("{} ", SPINNER_CHARS[state.spinner_frame as usize % SPINNER_CHARS.len()])
            } else {
                String::new()
            };
            Line::from(Span::styled(format!("   {}{}", spinner, text), theme.hint_style()))
        }
        PaletteRow::Action(id, target) => {
            let label = action_label(*id, target);
            let key = if Some(*id) == default {
                "↵".to_string()
            } else {
                id.spec().key.map(|k| k.display.to_string()).unwrap_or_default()
            };
            let used = prefix.chars().count() + label.chars().count() + key.chars().count() + 1;
            let mut spans = vec![Span::styled(prefix.to_string(), base)];
            spans.extend(highlighted(&label, &palette.query, base, theme));
            spans.push(Span::styled(" ".repeat(width.saturating_sub(used)), base));
            spans.push(Span::styled(key, theme.surface_style().fg(theme.azure_light)));
            Line::from(spans)
        }
        PaletteRow::Context(ctx) => {
            let mut spans = vec![Span::styled(prefix.to_string(), base)];
            spans.extend(highlighted(&ctx.label(), &palette.query, base, theme));
            Line::from(spans)
        }
        PaletteRow::Resource(idx) => {
            let Some(r) = state.global_resources.get(*idx) else {
                return Line::from("");
            };
            let name_style = if !is_selected && is_vm(&r.resource_type) { theme.vm_type_style() } else { base };
            let mut spans = vec![Span::styled(prefix.to_string(), name_style)];
            spans.extend(highlighted(&r.name, &palette.query, name_style, theme));
            let name_len = r.name.chars().count();
            if name_len < NAME_W {
                spans.push(Span::styled(" ".repeat(NAME_W - name_len), name_style));
            }
            spans.push(Span::styled(
                pad(abbreviate_resource_type(&r.resource_type), TYPE_W),
                theme.surface_style().fg(theme.azure_light),
            ));
            spans.push(Span::styled(pad(&r.resource_group, RG_W), theme.surface_style().fg(theme.text)));
            spans.push(Span::styled(
                subscription_name(state, &r.subscription_id),
                theme.surface_style().fg(theme.subtle),
            ));
            Line::from(spans)
        }
    }
}

/* ============================================================================================== */
fn highlighted(text: &str, query: &str, base: Style, theme: &Theme) -> Vec<Span<'static>> {
    let indices = fuzzy_match(text, query).map(|(_, idx)| idx).unwrap_or_default();
    highlight(text, &indices, base, theme.match_style())
}

/* ============================================================================================== */
/*                                              Tests                                             */
/* ============================================================================================== */

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::*;
    use crate::palette::{build_palette_rows, resource_haystacks};
    use crate::test_support::{global, state_with_contexts, VM_TYPE};

    fn screen_text(state: &AppState, palette: &PaletteState) -> String {
        let theme = Theme::default_dark();
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).expect("test terminal");
        terminal.draw(|f| render(f, state, palette, &theme)).expect("draw");
        terminal.backend().buffer().content().iter().map(|c| c.symbol()).collect()
    }

    #[test]
    fn renders_title_sections_and_resource_row() {
        let mut state = state_with_contexts(Some("sub-a"));
        state.global_resources = vec![global("web-01", VM_TYPE, "sub-a")];
        state.resource_haystacks = resource_haystacks(&state);
        let mut palette = PaletteState::new(&state, PaletteMode::All);
        palette.query = "web-01".into();
        palette.rows = build_palette_rows(&state, &palette.mode, &palette.query, &palette.opened_target);

        let text = screen_text(&state, &palette);
        assert!(text.contains("Command palette"));
        assert!(text.contains("RESOURCES"));
        assert!(text.contains("web-01"));
        assert!(text.contains("sub-a-name"));
    }

    #[test]
    fn contexts_only_uses_switch_title() {
        let state = state_with_contexts(Some("sub-a"));
        let palette = PaletteState::new(&state, PaletteMode::ContextsOnly);
        let text = screen_text(&state, &palette);
        assert!(text.contains("Switch context"));
        assert!(text.contains("ALL MATCHES"));
    }
}
