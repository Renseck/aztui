# PLAN.md — aztui Phase 1 Implementation Roadmap

## Plan: Phase 1 — Skeleton + Tenant/Subscription Switcher

**TL;DR**: Implement the end-to-end TUI that lets operators list tenants/subscriptions, select one, and have `az login --tenant` + `az account set --subscription` run behind the scenes. Build bottom-up from infrastructure → domain → application → UI, validating each layer with tests before moving up.

---

## Phase 1A — Foundation Types & Infrastructure

Build the types everything depends on and the lowest layer of the stack.

### Step 1: Error model (`src/errors.rs`)
- Implement `AppError`, `ErrorKind`, `RecoveryAction` per foundation-types.md §6
- Implement `Display`, `std::error::Error` for `AppError`
- Add convenience constructors for common error kinds (e.g. `AppError::cli_not_found()`, `AppError::cli_parse_error()`)
- This is the foundation — everything returns `Result<T, AppError>`

### Step 2: Domain models (`src/domain/models.rs`, `src/domain/mod.rs`)
- Implement `Tenant`, `Subscription`, `SubscriptionState`, `ResourceGroup`, `Resource`, `AzureContext` per foundation-types.md §1
- Phase 3/4 types (`CostSummary`, `CostScope`, `CostPeriod`, `CostLineItem`) can be defined as stubs or skipped entirely — they're not needed yet
- Wire up `src/domain/mod.rs` to re-export types

### Step 3: Configuration (`src/config/settings.rs`, `src/config/mod.rs`)
- Implement `AppConfig`, `GeneralConfig`, `CacheConfig`, `SecurityConfig`, `UiConfig`, `CliConfig`, `StatusBarPosition` per foundation-types.md §9
- Add `#[derive(Deserialize)]` with `#[serde(default)]` on everything for TOML loading
- Implement `Default` for all config structs with sensible defaults from architecture.md
- Implement `AppConfig::load(path: Option<PathBuf>) -> Result<AppConfig, AppError>`:
  - If path provided, load from there
  - Else look for `~/.aztui/config.toml` (use `dirs::home_dir()`)
  - If file doesn't exist, return defaults (app must work without config)
  - Parse TOML, merge with defaults via serde
- Wire up `src/config/mod.rs`

### Step 4: AZ CLI executor (`src/az/executor.rs`, `src/az/mod.rs`)
- Define `AzCliExecutor` trait per foundation-types.md §7
- Implement `SubprocessCliExecutor`:
  - `az_path: Option<PathBuf>` (from config, None = find on PATH)
  - Uses `tokio::process::Command` to spawn `az` with `--output json`
  - Captures stdout and stderr
  - Enforces timeout via `tokio::time::timeout`
  - Maps non-zero exit codes → `AppError` with `CliExecutionFailed`
  - Maps timeout → `AppError` with `CliTimeout`
  - Maps missing binary → `AppError` with `CliNotFound`
  - Returns raw stdout `String` on success

### Step 5: AZ CLI command builders (`src/az/commands.rs`)
- Helper functions that construct argument slices for each `az` command:
  - `account_list_all() -> Vec<&str>` — `["account", "list", "--all"]`
  - `account_show() -> Vec<&str>` — `["account", "show"]`
  - `login() -> Vec<&str>` — `["login"]`
  - `login_tenant(tenant_id) -> Vec<&str>` — `["login", "--tenant", tenant_id]`
  - `account_set(subscription_id) -> Vec<&str>` — `["account", "set", "--subscription", subscription_id]`
- These are pure functions, no I/O

### Step 6: AZ CLI JSON parser (`src/az/parser.rs`)
- `parse_account_list(json: &str) -> Result<(Vec<Tenant>, HashMap<String, Vec<Subscription>>), AppError>`
  - Parses `az account list --all` JSON output
  - Deduplicates tenants (multiple subs share a tenant)
  - Groups subscriptions by tenant_id
  - Maps `state` field to `SubscriptionState` enum
- `parse_account_show(json: &str) -> Result<AzureContext, AppError>`
  - Parses `az account show` JSON output
  - Returns current active tenant + subscription
- Use `serde_json::Value` or dedicated intermediate structs for raw JSON shapes
- Map parse failures to `AppError` with `CliParseError`

### Step 7: Cache store (`src/cache/store.rs`, `src/cache/mod.rs`)
- Implement `CacheEntry<T>`, `CacheScope`, `CacheKey` per foundation-types.md §8
- Implement `CacheStore`:
  - Internal storage: `HashMap<CacheKey, Box<dyn Any + Send + Sync>>`
  - `get<T>(&self, key: &CacheKey) -> Option<&CacheEntry<T>>`
  - `put<T>(&mut self, key: CacheKey, value: T, soft_ttl: Duration, hard_ttl: Duration)`
  - `invalidate(&mut self, key: &CacheKey)`
  - `invalidate_scope(&mut self, scope: &CacheScope)` — clear all entries in a scope
  - `invalidate_all(&mut self)`
- Thread safety: wrap in `Arc<RwLock<...>>` or make the store itself use `RwLock` internally

### Step 8: Mock executor for tests (`tests/mock_executor.rs`)
- Implement `MockCliExecutor` that implements `AzCliExecutor`
- Stores a `Vec<(Vec<String>, Result<String, AppError>)>` — expected args → canned response
- Useful for testing parser, auth provider, and integration tests without `az` binary

**Verification for Phase 1A**:
- `cargo build` compiles successfully
- Unit tests for parser with sample `az account list --all` and `az account show` JSON
- Unit tests for cache TTL logic (fresh/stale/expired)
- Unit tests for config default loading and TOML deserialization
- Unit tests for error model (Display impl, kind matching)

---

## Phase 1B — Domain & Provider Layer

Build the capability traits and their concrete implementations.

### Step 9: Auth capability trait (`src/domain/auth.rs`)
- Define `AuthProvider` trait per foundation-types.md §5
- Only define the trait + wire up in `src/domain/mod.rs`

### Step 10: Auth provider implementation (`src/providers/auth_provider.rs`, `src/providers/mod.rs`)
- Implement `AzAuthProvider` struct:
  - Holds `Arc<dyn AzCliExecutor>` and `Arc<RwLock<CacheStore>>` and `CacheConfig`
  - `login()`: calls `az login`, then `az account list --all`, parses, returns tenants
  - `login_to_tenant(id)`: calls `az login --tenant <id>`, then `az account list --all`, parses
  - `list_contexts()`:
    - Check cache for tenant/subscription data
    - If fresh → return cached
    - If stale → return cached + spawn background refresh
    - If expired → synchronous refresh via `az account list --all`
  - `set_subscription(id)`: calls `az account set --subscription <id>`
  - `get_active_context()`: calls `az account show`, parses into `AzureContext`
- Each method maps `AzCliExecutor` errors into appropriate `AppError` with recovery actions

### Step 11: Stub Phase 3/4 traits (`src/domain/resources.rs`, `src/domain/cost.rs`)
- Define `ResourceProvider` and `CostProvider` traits as specified
- Mark as `// Phase 3` / `// Phase 4` — no implementations needed yet
- Stub providers in `src/providers/resource_provider.rs` and `src/providers/cost_provider.rs` can remain empty

### Step 12: Stub security module (`src/security/`)
- `mod.rs` just declares submodules
- `master_key.rs` and `crypto.rs` remain empty with `// Phase 2` comments

**Verification for Phase 1B**:
- Integration tests using `MockCliExecutor` to verify `AzAuthProvider`:
  - `list_contexts()` returns correct tenants/subscriptions from canned JSON
  - `login_to_tenant()` calls correct `az` args
  - `set_subscription()` calls correct `az` args
  - Cache behavior: second call returns cached data without executor call
- `cargo test` passes

---

## Phase 1C — Application Layer

Wire up the event loop, commands, events, and state management.

### Step 13: Command enum (`src/command.rs`)
- Implement `Command` enum per foundation-types.md §3
- Phase 1 subset: `Login`, `LoginToTenant`, `SetSubscription`, `RefreshContextList`, `SwitchContext`, `NavigateTo`, `UpdateSearch`, `OpenModal`, `CloseModal`, `Lock`, `Unlock`, `Quit`, `InvalidateAllCaches`, `CancelOperation`
- Phase 3/4 variants (`ListResourceGroups`, `ListResources`, `FetchCostSummary`) included as variants but unhandled in dispatch

### Step 14: Event enum (`src/event.rs`)
- Implement `Event` enum per foundation-types.md §4
- Include all variants; Phase 3/4 variants won't be emitted yet

### Step 15: AppState + event loop (`src/app.rs`)
- Implement `AppState` per foundation-types.md §2:
  - All fields as specified
  - `View` and `Modal` enums
  - `PendingOperation` struct with `AbortHandle`
  - Constructor: `AppState::new(config: AppConfig) -> Self`
  - `should_quit: bool` field (not in foundation types but needed per loop sketch)
- Implement `dispatch_command(state: &mut AppState, cmd: Command, ...)`:
  - Match on `Command` variants
  - For async commands (Login, LoginToTenant, SetSubscription, RefreshContextList, SwitchContext):
    - Cancel existing operation in same slot via AbortHandle
    - Spawn tokio task that calls the appropriate `AuthProvider` method
    - Store `PendingOperation` in state
    - On completion, send result back via command channel
  - For sync commands (NavigateTo, UpdateSearch, OpenModal, CloseModal, Quit):
    - Mutate state directly
    - Emit corresponding event
  - `SwitchContext` is the key flow: check if login needed → `LoginToTenant` → `SetSubscription` → update `active_context` + `recent_contexts`
- Implement `handle_event(state: &mut AppState, event: Event)`:
  - Secondary state reactions to events
  - e.g. `LoginCompleted` → clear error bar, `ErrorOccurred` → set `last_error`

### Step 16: Main entry point (`src/main.rs`)
- Terminal setup using `crossterm`:
  - `enable_raw_mode()`
  - `EnterAlternateScreen`
  - Create Ratatui `Terminal` with `CrosstermBackend`
- Load config via `AppConfig::load(None)`
- Create `AppState::new(config)`
- Create channels: `mpsc::channel::<Command>` and `mpsc::channel::<Event>`
- Create `SubprocessCliExecutor` from config
- Create `CacheStore`
- Create `AzAuthProvider` with executor + cache
- On startup: dispatch `RefreshContextList` to populate initial data
- Main loop per architecture.md §10 sketch:
  1. Render UI
  2. Poll input (non-blocking via crossterm with timeout)
  3. Process commands
  4. Process events
  5. Check quit
- Cleanup: `disable_raw_mode()`, `LeaveAlternateScreen`
- Panic hook to restore terminal on crash

**Verification for Phase 1C**:
- App launches, shows empty TUI, and quits on `q`
- `RefreshContextList` triggers `az account list --all` subprocess call
- State updates correctly on command dispatch
- `cargo run` works end-to-end (requires `az` CLI installed)

---

## Phase 1D — TUI Layer

Build the visual interface top-to-bottom.

### Step 17: Theme (`src/ui/theme.rs`)
- Implement `Theme` struct with all color fields from color-scheme.md
- `Theme::default_dark() -> Self` using the hex values from color-scheme.md
- `Theme::default_256() -> Self` for 256-color terminals (approximate mappings)
- Helper: detect truecolor support via `COLORTERM` env var
- Convenience methods: `Theme::selected_style()`, `Theme::dimmed_style()`, etc. that return `ratatui::style::Style`

### Step 18: Layout (`src/ui/layout.rs`)
- Main `render(frame: &mut Frame, state: &AppState, theme: &Theme)` function
- Layout structure per wireframes:
  - Title bar (row 0): "aztui" branding + view indicator
  - Content area (rows 1 to h-2): delegates to active view widget
  - Status bar (row h-1): active context, pending ops, errors
- Modal overlay rendering: if `state.modal.is_some()`, render the modal on top
- Use `ratatui::layout::Layout` with constraints for splitting areas

### Step 19: Input handler (`src/ui/input.rs`)
- `handle_input(key: KeyEvent, state: &AppState) -> Option<Command>`
- Map keypresses to commands per keybinding table in CLAUDE.md:
  - `↑/k` → navigate up, `↓/j` → navigate down
  - `Enter` → select/confirm
  - `/` → focus search (`UpdateSearch`)
  - `Esc` → close modal / clear search
  - `Ctrl+P` → `OpenModal(QuickSwitch)`
  - `r` → `RefreshContextList`
  - `q` → `Quit`
  - `?` → `NavigateTo(Help)`
- Context-aware: in search mode, printable chars go to search; in modal, keys go to modal handler
- Return `None` for unhandled keys

### Step 20: Status bar widget (`src/ui/widgets/status_bar.rs`)
- Renders the bottom status bar per wireframe §1
- Left: active context (tenant name / subscription name) or "No context selected"
- Center: pending operation spinner + description (if any)
- Right: error summary (if any), refresh indicator
- Use `Theme` colors: `status_bar_bg`, status bar text colors

### Step 21: Context switcher widget (`src/ui/widgets/context_switcher.rs`)
- The main view — hierarchical tenant/subscription list per wireframe §1
- Renders tenants as section headers (bold, tenant color)
- Subscriptions indented underneath each tenant
- Active subscription highlighted with `[✓]` marker and azure color
- Disabled subscriptions shown dimmed
- Stateful list: tracks selected index for navigation
- Search filtering: when `state.search_query` is non-empty, filter tenants/subscriptions by fuzzy match on name
- Navigation: `select_next()`, `select_previous()` called via commands
- Enter on a subscription → `SwitchContext` command

### Step 22: Search input widget (`src/ui/widgets/search_input.rs`)
- Inline search bar that appears when `/` is pressed
- Renders search text with cursor
- Per wireframe: appears at top of content area when active
- On each keystroke: dispatches `UpdateSearch(new_text)` which triggers list filtering

### Step 23: Quick switch modal (`src/ui/widgets/quick_switch.rs`)
- Ctrl+P overlay per wireframe §2
- Centered modal (60-70% width per wireframes)
- Search input at top
- Flat list of all `AzureContext` pairs (tenant + subscription)
- "RECENT" section at top showing `state.recent_contexts`
- Fuzzy search filtering as user types
- Enter → `SwitchContext(selected)`, Esc → `CloseModal`
- Note: fuzzy matching can use simple substring match for Phase 1; proper fuzzy scoring (nucleo/fuzzy-matcher) can be added later

### Step 24: Modal widget (`src/ui/widgets/modal.rs`)
- Generic modal overlay component
- Renders a centered bordered box on top of existing content
- Used by quick switch, error detail, confirmation dialog, help view
- Takes title, content area, optional footer (keybinding hints)

### Step 25: Help view
- Simple keybinding reference per wireframe §5
- Renders the keybinding table from CLAUDE.md
- Accessible via `?` key
- Can be a dedicated view or a modal — wireframe shows it as a view

### Step 26: Widget module wiring (`src/ui/widgets/mod.rs`, `src/ui/mod.rs`)
- Re-export all widgets
- Wire up `src/ui/mod.rs` to expose `render`, `handle_input`, `Theme`

**Verification for Phase 1D**:
- `cargo run` shows the context switcher with tenants/subscriptions loaded from `az account list --all`
- Navigation with j/k/arrows works, selected row highlights
- `/` opens search, typing filters the list, Esc clears
- `Ctrl+P` opens quick switch modal
- Enter on a subscription triggers context switch (visible in status bar)
- `q` quits cleanly
- Status bar shows active context after selection
- Error bar appears if `az` command fails

---

## Phase 1E — Polish & Hardening

### Step 27: Loading / first launch state
- Per wireframe §9: show spinner animation on initial data load
- If `az` not found on PATH, show helpful error with `CliNotFound` recovery
- If no cached data and first load is slow, show "Loading tenants..."

### Step 28: Error detail modal
- Per wireframe §7: when user selects error in status bar, show detail modal
- Displays `AppError` fields: kind as title, message as body, source_detail, recovery action as button

### Step 29: Confirmation dialog
- Per wireframe §8: generic yes/no modal
- Used for tenant switch confirmation (if switching requires re-login)

### Step 30: Terminal panic hook
- Install custom panic hook that restores terminal before printing panic
- Prevents corrupted terminal on crash

### Step 31: Clap CLI args
- `--config <path>` to override config file location  
- `--version` / `-V`
- Keep minimal for Phase 1

**Verification for Phase 1E (full Phase 1)**:
- Manual testing: full workflow from launch → select tenant → select subscription → verify `az account show` reflects the change
- Manual testing: search filtering, quick switch, help view
- Manual testing: error scenarios (no `az` binary, expired auth, bad tenant ID)
- `cargo test` — all unit and integration tests pass
- `cargo clippy` — no warnings
- `cargo fmt --check` — formatted per rustfmt.toml
- Terminal restores cleanly on quit and on panic

---

## Dependency Graph

```
Step 1 (errors) ──────────────────────────────┐
Step 2 (domain models) ──┐                    │
Step 3 (config) ─────────┤                    │
                          ├→ Step 4 (executor) ├→ Step 6 (parser)
                          │  Step 5 (commands) ─┘        │
                          │                              │
                          ├→ Step 7 (cache) ─────────────┤
                          │                              │
Step 8 (mock executor) ──────────────────────────────────┤
                                                         │
Step 9 (auth trait) ─────────────────────────────────────┤
                                                         ▼
                                               Step 10 (auth provider)
                                                         │
Step 11 (stub traits) ─── parallel with 9-10             │
Step 12 (stub security) ─ parallel with 9-10             │
                                                         │
Step 13 (Command enum) ─┬→ Step 15 (AppState + loop)    │
Step 14 (Event enum) ───┘           │                    │
                                    ▼                    │
                          Step 16 (main.rs) ◄────────────┘
                                    │
                                    ▼
                    Steps 17-26 (TUI layer, parallelizable)
                                    │
                                    ▼
                    Steps 27-31 (polish, parallelizable)
```

**Parallelism notes**:
- Steps 1-3 can be done in parallel (no interdependencies)
- Steps 4, 5, 7 can be done in parallel (all depend on 1-3 but not each other)
- Step 6 depends on steps 2, 4, 5
- Steps 8, 9, 11, 12 can be done in parallel
- Step 10 depends on 4, 6, 7, 9
- Steps 13-14 can be done in parallel
- Step 15 depends on 13, 14, 10
- Step 16 depends on 15
- Steps 17-26 (UI) mostly parallelizable, but 18 (layout) depends on 17 (theme), and widgets need layout for integration
- Steps 27-31 depend on the full UI being functional

---

## Relevant Files

### Infrastructure layer
- `src/errors.rs` — `AppError`, `ErrorKind`, `RecoveryAction` (step 1)
- `src/az/executor.rs` — `AzCliExecutor` trait + `SubprocessCliExecutor` (step 4)
- `src/az/commands.rs` — arg builder helpers (step 5)
- `src/az/parser.rs` — JSON → domain model mapping (step 6)
- `src/az/mod.rs` — re-exports (steps 4-6)
- `src/cache/store.rs` — `CacheEntry`, `CacheStore`, `CacheScope`, `CacheKey` (step 7)
- `src/cache/mod.rs` — re-exports (step 7)
- `src/config/settings.rs` — `AppConfig` and sub-structs, TOML loading (step 3)
- `src/config/mod.rs` — re-exports (step 3)

### Domain layer
- `src/domain/models.rs` — `Tenant`, `Subscription`, `AzureContext`, etc. (step 2)
- `src/domain/auth.rs` — `AuthProvider` trait (step 9)
- `src/domain/resources.rs` — `ResourceProvider` trait stub (step 11)
- `src/domain/cost.rs` — `CostProvider` trait stub (step 11)
- `src/domain/mod.rs` — re-exports (steps 2, 9, 11)

### Provider layer
- `src/providers/auth_provider.rs` — `AzAuthProvider` impl (step 10)
- `src/providers/resource_provider.rs` — stub (step 11)
- `src/providers/cost_provider.rs` — stub (step 11)
- `src/providers/mod.rs` — re-exports (step 10-11)

### Application layer
- `src/command.rs` — `Command` enum (step 13)
- `src/event.rs` — `Event` enum (step 14)
- `src/app.rs` — `AppState`, `View`, `Modal`, `PendingOperation`, dispatch logic (step 15)
- `src/main.rs` — entry point, terminal setup, main loop (step 16)

### TUI layer
- `src/ui/theme.rs` — `Theme` struct, color scheme (step 17)
- `src/ui/layout.rs` — main render function, layout composition (step 18)
- `src/ui/input.rs` — keypress → Command mapping (step 19)
- `src/ui/widgets/status_bar.rs` — status bar (step 20)
- `src/ui/widgets/context_switcher.rs` — main view (step 21)
- `src/ui/widgets/search_input.rs` — search input (step 22)
- `src/ui/widgets/quick_switch.rs` — Ctrl+P modal (step 23)
- `src/ui/widgets/modal.rs` — generic modal overlay (step 24)
- `src/ui/widgets/mod.rs` — re-exports (step 26)
- `src/ui/mod.rs` — public UI interface (step 26)

### Tests
- `tests/mock_executor.rs` — `MockCliExecutor` (step 8)

### Security (stubs)
- `src/security/mod.rs`, `src/security/crypto.rs`, `src/security/master_key.rs` — Phase 2 stubs (step 12)

---

## Verification

1. **After Phase 1A**: `cargo build` succeeds; unit tests pass for parser, cache, config, errors
2. **After Phase 1B**: `cargo test` — integration tests with mock executor pass for auth provider
3. **After Phase 1C**: `cargo run` — app launches, shows basic TUI, quits on `q`, initial data load works
4. **After Phase 1D**: Full manual workflow test:
   - Launch → tenant/subscription list loads
   - Navigate with j/k, select subscription with Enter
   - `az account show` in separate terminal confirms context was switched
   - Search with `/`, fuzzy filtering works
   - Quick switch with `Ctrl+P`
   - Help with `?`
   - Error display when `az` fails
5. **After Phase 1E**: `cargo clippy`, `cargo fmt --check`, panic recovery test, loading states work

---

## Decisions

- **Phase 1 only**: No security/crypto, no resource browser, no cost explorer — those are future phases
- **Fuzzy matching**: Use simple substring/case-insensitive matching in Phase 1; add `nucleo` crate for proper fuzzy scoring later if needed
- **Background refresh**: Implement stale-while-revalidate in Phase 1 for the context list cache; this is core to the "zero-delay UI" principle
- **No persistent cache on disk in Phase 1**: Cache is in-memory only. Disk persistence (and encryption of that disk cache) is Phase 2's concern
- **Channel-based architecture from day one**: Use `mpsc` channels for Command/Event even though it's more complex than direct calls — this matches the architecture doc and enables future extension
- **`Box<dyn AuthProvider>`**: Use trait objects at the boundary between application and domain layers (in `main.rs` when wiring things together) for testability; within layers, use concrete types

## Further Considerations

1. **Navigation state in context switcher**: The `AppState` doesn't explicitly define a `selected_index` for list navigation. Either add it to `AppState` or keep it as widget-local state in the context switcher. Recommendation: add `selected_index: usize` to `AppState` since all state should live there per architecture.md.
2. **Async command results**: The architecture sketch shows async tasks sending results back via the command channel. Need to decide on exact mechanism — recommendation: define internal `Command` variants like `_ContextListResult(Result<...>)` prefixed with underscore to indicate they're system-generated, not user-initiated. Alternative: use a separate result channel.
3. **List navigation commands**: The `Command` enum in foundation-types.md doesn't include `MoveUp`/`MoveDown` navigation commands. These could be handled purely in the UI layer (widget state) rather than as full Commands, or we add `NavigateList(Direction)` variants. Recommendation: handle in UI layer since list navigation doesn't need to go through the full Command→Event cycle.
