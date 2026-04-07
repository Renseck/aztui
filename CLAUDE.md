# aztui

> A Terminal User Interface for Azure CLI operations, built in Rust.
> This file is the single source of truth for the project's architecture,
> conventions, and implementation guidance.

---

## Project Overview

**aztui** is a TUI wrapper around the Azure CLI (`az`) that eliminates the
friction of multi-tenant/subscription management, flag memorization, and
repetitive CLI workflows. It is designed for operations teams who work across
many Azure tenants and subscriptions daily.

The application does not replace `az` - it delegates all Azure interactions to
the `az` binary as a subprocess. This means we inherit the security model,
authentication flows, and API coverage of the official CLI without maintaining
Azure SDK bindings ourselves.

### Key value proposition

The killer feature is **fast context switching**: selecting a tenant and
subscription from a searchable list and having `az login --tenant` +
`az account set --subscription` happen behind the scenes in one action. Every
other feature builds on top of this foundation.

---

## Technology Stack

| Concern            | Choice                        | Rationale                                              |
|--------------------|-------------------------------|--------------------------------------------------------|
| Language           | Rust                          | Single binary distribution, zero-overhead, type safety |
| TUI framework      | Ratatui + crossterm           | Best-in-class TUI ecosystem, cross-platform            |
| Async runtime      | Tokio                         | Industry standard, needed for non-blocking CLI calls   |
| Serialization      | serde + serde_json            | AZ CLI outputs JSON; serde is the Rust standard        |
| Config format      | TOML (via `toml` crate)       | Human-friendly, Rust ecosystem convention              |
| CLI arg parsing    | clap                          | For aztui's own launch flags                           |
| Fuzzy matching     | nucleo or fuzzy-matcher       | For Ctrl+P style context switching                     |
| Crypto             | argon2 + aes-gcm (RustCrypto) | Master password KDF + cache encryption (Phase 2)       |
| OS keyring         | keyring crate                 | Optional alternative to master password (Phase 2)      |

---

## Project Structure

```
aztui/
├── Cargo.toml
├── CLAUDE.md                           # this file
├── docs/
│   ├── foundation-types.md             # reference type definitions
│   ├── styleguide.md                   # Rust coding style reference
│   └── architecture.md                 # deign rational and architectural contract
├── src/
│   ├── main.rs                         # entry point, terminal setup, bootstrap
│   ├── app.rs                          # AppState, main event loop
│   ├── command.rs                      # Command enum
│   ├── event.rs                        # Event enum
│   │
│   ├── domain/                         # domain models + capability traits
│   │   ├── mod.rs
│   │   ├── models.rs                   # Tenant, Subscription, ResourceGroup, etc.
│   │   ├── auth.rs                     # AuthProvider trait
│   │   ├── resources.rs                # ResourceProvider trait
│   │   └── cost.rs                     # CostProvider trait
│   │
│   ├── providers/                      # concrete capability implementations
│   │   ├── mod.rs
│   │   ├── auth_provider.rs            # AuthProvider impl using AZ CLI
│   │   ├── resource_provider.rs        # ResourceProvider impl (Phase 3)
│   │   └── cost_provider.rs            # CostProvider impl (Phase 4)
│   │
│   ├── az/                             # AZ CLI infrastructure
│   │   ├── mod.rs
│   │   ├── executor.rs                 # AzCliExecutor trait + SubprocessCliExecutor
│   │   ├── parser.rs                   # JSON → domain model mapping
│   │   └── commands.rs                 # command builder helpers (arg construction)
│   │
│   ├── cache/                          # caching infrastructure
│   │   ├── mod.rs
│   │   └── store.rs                    # CacheEntry, CacheScope, TTL logic
│   │
│   ├── security/                       # encryption and auth (Phase 2)
│   │   ├── mod.rs
│   │   ├── master_key.rs               # Argon2id key derivation
│   │   └── crypto.rs                   # AES-256-GCM encrypt/decrypt
│   │
│   ├── config/                         # configuration loading
│   │   ├── mod.rs
│   │   └── settings.rs                 # AppConfig, TOML deserialization, defaults
│   │
│   ├── error.rs                        # AppError, ErrorKind, RecoveryAction
│   │
│   └── ui/                             # TUI layer
│       ├── mod.rs
│       ├── layout.rs                   # screen layout and composition
│       ├── theme.rs                    # colors, styles, visual identity
│       ├── input.rs                    # keypress → Command mapping
│       └── widgets/                    # reusable TUI components
│           ├── mod.rs
│           ├── context_switcher.rs     # tenant/subscription list view
│           ├── quick_switch.rs         # Ctrl+P fuzzy finder modal
│           ├── status_bar.rs           # active context, pending ops, errors
│           ├── search_input.rs         # filter/search text field
│           └── modal.rs               # generic modal overlay
│
└── tests/
    ├── mock_executor.rs                # MockCliExecutor for testing
    └── ...
```

## Phase Plan

### Current Phase 1 - Skeleton + Tenant/Subscription Switcher

**Goal**: a working TUI that lets you list tenants/subscriptions, select one,
and have `az login --tenant` + `az account set --subscription` run behind the
scenes. This validates the architecture end-to-end.

**Scope**:
- `main.rs`: terminal setup, config loading, event loop bootstrap
- `AppState`, `Command`, `Event` types (core subset)
- `AzCliExecutor` trait + `SubprocessCliExecutor`
- `parser.rs`: parse `az account list --all` and `az account show` JSON
- `AuthProvider` trait + concrete implementation
- `CacheStore` with soft/hard TTL for context list
- `config.toml` loading with sensible defaults
- `AppError` type with full error model
- UI: context switcher view, status bar, search/filter, quick switch modal
- Keybindings: navigate, select, search, Ctrl+P quick switch, quit

**Not in scope for Phase 1**: master password, resource browsing, cost
explorer, OS keyring integration.

### Phase 2 - Security Layer

Master password setup, Argon2id key derivation, AES-256-GCM cache encryption,
inactivity lock, optional OS keyring integration.

### Phase 3 - Resource Browser

`ResourceProvider` trait + implementation. Browse resource groups and
resources within the active subscription. Drill-down navigation.

### Phase 4 - Cost Explorer (FinOps)

`CostProvider` trait + implementation. Cost summaries by subscription and
resource group. Period selection. Tabular breakdown by service.

### Future features

- **Resource-level metric querying**. By the time a user has navigated to a specific resource in the resource browser, we have the full ARM resource ID, the resource type, and the subscription context - everything Azure Monitor needs. We could offer a predefined set of common metrics per resource type (CPU% for VMs, DTU% for SQL, request count for App Services) and render sparkline charts inline using Ratatui's built-in sparkline widget. The `az monitor metrics list` command returns JSON that fits neatly into the existing adapter/parser pattern.
- **Resource action shortcuts**. Once you're looking at a resource, offer contextual actions: start/stop/restart a VM or App Service, scale a tier, view recent activity log entries. The resource type is known, so we can present only the actions that make sense. This turns aztui from a read-only browser into an operational tool.
- **Log tail** / **Activity Log viewer**. Stream recent activity log entries for a subscription or resource group. Not full Log Analytics (that's a rabbit hole), but `az monitor activity-log list` filtered to the current scope. Useful for "what just happened?" debugging.
- **Policy and compliance dashboard**. `az policy state list` to show non-compliant resources per subscription. For an ops team managing multiple tenants, a quick "where are we out of compliance?" view could save a lot of portal clicking.
- **Tag management**. Bulk view and edit tags across resource groups or resources. Tags are one of those things that are painful in the portal and painful in CLI - a TUI with multi-select and batch tagging would be genuinely useful.
- **Cost anomaly alerts**. Build on the Phase 4 cost explorer: compare current period cost to the previous period, highlight significant deviations. No need for a full alerting pipeline - just visual indicators in the cost view (arrows, color coding, percentage deltas).
- **Deployment status viewer**. `az deployment group list` to show recent ARM deployments, their status, and error messages. When a deployment fails, you currently have to dig through the portal or chain several CLI calls to find the error - the TUI could flatten that into a single drill-down.
- **Network topology overview**. A simplified view of VNets, subnets, NSGs, and peerings for the current subscription. Not a full diagram, but a tree structure that makes the network layout scannable. This is notoriously hard to get a quick picture of.
- **Saved command snippets** / **favorites**. Let operators bookmark commonly used `az` commands (with or without pre-filled arguments) and run them from the TUI. Like shell aliases but scoped to the current Azure context and searchable.
- **Export and reporting**. Export any current view (resource list, cost breakdown, compliance status) to CSV or JSON. Useful for feeding into team reports or pasting into tickets.
- **Multi-subscription comparison views**. Side-by-side comparisons: cost across subscriptions, resource counts, compliance scores. For a team managing many subscriptions across tenants, this "fleet overview" perspective is something the portal actively makes difficult.
- **RBAC viewer**. `az role assignment list` for the current scope, showing who has what access. Useful for security reviews and onboarding - "who can touch this subscription?" is a question ops teams answer constantly.

---

## Coding Conventions

### Style Guide

This project follows a custom Rust style guide defined in `docs/styleguide.md` at
the repository root. Key points summarized here for quick reference:

**File structure**:
- All `use` statements at the top of files.
- Use section separators to organize code within files. Separator style
  corresponds to the level of separation:
  - Between functions in the same type:
    ```rust
    /* ============================================================================================== */
    ```
  - Category headers within a type:
    ```rust
    /* ========================================== Category ========================================== */
    ```
  - Major concern boundaries (e.g. public vs private):
    ```rust
    /* ============================================================================================== */
    /*                                        Private methods                                         */
    /* ============================================================================================== */
    ```
  - Maximum separator length: 100 characters, indented to match the code level.
  - One blank line after a closing brace before a separator. Start the next
    function (or its doc comment) directly after the separator line.

**Indentation**: K&R style (opening brace on same line). Tabs are 4 spaces.

**Naming**: `snake_case` for variables and functions, `PascalCase` for types.
Descriptive names that indicate purpose.

**Error handling**: always use `Result`. No `.unwrap()` or `.expect()` in
production paths.

**Documentation**: rustdoc format on all public items. Include parameter types,
return types, and a brief description. Add `# Examples` sections for
non-trivial APIs.

**Comments**: `//` for brief explanations, `/* */` for longer ones. Explain
"why", not "what". Avoid obvious comments.

Consult `docs/styleguide.md` for the full specification and examples.

---

### Rust-Specific Conventions

- **No `.unwrap()` / `.expect()`** in non-test code. Use `?` operator with
  proper `From` impls or explicit error mapping.
- **Derive liberally**: `Debug, Clone` on most types. Add `PartialEq, Eq,
  Hash` where comparison or map-keying is needed.
- **Use `async_trait`** for async trait definitions until Rust stabilizes
  async-in-traits.
- **Prefer concrete types** within a layer. Use `dyn Trait` at layer
  boundaries only when there's a real second implementation (production vs
  mock).
- **Minimize `pub`**: expose only what's needed. Use `pub(crate)` for
  cross-module access within the crate.

---

### Commit Conventions

Use conventional commits:

```
feat(auth): add tenant fuzzy search in context switcher
fix(cache): prevent stale data after identity change
refactor(az): extract JSON parsing into dedicated module
docs: update CLAUDE.md with Phase 2 security spec
```

---

## Key Dependencies (Cargo.toml)

```toml
[dependencies]
ratatui = "0.30.0"
crossterm = "0.29.0"
tokio = { version = "1.51.0", features = ["full"] }
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.149"
toml = "1.1.2"
clap = { version = "4.6.0", features = ["derive"] }
async-trait = "0.1.89"
dirs = "6.0.0"
chrono = "0.4.44"

# Phase 2
# aes-gcm = "0.10.3"
# argon2 = "0.5.3"
# keyring = "3.6.3"
```

Version pins are approximate - use the latest compatible versions at project
init.

---

## Keybindings (Phase 1)

| Key         | Action                        |
|-------------|-------------------------------|
| ↑/↓ or j/k  | Navigate list                 |
| Enter       | Select / confirm              |
| /           | Focus search input            |
| Esc         | Clear search / close modal    |
| Ctrl+P      | Open quick switch             |
| r           | Refresh current view          |
| q           | Quit                          |
| ?           | Show help                     |

---

## Reference Files

- `docs/architecture.md` - layered design, data flow, capability traits, caching,
error model, security model, UX design rationale.
- `docs/foundation-types.md` - complete type definitions for AppState,
  Command, Event, capability traits, error model, cache, and config.
  This is the authoritative reference for type structures. If this CLAUDE.md
  and the foundation file ever disagree, the foundation file wins for type
  definitions and this file wins for architectural decisions.
- `docs/tui-wireframes.md` - visual reference of all views and overlays.
- `docs/styleguide.md` - full Rust coding style specification.

If CLAUDE.md and `foundation-types.md` disagree, foundation-types.md wins for
type definitions. If CLAUDE.md and architecture.md disagree, architecture.md
wins for design decisions.