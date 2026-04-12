use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{mpsc, RwLock};

use crate::cache::CacheStore;
use crate::command::Command;
use crate::config::AppConfig;
use crate::domain::auth::{AuthProvider};
use crate::domain::models::{AzureContext, Resource, ResourceGroup, Subscription, Tenant};
use crate::domain::resources::ResourceProvider;
use crate::errors::{AppError, ErrorKind};
use crate::event::Event;
use crate::security::{SecurityManager};

/* ============================================================================================== */
/*                                        Supporting types                                        */
/* ============================================================================================== */

pub type OperationId = u64;

/* ============================================================================================== */

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    ContextSwitcher,
    ResourceBrowser,
    CostExplorer,
    Help,
}

/* ============================================================================================== */

/// Overlay modal state. `Confirm.on_confirm` is boxed to break the recursive
/// type cycle with [`Command`].
#[derive(Debug, Clone)]
pub enum Modal {
    QuickSwitch {
        query: String,
        filtered: Vec<AzureContext>,
        cursor: usize,
    },
    Confirm {
        message: String,
        on_confirm: Box<Command>,
    },
    PasswordPrompt {
        input: String,
        error: Option<String>,
        mode: PasswordMode,
    },
    ErrorDetail(AppError),
}

/* ============================================================================================== */
#[derive(Debug, Clone)]
pub enum PasswordMode {
    /// Unlock with existing master password.
    Unlock,
    /// Setting up a new master password (first entry).
    Setup,
    /// Confirming the new master password (holds the first entry for comparison).
    SetupConfirm { first_password: String },

}

/* ============================================================================================== */
/// Which pane of the resource browser has focus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pane {
    Left,
    Right
}

/* ============================================================================================== */

/// An in-flight async operation. The [`AbortHandle`] allows cancellation.
#[derive(Debug, Clone)]
pub struct PendingOperation {
    pub id: OperationId,
    pub description: String,
    pub started_at: Instant,
    pub abort_handle: Option<tokio::task::AbortHandle>,
}

/* ============================================================================================== */
/*                                            AppState                                            */
/* ============================================================================================== */

/// Single source of truth for all application state.
/// All UI rendering reads from here; all mutations flow through command dispatch.
#[derive(Debug)]
pub struct AppState {
    // Auth & context
    pub tenants: Vec<Tenant>,
    pub subscriptions_by_tenant: HashMap<String, Vec<Subscription>>,
    pub active_context: Option<AzureContext>,
    pub recent_contexts: Vec<AzureContext>,

    // Navigation & UI
    pub active_view: View,
    pub search_query: String,
    pub search_focused: bool,
    pub modal: Option<Modal>,
    pub context_list_cursor: usize,

    // Resource browser (Phase 3)
    pub resource_groups: Vec<ResourceGroup>,
    pub resources: Vec<Resource>,
    pub resource_group_cursor: usize,
    pub resource_cursor: usize,
    pub resource_browser_focus: Pane,
    pub resource_search_query: String,
    
    // Async operations
    pub pending_operations: HashMap<OperationId, PendingOperation>,
    next_operation_id: OperationId,

    // Security
    pub locked: bool,
    pub last_interaction: Instant,
    pub security: SecurityManager,

    // Config
    pub config: AppConfig,
    
    // Errors
    pub last_error: Option<AppError>,

    // Spinner animation frame
    pub spinner_frame: u8,

    // Loop control
    pub should_quit: bool,
}

impl AppState {
    /// Creates a new [`AppState`] from the given config.
    pub fn new(config: AppConfig, security: SecurityManager) -> Self {
        let locked = security.is_enabled() && !security.is_unlocked();

        Self {
            tenants: Vec::new(),
            subscriptions_by_tenant: HashMap::new(),
            active_context: None,
            recent_contexts: Vec::new(),
            active_view: View::ContextSwitcher,
            search_query: String::new(),
            search_focused: false,
            modal: None,
            context_list_cursor: 0,
            resource_groups: Vec::new(),
            resources: Vec::new(),
            resource_group_cursor: 0,
            resource_cursor: 0,
            resource_browser_focus: Pane::Left,
            resource_search_query: String::new(),
            pending_operations: HashMap::new(),
            next_operation_id: 0,
            locked,
            security,
            last_interaction: Instant::now(),
            config,
            last_error: None,
            spinner_frame: 0,
            should_quit: false,
        }
    }

    /* ========================================================================================== */
    pub fn alloc_operation_id(&mut self) -> OperationId {
        let id = self.next_operation_id;
        self.next_operation_id += 1;
        id
    }

    /* ========================================================================================== */
    /// Records `ctx` in recent contexts (MRU order, deduplicated).
    pub fn push_recent_context(&mut self, ctx: AzureContext) {
        self.recent_contexts.retain(|c| c != &ctx);
        self.recent_contexts.insert(0, ctx);
        self.recent_contexts
            .truncate(self.config.general.max_recent_contexts);
    }

    /* ========================================================================================== */
    /// Returns true if the inactivity timeout has been reached.
    pub fn should_lock(&self) -> bool {
        if self.locked {
            return false;
        }
        if let Some(timeout) = self.config.security.inactivity_timeout() {
            return self.last_interaction.elapsed() >= timeout;
        }
        false
    }

    /* ========================================================================================== */
    pub fn tick_spinner(&mut self) {
        self.spinner_frame = self.spinner_frame.wrapping_add(1) % 10;
    }

}

/* ============================================================================================== */
/*                                        Command dispatch                                        */
/* ============================================================================================== */

const SLOT_CONTEXT_LIST: OperationId = u64::MAX - 1;
const SLOT_CONTEXT_SWITCH: OperationId = u64::MAX - 2;
const SLOT_RESOURCE_GROUPS: OperationId = u64::MAX - 3;
const SLOT_RESOURCES: OperationId = u64::MAX - 4;

/// Processes a single [`Command`], mutates `state`, may spawn async tasks
/// (sending results back via `cmd_tx`), and returns emitted [`Event`]s.
pub async fn dispatch_command(
    state: &mut AppState,
    cmd: Command,
    cmd_tx: &mpsc::Sender<Command>,
    auth: Arc<dyn AuthProvider>,
    resources: Arc<dyn ResourceProvider>,
) -> Vec<Event> {
    state.last_interaction = Instant::now();
    let mut events = Vec::new();

    match cmd {
        /* =========================== Synchronous navigation commands ========================== */
        Command::Quit => {
            state.should_quit = true;
        }

        Command::NavigateTo(view) => {
            let prev_view = state.active_view.clone();
            state.active_view = view.clone();
            state.search_query.clear();
            state.search_focused = false;
            state.resource_search_query.clear();
            events.push(Event::ViewChanged(view.clone()));

            // Auto-fetch resource groups when entering resource browser.
            if view == View::ResourceBrowser
                && state.resource_groups.is_empty()
                && state.active_context.is_some()
            {
                let _ = cmd_tx.try_send(Command::ListResourceGroups);
            }
        }

        Command::UpdateSearch(q) => {
            state.search_query = q;
        }

        Command::OpenModal(modal) => {
            let m = *modal;
            events.push(Event::ModalOpened(m.clone()));
            state.modal = Some(m);
        }

        Command::CloseModal => {
            state.modal = None;
            events.push(Event::ModalClosed);
        }

        Command::NavUp => {
            state.last_interaction = Instant::now();
            if let Some(Modal::QuickSwitch { cursor, filtered, .. }) =
                state.modal.as_mut()
            {
                if *cursor > 0 {
                    *cursor -= 1;
                }
                let _ = filtered; // used via state.modal above
            } else if state.context_list_cursor > 0 {
                state.context_list_cursor -= 1;
            }
        }

        Command::NavDown => {
            state.last_interaction = Instant::now();
            if let Some(Modal::QuickSwitch { cursor, filtered, .. }) =
                state.modal.as_mut()
            {
                let max = filtered.len().saturating_sub(1);
                if *cursor < max {
                    *cursor += 1;
                }
            } else {
                state.context_list_cursor += 1;
                // Clamping is handled by the widget since it knows the list length.
            }
        }

        Command::Lock => {
            state.locked = true;
            state.security.lock();
            // Clear sensitive state on lock.
            state.tenants.clear();
            state.subscriptions_by_tenant.clear();
            state.active_context = None;
            state.modal = Some(Modal::PasswordPrompt { 
                input: String::new(), 
                error: None, 
                mode: PasswordMode::Unlock 
            });
            events.push(Event::AppLocked);
        }

        Command::Unlock(password) => {
            if let Some(params) = state.security.stored_params().cloned() {
                let tx = cmd_tx.clone();
                tokio::task::spawn_blocking(move || {
                    let result = crate::security::master_key::derive_and_verify(&password, &params);
                    let mapped = result
                        .map(|key| crate::security::DerivedKey(key.to_vec()))
                        .map_err(|e| e);
                    // Send result back - blocking send since we're in spawn_blocking.
                    let _ = tx.blocking_send(Command::UnlockResult(mapped));
                });
            } else {
                // No stored params - shouldn't happen, but unlock to be safe.
                state.locked = false;
                state.modal = None;
                events.push(Event::AppUnlocked);
                events.push(Event::ModalClosed);
            }   
        }

        Command::SetupPassword(password) => {
            let tx = cmd_tx.clone();
            tokio::task::spawn_blocking(move || {
                let result = crate::security::master_key::create_params_and_key(&password);
                let mapped = result.map(|(params, key)| {
                    (params, crate::security::DerivedKey(key.to_vec()))
                });
                let _ = tx.blocking_send(Command::SetupPasswordResult(mapped));
            });
        }

        Command::ResetPassword => {
            if let Err(e) = state.security.reset() {
                state.last_error = Some(e.clone());
                events.push(Event::ErrorOccurred(e));
            } else {
                state.locked = true;
                state.modal = Some(Modal::PasswordPrompt { 
                    input: String::new(), 
                    error: None, 
                    mode: PasswordMode::Setup
                });
            }
        }

        Command::UnlockResult(result) => {
            match result {
                Ok(derived_key) => {
                    if let Some(key_arr) = derived_key.as_array() {
                        state.security.set_key(*key_arr);
                        // Store to keyring if enabled.
                        let _ = state.security.store_to_keyring();
                    }
                    state.locked = false;
                    state.modal = None;
                    events.push(Event::ModalClosed);
                    events.push(Event::AppUnlocked);
                    // Trigger context list refresh now that we're unlocked.
                    let tx = cmd_tx.clone();
                    let _ = tx.try_send(Command::RefreshContextList);
                }
                Err(e) => {
                    // Update the password modal with the error message.
                    if let Some(Modal::PasswordPrompt { error, .. }) = state.modal.as_mut() {
                        *error = Some(e.message.clone());
                    }
                    events.push(Event::UnlockFailed);
                }
            }
        }

        Command::SetupPasswordResult(result) => {
            match result {
                Ok((params, derived_key)) => {
                    if let Some(key_arr) = derived_key.as_array() {
                        if let Err(e) = state.security.save_setup(params, *key_arr) {
                            state.last_error = Some(e.clone());
                            events.push(Event::ErrorOccurred(e));
                            return events;
                        }
                        let _ = state.security.store_to_keyring();
                    }
                    state.locked = false;
                    state.modal = None;
                    events.push(Event::ModalClosed);
                    events.push(Event::AppUnlocked);
                    let tx = cmd_tx.clone();
                    let _ = tx.try_send(Command::RefreshContextList);
                }
                Err(e) => {
                    if let Some(Modal::PasswordPrompt { error, .. }) = state.modal.as_mut() {
                        *error = Some(e.message.clone());
                    }
                    events.push(Event::ErrorOccurred(e));
                }
            }
        }

        Command::InvalidateAllCaches => {
            // The cache is external to AppState: signal via event so the
            // caller can invalidate it.
            events.push(Event::ContextListRefreshed);
        }

        Command::CancelOperation(id) => {
            if let Some(op) = state.pending_operations.remove(&id) {
                if let Some(handle) = op.abort_handle {
                    handle.abort();
                }
                events.push(Event::OperationCancelled(id));
            }
        }

        /* ============================= Async: refresh context list ============================ */

        Command::Login | Command::RefreshContextList => {
            abort_slot(state, SLOT_CONTEXT_LIST);

            let op_id = SLOT_CONTEXT_LIST;
            let tx = cmd_tx.clone();
            let auth = Arc::clone(&auth);

            let handle = tokio::spawn(async move {
                let result = auth.list_contexts().await;
                let _ = tx.send(Command::ContextListResult(result)).await;
            })
            .abort_handle();

            let op = PendingOperation {
                id: op_id,
                description: "Loading tenants and subscriptions...".to_string(),
                started_at: Instant::now(),
                abort_handle: Some(handle),
            };
            events.push(Event::OperationStarted(op.clone()));
            state.pending_operations.insert(op_id, op);
        }

        Command::LoginToTenant(tenant_id) => {
            abort_slot(state, SLOT_CONTEXT_LIST);

            let op_id = SLOT_CONTEXT_LIST;
            let tx = cmd_tx.clone();
            let auth = Arc::clone(&auth);
            let tid = tenant_id.clone();

            let handle = tokio::spawn(async move {
                let _ = auth.login_to_tenant(&tid).await;
                let result = auth.list_contexts().await;
                let _ = tx.send(Command::ContextListResult(result)).await;
            })
            .abort_handle();

            let op = PendingOperation {
                id: op_id,
                description: format!("Logging in to tenant {}...", tenant_id),
                started_at: Instant::now(),
                abort_handle: Some(handle),
            };
            events.push(Event::LoginStarted { tenant_id: Some(tenant_id) });
            events.push(Event::OperationStarted(op.clone()));
            state.pending_operations.insert(op_id, op);
        }

        Command::FetchActiveContext => {
            let tx = cmd_tx.clone();
            let auth = Arc::clone(&auth);

            tokio::spawn(async move {
                let result = auth.get_active_context().await;
                let _ = tx.send(Command::ActiveContextResult(result)).await;
            });
        }

        /* ================================ Async: switch context =============================== */

        Command::SwitchContext(ctx) => {
            abort_slot(state, SLOT_CONTEXT_SWITCH);

            let op_id = SLOT_CONTEXT_SWITCH;
            let tx = cmd_tx.clone();
            let auth = Arc::clone(&auth);
            let sub_id = ctx.subscription.id.clone();
            let ctx_clone = ctx.clone();

            let handle = tokio::spawn(async move {
                let result = match auth.set_subscription(&sub_id).await {
                    Ok(()) => Ok(ctx_clone),
                    Err(e) => Err(e),
                };
                let _ = tx.send(Command::ContextSwitchResult(result)).await;
            })
            .abort_handle();

            let label = ctx.label();
            let op = PendingOperation {
                id: op_id,
                description: format!("Switching to {}...", label),
                started_at: Instant::now(),
                abort_handle: Some(handle),
            };
            events.push(Event::OperationStarted(op.clone()));
            state.pending_operations.insert(op_id, op);
        }

        Command::SetSubscription(sub_id) => {
            abort_slot(state, SLOT_CONTEXT_SWITCH);

            let op_id = SLOT_CONTEXT_SWITCH;
            let tx = cmd_tx.clone();
            let auth = Arc::clone(&auth);

            let ctx = state
                .subscriptions_by_tenant
                .values()
                .flatten()
                .find(|s| s.id == sub_id)
                .and_then(|sub| {
                    state
                        .tenants
                        .iter()
                        .find(|t| t.id == sub.tenant_id)
                        .map(|tenant| AzureContext {
                            tenant: tenant.clone(),
                            subscription: sub.clone(),
                        })
                });
            
            if let Some(ctx) = ctx {
                let ctx_clone = ctx.clone();
                let handle = tokio::spawn(async move {
                    let result = match auth.set_subscription(&sub_id).await {
                        Ok(()) => Ok(ctx_clone),
                        Err(e) => Err(e),
                    };
                    let _ = tx.send(Command::ContextSwitchResult(result)).await;
                })
                .abort_handle();

                let op = PendingOperation {
                    id: op_id,
                    description: format!("Switching subscription..."),
                    started_at: Instant::now(),
                    abort_handle: Some(handle),
                };
                events.push(Event::OperationStarted(op.clone()));
                state.pending_operations.insert(op_id, op);
            }
        }

        /* =============================== Internal async results =============================== */

        Command::ContextListResult(result) => {
            state.pending_operations.remove(&SLOT_CONTEXT_LIST);
            events.push(Event::OperationCompleted(SLOT_CONTEXT_LIST));

            match result {
                Ok((tenants, by_tenant)) => {
                    state.tenants = tenants;
                    state.subscriptions_by_tenant = by_tenant;
                    // Clamp cursor in case list shrank.
                    let total_subs: usize = 
                        state.subscriptions_by_tenant.values().map(|v| v.len()).sum();
                    if state.context_list_cursor >= total_subs && total_subs > 0 {
                        state.context_list_cursor = total_subs - 1;
                    }
                    events.push(Event::ContextListRefreshed);

                    // If no active context is known yet, fetch it.
                    if state.active_context.is_none() {
                        let tx = cmd_tx.clone();
                        let _ = tx.try_send(Command::FetchActiveContext);
                    }
                }
                Err(e) => {
                    state.last_error = Some(e.clone());
                    events.push(Event::ErrorOccurred(e));
                }
            }
        }

        Command::ContextSwitchResult(result) => {
            state.pending_operations.remove(&SLOT_CONTEXT_SWITCH);
            events.push(Event::OperationCompleted(SLOT_CONTEXT_SWITCH));

            match result {
                Ok(ctx) => {
                    state.last_error = None;
                    state.push_recent_context(ctx.clone());
                    state.active_context = Some(ctx.clone());
                    // Clear stale resource data from previous subscription.
                    state.resource_groups.clear();
                    state.resources.clear();
                    state.resource_group_cursor = 0;
                    state.resource_cursor = 0;
                    // Close quick switch modal if open.
                    if matches!(state.modal, Some(Modal::QuickSwitch { .. })) {
                        state.modal = None;
                        events.push(Event::ModalClosed);
                    }
                    events.push(Event::ContextChanged(ctx));
                }
                Err(e) => {
                    state.last_error = Some(e.clone());
                    events.push(Event::ErrorOccurred(e));
                }
            }
        }

        Command::ActiveContextResult(result) => {
            match result {
                Ok(Some(ctx)) => {
                    state.active_context = Some(ctx.clone());
                    events.push(Event::ContextChanged(ctx));
                }
                Ok(None) => {}
                Err(e) => {
                    state.last_error = Some(e.clone());
                    events.push(Event::ErrorOccurred(e));
                }
            }
        }

        /* ================================= Resource (Phase 3) ================================= */

         Command::ListResourceGroups => {
            let sub_id = match &state.active_context {
                Some(ctx) => ctx.subscription.id.clone(),
                None => {
                    state.last_error = Some(AppError::new(
                        ErrorKind::SubscriptionNotFound,
                        "Select a subscription before browsing resources",
                    ));
                    events.push(Event::ErrorOccurred(state.last_error.clone().unwrap()));
                    return events;
                }
            };

            abort_slot(state, SLOT_RESOURCE_GROUPS);

            let op_id = SLOT_RESOURCE_GROUPS;
            let tx = cmd_tx.clone();
            let resources = Arc::clone(&resources);

            let handle = tokio::spawn(async move {
                let result = resources.list_resource_groups(&sub_id).await;
                let _ = tx.send(Command::ResourceGroupsResult(result)).await;
            })
            .abort_handle();

            let op = PendingOperation {
                id: op_id,
                description: "Loading resource groups...".to_string(),
                started_at: Instant::now(),
                abort_handle: Some(handle),
            };
            events.push(Event::OperationStarted(op.clone()));
            state.pending_operations.insert(op_id, op);
        }

        Command::ListResources(ref rg_name) => {
            let sub_id = match &state.active_context {
                Some(ctx) => ctx.subscription.id.clone(),
                None => return events,
            };

            abort_slot(state, SLOT_RESOURCES);

            let op_id = SLOT_RESOURCES;
            let tx = cmd_tx.clone();
            let resources_provider = Arc::clone(&resources);
            let rg = rg_name.clone();

            let handle = tokio::spawn(async move {
                let result = resources_provider.list_resources(&sub_id, &rg).await;
                let _ = tx.send(Command::ResourcesResult {
                    resource_group: rg,
                    result,
                }).await;
            })
            .abort_handle();

            let op = PendingOperation {
                id: op_id,
                description: format!("Loading resources for {}...", rg_name),
                started_at: Instant::now(),
                abort_handle: Some(handle),
            };
            events.push(Event::OperationStarted(op.clone()));
            state.pending_operations.insert(op_id, op);
        }

        Command::ResourceGroupsResult(result) => {
            state.pending_operations.remove(&SLOT_RESOURCE_GROUPS);
            events.push(Event::OperationCompleted(SLOT_RESOURCE_GROUPS));

            match result {
                Ok(groups) => {
                    state.resource_groups = groups.clone();
                    state.resource_group_cursor = 0;
                    state.resources.clear();
                    state.resource_cursor = 0;
                    events.push(Event::ResourceGroupsLoaded(groups));

                    // Auto-load resources for the first group.
                    if let Some(first) = state.resource_groups.first() {
                        let _ = cmd_tx.try_send(Command::ListResources(first.name.clone()));
                    }
                }
                Err(e) => {
                    state.last_error = Some(e.clone());
                    events.push(Event::ErrorOccurred(e));
                }
            }
        }

        Command::ResourcesResult { resource_group, result } => {
            state.pending_operations.remove(&SLOT_RESOURCES);
            events.push(Event::OperationCompleted(SLOT_RESOURCES));

            match result {
                Ok(resources) => {
                    state.resources = resources.clone();
                    state.resource_cursor = 0;
                    events.push(Event::ResourcesLoaded {
                        resource_group,
                        resources,
                    });
                }
                Err(e) => {
                    state.last_error = Some(e.clone());
                    events.push(Event::ErrorOccurred(e));
                }
            }
        }
        
        /* ================================ Phase 4 placeholders ================================ */
        
        Command::FetchCostSummary(_) => {
            // TODO Phase 4.
        }
    }

    events
}

/* ============================================================================================== */
/// Secondary state reactions to already-processed events.
pub fn handle_event(state: &mut AppState, event: &Event) {
    match event {
        Event::LoginCompleted { .. } => {
            state.last_error = None;
        }
        Event::ContextChanged(_) => {
            // Resource data is cleared in dispatch; nothing extra needed here.
            state.last_error = None;
        }
        Event::ErrorOccurred(e) => {
            state.last_error = Some(e.clone());
        }
        Event::ErrorCleared => {
            state.last_error = None;
        }
        _ => {}
    }
}

/* ============================================================================================== */
/*                                         Private helpers                                        */
/* ============================================================================================== */

fn abort_slot(state: &mut AppState, slot_id: OperationId) {
    if let Some(op) = state.pending_operations.remove(&slot_id) {
        if let Some(handle) = op.abort_handle {
            handle.abort();
        }
    }
}