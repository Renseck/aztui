use crate::domain::models::{AzureContext, CostPeriod};
use crate::errors::AppError;

/* ============================================================================================== */

/// Commands represent "something that should happen."
/// They are the ONLY way to mutate [`AppState`].
/// Plain data - no async, no side effects.
#[derive(Debug, Clone)]
pub enum Command {
    /* ===================================== Auth & Context ===================================== */

    /// Trigger a full interactive login (opens browser).
    Login,

    /// Login to a specific tenant.
    LoginToTenant(String),

    /// Set the active subscription (must already be logged into its tenant).
    SetSubscription(String),

    /// Refresh the tenant/subscription list from AZ CLI.
    RefreshContextList,

    /// Switch to a full context (tenant + subscription) in one action.
    SwitchContext(AzureContext),

    /// Fetch the currently active context from `az account show --output json`.
    FetchActiveContext,

    /* ======================================= Navigation ======================================= */

    /// Change the active top-level view.
    NavigateTo(crate::app::View),

    /// Update the search/filter query.
    UpdateSearch(String),

    /// Open a modal overlay.
    OpenModal(Box<crate::app::Modal>),

    /// Close the current modal.
    CloseModal,

    /* ===================================== List navigation ==================================== */

    /// Move the list cursor up by one row.
    NavUp,

    /// Move the list cursor down by one row.
    NavDown,

    /* =================================== Resource (Phase 3) =================================== */

    /// Fetch resource groups for the active subscription.
    ListResourceGroups,

    /// Fetch resources within a specific resource group.
    ListResources(String),

    /// Toggle focus between left and right pane in resource browser.
    ToggleResourcePane,

    /// Update the resource browser search query.
    UpdateResourceSearch(String),

    /* ===================================== Cost (Phase 4) ===================================== */

    FetchCostSummary(CostPeriod),

    /* ===================================== VM run-command ===================================== */

    /// Open the run-command view for a specific VM.
    OpenRunCommand {
        subscription_id: String,
        resource_group: String,
        vm_name: String,
    },

    /// Forward a key event to the script editor.
    ScriptInput(crossterm::event::KeyEvent),

    /// Toggle focus between the editor and the output pane.
    ToggleRunPane,

    /// Scroll the output pane by a signed delta (rows).
    ScrollRunOutput(i16),

    /// Execute the current script on the selected VM.
    RunVmCommand,

    /* ======================================== Security ======================================== */

    Lock,
    Unlock(String),

    // Set up a new master password (first time).
    SetupPassword(String),

    // Reset the master password (via --reset-password).
    ResetPassword,

    /* ========================================= System ========================================= */

    Quit,
    InvalidateAllCaches,
    CancelOperation(crate::app::OperationId),

    /// Delivered by the background update check when a newer release exists.
    #[doc(hidden)]
    NotifyUpdateAvailable(String),

    /* ================================= Internal async results ================================= */

    /// Delivered by async tasks when context list fetch completes.
    #[doc(hidden)]
    ContextListResult(Result<(Vec<crate::domain::models::Tenant>, std::collections::HashMap<String, Vec<crate::domain::models::Subscription>>), AppError>),

    /// Delivered by async tasks when a context switch completes.
    #[doc(hidden)]
    ContextSwitchResult(Result<AzureContext, AppError>),

    /// Delivered on startup when the active context is resolved.
    #[doc(hidden)]
    ActiveContextResult(Result<Option<AzureContext>, AppError>),

    /// Delivered after Argon2id password verification completes.
    #[doc(hidden)]
    UnlockResult(Result<crate::security::DerivedKey, AppError>),

    /// Delivered after Argon2id password setup completes.
    #[doc(hidden)]
    SetupPasswordResult(Result<(crate::security::StoredKeyParams, crate::security::DerivedKey), AppError>),

    /// Delivered by async tasks when resource groups fetch completes.
    #[doc(hidden)]
    ResourceGroupsResult(Result<Vec<crate::domain::models::ResourceGroup>, AppError>),

    /// Delivered by async tasks when resources fetch completes.
    #[doc(hidden)]
    ResourcesResult {
        resource_group: String,
        result: Result<Vec<crate::domain::models::Resource>, AppError>,
    },

    /// Delivered by async tasks when cost summary fetch completes.
    #[doc(hidden)]
    CostSummaryResult(Result<crate::domain::models::CostSummary, AppError>),

    /// Delivered when a VM run-command invocation completes.
    #[doc(hidden)]
    VmCommandResult(Result<crate::domain::models::RunCommandOutput, AppError>),
}