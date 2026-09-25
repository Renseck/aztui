use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::actions;
use crate::app::{AppState, CostGrouping, CostView, Modal, PasswordMode, Pane, RunPane, View};
use crate::command::Command;
use crate::palette::{PaletteMode, PaletteState};
use crate::ui::widgets::{activity_log, cost_explorer, resource_browser};

/* ============================================================================================== */

/// Result of a view's mechanics handler: the key was consumed (possibly
/// producing a command), or it passes through to the action registry.
enum Flow {
    Done(Option<Command>),
    Pass,
}

/* ============================================================================================== */
/// Maps a key event to a [`Command`], taking application state into account.
/// Order: lock → modal → text entry → view mechanics → action registry.
pub fn handle_input(key: KeyEvent, state: &AppState) -> Option<Command> {
    // Locked: only allow password input or quit.
    if state.locked {
        if let Some(Modal::PasswordPrompt { .. }) = &state.modal {
            return handle_password_input(key, state);
        }
        return match key.code {
            KeyCode::Char('q') => Some(Command::Quit),
            _ => None,
        };
    }

    if let Some(modal) = &state.modal {
        return handle_modal_input(key, modal, state);
    }

    // Text entry.
    if state.search_focused {
        return match state.active_view {
            View::ResourceBrowser => handle_resource_search_input(key, state),
            View::GlobalSearch => handle_global_search_text_input(key, state),
            _ => handle_search_input(key, state),
        };
    }
    if state.active_view == View::ActivityLog
        && state.activity.as_ref().map_or(false, |a| a.search_focused)
    {
        return handle_activity_search_input(key, state);
    }

    let flow = match state.active_view {
        View::ContextSwitcher => context_switcher_mechanics(key, state),
        View::ResourceBrowser => resource_browser_mechanics(key, state),
        View::CostExplorer => cost_explorer_mechanics(key, state),
        View::RunCommand => run_command_mechanics(key, state),
        View::ActivityLog => activity_log_mechanics(key, state),
        View::GlobalSearch => global_search_mechanics(key, state),
        View::Help => help_mechanics(key, state),
    };

    match flow {
        Flow::Done(cmd) => cmd,
        Flow::Pass => actions::resolve_key(key, state),
    }
}

/* ============================================================================================== */
/*                                         View mechanics                                         */
/* ============================================================================================== */

fn context_switcher_mechanics(key: KeyEvent, state: &AppState) -> Flow {
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Up | KeyCode::Char('k')) => Flow::Done(Some(Command::NavUp)),
        (KeyModifiers::NONE, KeyCode::Down | KeyCode::Char('j')) => Flow::Done(Some(Command::NavDown)),
        (KeyModifiers::NONE, KeyCode::Enter) => Flow::Done(actions::default_command(state)),
        _ => Flow::Pass,
    }
}

/* ============================================================================================== */
fn help_mechanics(key: KeyEvent, state: &AppState) -> Flow {
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Esc) => {
            Flow::Done(Some(Command::NavigateTo(state.previous_view.clone())))
        }
        _ => Flow::Pass,
    }
}

/* ============================================================================================== */
fn resource_browser_mechanics(key: KeyEvent, state: &AppState) -> Flow {
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Tab | KeyCode::Right | KeyCode::Left) => {
            Flow::Done(Some(Command::ToggleResourcePane))
        }
        (KeyModifiers::NONE, KeyCode::Up | KeyCode::Char('k')) => Flow::Done(Some(Command::NavUp)),
        (KeyModifiers::NONE, KeyCode::Down | KeyCode::Char('j')) => Flow::Done(Some(Command::NavDown)),
        (KeyModifiers::NONE, KeyCode::Enter) => Flow::Done(match state.resource_browser_focus {
            Pane::Left => resource_browser::selected_resource_group_name(state).map(Command::ListResources),
            Pane::Right => actions::default_command(state),
        }),
        (KeyModifiers::NONE, KeyCode::Esc) => Flow::Done(Some(Command::NavigateTo(View::ContextSwitcher))),
        _ => Flow::Pass,
    }
}

/* ============================================================================================== */
fn cost_explorer_mechanics(key: KeyEvent, state: &AppState) -> Flow {
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Up | KeyCode::Char('k')) => Flow::Done(Some(Command::NavUp)),
        (KeyModifiers::NONE, KeyCode::Down | KeyCode::Char('j')) => Flow::Done(Some(Command::NavDown)),
        (KeyModifiers::NONE, KeyCode::Enter) => Flow::Done(match state.cost_view {
            CostView::Subscription(CostGrouping::ByResourceGroup) => {
                cost_explorer::selected_row_label(state).map(Command::DrillIntoResourceGroup)
            }
            _ => None,
        }),
        (KeyModifiers::NONE, KeyCode::Backspace) => Flow::Done(Some(Command::CostScopeUp)),
        (KeyModifiers::NONE, KeyCode::Esc) => Flow::Done(Some(match state.cost_view {
            CostView::ResourceGroup(_) => Command::CostScopeUp,
            _ => Command::NavigateTo(View::ContextSwitcher),
        })),
        _ => Flow::Pass,
    }
}

/* ============================================================================================== */
fn run_command_mechanics(key: KeyEvent, state: &AppState) -> Flow {
    let session = match state.run_command.as_ref() {
        Some(s) => s,
        None => return Flow::Pass,
    };

    match (key.modifiers, key.code) {
        // F5 is the registry's RunScript action, even while typing in the editor.
        (_, KeyCode::F(5)) => Flow::Pass,
        (KeyModifiers::NONE, KeyCode::Esc) => Flow::Done(Some(Command::NavigateTo(View::ResourceBrowser))),
        (KeyModifiers::NONE, KeyCode::Tab) => Flow::Done(Some(Command::ToggleRunPane)),
        (KeyModifiers::NONE, KeyCode::Up | KeyCode::Char('k')) if session.focus == RunPane::Output => {
            Flow::Done(Some(Command::ScrollRunOutput(-1)))
        }
        (KeyModifiers::NONE, KeyCode::Down | KeyCode::Char('j')) if session.focus == RunPane::Output => {
            Flow::Done(Some(Command::ScrollRunOutput(1)))
        }
        _ if session.focus == RunPane::Editor => Flow::Done(Some(Command::ScriptInput(key))),
        _ => Flow::Pass,
    }
}

/* ============================================================================================== */
fn activity_log_mechanics(key: KeyEvent, state: &AppState) -> Flow {
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Up | KeyCode::Char('k')) => Flow::Done(Some(Command::NavUp)),
        (KeyModifiers::NONE, KeyCode::Down | KeyCode::Char('j')) => Flow::Done(Some(Command::NavDown)),
        (KeyModifiers::NONE, KeyCode::Enter) => Flow::Done(
            activity_log::selected_entry(state)
                .map(|e| Command::OpenModal(Box::new(Modal::ActivityDetail(Box::new(e))))),
        ),
        (KeyModifiers::NONE, KeyCode::Esc) => Flow::Done(Some(Command::NavigateTo(View::ContextSwitcher))),
        _ => Flow::Pass,
    }
}

/* ============================================================================================== */
fn global_search_mechanics(key: KeyEvent, state: &AppState) -> Flow {
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Up | KeyCode::Char('k')) => Flow::Done(Some(Command::NavUp)),
        (KeyModifiers::NONE, KeyCode::Down | KeyCode::Char('j')) => Flow::Done(Some(Command::NavDown)),
        (KeyModifiers::NONE, KeyCode::Enter) => Flow::Done(actions::default_command(state)),
        // Esc clears an active filter first; with no filter, it backs out to the context switcher.
        (KeyModifiers::NONE, KeyCode::Esc) => Flow::Done(Some(if state.global_search_query.is_empty() {
            Command::NavigateTo(View::ContextSwitcher)
        } else {
            Command::UpdateGlobalSearch(String::new())
        })),
        _ => Flow::Pass,
    }
}

/* ============================================================================================== */
/*                                           Text entry                                           */
/* ============================================================================================== */

fn handle_search_input(key: KeyEvent, state: &AppState) -> Option<Command> {
    match key.code {
        // Sentinel: the main loop clears and unfocuses the search on this value.
        KeyCode::Esc => Some(Command::UpdateSearch(String::from("\x1B"))),
        KeyCode::Enter => actions::default_command(state),
        KeyCode::Backspace => {
            let mut q = state.search_query.clone();
            q.pop();
            Some(Command::UpdateSearch(q))
        }
        KeyCode::Char(c) => {
            let mut q = state.search_query.clone();
            q.push(c);
            Some(Command::UpdateSearch(q))
        }
        _ => None,
    }
}

/* ============================================================================================== */
fn handle_resource_search_input(key: KeyEvent, state: &AppState) -> Option<Command> {
    match key.code {
        KeyCode::Esc => Some(Command::UpdateSearch(String::from("\x1B"))),
        KeyCode::Backspace => {
            let mut q = state.resource_search_query.clone();
            q.pop();
            Some(Command::UpdateResourceSearch(q))
        }
        KeyCode::Char(c) => {
            let mut q = state.resource_search_query.clone();
            q.push(c);
            Some(Command::UpdateResourceSearch(q))
        }
        _ => None,
    }
}

/* ============================================================================================== */
/// Global-search text entry. Esc/Enter commit the filter and drop back to list
/// navigation (the query stays applied); arrows move the selection while still
/// in the search box; `/` is ignored so it never types a literal slash.
fn handle_global_search_text_input(key: KeyEvent, state: &AppState) -> Option<Command> {
    match key.code {
        KeyCode::Esc | KeyCode::Enter => Some(Command::SetGlobalSearchFocus(false)),
        KeyCode::Up => Some(Command::NavUp),
        KeyCode::Down => Some(Command::NavDown),
        KeyCode::Char('/') => None,
        KeyCode::Backspace => {
            let mut q = state.global_search_query.clone();
            q.pop();
            Some(Command::UpdateGlobalSearch(q))
        }
        KeyCode::Char(c) => {
            let mut q = state.global_search_query.clone();
            q.push(c);
            Some(Command::UpdateGlobalSearch(q))
        }
        _ => None,
    }
}

/* ============================================================================================== */
fn handle_activity_search_input(key: KeyEvent, state: &AppState) -> Option<Command> {
    let activity = state.activity.as_ref()?;
    match key.code {
        KeyCode::Esc | KeyCode::Enter => Some(Command::SetActivitySearchFocus(false)),
        KeyCode::Backspace => {
            let mut q = activity.search.clone();
            q.pop();
            Some(Command::UpdateActivitySearch(q))
        }
        KeyCode::Char(c) => {
            let mut q = activity.search.clone();
            q.push(c);
            Some(Command::UpdateActivitySearch(q))
        }
        _ => None,
    }
}

/* ============================================================================================== */
/*                                         Modal handlers                                         */
/* ============================================================================================== */


fn handle_modal_input(key: KeyEvent, modal: &Modal, state: &AppState) -> Option<Command> {
    match modal {
        Modal::Palette(p) => handle_palette_input(key, p),
        Modal::Confirm { on_confirm, .. } => match key.code {
            KeyCode::Enter => Some(*on_confirm.clone()),
            KeyCode::Esc => Some(Command::CloseModal),
            _ => None,
        },
        Modal::ErrorDetail(_) => match key.code {
            KeyCode::Esc | KeyCode::Enter => Some(Command::CloseModal),
            _ => None,
        },
        Modal::ActivityDetail(_) => match key.code {
            KeyCode::Esc | KeyCode::Enter => Some(Command::CloseModal),
            _ => None,
        },
        Modal::PasswordPrompt { .. } => handle_password_input(key, state),
    }
}

/* ============================================================================================== */
fn handle_palette_input(key: KeyEvent, p: &PaletteState) -> Option<Command> {
    let in_target = matches!(p.mode, PaletteMode::TargetActions(_));
    match key.code {
        KeyCode::Esc => Some(if in_target { Command::PaletteBack } else { Command::CloseModal }),
        KeyCode::Enter => Some(Command::PaletteActivate),
        KeyCode::Tab | KeyCode::Right => Some(Command::PaletteDrill),
        KeyCode::Up => Some(Command::NavUp),
        KeyCode::Down => Some(Command::NavDown),
        KeyCode::Backspace => {
            if p.query.is_empty() {
                in_target.then_some(Command::PaletteBack)
            } else {
                let mut q = p.query.clone();
                q.pop();
                Some(Command::PaletteQuery(q))
            }
        }
        // j/k are printable here: the palette is a text box first.
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            let mut q = p.query.clone();
            q.push(c);
            Some(Command::PaletteQuery(q))
        }
        _ => None,
    }
}

/* ============================================================================================== */
fn handle_password_input(key: KeyEvent, state: &AppState) -> Option<Command> {
    let (input, mode) = match &state.modal {
        Some(Modal::PasswordPrompt { input, mode, .. }) => (input.clone(), mode.clone()),
        _ => return None,
    };

    match key.code {
        KeyCode::Char('q') if input.is_empty() => Some(Command::Quit),

        KeyCode::Esc => {
            // In unlock mode, Esc does nothing (must enter password or quit).
            // In setup mode, Esc goes back to the first entry step.
            match &mode {
                PasswordMode::SetupConfirm { .. } => {
                    Some(Command::OpenModal(Box::new(Modal::PasswordPrompt {
                        input: String::new(),
                        error: None,
                        mode: PasswordMode::Setup,
                    })))
                }
                _ => None,
            }
        }

        KeyCode::Enter => {
            if input.is_empty() {
                return None;
            }
            match &mode {
                PasswordMode::Unlock => Some(Command::Unlock(input)),
                PasswordMode::Setup => {
                    // Move to confirmation step.
                    Some(Command::OpenModal(Box::new(Modal::PasswordPrompt {
                        input: String::new(),
                        error: None,
                        mode: PasswordMode::SetupConfirm {
                            first_password: input,
                        },
                    })))
                }
                PasswordMode::SetupConfirm { first_password } => {
                    if input == *first_password {
                        Some(Command::SetupPassword(input))
                    } else {
                        Some(Command::OpenModal(Box::new(Modal::PasswordPrompt {
                            input: String::new(),
                            error: Some("Passwords do not match. Try again.".into()),
                            mode: PasswordMode::Setup,
                        })))
                    }
                }
            }
        }

        KeyCode::Backspace => {
            let mut new_input = input;
            new_input.pop();
            Some(Command::OpenModal(Box::new(Modal::PasswordPrompt {
                input: new_input,
                error: None,
                mode,
            })))
        }

        KeyCode::Char(c) => {
            let mut new_input = input;
            new_input.push(c);
            Some(Command::OpenModal(Box::new(Modal::PasswordPrompt {
                input: new_input,
                error: None,
                mode,
            })))
        }

        _ => None,
    }
}


/* ============================================================================================== */
/*                                              Tests                                             */
/* ============================================================================================== */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::RunCommandSession;
    use crate::test_support::{global, key, key_mod, state_with_contexts, STORAGE_TYPE, VM_TYPE};

    #[test]
    fn ctrl_g_works_in_context_switcher_both_cases() {
        let s = state_with_contexts(Some("sub-a"));
        for c in ['g', 'G'] {
            let cmd = handle_input(key_mod(KeyCode::Char(c), KeyModifiers::CONTROL), &s);
            assert!(
                matches!(cmd, Some(Command::OpenPalette(PaletteMode::ContextsOnly))),
                "Ctrl+{c}"
            );
        }
    }

    #[test]
    fn digit_navigates_from_every_list_view() {
        for view in [View::ResourceBrowser, View::CostExplorer, View::ActivityLog, View::GlobalSearch] {
            let mut s = state_with_contexts(Some("sub-a"));
            s.active_view = view.clone();
            assert!(
                matches!(handle_input(key(KeyCode::Char('1')), &s), Some(Command::NavigateTo(View::ContextSwitcher))),
                "{:?}",
                view
            );
        }
    }

    #[test]
    fn question_mark_with_shift_opens_help() {
        let s = state_with_contexts(Some("sub-a"));
        let cmd = handle_input(key_mod(KeyCode::Char('?'), KeyModifiers::SHIFT), &s);
        assert!(matches!(cmd, Some(Command::NavigateTo(View::Help))));
    }

    #[test]
    fn esc_in_help_returns_to_previous_view() {
        let mut s = state_with_contexts(Some("sub-a"));
        s.active_view = View::Help;
        s.previous_view = View::CostExplorer;
        assert!(matches!(handle_input(key(KeyCode::Esc), &s), Some(Command::NavigateTo(View::CostExplorer))));
    }

    #[test]
    fn enter_in_global_search_routes_by_type() {
        let mut s = state_with_contexts(Some("sub-a"));
        s.active_view = View::GlobalSearch;
        s.global_resources = vec![global("web-01", VM_TYPE, "sub-a")];
        assert!(matches!(
            handle_input(key(KeyCode::Enter), &s),
            Some(Command::OpenRunCommand { ref vm_name, .. }) if vm_name == "web-01"
        ));
        s.global_resources = vec![global("st01", STORAGE_TYPE, "sub-b")];
        assert!(matches!(
            handle_input(key(KeyCode::Enter), &s),
            Some(Command::InContext { ref subscription_id, .. }) if subscription_id == "sub-b"
        ));
    }

    #[test]
    fn editor_focus_swallows_letters_but_f5_reaches_registry() {
        let mut s = state_with_contexts(Some("sub-a"));
        s.active_view = View::RunCommand;
        s.run_command = Some(RunCommandSession::new("sub-a".into(), "rg-app".into(), "vm-1".into()));
        assert!(matches!(handle_input(key(KeyCode::Char('q')), &s), Some(Command::ScriptInput(_))));
        // Empty script: RunScript does not apply.
        assert!(handle_input(key(KeyCode::F(5)), &s).is_none());
        if let Some(session) = s.run_command.as_mut() {
            session.editor.insert_str("Get-Date");
        }
        assert!(matches!(handle_input(key(KeyCode::F(5)), &s), Some(Command::OpenModal(_))));
    }

    #[test]
    fn cost_backspace_scopes_up() {
        let mut s = state_with_contexts(Some("sub-a"));
        s.active_view = View::CostExplorer;
        assert!(matches!(handle_input(key(KeyCode::Backspace), &s), Some(Command::CostScopeUp)));
    }

    #[test]
    fn palette_keys_map_to_palette_commands() {
        let mut s = state_with_contexts(Some("sub-a"));
        s.modal = Some(Modal::Palette(crate::palette::PaletteState::new(&s, crate::palette::PaletteMode::All)));
        assert!(matches!(handle_input(key(KeyCode::Char('j')), &s), Some(Command::PaletteQuery(q)) if q == "j"));
        assert!(matches!(handle_input(key(KeyCode::Tab), &s), Some(Command::PaletteDrill)));
        assert!(matches!(handle_input(key(KeyCode::Enter), &s), Some(Command::PaletteActivate)));
        assert!(matches!(handle_input(key(KeyCode::Esc), &s), Some(Command::CloseModal)));
        // Backspace on an empty query in All mode does nothing.
        assert!(handle_input(key(KeyCode::Backspace), &s).is_none());

        let vm = crate::actions::Target::from_global(&global("web-01", VM_TYPE, "sub-a"));
        s.modal = Some(Modal::Palette(crate::palette::PaletteState::new(
            &s,
            crate::palette::PaletteMode::TargetActions(vm),
        )));
        assert!(matches!(handle_input(key(KeyCode::Backspace), &s), Some(Command::PaletteBack)));
        assert!(matches!(handle_input(key(KeyCode::Esc), &s), Some(Command::PaletteBack)));
    }

}
