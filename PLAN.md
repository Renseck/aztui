# PLAN.md — aztui Phase 1 Implementation Roadmap

<!-- =========================================================================================== -->
<!--                                        PHASE 1 DONE                                         -->
<!-- =========================================================================================== -->

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
                          Step 16 (main.rs) �-�────────────┘
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

<!-- =========================================================================================== -->
<!--                                        PHASE 1 DONE                                         -->
<!-- =========================================================================================== -->

---

<!-- =========================================================================================== -->
<!--                                        PHASE 2 DONE                                         -->
<!-- =========================================================================================== -->

# Phase 2 — Security Layer

**TL;DR**: Add optional master password protection for local cached data. Argon2id for key derivation, AES-256-GCM for cache encryption at rest, inactivity auto-lock, and optional OS keyring integration. The app must remain fully functional when the security layer is disabled.

**Prerequisite**: Phase 1 complete and stable.

---

## Phase 2A — Disk-Persistent Cache + Encryption Primitives

Before encrypting anything, the cache needs to persist to disk (Phase 1 cache is in-memory only). Then build the low-level crypto building blocks.

### Step 32: Disk-persistent cache (`src/cache/store.rs`)
- Extend `CacheStore` from Phase 1 to support disk persistence:
  - `save_to_disk(&self, path: &Path) -> Result<(), AppError>` — serialize all entries to JSON file
  - `load_from_disk(path: &Path) -> Result<CacheStore, AppError>` — deserialize from JSON file
  - Storage location: `{data_dir}/cache.json` (data_dir from `GeneralConfig`, default `~/.aztui/`)
  - Save on every cache mutation (debounced) or on graceful shutdown
  - `CacheEntry<T>` needs `Serialize`/`Deserialize` — replace `Instant` with `chrono::DateTime<Utc>` for serializable timestamps (or store duration-since-epoch)
  - Handle missing/corrupt file gracefully: log warning, start fresh

### Step 33: Argon2id key derivation (`src/security/master_key.rs`)
- `derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32], AppError>`
  - Uses Argon2id with recommended parameters (m=19456 KiB, t=2, p=1 — or OWASP-recommended)
  - Returns 256-bit key suitable for AES-256-GCM
- `generate_salt() -> [u8; 16]` — cryptographically random salt via `rand` or `getrandom`
- `StoredKeyParams` struct:
  - `salt: Vec<u8>`, `m_cost`, `t_cost`, `p_cost`
  - Serializable to JSON for storage in `{data_dir}/master.json`
  - Created once during password setup, loaded on subsequent launches
- `verify_password(password: &str, params: &StoredKeyParams, verification_blob: &[u8]) -> Result<bool, AppError>`
  - Derive key, attempt to decrypt a known verification blob to confirm correctness

### Step 34: AES-256-GCM encrypt/decrypt (`src/security/crypto.rs`)
- `encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, AppError>`
  - Generate random 96-bit nonce per encryption
  - Prepend nonce to ciphertext in output: `[nonce (12 bytes)][ciphertext+tag]`
- `decrypt(key: &[u8; 32], ciphertext: &[u8]) -> Result<Vec<u8>, AppError>`
  - Extract nonce from first 12 bytes, decrypt remainder
  - Map authentication failures to `AppError` with `CacheDecryptionFailed`
- No key reuse — every encrypt call generates a fresh nonce
- Map all crypto errors to `AppError` appropriately

### Step 35: Security module wiring (`src/security/mod.rs`)
- Re-export `derive_key`, `generate_salt`, `StoredKeyParams`, `encrypt`, `decrypt`
- `SecurityManager` struct (optional):
  - Holds the derived key in memory while unlocked
  - Methods: `setup_password()`, `unlock()`, `lock()`, `is_locked()`, `encrypt_data()`, `decrypt_data()`
  - Key is zeroized on lock (use `zeroize` crate or manual zeroing)

**Verification for Phase 2A**:
- Unit tests for Argon2id: derive key, re-derive with same params → same key
- Unit tests for AES-256-GCM: encrypt → decrypt round-trip, tampered ciphertext → `CacheDecryptionFailed`
- Unit tests for disk cache: save → load round-trip, corrupt file → graceful fallback
- New dependency: possibly `zeroize` crate for secure memory clearing

---

## Phase 2B — Encrypted Cache Integration

Wire the crypto primitives into the cache store so all persisted data is encrypted at rest.

### Step 36: Encrypted disk cache (`src/cache/store.rs`)
- When `security.master_password_enabled` is true in config:
  - `save_to_disk()` serializes to JSON → encrypts with derived key → writes encrypted blob
  - `load_from_disk()` reads encrypted blob → decrypts with derived key → deserializes JSON
  - If decryption fails (wrong password, corrupt file): return `CacheDecryptionFailed` error, app falls back to fresh CLI calls
- When disabled: plain JSON file (Phase 2A behavior)
- Cache file format: version byte + encrypted payload (allows future format changes)

### Step 37: Master password setup flow
- First launch with `security.master_password_enabled = true` and no `master.json`:
  - Prompt user to create a master password (via `Modal::PasswordPrompt` variant)
  - Derive key via Argon2id, generate salt, encrypt a verification blob
  - Store `StoredKeyParams` + encrypted verification blob in `{data_dir}/master.json`
- Subsequent launches:
  - Load `StoredKeyParams` from `master.json`
  - Prompt for password → derive key → verify against verification blob
  - If correct: decrypt cache, proceed normally
  - If incorrect: show inline error below password field (per wireframe §6), allow retry
  - 3 failed attempts: show hint, do not lock out (user can keep trying or quit)

### Step 38: Unlock command integration (`src/app.rs`)
- Wire `Command::Unlock(password)` in `dispatch_command()`:
  - Call `SecurityManager::unlock(password)` → derive key → verify
  - On success: decrypt cache, emit `Event::AppUnlocked`, set `state.locked = false`
  - On failure: emit `Event::UnlockFailed`, keep `state.locked = true`
- Wire `Command::Lock`:
  - Zeroize the derived key in `SecurityManager`
  - Set `state.locked = true`, emit `Event::AppLocked`
  - Clear any sensitive data from AppState (subscription lists, etc.) — they'll reload on unlock

**Verification for Phase 2B**:
- Integration test: setup password → lock → unlock with correct password → cache accessible
- Integration test: unlock with wrong password → `UnlockFailed` event → state stays locked
- Integration test: enable master password → cache file is encrypted on disk (not readable as JSON)
- Manual test: launch with password → enter correct → data loads; enter wrong → error shown inline

---

## Phase 2C — Inactivity Lock & OS Keyring

### Step 39: Inactivity timeout (`src/app.rs`)
- In the main event loop (step 16, existing code), add inactivity check:
  - Track `state.last_interaction` — update on every user input event
  - If `security.inactivity_timeout` is set and elapsed > timeout:
    - Dispatch `Command::Lock`
    - Zeroize key, show password prompt modal
  - Default timeout: 10 minutes (configurable, `None` = never)
- When locked:
  - UI renders `Modal::PasswordPrompt` overlay
  - Background is blank/dimmed — no data visible (per wireframe §6)
  - Only `q` (quit) and password entry are accepted

### Step 40: Password prompt widget updates (`src/ui/widgets/modal.rs`)
- Extend the `Modal::PasswordPrompt` rendering:
  - Lock icon + "aztui is locked" header
  - Password input field with `•` masking
  - Inline error message below input on wrong password (red text)
  - Footer: "Enter: unlock   q: quit"
  - Per wireframe §6

### Step 41: OS keyring integration (`src/security/master_key.rs`)
- When `security.use_os_keyring = true`:
  - On first setup: derive key, store derived key in OS keyring via `keyring` crate
  - On subsequent launches: retrieve key from keyring → skip password prompt
  - Fallback: if keyring retrieval fails (locked keyring, missing entry), fall back to password prompt
  - Service name: `aztui`, username: current OS user
- `KeyringManager` struct:
  - `store_key(key: &[u8; 32]) -> Result<(), AppError>`
  - `retrieve_key() -> Result<Option<[u8; 32]>, AppError>`
  - `delete_key() -> Result<(), AppError>`
- Config toggle: `security.use_os_keyring` in `config.toml`

### Step 42: Security config commands
- Add CLI arg: `--reset-password` to re-run the password setup flow
- Add Command variant (or CLI-only path): reset master password — re-derive with new password, re-encrypt cache, update `master.json`

**Verification for Phase 2C**:
- Manual test: set `inactivity_timeout = 30s` in config → wait → app locks → enter password → unlocks
- Manual test: `use_os_keyring = true` → first launch prompts password → second launch skips prompt
- Manual test: delete keyring entry → falls back to password prompt
- Unit test: inactivity check fires at correct threshold
- Ensure `zeroize` properly clears key from memory (debug build inspection)

---

## Phase 2 Relevant Files

- `src/security/master_key.rs` — Argon2id key derivation, `StoredKeyParams`, OS keyring (steps 33, 41)
- `src/security/crypto.rs` — AES-256-GCM encrypt/decrypt (step 34)
- `src/security/mod.rs` — `SecurityManager`, re-exports (step 35)
- `src/cache/store.rs` — disk persistence, encrypted storage (steps 32, 36)
- `src/app.rs` — `Command::Lock`/`Unlock` handling, inactivity check (steps 38, 39)
- `src/ui/widgets/modal.rs` — password prompt rendering (step 40)
- `src/config/settings.rs` — `SecurityConfig` already defined; no changes needed unless adding new fields
- `src/main.rs` — wire `SecurityManager` into startup, conditional password prompt (steps 37, 38)

## Phase 2 Dependency Graph

```
Step 32 (disk cache) ──────────────────────────┐
Step 33 (Argon2id) ──┬→ Step 35 (module) ──────┤
Step 34 (AES-GCM) ───┘                         │
                                                ▼
                                      Step 36 (encrypted cache)
                                                │
                                                ▼
                                      Step 37 (setup flow)
                                                │
                                                ▼
                                      Step 38 (unlock commands)
                                                │
                              ┌─────────────────┼─────────────────┐
                              ▼                 ▼                 ▼
                         Step 39           Step 40           Step 41
                      (inactivity)     (prompt widget)    (OS keyring)
                              └─────────────────┼─────────────────┘
                                                ▼
                                      Step 42 (reset password)
```


**Parallelism notes**:
- Steps 32, 33, 34 can be done in parallel (no interdependencies)
- Step 35 depends on 33, 34
- Step 36 depends on 32, 35
- Steps 37, 38 are sequential (setup before unlock handling)
- Steps 39, 40, 41 can be done in parallel after 38

## Phase 2 Decisions

- **Opt-in by default**: Master password is disabled unless `security.master_password_enabled = true` in config. App works identically to Phase 1 when disabled.
- **No lockout**: Wrong password allows unlimited retries. The point is casual access prevention, not Fort Knox.
- **Cache fallback**: If decryption fails, the app logs a warning and works with fresh CLI calls. Encrypted cache is a convenience, not a hard dependency.
- **Key zeroization**: Derived key is zeroized from memory on lock. Use `zeroize` crate for proper clearing.
- **Keyring is optional alternative**: OS keyring replaces the password prompt, it doesn't add a second factor. One or the other, not both.
- **New dependency**: `zeroize` crate for secure memory clearing. `getrandom` (or `rand`) for salt generation. `keyring` already in Cargo.toml.


<!-- =========================================================================================== -->
<!--                                        PHASE 2 DONE                                         -->
<!-- =========================================================================================== -->

---

# Phase 3 — Resource Browser

**TL;DR**: Add a two-pane resource browsing view. Left pane shows resource groups for the active subscription; right pane shows resources within the selected group. Drill-down navigation with search filtering per pane.

**Prerequisite**: Phase 1 complete. Phase 2 is independent (can be done before or after Phase 3).

---

## Phase 3A — Resource Infrastructure

Build the data pipeline for fetching and parsing resource data from `az` CLI.

### Step 43: Resource command builders (`src/az/commands.rs`)
- Add to existing command builders:
  - `resource_group_list(subscription_id: &str) -> Vec<String>` — `["group", "list", "--subscription", subscription_id]`
  - `resource_list(subscription_id: &str, resource_group: &str) -> Vec<String>` — `["resource", "list", "--subscription", subscription_id, "--resource-group", resource_group]`
- Both produce JSON output (inherited from executor's `--output json` flag)

### Step 44: Resource JSON parsers (`src/az/parser.rs`)
- `parse_resource_group_list(json: &str) -> Result<Vec<ResourceGroup>, AppError>`
  - Parses `az group list` JSON: array of objects with `name`, `location`, `tags`
  - Maps to `ResourceGroup` domain model (already defined in Phase 1 step 2)
  - Injects `subscription_id` (not in CLI output, provided by caller)
- `parse_resource_list(json: &str) -> Result<Vec<Resource>, AppError>`
  - Parses `az resource list` JSON: array of objects with `id`, `name`, `type`, `resourceGroup`, `location`, `tags`
  - Maps `type` field → `resource_type` (e.g. `"Microsoft.Compute/virtualMachines"`)
  - Maps to `Resource` domain model

### Step 45: Resource cache entries (`src/cache/store.rs`)
- No structural changes needed — `CacheStore` is generic
- Define cache key conventions:
  - Resource groups: `CacheKey { scope: Subscription(id), kind: "resource_groups" }`
  - Resources: `CacheKey { scope: Subscription(id), kind: "resources:{resource_group_name}" }`
- Use `CacheConfig.resource_soft_ttl` / `resource_hard_ttl` (already defined in Phase 1)

**Verification for Phase 3A**:
- Unit tests for `parse_resource_group_list` with sample `az group list` JSON
- Unit tests for `parse_resource_list` with sample `az resource list` JSON
- Unit tests for edge cases: empty resource group, resources with no tags, unusual resource types

---

## Phase 3B — Resource Provider

### Step 46: ResourceProvider implementation (`src/providers/resource_provider.rs`)
- Implement `AzResourceProvider` struct:
  - Holds `Arc<dyn AzCliExecutor>`, `Arc<RwLock<CacheStore>>`, `CacheConfig`
  - `list_resource_groups(subscription_id)`:
    - Check cache → if fresh return cached; if stale return cached + background refresh; if expired synchronous refresh
    - Call `az group list --subscription <id>` via executor, parse with `parse_resource_group_list`
    - Cache result with `resource_soft_ttl` / `resource_hard_ttl`
  - `list_resources(subscription_id, resource_group)`:
    - Same cache pattern with per-resource-group cache key
    - Call `az resource list --subscription <id> --resource-group <name>` via executor, parse
- Wire into `src/providers/mod.rs`

### Step 47: Resource commands in dispatch (`src/app.rs`)
- Handle `Command::ListResourceGroups` in `dispatch_command()`:
  - Spawn async task calling `ResourceProvider::list_resource_groups(active_subscription_id)`
  - On completion: emit `Event::ResourceGroupsLoaded(groups)`
  - Handle error: emit `Event::ErrorOccurred`
- Handle `Command::ListResources(resource_group_name)`:
  - Spawn async task calling `ResourceProvider::list_resources(active_subscription_id, name)`
  - On completion: emit `Event::ResourcesLoaded { resource_group, resources }`
- Add resource state fields to `AppState`:
  - `resource_groups: Vec<ResourceGroup>` — current subscription's resource groups
  - `resources: Vec<Resource>` — currently selected resource group's resources
  - `selected_resource_group_index: usize`
  - `selected_resource_index: usize`
  - `resource_browser_focus: Pane` — `enum Pane { Left, Right }`

### Step 48: Event handling for resources (`src/app.rs`)
- `Event::ResourceGroupsLoaded(groups)`:
  - Set `state.resource_groups = groups`
  - Reset `selected_resource_group_index` to 0
  - Auto-trigger `Command::ListResources` for the first group (if any)
- `Event::ResourcesLoaded { resource_group, resources }`:
  - Set `state.resources = resources`
  - Reset `selected_resource_index` to 0
- On `Event::ContextChanged`: clear resource groups and resources (stale data from previous subscription)

**Verification for Phase 3B**:
- Integration tests with `MockCliExecutor`:
  - `list_resource_groups` returns correct groups from canned JSON
  - `list_resources` returns correct resources from canned JSON
  - Cache behavior: second call returns cached without executor call
  - Context change clears resource state

---

## Phase 3C — Resource Browser UI

### Step 49: Resource browser widget (`src/ui/widgets/`) — new file or extend context_switcher
- Two-pane layout per wireframe §3:
  - Left pane (~35% width): resource group list
    - Each row: resource group name
    - Bottom: count summary ("5 groups")
    - Selected row highlighted
  - Right pane (~65% width): resources in selected group
    - Columns: Name, Type (abbreviated), Region (abbreviated)
    - Bottom: count summary ("7 resources")
    - Selected row highlighted
  - Focused pane has `azure` border, unfocused pane has `muted` border (per color-scheme.md)
- Resource type abbreviation helper: `"Microsoft.Compute/virtualMachines"` → `"VM"`, `"Microsoft.Storage/storageAccounts"` → `"Storage"`, etc. — maintain a lookup map for common types, fall back to last segment

### Step 50: Resource browser input handling (`src/ui/input.rs`)
- When `state.active_view == View::ResourceBrowser`:
  - `Tab` or `→` / `←`: switch focus between panes (toggle `resource_browser_focus`)
  - `j/k` / `↑/↓`: navigate within focused pane
  - `/`: search within focused pane (filter resource groups or resources by name)
  - `Enter` on a resource group: same as navigating right (load resources, focus right pane)
  - `Esc`: clear search, or if search empty, go back to context switcher
  - `r`: refresh resource groups + resources
- Navigation in left pane auto-triggers `Command::ListResources` for the newly selected group

### Step 51: Resource browser layout integration (`src/ui/layout.rs`)
- When `state.active_view == View::ResourceBrowser`:
  - Render title bar with breadcrumb: "aztui > Resources" (per wireframe)
  - Split content area horizontally: 35% / 65%
  - Render resource group list widget in left, resource list widget in right
- Search bar appears at top of content area when active (same as context switcher)

### Step 52: Navigation to resource browser
- Keybinding: `2` key → `Command::NavigateTo(View::ResourceBrowser)`
- On navigation: if `state.resource_groups` is empty, auto-dispatch `Command::ListResourceGroups`
- If no active subscription: show "Select a subscription first" message in content area
- Breadcrumb rendering in title bar: "aztui" for context switcher, "aztui > Resources" for resource browser

### Step 53: Resource type abbreviation map
- Utility function or constant map: full ARM type → short display name
- Common mappings (extend as needed):
  - `Microsoft.Compute/virtualMachines` → `VM`
  - `Microsoft.Storage/storageAccounts` → `Storage`
  - `Microsoft.KeyVault/vaults` → `KeyVault`
  - `Microsoft.Sql/servers/databases` → `SQL DB`
  - `Microsoft.Network/networkInterfaces` → `NIC`
  - `Microsoft.Network/networkSecurityGroups` → `NSG`
  - `Microsoft.ContainerService/managedClusters` → `AKS`
  - `Microsoft.Web/sites` → `App Service`
  - Fallback: last segment of the type string (e.g. `"publicIPAddresses"`)

**Verification for Phase 3C**:
- Manual test: navigate to resource browser with `2` → resource groups load for active subscription
- Manual test: select a resource group → resources load in right pane
- Manual test: Tab switches focus between panes, search filters within focused pane
- Manual test: switch subscription → resource data clears, re-loads for new subscription
- Resource types display as abbreviated names
- Both panes show item counts at bottom

---

## Phase 3 Relevant Files

- `src/az/commands.rs` — resource command builders (step 43)
- `src/az/parser.rs` — resource JSON parsers (step 44)
- `src/providers/resource_provider.rs` — `AzResourceProvider` impl (step 46)
- `src/providers/mod.rs` — re-export (step 46)
- `src/app.rs` — resource state fields, command dispatch, event handling (steps 47, 48)
- `src/ui/widgets/` — resource browser widget (step 49), possibly new file `resource_browser.rs`
- `src/ui/input.rs` — resource browser keybindings (step 50)
- `src/ui/layout.rs` — two-pane layout, breadcrumbs (step 51)
- `src/domain/resources.rs` — `ResourceProvider` trait (defined in Phase 1, now implemented)
- `src/cache/store.rs` — no changes, just new cache key conventions (step 45)

## Phase 3 Dependency Graph

```
Step 43 (command builders) ──┬→ Step 44 (parsers)
                             │                │
                             └────────────────┤
                                              ▼
Step 45 (cache keys) ──────→ Step 46 (provider)
                                              │
                              ┌───────────────┤
                              ▼               ▼
                     Step 47 (dispatch)  Step 48 (events)
                              │
                              ▼
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
        Step 49          Step 50          Step 51
       (widget)      (input handler)     (layout)
              └───────────────┼───────────────┘
                              ▼
                    Step 52 (navigation)
                    Step 53 (type abbrev) ← parallel with 49-51
```


**Parallelism notes**:
- Steps 43, 45 can be done in parallel
- Step 44 depends on 43 (needs to know what JSON comes back)
- Step 46 depends on 43, 44, 45
- Steps 47, 48 can be done together after 46
- Steps 49, 50, 51 can be done in parallel after 47-48
- Step 53 is independent, can be done anytime

## Phase 3 Decisions

- **No resource detail view in Phase 3**: Selecting a resource in the right pane doesn't drill down further. Resource detail view (showing tags, properties, metrics) is a future feature.
- **Resource type abbreviation is best-effort**: Unknown types fall back to the last segment of the ARM type string. The map is hardcoded, not configurable.
- **Search filters current pane only**: When typing in the resource browser, only the focused pane is filtered.
- **Auto-load on navigation**: Navigating to the resource browser auto-fetches resource groups if not cached. Selecting a resource group auto-fetches its resources.
- **New AppState fields**: `resource_groups`, `resources`, `selected_resource_group_index`, `selected_resource_index`, `resource_browser_focus` added to `AppState`.

---
---

# Phase 4 — Cost Explorer (FinOps)

**TL;DR**: Add a cost summary view showing per-service cost breakdown for the active subscription, with period navigation and inline bar charts. Uses `az consumption usage list` or `az costmanagement` commands.

**Prerequisite**: Phase 1 complete. Independent of Phases 2 and 3.

---

## Phase 4A — Cost Infrastructure

### Step 54: Cost command builders (`src/az/commands.rs`)
- Research the correct `az` command for cost data. Options:
  - `az consumption usage list --subscription <id> --start-date <from> --end-date <to>` — per-resource usage
  - `az cost-management query --type ActualCost --timeframe Custom --time-period from=<from> to=<to> --dataset-aggregation '{"totalCost":{"name":"Cost","function":"Sum"}}' --dataset-grouping name=ServiceName type=Dimension --scope /subscriptions/<id>` — aggregated by service
- Prefer `az cost-management query` if available — it returns pre-aggregated data, avoiding client-side aggregation of potentially thousands of usage records
- Fallback: if `cost-management` extension not installed, use `consumption usage list` and aggregate client-side
- Command builders:
  - `cost_query_by_service(subscription_id, from, to) -> Vec<String>`
  - `cost_query_by_resource_group(subscription_id, resource_group, from, to) -> Vec<String>`

### Step 55: Cost JSON parsers (`src/az/parser.rs`)
- `parse_cost_query(json: &str, scope: CostScope) -> Result<CostSummary, AppError>`
  - Parse the `az cost-management query` response format:
    - Rows with `[cost, service_name]` or similar structure
    - Aggregate into `CostLineItem` list
    - Sum total
    - Extract currency from response metadata
  - Map to `CostSummary` domain model (already defined in Phase 1 step 2)
- Handle edge cases: no data for period, zero-cost subscriptions, mixed currencies
- If using `consumption usage list` fallback:
  - `parse_consumption_usage(json: &str, scope: CostScope, period: CostPeriod) -> Result<CostSummary, AppError>`
  - Group by service name, sum costs per service, compute total

### Step 56: Cost cache entries
- Cache key conventions:
  - Subscription cost: `CacheKey { scope: Subscription(id), kind: "cost:{from}:{to}" }`
  - Resource group cost: `CacheKey { scope: Subscription(id), kind: "cost:{rg_name}:{from}:{to}" }`
- Use `CacheConfig.cost_soft_ttl` / `cost_hard_ttl` (already defined, defaults 15min / 2hr)

**Verification for Phase 4A**:
- Unit tests for cost parser with sample `az cost-management query` JSON output
- Unit tests for aggregation logic (multiple rows → sorted by amount, total computation)
- Unit tests for edge cases (empty result, zero costs)
- Determine which `az` command is reliably available and document the choice

---

## Phase 4B — Cost Provider

### Step 57: CostProvider implementation (`src/providers/cost_provider.rs`)
- Implement `AzCostProvider` struct:
  - Holds `Arc<dyn AzCliExecutor>`, `Arc<RwLock<CacheStore>>`, `CacheConfig`
  - `get_cost_summary(subscription_id, period)`:
    - Cache check with stale-while-revalidate pattern
    - Call cost query command via executor, parse
    - Sort breakdown by amount descending
    - Cache result
  - `get_resource_group_cost(subscription_id, resource_group, period)`:
    - Same pattern, scoped to resource group
    - Used for future drill-down (not exposed in Phase 4 UI, but provider ready)
- Error mapping:
  - Cost management extension not installed → `AppError` with `CliExecutionFailed` and `Manual("Install the cost-management extension: az extension add --name costmanagement")` recovery
  - Permission denied (no Reader role on billing) → `AppError` with `AuthFailed` and `Manual` hint
  - No data → return `CostSummary` with zero total and empty breakdown (not an error)
- Wire into `src/providers/mod.rs`

### Step 58: Cost commands in dispatch (`src/app.rs`)
- Handle `Command::FetchCostSummary(period)` in `dispatch_command()`:
  - Spawn async task calling `CostProvider::get_cost_summary(active_subscription_id, period)`
  - On completion: emit `Event::CostSummaryLoaded(summary)`
  - Handle error: emit `Event::ErrorOccurred`
- Add cost state fields to `AppState`:
  - `cost_summary: Option<CostSummary>` — current cost data
  - `cost_period: CostPeriod` — currently selected period (default: current month)
  - `cost_selected_index: usize` — selected row in cost breakdown

### Step 59: Event handling for cost (`src/app.rs`)
- `Event::CostSummaryLoaded(summary)`:
  - Set `state.cost_summary = Some(summary)`
  - Reset `cost_selected_index` to 0
- On `Event::ContextChanged`: clear `cost_summary` (stale data from previous subscription)

**Verification for Phase 4B**:
- Integration tests with `MockCliExecutor`:
  - `get_cost_summary` returns correct summary from canned JSON
  - Breakdown sorted by amount descending
  - Cache behavior: second call returns cached without executor call
  - Context change clears cost data
- Error scenario: cost extension not installed → helpful error message

---

## Phase 4C — Cost Explorer UI

### Step 60: Cost explorer widget (`src/ui/widgets/`) — new file `cost_explorer.rs`
- Per wireframe §4:
  - Header: subscription name, period range, period navigation arrows
  - Total cost display: `€1,247.83 EUR` in bright text with currency symbol in muted
  - Service breakdown table:
    - Columns: Service name, Cost (right-aligned), inline bar chart, Percentage
    - Sorted by cost descending (from provider)
    - Top N services shown individually, remainder grouped as "Other (N services)"
    - N = configurable or dynamic based on terminal height (default: show as many as fit)
  - Inline bar charts: 10 chars wide, `█` filled portion (azure color), `░` empty portion (overlay color)
    - Bar width = (service_amount / total_amount) * 10, rounded

### Step 61: Period navigation
- Period navigation keybindings:
  - `[` or `h` : previous month
  - `]` or `l` : next month
  - Can't navigate past current month
- `CostPeriod` helpers:
  - `CostPeriod::current_month() -> CostPeriod` — from 1st of current month to today
  - `CostPeriod::previous_month(current: &CostPeriod) -> CostPeriod`
  - `CostPeriod::next_month(current: &CostPeriod) -> Option<CostPeriod>` — None if already current month
- Period change dispatches `Command::FetchCostSummary(new_period)` — triggers data fetch for new period

### Step 62: Cost explorer input handling (`src/ui/input.rs`)
- When `state.active_view == View::CostExplorer`:
  - `j/k` / `↑/↓`: navigate service breakdown rows (for future drill-down)
  - `[` / `]` or `h` / `l`: period navigation
  - `r`: refresh cost data for current period
  - `Esc`: return to context switcher
  - `/`: search/filter service names in breakdown

### Step 63: Cost explorer layout integration (`src/ui/layout.rs`)
- When `state.active_view == View::CostExplorer`:
  - Render title bar with breadcrumb: "aztui > Cost Explorer"
  - Content area: single-pane with header section (subscription, period, total) + scrollable breakdown table
  - Period navigation arrows rendered as `[ �-� prev ] [ next ▸ ]` (per wireframe)

### Step 64: Navigation to cost explorer
- Keybinding: `3` key → `Command::NavigateTo(View::CostExplorer)`
- On navigation: if `state.cost_summary.is_none()`, auto-dispatch `Command::FetchCostSummary(current_month)`
- If no active subscription: show "Select a subscription first" message
- If cost data is loading: show spinner with "Loading cost data..." (reuse loading widget from Phase 1E)

### Step 65: Cost formatting utilities
- Currency formatting: `format_cost(amount: f64, currency: &str) -> String` — e.g. `"€1,247.83"`
  - Handle common currency symbols: EUR→€, USD→$, GBP→£, fallback to currency code
  - Thousand separators
- Percentage formatting: `format_percentage(amount: f64, total: f64) -> String` — e.g. `"49.1%"`
- Bar chart rendering: `render_bar(fraction: f64, width: usize) -> String` — e.g. `"████████░░"`

**Verification for Phase 4C**:
- Manual test: navigate to cost explorer with `3` → cost data loads for current month
- Manual test: period navigation with `[`/`]` → data refreshes for new period
- Manual test: bar charts render proportionally, total is correct
- Manual test: with no billing permissions → helpful error message displayed
- Manual test: switch subscription → cost data clears, re-loads
- Currency symbols display correctly for EUR, USD, GBP
- "Other (N services)" grouping works for subscriptions with many services

---

## Phase 4 Relevant Files

- `src/az/commands.rs` — cost command builders (step 54)
- `src/az/parser.rs` — cost JSON parsers (step 55)
- `src/providers/cost_provider.rs` — `AzCostProvider` impl (step 57)
- `src/providers/mod.rs` — re-export (step 57)
- `src/app.rs` — cost state fields, command dispatch, event handling (steps 58, 59)
- `src/ui/widgets/cost_explorer.rs` — cost explorer widget (step 60), new file
- `src/ui/input.rs` — cost explorer keybindings (step 62)
- `src/ui/layout.rs` — cost explorer layout, breadcrumbs (step 63)
- `src/domain/cost.rs` — `CostProvider` trait (defined in Phase 1, now implemented)
- `src/domain/models.rs` — `CostSummary`, `CostPeriod`, `CostLineItem` (ensure fully implemented)

## Phase 4 Dependency Graph

```
Step 54 (command builders) → Step 55 (parsers)
                                      │
Step 56 (cache keys) ─────────→ Step 57 (provider)
                                      │
                              ┌───────┤
                              ▼       ▼
                     Step 58     Step 59
                   (dispatch)   (events)
                              │
                              ▼
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
        Step 60          Step 62          Step 63
       (widget)      (input handler)     (layout)
              └───────────────┼───────────────┘
                              ▼
                    Step 64 (navigation)
                    Step 61 (period nav)  ← parallel with 60-63
                    Step 65 (formatting)  ← parallel with 60-63
```


**Parallelism notes**:
- Steps 54, 56 can be done in parallel
- Step 55 depends on 54
- Step 57 depends on 54, 55, 56
- Steps 58, 59 together after 57
- Steps 60, 61, 62, 63, 65 can largely be done in parallel after 58-59
- Step 64 ties it all together last

## Phase 4 Decisions

- **`az cost-management query` preferred**: Pre-aggregated data is far more efficient than aggregating thousands of consumption records client-side. If the extension isn't installed, show a helpful error rather than falling back silently.
- **No resource-group drill-down in Phase 4 UI**: The provider supports `get_resource_group_cost` but the UI shows subscription-level only. Drill-down is a future feature.
- **"Other" grouping**: Services beyond what fits on screen are grouped into a single "Other (N services)" row to keep the view scannable.
- **Period defaults to current month**: On first navigation to cost explorer, show current month-to-date costs.
- **New AppState fields**: `cost_summary`, `cost_period`, `cost_selected_index` added to `AppState`.

---
---

# Cross-Phase Summary

| Phase | Steps | Focus | Key Deliverable |
|-------|-------|-------|-----------------|
| 1A | 1–8 | Foundation + infrastructure | Types, executor, parser, cache, config |
| 1B | 9–12 | Domain + provider layer | AuthProvider trait + implementation |
| 1C | 13–16 | Application layer | Event loop, command dispatch, main.rs |
| 1D | 17–26 | TUI layer | Full context switcher UI |
| 1E | 27–31 | Polish | Loading states, error modals, panic hook |
| 2A | 32–35 | Crypto primitives | Disk cache, Argon2id, AES-GCM |
| 2B | 36–38 | Encrypted cache | Master password setup + unlock flow |
| 2C | 39–42 | Lock + keyring | Inactivity lock, OS keyring, reset password |
| 3A | 43–45 | Resource infrastructure | Command builders + parsers |
| 3B | 46–48 | Resource provider | Provider impl + dispatch |
| 3C | 49–53 | Resource browser UI | Two-pane widget + navigation |
| 4A | 54–56 | Cost infrastructure | Command builders + parsers |
| 4B | 57–59 | Cost provider | Provider impl + dispatch |
| 4C | 60–65 | Cost explorer UI | Bar charts + period navigation |

**Total**: 65 steps across 4 phases.

**Phase independence**: Phase 1 is the foundation. Phases 2, 3, and 4 are independent of each other and can be done in any order after Phase 1. Within each phase, the sub-phases (A → B → C) are sequential.