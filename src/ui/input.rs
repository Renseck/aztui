use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{AppState, Modal, View};
use crate::command::Command;
use crate::ui::widgets::{context_switcher, quick_switch};

/* ============================================================================================== */
/// Maps a key event to a [`Command`], taking application state into account.
/// Returns `None` for unhandled keys.
pub fn handle_input(key: KeyEvent, state: &AppState) -> Option<Command> {
    // Locked: only allow password input (Phase 2) or quit.
    if state.locked {
        return match key.code {
            KeyCode::Char('q') => Some(Command::Quit),
            _ => None,
        };
    }

    // Modal-specific input.
    if let Some(modal) = &state.modal {
        return handle_modal_input(key, modal, state);
    }

    // Search mode: printable chars feed the search query.
    if state.search_focused {
        return handle_search_input(key, state);
    }

    // Normal navigation.
    handle_normal_input(key, state)
}

/* ============================================================================================== */
/*                                     Private input handlers                                     */
/* ============================================================================================== */

fn handle_normal_input(key: KeyEvent, state: &AppState) -> Option<Command> {
    match (key.modifiers, key.code) {
        // Quit
        (KeyModifiers::NONE, KeyCode::Char('q')) => Some(Command::Quit),

        // Navigation
        (KeyModifiers::NONE, KeyCode::Up | KeyCode::Char('k')) => Some(Command::NavUp),
        (KeyModifiers::NONE, KeyCode::Down | KeyCode::Char('j')) => Some(Command::NavDown),

        // Select / confirm
        (KeyModifiers::NONE, KeyCode::Enter) => {
            if let Some(ctx) = context_switcher::selected_context(state) {
                Some(Command::SwitchContext(ctx))
            } else {
                None
            }
        }

        // Focus search
        (KeyModifiers::NONE, KeyCode::Char('/')) => Some(Command::UpdateSearch(String::new())),

        // Quick switch
        (KeyModifiers::CONTROL, KeyCode::Char('p')) => {
            let filtered = quick_switch::build_filtered(state, "");
            Some(Command::OpenModal(Box::new(Modal::QuickSwitch { 
                query: String::new(), 
                filtered, 
                cursor: 0 
            })))
        }

        // Refresh
        (KeyModifiers::NONE, KeyCode::Char('r')) => Some(Command::RefreshContextList),

        // Help
        (KeyModifiers::NONE, KeyCode::Char('?')) => {
            if state.active_view == View::Help {
                Some(Command::NavigateTo(View::ContextSwitcher))
            } else {
                Some(Command::NavigateTo(View::Help))
            }
        }

        // View shortcuts
        (KeyModifiers::NONE, KeyCode::Char('1')) => {
            Some(Command::NavigateTo(View::ContextSwitcher))
        }
        (KeyModifiers::NONE, KeyCode::Char('2')) => {
            Some(Command::NavigateTo(View::ResourceBrowser))
        }
        (KeyModifiers::NONE, KeyCode::Char('3')) => Some(Command::NavigateTo(View::CostExplorer)),

        _ => None
    }
}

/* ============================================================================================== */
fn handle_search_input(key: KeyEvent, state: &AppState) -> Option<Command> {
    match key.code {
        // Exit search mode
        KeyCode::Esc => {
            // Clear query and unfocus — handled in dispatch by UpdateSearch("") then we need
            // to signal search unfocused. We use an empty UpdateSearch and rely on dispatch
            // to clear search_focused when query is cleared via Esc.
            Some(Command::UpdateSearch(String::from("\x1B"))) // sentinel for Esc in search
        }

        // Confirm selection from filtered results.s
        KeyCode::Enter => {
            if let Some(ctx) = context_switcher::selected_context(state) {
                Some(Command::SwitchContext(ctx))
            } else {
                None
            }
        }

        // Remove characters from search bar
        KeyCode::Backspace => {
            let mut q = state.search_query.clone();
            q.pop();
            Some(Command::UpdateSearch(q))
        }

        // Add characters to search bar
        KeyCode::Char(c) => {
            let mut q = state.search_query.clone();
            q.push(c);
            Some(Command::UpdateSearch(q))
        }

        _ => None,
    }
}

/* ============================================================================================== */
fn handle_modal_input(key: KeyEvent, modal: &Modal, state: &AppState) -> Option<Command> {
    match modal {
        Modal::QuickSwitch { query, filtered, cursor } => {
            handle_quick_switch_input(key, query, filtered, *cursor, state)
        }
        Modal::Confirm { on_confirm, .. } => match key.code {
            KeyCode::Enter => Some(*on_confirm.clone()),
            KeyCode::Esc => Some(Command::CloseModal),
            _ => None,
        },
        Modal::ErrorDetail(_) => match key.code {
            KeyCode::Esc | KeyCode::Enter => Some(Command::CloseModal),
            _ => None,
        },
        Modal::PasswordPrompt => match key.code {
            KeyCode::Char('q') => Some(Command::Quit),
            KeyCode::Esc => Some(Command::CloseModal),
            _ => None,
        },
    }
}

/* ============================================================================================== */
fn handle_quick_switch_input(
    key: KeyEvent,
    query: &str,
    filtered: &[crate::domain::models::AzureContext],
    cursor: usize,
    state: &AppState,
) -> Option<Command> {
    match key.code {
        KeyCode::Esc => Some(Command::CloseModal),

        KeyCode::Enter => {
            if let Some(ctx) = quick_switch::selected_context(state) {
                Some(Command::SwitchContext(ctx))
            } else { 
                None
            }
        }

        KeyCode::Up | KeyCode::Char('k') => Some(Command::NavUp),
        KeyCode::Down | KeyCode::Char('j') => Some(Command::NavDown),

        KeyCode::Backspace => {
            let mut q = query.to_string();
            q.pop();
            let new_filtered = quick_switch::build_filtered(state, &q);
            let new_cursor = cursor.min(new_filtered.len().saturating_sub(1));
            Some(Command::OpenModal(Box::new(Modal::QuickSwitch { 
                query: q, 
                filtered: new_filtered, 
                cursor: new_cursor 
            })))
        }

        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            let mut q = query.to_string();
            q.push(c);
            let new_filtered = quick_switch::build_filtered(state, query);
            Some(Command::OpenModal(Box::new(Modal::QuickSwitch { 
                query: q, 
                filtered: new_filtered, 
                cursor: 0 
            })))
        }

        _ => None,
    }
}