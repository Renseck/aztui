# aztui

A Terminal User Interface for Azure CLI operations. Built for operations teams
who work across many Azure tenants and subscriptions daily.

aztui wraps the Azure CLI (`az`) in a fast, keyboard-driven TUI that eliminates
flag memorization and repetitive CLI workflows.

## Features

- **Context switching** — Select a tenant and subscription from a searchable
  list. `az login --tenant` and `az account set --subscription` happen behind
  the scenes in one action.
- **Resource browser** — Browse resource groups and resources within the active
  subscription. Drill-down navigation with search and filtering.
- **Cost explorer** — View cost summaries by subscription or resource group.
  Per-service breakdown with inline bar charts and period navigation.
- **Quick switch** (`Ctrl+G`) — Fuzzy-find any tenant/subscription combo
  without leaving your current view.
- **Encrypted cache** — Optionally protect cached data with a master password
  (Argon2id + AES-256-GCM). OS keyring integration available.

## Prerequisites

- **Windows 10/11** (primary target)
- **Azure CLI** (`az`) installed and on your PATH
  — [Install the Azure CLI](https://docs.microsoft.com/cli/azure/install-azure-cli)
- At least one Azure tenant you can authenticate to via `az login`

## Installation

### 1. Download

Download `aztui.exe` from the
[latest release](../../releases/latest).

### 2. Run setup

Open a terminal (PowerShell, Windows Terminal, or cmd) and run the downloaded
executable with the `init` subcommand:

```bash
.\aztui.exe init
```

This will:

| Step | What it does |
|------|-------------|
| 1 | Create `%USERPROFILE%\.aztui\` directory |
| 2 | Copy `aztui.exe` into that directory (permanent location) |
| 3 | Add `%USERPROFILE%\.aztui\` to your user PATH |
| 4 | Write a default `config.toml` with commented options |

It also checks whether the Azure CLI is installed and warns you if it isn't.

### 3. Restart your terminal

Close and reopen your terminal so the PATH change takes effect.

### 4. Launch

From any directory, type:

```
aztui
```

You can now delete the original downloaded `aztui.exe` from your Downloads
folder — the setup copied it to its permanent location.

## Upgrading

Download the new `aztui.exe` and run `aztui init` again. It will overwrite the
binary in `%USERPROFILE%\.aztui\` with the new version. Your `config.toml` is
preserved (use `aztui init --force` to regenerate it with new defaults).

## Usage

### Keybindings

| Key | Action |
|-----|--------|
| `↑`/`↓` or `j`/`k` | Navigate list |
| `Enter` | Select / confirm |
| `/` | Focus search input |
| `Esc` | Clear search / close modal / back |
| `Ctrl+G` | Quick switch context |
| `Tab` / `←`/`→` | Switch pane (resource browser) |
| `[` or `h` | Previous month (cost explorer) |
| `]` or `l` | Next month (cost explorer) |
| `r` | Refresh current view |
| `1` / `2` / `3` / `4` | Context switcher / Resource browser / Cost explorer / Activity log viewer |
| `g` | Toggle cost grouping: service ↔ resource group (cost explorer) |
| `Enter` | Drill into resource group (cost explorer, grouped by RG) |
| `Backspace` | Up one level (cost explorer) |
| `c` | Cost for selected resource group (resource browser) |
| `?` | Toggle help screen |
| `q` | Quit |

### Views

- **1 — Context switcher**: List all tenants and subscriptions. Select one to
  set it as the active Azure context.
- **2 — Resource browser**: Two-pane view of resource groups (left) and
  resources (right) in the active subscription.
- **3 — Cost explorer**: Cost breakdown for the active subscription, grouped by
  service or by resource group (`g` to toggle). Drill into a resource group
  (`Enter`) for its per-service cost; `Backspace`/`Esc` to go back. Search is
  fuzzy across all list views.
- **4 - Activity log viewer**: Subscription-level timestamped activity logs. Pressing `a` from 
  the resource browser put you in scoped activity logs - pressing `s` takes you back
  to subscription level. `f` toggles to showing failures only.

### Configuration

Configuration lives at `%USERPROFILE%\.aztui\config.toml`. All settings have
sensible defaults — the file is fully commented so you can see what's available
without reading docs.

Key settings:

| Setting | Default | Description |
|---------|---------|-------------|
| `cache.context_soft_ttl` | 300 | Seconds before context list refreshes in background |
| `cache.cost_hard_ttl` | 1800 | Seconds before cost data must be re-fetched |
| `security.master_password_enabled` | false | Encrypt cached data at rest |
| `security.inactivity_timeout_secs` | 600 | Lock the app after idle time |
| `cli.az_path` | (auto) | Explicit path to `az` binary |
| `cli.login_timeout` | 120 | Seconds to wait for `az login` (MFA can be slow) |

### Command-line options

```
aztui              Launch the TUI
aztui init         First-time setup (install to PATH, create config)
aztui init --force Re-create config.toml with defaults
aztui --config <path>  Use a specific config file
aztui --reset-password Re-run master password setup
aztui --version    Print version
aztui --help       Print help
```

## How it works

aztui does not replace the Azure CLI — it delegates all Azure interactions to
the `az` binary as a subprocess. This means you get the same security model,
authentication flows, and API coverage as the official CLI, without aztui
needing Azure SDK bindings.

Data flow: **User input → Command → dispatch → mutate state → render UI**.
All CLI calls are async (via Tokio) so the TUI never blocks while waiting for
Azure responses.

## License

MIT