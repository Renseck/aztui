use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{AppState, CostGrouping, CostView, Modal, PasswordMode, Pane, RunPane, View};
use crate::command::Command;
use crate::ui::widgets::{context_switcher, quick_switch, resource_browser};

/* ============================================================================================== */
/// Maps a key event to a [`Command`], taking application state into account.
/// Returns `None` for unhandled keys.
pub fn handle_input(key: KeyEvent, state: &AppState) -> Option<Command> {
    // Locked: only allow password input (Phase 2) or quit.
    if state.locked {
        if let Some(Modal::PasswordPrompt { .. }) = &state.modal {
            return handle_password_input(key, state);
        }
        return match key.code {
            KeyCode::Char('q') => Some(Command::Quit),
            _ => None,
        };
    }

    // Modal-specific input.
    if let Some(modal) = &state.modal {
        return handle_modal_input(key, modal, state);
    }

    // Search mode: route to the active view's search handler.
    if state.search_focused {
        return match state.active_view {
            View::ResourceBrowser => handle_resource_search_input(key, state),
            View::GlobalSearch => handle_global_search_input(key, state),
            _ => handle_search_input(key, state),
        };
    }

    // Normal navigation.
    handle_normal_input(key, state)
}

/* ============================================================================================== */
/*                                     Private input handlers                                     */
/* ============================================================================================== */

fn handle_normal_input(key: KeyEvent, state: &AppState) -> Option<Command> {
    // Resource browser has its own keybindings.
    if state.active_view == View::ResourceBrowser {
        return handle_resource_browser_input(key, state);
    }

    // Cost explorer has its own keybindings.
    if state.active_view == View::CostExplorer {
        return handle_cost_explorer_input(key, state);
    }

    // Run-command view has its own keybindings.
    if state.active_view == View::RunCommand {
        return handle_run_command_input(key, state);
    }

    // Activity log view has its own keybindings.
    if state.active_view == View::ActivityLog {
        return handle_activity_log_input(key, state);
    }

    // Global search view has its own keybindings.
    if state.active_view == View::GlobalSearch {
        return handle_global_search_input(key, state);
    }

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
        (KeyModifiers::CONTROL, KeyCode::Char('G')) => {
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
        (KeyModifiers::SHIFT, KeyCode::Char('?')) => {
            if state.active_view == View::Help {
                Some(Command::NavigateTo(state.previous_view.clone()))
            } else {
                Some(Command::NavigateTo(View::Help))
            }
        }

        // Esc closes the help screen back to where you were.
        (KeyModifiers::NONE, KeyCode::Esc) => {
            if state.active_view == View::Help {
                Some(Command::NavigateTo(state.previous_view.clone()))
            } else {
                None
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
        (KeyModifiers::NONE, KeyCode::Char('4')) => Some(Command::NavigateTo(View::ActivityLog)),
        (KeyModifiers::NONE, KeyCode::Char('5')) => Some(Command::NavigateTo(View::GlobalSearch)),

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
fn handle_resource_browser_input(key: KeyEvent, state: &AppState) -> Option<Command> {
    // Search mode within resource browser.
    if state.search_focused {
        return handle_resource_search_input(key, state);
    }

    match (key.modifiers, key.code) {
        // Quit
        (KeyModifiers::NONE, KeyCode::Char('q')) => Some(Command::Quit),

        // Pane switching
        (KeyModifiers::NONE, KeyCode::Tab)
        | (KeyModifiers::NONE, KeyCode::Right)
        | (KeyModifiers::NONE, KeyCode::Left) => Some(Command::ToggleResourcePane),

        // Navigation within focused pane
        (KeyModifiers::NONE, KeyCode::Up | KeyCode::Char('k')) => Some(Command::NavUp),
        (KeyModifiers::NONE, KeyCode::Down | KeyCode::Char('j')) => Some(Command::NavDown),

        // Enter: in left pane, load resources + focus right; in right pane, no-op for now
        (KeyModifiers::NONE, KeyCode::Enter) => {
            if state.resource_browser_focus == Pane::Left {
                if let Some(rg_name) = resource_browser::selected_resource_group_name(state) {
                    Some(Command::ListResources(rg_name))
                } else {
                    None
                }
            } else {
                resource_browser::selected_vm_target(state).map(|t| Command::OpenRunCommand {
                    subscription_id: t.subscription_id,
                    resource_group: t.resource_group,
                    vm_name: t.vm_name,
                })
            }
        }

        // Activity log for the selected resource / resource group
        (KeyModifiers::NONE, KeyCode::Char('a')) => {
            resource_browser::activity_scope_for_selection(state)
                .map(|scope| Command::OpenResourceActivity { scope })
        }

        // Cost for the selected resource group
        (KeyModifiers::NONE, KeyCode::Char('c')) => {
            resource_browser::selected_resource_group_name(state)
                .map(|rg| Command::OpenResourceGroupCost { resource_group: rg })
        }

        // Search
        (KeyModifiers::NONE, KeyCode::Char('/')) => Some(Command::UpdateSearch(String::new())),

        // Refresh
        (KeyModifiers::NONE, KeyCode::Char('r')) => Some(Command::ListResourceGroups),

        // Back to context switcher
        (KeyModifiers::NONE, KeyCode::Esc) => {
            Some(Command::NavigateTo(View::ContextSwitcher))
        }

        // Quick switch
        (KeyModifiers::CONTROL, KeyCode::Char('g')) => {
            let filtered = quick_switch::build_filtered(state, "");
            Some(Command::OpenModal(Box::new(Modal::QuickSwitch {
                query: String::new(),
                filtered,
                cursor: 0,
            })))
        }

        // Help
        (_, KeyCode::Char('?')) => Some(Command::NavigateTo(View::Help)),

        // View shortcuts
        (KeyModifiers::NONE, KeyCode::Char('1')) => Some(Command::NavigateTo(View::ContextSwitcher)),
        (KeyModifiers::NONE, KeyCode::Char('2')) => Some(Command::NavigateTo(View::ResourceBrowser)),
        (KeyModifiers::NONE, KeyCode::Char('3')) => Some(Command::NavigateTo(View::CostExplorer)),
        (KeyModifiers::NONE, KeyCode::Char('4')) => Some(Command::NavigateTo(View::ActivityLog)),
        (KeyModifiers::NONE, KeyCode::Char('5')) => Some(Command::NavigateTo(View::GlobalSearch)),

        _ => None,
    }
}

/* ============================================================================================== */
fn handle_resource_search_input(key: KeyEvent, state: &AppState) -> Option<Command> {
    match key.code {
        KeyCode::Esc => {
            // Clear resource search and unfocus.
            Some(Command::UpdateSearch(String::from("\x1B")))
        }
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
fn handle_cost_explorer_input(key: KeyEvent, state: &AppState) -> Option<Command> {
    match (key.modifiers, key.code) {
        // Quit
        (KeyModifiers::NONE, KeyCode::Char('q')) => Some(Command::Quit),

        // Navigate breakdown rows
        (KeyModifiers::NONE, KeyCode::Up | KeyCode::Char('k')) => Some(Command::NavUp),
        (KeyModifiers::NONE, KeyCode::Down | KeyCode::Char('j')) => Some(Command::NavDown),

        // Period navigation (stays within the current view)
        (KeyModifiers::NONE, KeyCode::Char('[') | KeyCode::Char('h')) => {
            Some(Command::FetchCostSummary {
                period: state.cost_period.previous_month(),
                view: state.cost_view.clone(),
            })
        }
        (KeyModifiers::NONE, KeyCode::Char(']') | KeyCode::Char('l')) => {
            state.cost_period.next_month().map(|next| Command::FetchCostSummary {
                period: next,
                view: state.cost_view.clone(),
            })
        }

        // Toggle subscription grouping (service <-> resource group)
        (KeyModifiers::NONE, KeyCode::Char('g')) => Some(Command::ToggleCostGrouping),

        // Drill into the selected resource group (only when grouped by RG)
        (KeyModifiers::NONE, KeyCode::Enter) => {
            if let CostView::Subscription(CostGrouping::ByResourceGroup) = state.cost_view {
                crate::ui::widgets::cost_explorer::selected_row_label(state)
                    .map(Command::DrillIntoResourceGroup)
            } else {
                None
            }
        }

        // Pop back to the subscription level
        (KeyModifiers::NONE, KeyCode::Backspace) => Some(Command::CostScopeUp),

        // Refresh current view
        (KeyModifiers::NONE, KeyCode::Char('r')) => Some(Command::FetchCostSummary {
            period: state.cost_period.clone(),
            view: state.cost_view.clone(),
        }),

        // Esc: pop one level if drilled in; otherwise back to context switcher
        (KeyModifiers::NONE, KeyCode::Esc) => {
            if let CostView::ResourceGroup(_) = state.cost_view {
                Some(Command::CostScopeUp)
            } else {
                Some(Command::NavigateTo(View::ContextSwitcher))
            }
        }

        // Quick switch
        (KeyModifiers::CONTROL, KeyCode::Char('g')) => {
            let filtered = quick_switch::build_filtered(state, "");
            Some(Command::OpenModal(Box::new(Modal::QuickSwitch {
                query: String::new(),
                filtered,
                cursor: 0,
            })))
        }

        // Help
        (_, KeyCode::Char('?')) => Some(Command::NavigateTo(View::Help)),

        // View shortcuts
        (KeyModifiers::NONE, KeyCode::Char('1')) => Some(Command::NavigateTo(View::ContextSwitcher)),
        (KeyModifiers::NONE, KeyCode::Char('2')) => Some(Command::NavigateTo(View::ResourceBrowser)),
        (KeyModifiers::NONE, KeyCode::Char('3')) => Some(Command::NavigateTo(View::CostExplorer)),
        (KeyModifiers::NONE, KeyCode::Char('4')) => Some(Command::NavigateTo(View::ActivityLog)),
        (KeyModifiers::NONE, KeyCode::Char('5')) => Some(Command::NavigateTo(View::GlobalSearch)),

        _ => None,
    }
}

/* ============================================================================================== */
fn handle_run_command_input(key: KeyEvent, state: &AppState) -> Option<Command> {
    let session = state.run_command.as_ref()?;

    match (key.modifiers, key.code) {
        // Run the script (F5): confirm first.
        (_, KeyCode::F(5)) => {
            if session.script().trim().is_empty() {
                return None;
            }
            let message = format!(
                "Run this PowerShell script on {} (rg: {})?",
                session.vm_name, session.resource_group
            );
            Some(Command::OpenModal(Box::new(Modal::Confirm {
                message,
                on_confirm: Box::new(Command::RunVmCommand),
            })))
        }

        // Back to the resource browser.
        (KeyModifiers::NONE, KeyCode::Esc) => Some(Command::NavigateTo(View::ResourceBrowser)),

        // Toggle editor/output focus.
        (KeyModifiers::NONE, KeyCode::Tab) => Some(Command::ToggleRunPane),

        // Scroll output when it is focused.
        (KeyModifiers::NONE, KeyCode::Up | KeyCode::Char('k')) if session.focus == RunPane::Output => {
            Some(Command::ScrollRunOutput(-1))
        }
        (KeyModifiers::NONE, KeyCode::Down | KeyCode::Char('j')) if session.focus == RunPane::Output => {
            Some(Command::ScrollRunOutput(1))
        }

        // Otherwise, feed the key to the editor when it has focus.
        _ if session.focus == RunPane::Editor => Some(Command::ScriptInput(key)),
        _ => None,
    }
}

/* ============================================================================================== */
fn handle_activity_log_input(key: KeyEvent, state: &AppState) -> Option<Command> {
    let activity = state.activity.as_ref()?;

    // Search-entry mode (activity-local; does not use the global search flag).
    if activity.search_focused {
        return match key.code {
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
        };
    }

    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Char('q')) => Some(Command::Quit),

        (KeyModifiers::NONE, KeyCode::Up | KeyCode::Char('k')) => Some(Command::NavUp),
        (KeyModifiers::NONE, KeyCode::Down | KeyCode::Char('j')) => Some(Command::NavDown),

        // Detail modal for the selected entry.
        (KeyModifiers::NONE, KeyCode::Enter) => {
            crate::ui::widgets::activity_log::selected_entry(state)
                .map(|e| Command::OpenModal(Box::new(Modal::ActivityDetail(Box::new(e)))))
        }

        // Window cycling.
        (KeyModifiers::NONE, KeyCode::Char('[') | KeyCode::Char('h')) => Some(Command::CycleActivityWindow(-1)),
        (KeyModifiers::NONE, KeyCode::Char(']') | KeyCode::Char('l')) => Some(Command::CycleActivityWindow(1)),

        // Scope broaden, failed-only, search, refresh.
        (KeyModifiers::NONE, KeyCode::Char('s')) => Some(Command::CycleActivityScope),
        (KeyModifiers::NONE, KeyCode::Char('f')) => Some(Command::ToggleActivityFailedOnly),
        (KeyModifiers::NONE, KeyCode::Char('/')) => Some(Command::SetActivitySearchFocus(true)),
        (KeyModifiers::NONE, KeyCode::Char('r')) => Some(Command::FetchActivityLog),

        (KeyModifiers::NONE, KeyCode::Esc) => Some(Command::NavigateTo(View::ContextSwitcher)),

        (KeyModifiers::CONTROL, KeyCode::Char('g')) => {
            let filtered = quick_switch::build_filtered(state, "");
            Some(Command::OpenModal(Box::new(Modal::QuickSwitch {
                query: String::new(),
                filtered,
                cursor: 0,
            })))
        }

        (_, KeyCode::Char('?')) => Some(Command::NavigateTo(View::Help)),
        (KeyModifiers::NONE, KeyCode::Char('1')) => Some(Command::NavigateTo(View::ContextSwitcher)),
        (KeyModifiers::NONE, KeyCode::Char('2')) => Some(Command::NavigateTo(View::ResourceBrowser)),
        (KeyModifiers::NONE, KeyCode::Char('3')) => Some(Command::NavigateTo(View::CostExplorer)),
        (KeyModifiers::NONE, KeyCode::Char('4')) => None, // already here
        (KeyModifiers::NONE, KeyCode::Char('5')) => Some(Command::NavigateTo(View::GlobalSearch)),

        _ => None,
    }
}

/* ============================================================================================== */
fn handle_global_search_input(key: KeyEvent, state: &AppState) -> Option<Command> {
    // Search-entry mode. Esc/Enter commit the filter and drop back to list
    // navigation (the query stays applied); arrows move the selection while
    // still in the search box; `/` is ignored so it never types a literal slash.
    if state.search_focused {
        return match key.code {
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
        };
    }

    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Char('q')) => Some(Command::Quit),
        (KeyModifiers::NONE, KeyCode::Up | KeyCode::Char('k')) => Some(Command::NavUp),
        (KeyModifiers::NONE, KeyCode::Down | KeyCode::Char('j')) => Some(Command::NavDown),
        (KeyModifiers::NONE, KeyCode::Enter) => Some(Command::OpenGlobalResource),
        // `/` focuses the search input, keeping any existing query so it can be refined.
        (KeyModifiers::NONE, KeyCode::Char('/')) => Some(Command::SetGlobalSearchFocus(true)),
        (KeyModifiers::NONE, KeyCode::Char('r')) => Some(Command::FetchGlobalInventory),
        // Esc clears an active filter first; with no filter, it backs out to the context switcher.
        (KeyModifiers::NONE, KeyCode::Esc) => {
            if state.global_search_query.is_empty() {
                Some(Command::NavigateTo(View::ContextSwitcher))
            } else {
                Some(Command::UpdateGlobalSearch(String::new()))
            }
        }
        (KeyModifiers::CONTROL, KeyCode::Char('g')) => {
            let filtered = quick_switch::build_filtered(state, "");
            Some(Command::OpenModal(Box::new(Modal::QuickSwitch {
                query: String::new(),
                filtered,
                cursor: 0,
            })))
        }
        (_, KeyCode::Char('?')) => Some(Command::NavigateTo(View::Help)),
        (KeyModifiers::NONE, KeyCode::Char('1')) => Some(Command::NavigateTo(View::ContextSwitcher)),
        (KeyModifiers::NONE, KeyCode::Char('2')) => Some(Command::NavigateTo(View::ResourceBrowser)),
        (KeyModifiers::NONE, KeyCode::Char('3')) => Some(Command::NavigateTo(View::CostExplorer)),
        (KeyModifiers::NONE, KeyCode::Char('4')) => Some(Command::NavigateTo(View::ActivityLog)),
        (KeyModifiers::NONE, KeyCode::Char('5')) => None, // already here
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
        Modal::ActivityDetail(_) => match key.code {
            KeyCode::Esc | KeyCode::Enter => Some(Command::CloseModal),
            _ => None,
        },
        Modal::PasswordPrompt { .. } => handle_password_input(key, state),
    }
}

/* ============================================================================================== */
fn handle_quick_switch_input(
    key: KeyEvent,
    query: &str,
    _filtered: &[crate::domain::models::AzureContext],
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
