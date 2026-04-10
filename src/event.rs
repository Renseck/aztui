use crate::app::{Modal, OperationId, PendingOperation, View};
use crate::domain::models::{AzureContext, CostSummary, Resource, ResourceGroup};
use crate::errors::AppError;

/// Events represent "something that happened."
/// Emitted after a command has been processed and state updated.
#[derive(Debug, Clone)]
pub enum Event {
    /* ===================================== Auth & Context ===================================== */

    LoginStarted { tenant_id: Option<String> },
    LoginCompleted { tenant_id: String },
    LoginFailed { tenant_id: Option<String>, error: AppError },
    ContextListRefreshed,
    ContextChanged(AzureContext),

    /* ======================================= Navigation ======================================= */

    ViewChanged(View),
    ModalOpened(Modal),
    ModalClosed,

    /* =================================== Resource (Phase 3) =================================== */

    ResourceGroupsLoaded(Vec<ResourceGroup>),
    ResourcesLoaded { resource_group: String, resources: Vec<Resource> },

    /* ===================================== Cost (Phase 4) ===================================== */

    CostSummaryLoaded(CostSummary),

    /* ======================================== Security ======================================== */

    AppLocked,
    AppUnlocked,
    UnlockFailed,

    /* ======================================== Async ops ======================================= */

    OperationStarted(PendingOperation),
    OperationCompleted(OperationId),
    OperationCancelled(OperationId),

    /* ========================================= Errors ========================================= */

    ErrorOccurred(AppError),
    ErrorCleared,
}