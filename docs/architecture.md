# architecture.md — aztui

> Design rationale and architectural contracts.
> This document changes rarely — only when a fundamental design decision is
> made or revised.

---

## Layered Design

The application is organized into four layers. Each layer depends only on the
layers below it, never above.

```
┌─────────────────────────────────────────────────────┐
│  TUI Layer                                          │
│  Ratatui rendering, input capture, widget layout    │
│  src/ui/                                            │
├─────────────────────────────────────────────────────┤
│  Application Layer                                  │
│  Commands, Events, AppState, dispatch loop          │
│  src/app.rs, src/command.rs, src/event.rs           │
├─────────────────────────────────────────────────────┤
│  Domain Layer                                       │
│  Capability traits (AuthProvider, ResourceProvider, │
│  CostProvider), domain models                       │
│  src/domain/, src/providers/                        │
├─────────────────────────────────────────────────────┤
│  Infrastructure Layer                               │
│  AZ CLI adapter, cache, crypto, config I/O          │
│  src/az/, src/cache/, src/security/, src/config/    │
└─────────────────────────────────────────────────────┘
```

**Enforcement strategy**: layers are enforced via module structure and `use`
paths. Concrete types are used within layers; trait boundaries exist at the
seams that matter for testability (primarily `AzCliExecutor`). As the project
matures, extract additional trait boundaries only when a concrete need arises
(a second implementation, a mock, or a genuine decoupling requirement). Avoid
premature trait-object indirection that fights the borrow checker without
earning its keep.

**Future note**: if this project ever outgrows the TUI (e.g. a GUI or web
frontend), the Application and Domain layers are already decoupled from Ratatui
and can be reused. This is a happy side effect of the layering, not a design
goal to optimize for today.

---

## Core Principle: Configuration is First-Class

Configuration is not an afterthought. The `AppConfig` struct is loaded at
startup from `~/.aztui/config.toml`, passed as read-only into `AppState`, and
is the single place where all tunable behavior lives.

**Every new feature must consider its config surface.** When adding a
capability, ask:
- Does this have a tunable parameter (timeout, TTL, default, toggle)?
- If yes, it goes in `AppConfig` under the appropriate section.
- Capabilities declare their config needs; they do not read env vars or
  hard-code values.

Default values must always be sensible so the app works with an empty or absent
config file. Use `serde(default)` extensively. The config file is optional —
aztui must launch and function without one.

The `AppConfig` struct is composed of scoped sub-structs: `GeneralConfig`,
`CacheConfig`, `SecurityConfig`, `UiConfig`, `CliConfig`. New capabilities
that need configuration add a new sub-struct and a corresponding section in the
TOML file.

---

## Command / Event Separation

The application uses a **unidirectional data flow**:

```
User Input → Command → dispatch() → mutate AppState → emit Event(s) → UI re-renders
```

### Commands (request side)

`Command` is an enum representing intent: "something that should happen."
Commands are the ONLY way to mutate `AppState`. They are plain data — no async,
no side effects in construction. The `dispatch_command` function in the
application layer interprets commands, spawns async work, updates state, and
emits events.

### Events (broadcast side)

`Event` is an enum representing facts: "something that happened." Events are
emitted after state has been updated. The UI reads state to render; events are
available for secondary reactions.

Events carry the data they produced (e.g. `ContextChanged(AzureContext)`)
rather than referencing state. This keeps future subscribers self-contained.

### Delivery mechanism

Both commands and events flow through `tokio::sync::mpsc` channels. In Phase 1,
events have a single consumer (the main loop / UI).

**Evolution path**: if a need arises for multiple subscribers to react to the
same event (e.g. a logging plugin, a notification system), upgrade the event
channel to `tokio::sync::broadcast`. The type separation (Command vs Event)
exists from day one so this upgrade is a channel swap, not a redesign.

---

## AppState — Single Source of Truth

All UI rendering reads from `AppState`. All mutations flow through command
dispatch. There is no state stored in widgets, providers, or async tasks.

### Flat structure

`AppState` is intentionally flat (not deeply nested). Fields like `tenants` and
`subscriptions_by_tenant` live at the top level rather than behind an
`auth: AuthState` wrapper. This avoids borrow checker friction where mutable
access to one logical group conflicts with reads from another.

If the state grows large enough to warrant sub-structs, introduce them — but
only when the flat layout causes real confusion, not as a preemptive measure.

### Key fields

- `tenants`, `subscriptions_by_tenant` — the Azure context tree
- `active_context: Option<AzureContext>` — current tenant + subscription
- `recent_contexts: Vec<AzureContext>` — quick switching MRU list
- `active_view: View` — which screen is showing
- `search_query: String` — current filter text
- `modal: Option<Modal>` — overlay state (quick switch, confirm, password)
- `pending_operations: HashMap<OperationId, PendingOperation>` — in-flight
  async work, each carrying a `tokio::task::AbortHandle`
- `locked: bool` — inactivity lock state
- `config: AppConfig` — read-only at runtime
- `last_error: Option<AppError>` — most recent error for the notification bar

See `docs/foundation-types.md` for the full struct definition.

---

## Capability Traits

Instead of a single monolithic `Plugin` trait, domain capabilities are
expressed as **focused, domain-specific trait interfaces**:

- **`AuthProvider`** — login, tenant/subscription listing, context switching
- **`ResourceProvider`** — resource group and resource browsing
- **`CostProvider`** — cost summaries and breakdowns

Each trait is defined in `src/domain/` and implemented by a concrete struct in
`src/providers/` that depends on infrastructure (AZ CLI adapter, cache, etc.).

This enforces domain boundaries. A struct can implement multiple capabilities
if appropriate, but cannot accumulate unrelated responsibilities by default.
New capabilities are added by defining a new trait and a new provider — existing
code does not change.

---

## AZ CLI Adapter + Normalization

All `az` subprocess calls go through the `AzCliExecutor` trait:

```rust
#[async_trait]
pub trait AzCliExecutor: Send + Sync {
    async fn execute(
        &self,
        args: &[&str],
        timeout: Duration,
    ) -> Result<String, AppError>;
}
```

### Concrete implementation (`SubprocessCliExecutor`)

- Spawns `az` via `tokio::process::Command`
- Captures stdout/stderr
- Enforces timeout
- Maps exit codes to `AppError`

### Normalization layer

Raw JSON from `az` is **never** passed to the domain layer. A parsing/mapping
layer (`src/az/parser.rs`) converts CLI JSON into internal domain models
(`Tenant`, `Subscription`, `ResourceGroup`, etc.). This protects the rest of
the system from CLI output format changes, inconsistencies, and parsing edge
cases. If Microsoft changes the JSON schema, only the parser changes.

### Testability

`AzCliExecutor` is the primary mockable boundary. In tests, swap in a
`MockCliExecutor` that returns canned JSON strings. The entire capability layer
works without the `az` binary or Azure access.

---

## Async Model & Task Cancellation

The TUI event loop runs on the main Tokio task. Long-running operations (CLI
calls) are spawned as separate tasks via `tokio::spawn`.

### Race condition handling

When a user triggers a new operation that supersedes a previous one (e.g.
switching subscriptions rapidly), the previous task is **cancelled** via its
`AbortHandle`. There is only ever one active task per "slot" (e.g. one active
context-switch operation). Cancelled tasks are dropped; their results are
discarded.

We use task cancellation rather than request versioning because:
- Simpler mental model (one task per slot, not ID comparison on completion)
- Lower memory usage (no accumulation of stale in-flight requests)
- Idiomatic Tokio (AbortHandle is built for this)

### Dispatch flow

1. Check if there's an existing operation for the same slot.
2. If yes, abort it via `AbortHandle`.
3. Spawn the new task, store its `AbortHandle` in `PendingOperation`.
4. On completion, the task sends a result back via the command channel.

---

## Caching Strategy

Caching is essential because `az` CLI commands are slow (often 1–3 seconds).
The cache lives in the infrastructure layer and is used by capability providers.

### TTL model (three tiers)

- **Fresh** (within soft TTL): serve immediately, no refresh.
- **Stale** (past soft TTL, within hard TTL): serve from cache, trigger a
  background refresh. UI shows cached data instantly while fresh data loads.
- **Expired** (past hard TTL): force a synchronous refresh before serving.

Users can always force a manual refresh via keybinding or command.

### Default TTLs (configurable in `config.toml`)

| Data type                | Soft TTL | Hard TTL |
|--------------------------|----------|----------|
| Tenant/subscription list | 5 min    | 1 hour   |
| Resource listings        | 2 min    | 30 min   |
| Cost data                | 15 min   | 2 hours  |

### Scoping

Every cache entry is keyed by `CacheScope` + a kind string:

- `CacheScope::Global` — data like the tenant list itself
- `CacheScope::Tenant(id)` — data scoped to a tenant (e.g. subscription list)
- `CacheScope::Subscription(id)` — data scoped to a subscription

This prevents cross-tenant/cross-subscription data leakage.

### Identity binding

Cache is invalidated when the authenticated identity changes (detected via
`az account show`). A tenant switch implies a new identity context, so
tenant-scoped and subscription-scoped caches are cleared.

---

## Error Handling

All errors flow through a single `AppError` type:

```rust
pub struct AppError {
    pub kind: ErrorKind,
    pub message: String,
    pub recovery: Option<RecoveryAction>,
    pub source_detail: Option<String>,
}
```

### ErrorKind categories

- **Auth**: `AuthExpired`, `AuthFailed`, `TenantNotFound`, `SubscriptionNotFound`
- **CLI**: `CliNotFound`, `CliExecutionFailed`, `CliTimeout`, `CliParseError`
- **Network**: `NetworkError`
- **Security**: `MasterPasswordWrong`, `CacheDecryptionFailed`
- **System**: `ConfigError`, `CacheError`, `Unknown`

### Recovery actions

`RecoveryAction` tells the UI what to offer the user:

- `ReLogin` — prompt re-authentication
- `LoginToTenant(id)` — prompt login to a specific tenant
- `Retry(Command)` — retry the failed command
- `OpenSettings` — open config to fix a configuration issue
- `Manual(hint)` — display a hint, let the user decide

Infrastructure code maps low-level errors into `AppError` with appropriate
kind, message, and recovery. The UI never needs to interpret raw error types.

---

## Security Model

The TUI's security layer protects **local cached data and preferences**, not
Azure credentials (those are managed by `az` CLI in `~/.azure/`).

### What the security layer protects

- Encrypted cache at rest (tenant lists, resource data, cost data)
- Saved preferences (pinned tenants, recent contexts, aliases)
- Casual access prevention (someone at an unlocked workstation)

### What it does NOT protect

- Azure CLI tokens in `~/.azure/` (accessible to anything running as the user)
- Data in transit (handled by Azure/TLS)

### Implementation (Phase 2)

- **Master password** (optional): user sets a password on first setup. Key
  derived via Argon2id, used to encrypt/decrypt local cache with AES-256-GCM.
  On launch: password prompt → derive key → decrypt cache. Failure locks out
  cached data but the app still works with fresh CLI calls.
- **Inactivity lock**: after configurable timeout (default 10 min), the app
  locks and requires the master password. Can be disabled.
- **OS keyring** (optional alternative): store the derived key in the OS
  keyring instead of prompting each launch. Configurable via
  `security.use_os_keyring` in config.

---

## UX Design

See `docs/tui-wireframe.md` for visual reference of all views and overlays.

### Context Switcher (main view)

The primary view and the application's core value. Design goals: instant
feedback, minimal keystrokes to switch context.

- Tenants displayed as section headers
- Subscriptions nested under their tenant
- Arrow keys / j/k to navigate, Enter to select
- Typing filters the list instantly (fuzzy match)
- Active context highlighted distinctly
- Disabled/warned subscriptions shown but visually dimmed

### Quick Switch (Ctrl+P modal)

Inspired by VS Code's command palette. For operators who know what they want.

- Full-screen overlay with search input at top
- All contexts (tenant + subscription pairs) in a flat list
- Fuzzy search narrows as you type
- Recent contexts pinned at top (MRU order)
- Enter to switch, Escape to dismiss

### Status Bar

Persistent awareness of current state.

- Always visible (bottom by default, configurable)
- Shows: active tenant name, active subscription name, pending operation
  indicator (spinner + description), last error summary
- Selecting the error expands to error detail modal

### Design principles

- **Zero-delay UI**: the TUI never blocks on Azure. Cached data appears
  instantly; refreshes happen in the background.
- **Progressive disclosure**: show the essential information first, let the
  user drill down for details.
- **Keyboard-first**: every action reachable without mouse. Mouse support
  available but optional.
- **Recoverable errors**: errors always suggest a next step, never dead-ends.

---

## Application Event Loop

Simplified sketch of the main loop showing how all pieces connect:

```
┌──────────────┐
│  Terminal     │
│  Input        │──→ Command ──→ dispatch_command()
└──────────────┘                      │
                                      ├──→ mutate AppState
                                      ├──→ spawn async tasks (CLI calls)
                                      └──→ emit Event(s)
                                                │
                              ┌─────────────────┘
                              ▼
                        handle_event()
                              │
                              └──→ secondary state updates
                                   (future: notify subscribers)

                     ┌──────────────┐
AppState ──────────→ │  UI Render   │ ──→ terminal output
                     └──────────────┘
```

The loop runs continuously:
1. Render UI from current `AppState`
2. Poll terminal input (non-blocking)
3. Map input to `Command`, send to command channel
4. Process pending commands: mutate state, spawn work, emit events
5. Process pending events: secondary reactions
6. Check inactivity timeout
7. Repeat

Async tasks (CLI calls) send their results back as commands via the same
channel, which the loop picks up in step 4 on the next iteration.

---

## Reference

- `docs/foundation-types.md` — authoritative type definitions for all structs,
  enums, and traits described in this document.
- `docs/tui-wireframes.md` - visual reference of all views and overlays.
- `docs/color-scheme.md` - visual reference for the color scheme of the TUI.
- `CLAUDE.md` — operational guidance, conventions, current phase scope.