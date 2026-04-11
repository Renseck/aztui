# AZTUI — Foundation Types

## 1. DOMAIN MODELS — internal representations, decoupled from AZ CLI JSON

```rs
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// A normalized Azure tenant.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Tenant {
    pub id: String,            // GUID
    pub tenant_display_name: String,
    pub tenant_default_domain: String, // e.g. "contoso.onmicrosoft.com"
}

/// A normalized Azure subscription.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Subscription {
    pub id: String,             // GUID
    pub name: String,
    pub tenant_id: String,      // FK → Tenant.id
    pub state: SubscriptionState,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SubscriptionState {
    Enabled,
    Disabled,
    Warned,
    PastDue,
    Unknown(String),
}

/// An Azure resource group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceGroup {
    pub name: String,
    pub subscription_id: String,
    pub location: String,
    pub tags: HashMap<String, String>,
}

/// A single Azure resource (generic enough for browsing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    pub id: String,             // full ARM resource ID
    pub name: String,
    pub resource_type: String,  // e.g. "Microsoft.Compute/virtualMachines"
    pub resource_group: String,
    pub location: String,
    pub tags: HashMap<String, String>,
}

/// Cost summary for a scope (subscription or resource group).
#[derive(Debug, Clone)]
pub struct CostSummary {
    pub scope: CostScope,
    pub currency: String,
    pub total: f64,
    pub period: CostPeriod,
    pub breakdown: Vec<CostLineItem>,
}

#[derive(Debug, Clone)]
pub enum CostScope {
    Subscription(String),
    ResourceGroup { subscription_id: String, name: String },
}

#[derive(Debug, Clone)]
pub struct CostPeriod {
    pub from: String, // ISO date
    pub to: String,
}

#[derive(Debug, Clone)]
pub struct CostLineItem {
    pub service_name: String,
    pub amount: f64,
}

/// Represents the user's current "working context" in Azure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AzureContext {
    pub tenant: Tenant,
    pub subscription: Subscription,
}
```

## 2. APP STATE — single source of truth

```rs
/// The central application state. All UI rendering reads from this.
/// All mutations go through command dispatch → state update → event emission.
#[derive(Debug)]
pub struct AppState {
    // --- Auth & Context ---
    /// All known tenants (populated after login/cache load).
    pub tenants: Vec<Tenant>,

    /// All known subscriptions, keyed by tenant ID for fast lookup.
    pub subscriptions_by_tenant: HashMap<String, Vec<Subscription>>,

    /// The currently active Azure context (tenant + subscription).
    /// None if the user hasn't selected one yet.
    pub active_context: Option<AzureContext>,

    /// Recently used contexts, ordered most-recent-first.
    /// Used for quick switching (Ctrl+P style).
    pub recent_contexts: Vec<AzureContext>,

    // --- Navigation & UI ---
    /// Which top-level view is currently active.
    pub active_view: View,

    /// The current search/filter query in the active view.
    pub search_query: String,

    /// Whether a modal is currently displayed (and which one).
    pub modal: Option<Modal>,

    // --- Async operation tracking ---
    /// In-flight operations. The UI can show spinners / progress for these.
    /// Keyed by a unique operation ID (used for cancellation).
    pub pending_operations: HashMap<OperationId, PendingOperation>,

    // --- Security ---
    /// Whether the app is currently locked (inactivity timeout).
    pub locked: bool,

    /// Timestamp of last user interaction (for inactivity detection).
    pub last_interaction: Instant,

    // --- Configuration (read-only at runtime) ---
    pub config: AppConfig,

    // --- Errors ---
    /// The most recent error, if any. Shown as a notification bar.
    /// Cleared on next user action or after a timeout.
    pub last_error: Option<AppError>,
}

pub type OperationId = u64;

#[derive(Debug, Clone)]
pub struct PendingOperation {
    pub id: OperationId,
    pub description: String,       // human-readable, e.g. "Switching to tenant Contoso"
    pub started_at: Instant,
    pub abort_handle: Option<tokio::task::AbortHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    ContextSwitcher,   // Tenant/subscription selection (the main view)
    ResourceBrowser,   // Browse resource groups and resources
    CostExplorer,      // FinOps cost summaries
    Help,              // Keybindings, command reference
}

#[derive(Debug, Clone)]
pub enum Modal {
    /// Ctrl+P style fuzzy finder for quick context switching.
    QuickSwitch { query: String, filtered: Vec<AzureContext> },
    /// Confirmation dialog (e.g. "Switch to tenant X?").
    Confirm { message: String, on_confirm: Command },
    /// Master password prompt.
    PasswordPrompt,
    /// Error detail view.
    ErrorDetail(AppError),
}
```

## 3. COMMANDS — user/system intent (request side)

```rs
/// Commands represent "something that should happen."
/// They are dispatched by the UI or by plugins, processed by the application
/// layer, and result in state mutations + event emissions.
///
/// Commands are the ONLY way to mutate AppState.
#[derive(Debug, Clone)]
pub enum Command {
    // --- Auth & Context ---
    /// Trigger a full login flow (opens browser via az login).
    Login,

    /// Login to a specific tenant.
    LoginToTenant(String),  // tenant_id

    /// Set the active subscription (must already be logged into its tenant).
    SetSubscription(String),  // subscription_id

    /// Refresh the tenant/subscription list from AZ CLI.
    RefreshContextList,

    /// Switch to a full context (tenant + subscription) in one action.
    /// This is the "quick switch" — may trigger LoginToTenant if needed.
    SwitchContext(AzureContext),

    // --- Navigation ---
    /// Change the active top-level view.
    NavigateTo(View),

    /// Update the search/filter query.
    UpdateSearch(String),

    /// Open a modal.
    OpenModal(Modal),

    /// Close the current modal.
    CloseModal,

    // --- Resource browsing (Phase 3) ---
    /// List resource groups for the active subscription.
    ListResourceGroups,

    /// List resources in a specific resource group.
    ListResources(String),  // resource_group_name

    // --- Cost (Phase 4) ---
    /// Fetch cost summary for the active subscription.
    FetchCostSummary(CostPeriod),

    // --- Security ---
    /// Lock the application.
    Lock,

    /// Attempt to unlock with a password.
    Unlock(String),  // password attempt

    // --- System ---
    /// Graceful shutdown.
    Quit,

    /// Force-refresh all caches.
    InvalidateAllCaches,

    /// Cancel a pending async operation.
    CancelOperation(OperationId),
}
```

## 4. EVENTS — state changes that occurred (broadcast side)

```rs
/// Events represent "something that happened." They are emitted AFTER a
/// command has been processed and state has been updated.
///
/// In Phase 1, the UI is the only consumer. Later, plugins can subscribe
/// to react to events (e.g. a logging plugin, a notification plugin).
#[derive(Debug, Clone)]
pub enum Event {
    // --- Auth & Context ---
    /// Tenant/subscription list was refreshed (from CLI or cache).
    ContextListRefreshed,

    /// Active context changed.
    ContextChanged(AzureContext),

    /// Login flow started (UI can show a "waiting for browser" indicator).
    LoginStarted { tenant_id: Option<String> },

    /// Login completed successfully.
    LoginCompleted { tenant_id: String },

    /// Login failed.
    LoginFailed { tenant_id: Option<String>, error: AppError },

    // --- Resources ---
    ResourceGroupsLoaded(Vec<ResourceGroup>),
    ResourcesLoaded { resource_group: String, resources: Vec<Resource> },

    // --- Cost ---
    CostSummaryLoaded(CostSummary),

    // --- Navigation ---
    ViewChanged(View),
    ModalOpened(Modal),
    ModalClosed,

    // --- Security ---
    AppLocked,
    AppUnlocked,
    UnlockFailed,

    // --- Async operations ---
    OperationStarted(PendingOperation),
    OperationCompleted(OperationId),
    OperationCancelled(OperationId),

    // --- Errors ---
    ErrorOccurred(AppError),
    ErrorCleared,
}
```

## 5. CAPABILITY TRAITS — domain-specific interfaces

```rs
/// Provides authentication and context management.
/// This is the core capability for Phase 1.
#[async_trait::async_trait]
pub trait AuthProvider: Send + Sync {
    /// Perform interactive login (opens browser).
    /// Returns the list of accessible tenants.
    async fn login(&self) -> Result<Vec<Tenant>, AppError>;

    /// Login to a specific tenant.
    /// Returns the subscriptions accessible under that tenant.
    async fn login_to_tenant(&self, tenant_id: &str) -> Result<Vec<Subscription>, AppError>;

    /// Get all known tenants and subscriptions (from cache or CLI).
    async fn list_contexts(&self) -> Result<(Vec<Tenant>, HashMap<String, Vec<Subscription>>), AppError>;

    /// Set the active subscription in AZ CLI.
    async fn set_subscription(&self, subscription_id: &str) -> Result<(), AppError>;

    /// Get the currently active context from AZ CLI state.
    async fn get_active_context(&self) -> Result<Option<AzureContext>, AppError>;
}

/// Provides resource browsing capabilities.
/// Phase 3 — interface defined now for architectural clarity.
#[async_trait::async_trait]
pub trait ResourceProvider: Send + Sync {
    /// List resource groups in the active subscription.
    async fn list_resource_groups(
        &self,
        subscription_id: &str,
    ) -> Result<Vec<ResourceGroup>, AppError>;

    /// List resources in a resource group.
    async fn list_resources(
        &self,
        subscription_id: &str,
        resource_group: &str,
    ) -> Result<Vec<Resource>, AppError>;
}

/// Provides cost and usage data.
/// Phase 4 — interface defined now for architectural clarity.
#[async_trait::async_trait]
pub trait CostProvider: Send + Sync {
    /// Get cost summary for a subscription.
    async fn get_cost_summary(
        &self,
        subscription_id: &str,
        period: &CostPeriod,
    ) -> Result<CostSummary, AppError>;

    /// Get cost summary for a specific resource group.
    async fn get_resource_group_cost(
        &self,
        subscription_id: &str,
        resource_group: &str,
        period: &CostPeriod,
    ) -> Result<CostSummary, AppError>;
}
```

## 6. ERROR MODEL — unified, actionable errors

```rs
/// The single error type for the entire application.
/// Every error carries enough context for the UI to guide the user.
#[derive(Debug, Clone)]
pub struct AppError {
    pub kind: ErrorKind,
    pub message: String,
    pub recovery: Option<RecoveryAction>,
    pub source_detail: Option<String>, // raw stderr or additional context
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    // --- Auth ---
    AuthExpired,         // Token expired, needs re-login
    AuthFailed,          // Login attempt failed
    TenantNotFound,      // Tenant ID doesn't exist
    SubscriptionNotFound,

    // --- AZ CLI ---
    CliNotFound,         // `az` binary not found on PATH
    CliExecutionFailed,  // Non-zero exit code
    CliTimeout,          // Command exceeded timeout
    CliParseError,       // JSON output couldn't be parsed

    // --- Network ---
    NetworkError,        // DNS, connection, TLS failures

    // --- Security ---
    MasterPasswordWrong,
    CacheDecryptionFailed,

    // --- System ---
    ConfigError,         // Config file missing or malformed
    CacheError,          // Cache read/write failure
    Unknown,
}

/// Suggested recovery the UI can offer to the user.
#[derive(Debug, Clone)]
pub enum RecoveryAction {
    /// Prompt the user to re-login.
    ReLogin,
    /// Prompt to login to a specific tenant.
    LoginToTenant(String),
    /// Retry the failed command.
    Retry(Box<Command>),
    /// Open settings to fix configuration.
    OpenSettings,
    /// No automated recovery; show the error and let the user decide.
    Manual(String),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for AppError {}
```


## 7. AZ CLI ADAPTER — infrastructure interface

```rs
/// The contract for executing AZ CLI commands.
/// Concrete implementation calls `az` as a subprocess.
/// Test implementation returns canned responses.
#[async_trait::async_trait]
pub trait AzCliExecutor: Send + Sync {
    /// Execute an az command and return the raw stdout as a String.
    ///
    /// # Arguments
    /// * `args` — CLI arguments, e.g. ["account", "list", "--all"]
    /// * `timeout` — maximum time to wait for the command
    ///
    /// # Returns
    /// Raw stdout string (typically JSON) on success, AppError on failure.
    async fn execute(
        &self,
        args: &[&str],
        timeout: Duration,
    ) -> Result<String, AppError>;
}
```

## 8. CACHE — infrastructure interface

```rs
/// Generic cache with soft/hard TTL and scoping.
#[derive(Debug, Clone)]
pub struct CacheEntry<T> {
    pub value: T,
    pub created_at: Instant,
    pub soft_ttl: Duration,   // after this: serve cached, refresh in background
    pub hard_ttl: Duration,   // after this: force refresh
}

impl<T> CacheEntry<T> {
    pub fn is_fresh(&self) -> bool {
        self.created_at.elapsed() < self.soft_ttl
    }

    pub fn is_stale(&self) -> bool {
        let elapsed = self.created_at.elapsed();
        elapsed >= self.soft_ttl && elapsed < self.hard_ttl
    }

    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() >= self.hard_ttl
    }
}

/// Cache key scoping — ensures tenant A's data doesn't leak to tenant B.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CacheScope {
    /// Global data (e.g. the tenant list itself).
    Global,
    /// Scoped to a specific tenant.
    Tenant(String),
    /// Scoped to a specific subscription.
    Subscription(String),
}

/// Full cache key = scope + data type identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub scope: CacheScope,
    pub kind: String,  // e.g. "subscriptions", "resource_groups", "cost_summary"
}
```

## 9. CONFIGURATION

```rs
/// Application configuration, loaded from ~/.aztui/config.toml at startup.
/// Immutable at runtime (restart to apply changes).
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub cache: CacheConfig,
    pub security: SecurityConfig,
    pub ui: UiConfig,
    pub cli: CliConfig,
}

#[derive(Debug, Clone)]
pub struct GeneralConfig {
    /// Path to the aztui data directory.
    /// Default: ~/.aztui
    pub data_dir: PathBuf,

    /// Default tenant to select on startup (optional).
    pub default_tenant: Option<String>,

    /// Default subscription to select on startup (optional).
    pub default_subscription: Option<String>,

    /// Maximum number of recent contexts to remember.
    pub max_recent_contexts: usize, // default: 10
}

#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Soft TTL for tenant/subscription lists.
    pub context_soft_ttl: Duration,  // default: 5 min

    /// Hard TTL for tenant/subscription lists.
    pub context_hard_ttl: Duration,  // default: 1 hour

    /// Soft TTL for resource listings.
    pub resource_soft_ttl: Duration, // default: 2 min

    /// Hard TTL for resource listings.
    pub resource_hard_ttl: Duration, // default: 30 min

    /// Soft TTL for cost data.
    pub cost_soft_ttl: Duration,     // default: 15 min

    /// Hard TTL for cost data.
    pub cost_hard_ttl: Duration,     // default: 2 hours
}

#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// Whether the master password feature is enabled.
    pub master_password_enabled: bool,

    /// Inactivity timeout before auto-lock. None = never lock.
    pub inactivity_timeout: Option<Duration>,  // default: 10 min

    /// Whether to use the OS keyring for key storage.
    pub use_os_keyring: bool,  // default: false
}

#[derive(Debug, Clone)]
pub struct UiConfig {
    /// Enable/disable mouse support.
    pub mouse_enabled: bool,  // default: true

    /// Status bar position.
    pub status_bar_position: StatusBarPosition,

    /// Whether to show operation durations in the status bar.
    pub show_operation_timing: bool,  // default: true
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusBarPosition {
    Top,
    Bottom,  // default
}

#[derive(Debug, Clone)]
pub struct CliConfig {
    /// Path to the `az` binary. None = find on PATH.
    pub az_path: Option<PathBuf>,

    /// Default timeout for az commands.
    pub default_timeout: Duration,  // default: 30s

    /// Timeout for login commands (they wait for browser interaction).
    pub login_timeout: Duration,    // default: 120s

    /// Output format to request from az. Should always be "json".
    pub output_format: String,      // default: "json"
}
```


## 10. APPLICATION LOOP SKETCH

```rs
// This is a simplified sketch of the main event loop, showing how
// Commands, Events, and AppState interact.
//
// ```
// #[tokio::main]
// async fn main() -> Result<(), AppError> {
//     let config = load_config()?;
//     let mut state = AppState::new(config);
//     let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<Command>(64);
//     let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<Event>(64);
//
//     // Terminal setup
//     let mut terminal = setup_terminal()?;
//
//     loop {
//         // 1. Render UI from current state
//         terminal.draw(|frame| ui::render(frame, &state))?;
//
//         // 2. Collect input (non-blocking)
//         if let Some(input_cmd) = poll_input()? {
//             cmd_tx.send(input_cmd).await?;
//         }
//
//         // 3. Process commands → mutate state, spawn async work, emit events
//         while let Ok(cmd) = cmd_rx.try_recv() {
//             let events = dispatch_command(&mut state, cmd, &cmd_tx).await;
//             for event in events {
//                 event_tx.send(event).await?;
//             }
//         }
//
//         // 4. Process events → UI reactions, plugin notifications
//         while let Ok(event) = event_rx.try_recv() {
//             handle_event(&mut state, event);
//         }
//
//         // 5. Check inactivity timeout
//         if should_lock(&state) {
//             cmd_tx.send(Command::Lock).await?;
//         }
//
//         // 6. Check for quit
//         if state.should_quit {
//             break;
//         }
//     }
//
//     restore_terminal()?;
//     Ok(())
// }
// ```
```