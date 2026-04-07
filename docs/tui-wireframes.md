# TUI Wireframes — aztui

> Visual reference for all TUI views and overlays.
> Box-drawing characters used for rendering boundaries.
> `[highlighted]` = selected/focused element, `(dimmed)` = inactive/disabled.

---

## 1. Context Switcher (main view)

The default view on launch. Tenants as section headers, subscriptions nested
beneath. The active context is marked, search filters the list in real time.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  aztui                                                          ? for help  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Search: terraform-prod_                                                    │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                                                                     │    │
│  │  ▸ Contoso Ltd  (contoso.onmicrosoft.com)                           │    │
│  │      [» contoso-terraform-prod       ] ● active    Enabled          │    │
│  │        contoso-terraform-dev                       Enabled          │    │
│  │        contoso-shared-services                     Enabled          │    │
│  │       (contoso-legacy-sandbox)                     Disabled         │    │
│  │                                                                     │    │
│  │  ▸ Fabrikam Inc  (fabrikam.onmicrosoft.com)                         │    │
│  │        fabrikam-terraform-prod                     Enabled          │    │
│  │                                                                     │    │
│  │                                                                     │    │
│  │                                                                     │    │
│  │                                                                     │    │
│  │                                                                     │    │
│  │                                                                     │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  ● Contoso Ltd / contoso-terraform-prod        ↻ Refreshing...    3s ago    │ 
└─────────────────────────────────────────────────────────────────────────────┘
```

**Legend**:
- `▸` — tenant section header (collapsible in future)
- `[» ... ]` — currently selected row (cursor)
- `● active` — the context that `az` is currently pointed at
- `(parentheses)` — disabled/warned subscription, visually dimmed
- Bottom bar — status: active context, pending operation spinner, cache age

---

## 2. Quick Switch Modal (Ctrl+P overlay)

Overlays on top of any view. Flat list of all tenant+subscription pairs,
fuzzy-filtered. Recent contexts appear first.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  aztui                                                          ? for help │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌───────────────────────────────────────────────────────────────┐          │
│  │  Switch context: fab-prod_                                    │          │
│  │                                                               │          │
│  │  RECENT                                                       │          │
│  │  [» Fabrikam Inc / fabrikam-terraform-prod  ]                 │          │
│  │    Contoso Ltd / contoso-terraform-prod                       │          │
│  │                                                               │          │
│  │  ALL MATCHES                                                  │          │
│  │    Fabrikam Inc / fabrikam-terraform-prod                     │          │
│  │    Fabrikam Inc / fabrikam-production                         │          │
│  │                                                               │          │
│  │                                                               │          │
│  │                                                               │          │
│  │  Enter: switch  Esc: cancel                                   │          │
│  └───────────────────────────────────────────────────────────────┘          │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  ● Contoso Ltd / contoso-terraform-prod                            3s ago   │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Notes**:
- The modal is centered and does not fill the full terminal width
- Background view (context switcher or any other) is dimmed behind the overlay
- `RECENT` section shows MRU contexts, limited by `max_recent_contexts` config
- `ALL MATCHES` shows fuzzy-filtered results from the full context list
- If a result appears in both RECENT and ALL MATCHES, it's deduplicated

---

## 3. Resource Browser (Phase 3)

Two-pane view: resource groups on the left, resources in the selected group on
the right.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  aztui > Resources                                              ? for help │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Search: _                                                                  │
│                                                                             │
│  ┌──────────────────────────┐  ┌──────────────────────────────────────┐     │
│  │  Resource Groups          │  │  Resources in: rg-prod-westeurope    │     │
│  │                           │  │                                      │     │
│  │  [» rg-prod-westeurope  ] │  │  Name              Type      Region  │     │
│  │    rg-prod-northeurope    │  │  ──────────────────────────────────  │     │
│  │    rg-dev-westeurope      │  │  vm-web-01          VM        westeu │     │
│  │    rg-shared-networking   │  │  vm-web-02          VM        westeu │     │
│  │    rg-monitoring          │  │  st-prod-data       Storage   westeu │     │
│  │                           │  │  kv-prod-secrets    KeyVault  westeu │     │
│  │                           │  │  sql-prod-main      SQL DB    westeu │     │
│  │                           │  │  nic-web-01         NIC       westeu │     │
│  │                           │  │  nsg-web-tier       NSG       westeu │     │
│  │                           │  │                                      │     │
│  │                           │  │                                      │     │
│  │  5 groups                 │  │  7 resources                         │     │
│  └──────────────────────────┘  └──────────────────────────────────────┘     │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  ● Contoso Ltd / contoso-terraform-prod                            5s ago   │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Notes**:
- Tab or arrow-right to move focus between panes
- Search filters whichever pane is focused
- Resource type column shows abbreviated type (VM, Storage, KeyVault, etc.)
- Count summaries at the bottom of each pane

---

## 4. Cost Explorer (Phase 4)

Cost summary for the active subscription, with per-service breakdown.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  aztui > Cost Explorer                                          ? for help │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Subscription: contoso-terraform-prod                                       │
│  Period: 2026-03-01 → 2026-03-31                    [ ◂ prev ] [ next ▸ ]  │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                                                                     │    │
│  │  Total: €1,247.83 EUR                                               │    │
│  │                                                                     │    │
│  │  Service                           Cost          % of total         │    │
│  │  ─────────────────────────────────────────────────────────────      │    │
│  │  Virtual Machines                  €612.40       ████████░░  49.1%  │    │
│  │  Azure SQL Database                €284.15       ████░░░░░░  22.8%  │    │
│  │  Storage Accounts                  €156.22       ███░░░░░░░  12.5%  │    │
│  │  Azure Kubernetes Service           €98.50       ██░░░░░░░░   7.9%  │    │
│  │  Key Vault                          €42.30       █░░░░░░░░░   3.4%  │    │
│  │  Other (6 services)                 €54.26       █░░░░░░░░░   4.3%  │    │
│  │                                                                     │    │
│  │                                                                     │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  ● Contoso Ltd / contoso-terraform-prod                           12s ago   │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Notes**:
- Bar chart uses block characters (`█` filled, `░` empty) for inline bars
- Period navigation with keybindings (e.g. `[` / `]` or `h` / `l`)
- Drill-down into a service row could show per-resource-group breakdown (future)
- Currency and period are driven by the API response

---

## 5. Help View

Full keybinding reference and command overview.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  aztui > Help                                                   ? to close │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Navigation                                                                 │
│  ──────────────────────────────────────────────                             │
│  ↑/↓  or  j/k         Navigate list                                        │
│  Enter                 Select / confirm                                     │
│  Tab                   Switch pane (resource browser)                       │
│  Esc                   Clear search / close modal / back                    │
│                                                                             │
│  Actions                                                                    │
│  ──────────────────────────────────────────────                             │
│  /                     Focus search input                                   │
│  Ctrl+P                Quick switch context                                 │
│  r                     Refresh current view                                 │
│  1                     Context switcher                                     │
│  2                     Resource browser                                     │
│  3                     Cost explorer                                        │
│                                                                             │
│  System                                                                     │
│  ──────────────────────────────────────────────                             │
│  ?                     Toggle this help screen                              │
│  q                     Quit                                                 │
│                                                                             │
│  aztui v0.1.0                                                               │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  ● Contoso Ltd / contoso-terraform-prod                            3s ago   │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 6. Password Prompt (Phase 2, modal overlay)

Shown on launch when master password is enabled, or when resuming from
inactivity lock.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  aztui                                                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│                                                                             │
│                                                                             │
│                                                                             │
│                ┌─────────────────────────────────────┐                      │
│                │                                     │                      │
│                │   🔒  aztui is locked                │                      │
│                │                                     │                      │
│                │   Password: ••••••••_               │                      │
│                │                                     │                      │
│                │   Enter: unlock   q: quit           │                      │
│                │                                     │                      │
│                └─────────────────────────────────────┘                      │
│                                                                             │
│                                                                             │
│                                                                             │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  Locked — enter master password to continue                                 │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Notes**:
- Input is masked with `•` characters
- On wrong password, show inline error below the input field (not a new modal)
- The background is blank / dimmed — no data visible while locked
- `q` quits the application entirely without unlocking

---

## 7. Error Detail Modal

Shown when the user selects an error in the status bar, or when a critical
error occurs.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  aztui                                                          ? for help │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│       ┌──────────────────────────────────────────────────────┐              │
│       │                                                      │              │
│       │  ⚠  Authentication expired                           │              │
│       │                                                      │              │
│       │  Your session for tenant "Contoso Ltd" has expired.  │              │
│       │  Azure CLI returned exit code 1.                     │              │
│       │                                                      │              │
│       │  Detail:                                              │              │
│       │  AADSTS700082: The refresh token has expired due     │              │
│       │  to inactivity. The token was issued on              │              │
│       │  2026-03-15T08:30:00Z.                               │              │
│       │                                                      │              │
│       │  Suggested action:                                   │              │
│       │  [» Re-login to Contoso Ltd ]                        │              │
│       │                                                      │              │
│       │  Esc: close                                          │              │
│       └──────────────────────────────────────────────────────┘              │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  ● Contoso Ltd / contoso-terraform-prod   ⚠ Auth expired          3s ago   │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Notes**:
- Modal maps directly to `AppError` fields:
  - Title ← `error.kind` (human-readable label)
  - Body ← `error.message`
  - Detail ← `error.source_detail` (collapsible if long)
  - Action button ← `error.recovery` (e.g. `RecoveryAction::ReLogin`)
- The suggested action is a selectable button; Enter executes the recovery
  command, Esc dismisses the modal
- Status bar shows a compact error indicator while the modal is closed

---

## 8. Confirmation Dialog (modal overlay)

Generic confirmation for destructive or significant actions.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  aztui                                                          ? for help │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│                                                                             │
│                                                                             │
│                                                                             │
│                ┌─────────────────────────────────────┐                      │
│                │                                     │                      │
│                │  Switch to Fabrikam Inc /            │                      │
│                │  fabrikam-terraform-prod?            │                      │
│                │                                     │                      │
│                │  This will run az login --tenant     │                      │
│                │  and change your active CLI context. │                      │
│                │                                     │                      │
│                │  [» Yes, switch ]     [ Cancel ]     │                      │
│                │                                     │                      │
│                └─────────────────────────────────────┘                      │
│                                                                             │
│                                                                             │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  ● Contoso Ltd / contoso-terraform-prod                            3s ago   │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Notes**:
- Tab or arrow keys to move between buttons
- Enter to confirm, Esc to cancel (always safe to dismiss)
- The `on_confirm` field in `Modal::Confirm` holds the command to dispatch

---

## 9. Loading / First Launch State

What the user sees when aztui launches for the first time or when the context
list is being fetched.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  aztui                                                          ? for help │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│                                                                             │
│                                                                             │
│                                                                             │
│                                                                             │
│                                                                             │
│                       ↻ Loading tenants and                                 │
│                         subscriptions...                                    │
│                                                                             │
│                         Running: az account list --all                      │
│                                                                             │
│                                                                             │
│                                                                             │
│                                                                             │
│                                                                             │
│                                                                             │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  No active context                                                          │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Notes**:
- Spinner character (`↻`) animates through a sequence (e.g. `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`)
- Shows which `az` command is running, so the user knows what's happening
- If `az` is not found on PATH, this transitions directly to an error view
  with `ErrorKind::CliNotFound` and recovery hint to install Azure CLI

---

## Layout Constants

These are guidelines, not pixel-perfect specs. The TUI should adapt to
terminal size.

- **Minimum terminal size**: 80 columns × 24 rows
- **Status bar**: 1 row, always at bottom (configurable to top)
- **Title bar**: 1 row, always at top
- **Content area**: remaining rows between title and status bar
- **Modals**: centered, 60–70% of terminal width, height fits content
- **Two-pane views** (resource browser): left pane ~35%, right pane ~65%
- **Inline bars** (cost explorer): 10 characters wide (`██████████`)