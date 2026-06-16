use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::errors::AppError;

/* ============================================================================================== */
/*                                        Default config                                         */
/* ============================================================================================== */

const DEFAULT_CONFIG_TOML: &str = r#"# aztui configuration
# All values shown below are defaults. Uncomment and modify as needed.

[general]
# data_dir = "~/.aztui"            # Where aztui stores its data
# default_tenant = ""              # Tenant ID to select on startup
# default_subscription = ""        # Subscription ID to select on startup
# max_recent_contexts = 10         # Number of recent contexts to remember

[cache]
# context_soft_ttl = 300           # 5 min  — serve cached, refresh in background
# context_hard_ttl = 3600          # 1 hour — force refresh
# resource_soft_ttl = 60           # 1 min
# resource_hard_ttl = 300          # 5 min
# cost_soft_ttl = 300              # 5 min
# cost_hard_ttl = 1800             # 30 min

[security]
# master_password_enabled = false  # Encrypt cached data with a master password
# inactivity_timeout_secs = 600    # Lock after 10 min idle (null to disable)
# use_os_keyring = false           # Store master key in OS keyring

[ui]
# mouse_enabled = true
# status_bar_position = "bottom"   # "top" or "bottom"
# show_operation_timing = true     # Show elapsed time for operations
# scroll_off = 3                   # Rows kept above/below cursor before scrolling (0 = edge)

[cli]
# az_path = ""                     # Path to az binary (auto-detect if empty)
# default_timeout = 30             # Seconds before a CLI command times out
# login_timeout = 120              # Seconds for login (MFA can be slow)
# output_format = "json"
"#;

/* ============================================================================================== */
/*                                         Public API                                            */
/* ============================================================================================== */

/// Result type for individual setup steps.
pub struct SetupResult {
    pub config_dir: PathBuf,
    pub binary_installed: bool,
    pub path_updated: bool,
    pub config_written: bool,
    pub az_found: Option<String>, // version string if found
}

/* ============================================================================================== */
/// Runs the full first-time setup flow, printing progress to stdout.
///
/// Steps:
/// 1. Create `~/.aztui/` directory
/// 2. Copy the running binary into `~/.aztui/aztui.exe`
/// 3. Add `~/.aztui/` to the user PATH (Windows registry)
/// 4. Write default `config.toml`
/// 5. Check for `az` CLI
pub fn run_init(force: bool) -> Result<SetupResult, AppError> {
    let config_dir = aztui_dir()?;

    println!();
    println!("  aztui v{} — first-time setup", env!("CARGO_PKG_VERSION"));
    println!("  {}", "─".repeat(40));
    println!();

    // Step 1: Create config directory.
    print!("  [1/4] Creating config directory...");
    io::stdout().flush().ok();
    fs::create_dir_all(&config_dir).map_err(|e| {
        AppError::config_error(format!("Cannot create {:?}: {}", config_dir, e))
    })?;
    println!("\r  [1/4] Creating config directory...     ");
    println!("        → {}  ✓", config_dir.display());

    // Step 2: Copy binary.
    let binary_installed = install_binary(&config_dir)?;

    // Step 3: Add to PATH.
    let path_updated = add_to_path(&config_dir)?;

    // Step 4: Write config.
    let config_path = config_dir.join("config.toml");
    let config_written = write_default_config(&config_path, force)?;

    // Step 5: Check az CLI.
    println!();
    println!("  Checking prerequisites:");
    let az_found = check_az_cli();
    match &az_found {
        Some(version) => {
            println!("    az CLI: found ({})  ✓", version.trim());
        }
        None => {
            println!("    az CLI: NOT FOUND  ✗");
            println!();
            println!("    aztui requires the Azure CLI. Install it from:");
            println!("    https://docs.microsoft.com/cli/azure/install-azure-cli");
        }
    }

    println!();
    println!("  {}", "─".repeat(40));
    if path_updated {
        println!("  Setup complete! Restart your terminal, then type: aztui");
    } else {
        println!("  Setup complete! Type: aztui");
    }
    println!();

    Ok(SetupResult {
        config_dir,
        binary_installed,
        path_updated,
        config_written,
        az_found,
    })
}

/* ============================================================================================== */
/// Returns true if `~/.aztui/` does not exist, indicating a first run.
pub fn is_first_run() -> bool {
    aztui_dir()
        .map(|d| !d.exists())
        .unwrap_or(false)
}

/* ============================================================================================== */
/*                                       Private helpers                                         */
/* ============================================================================================== */

/// Returns the path to `~/.aztui/` (i.e. `%USERPROFILE%\.aztui\`).
fn aztui_dir() -> Result<PathBuf, AppError> {
    dirs::home_dir()
        .map(|h| h.join(".aztui"))
        .ok_or_else(|| AppError::config_error("Cannot determine home directory"))
}

/* ============================================================================================== */
/// Copies the currently running executable into the config directory.
fn install_binary(config_dir: &Path) -> Result<bool, AppError> {
    print!("  [2/4] Installing binary...");
    io::stdout().flush().ok();

    let current_exe = env::current_exe().map_err(|e| {
        AppError::config_error(format!("Cannot determine current executable path: {}", e))
    })?;

    let target = config_dir.join("aztui.exe");

    // If we're already running the installed copy, there's nothing to do.
    if current_exe == target {
        println!("\r  [2/4] Installing binary...              ");
        println!("        → {} (running in place)  ✓", target.display());
        return Ok(false);
    }

    fs::copy(&current_exe, &target).map_err(|e| {
        AppError::config_error(format!("Cannot copy binary to {:?}: {}", target, e))
    })?;

    println!("\r  [2/4] Installing binary...              ");
    println!("        → {}  ✓", target.display());
    Ok(true)
}

/* ============================================================================================== */
/// Adds the config directory to the user's PATH via the Windows registry.
/// Returns `true` if PATH was modified, `false` if it was already present.
fn add_to_path(config_dir: &Path) -> Result<bool, AppError> {
    print!("  [3/4] Adding to PATH...");
    io::stdout().flush().ok();

    let dir_str = config_dir
        .to_str()
        .ok_or_else(|| AppError::config_error("Config directory path is not valid UTF-8"))?;

    #[cfg(windows)]
    {
        use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let env_key = hkcu
            .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
            .map_err(|e| {
                AppError::config_error(format!("Cannot open registry Environment key: {}", e))
            })?;

        let current_path: String = env_key.get_value("Path").unwrap_or_default();

        // Check if already on PATH (case-insensitive on Windows).
        let already = current_path
            .split(';')
            .any(|entry| entry.eq_ignore_ascii_case(dir_str));

        if already {
            println!("\r  [3/4] Adding to PATH...                 ");
            println!("        → Already on PATH  ✓");
            return Ok(false);
        }

        let new_path = if current_path.is_empty() {
            dir_str.to_string()
        } else {
            format!("{};{}", current_path, dir_str)
        };

        env_key.set_value("Path", &new_path).map_err(|e| {
            AppError::config_error(format!("Cannot update PATH in registry: {}", e))
        })?;

        // Broadcast WM_SETTINGCHANGE so Explorer picks up the change.
        broadcast_environment_change();

        println!("\r  [3/4] Adding to PATH...                 ");
        println!("        → Updated user PATH  ✓");
        Ok(true)
    }

    #[cfg(not(windows))]
    {
        // On non-Windows, just inform the user.
        println!("\r  [3/4] Adding to PATH...                 ");
        println!(
            "        → Add {} to your PATH manually  ⚠",
            dir_str
        );
        Ok(false)
    }
}

/* ============================================================================================== */
/// Broadcasts WM_SETTINGCHANGE so running Explorer shells pick up PATH changes.
#[cfg(windows)]
fn broadcast_environment_change() {
    use std::process::Command;
    // Use a small PowerShell snippet to send the broadcast.
    // This is more reliable than FFI and avoids pulling in the windows crate.
    let _ = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            r#"Add-Type -Namespace Win32 -Name NativeMethods -MemberDefinition '[DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Auto)] public static extern IntPtr SendMessageTimeout(IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam, uint fuFlags, uint uTimeout, out UIntPtr lpdwResult);'; $HWND_BROADCAST = [IntPtr]0xffff; $WM_SETTINGCHANGE = 0x1a; $result = [UIntPtr]::Zero; [Win32.NativeMethods]::SendMessageTimeout($HWND_BROADCAST, $WM_SETTINGCHANGE, [UIntPtr]::Zero, 'Environment', 2, 5000, [ref]$result) | Out-Null"#,
        ])
        .output();
}

/* ============================================================================================== */
/// Writes the default config.toml if it doesn't exist (or if `force` is true).
fn write_default_config(path: &Path, force: bool) -> Result<bool, AppError> {
    print!("  [4/4] Writing default config...");
    io::stdout().flush().ok();

    if path.exists() && !force {
        println!("\r  [4/4] Writing default config...          ");
        println!("        → {} (already exists, use --force to overwrite)  ✓", path.display());
        return Ok(false);
    }

    fs::write(path, DEFAULT_CONFIG_TOML).map_err(|e| {
        AppError::config_error(format!("Cannot write {:?}: {}", path, e))
    })?;

    println!("\r  [4/4] Writing default config...          ");
    println!("        → {}  ✓", path.display());
    Ok(true)
}

/* ============================================================================================== */
/// Attempts to run `az --version` and returns the first line if successful.
fn check_az_cli() -> Option<String> {
    let candidates = if cfg!(windows) {
        vec!["az.cmd", "az"]
    } else {
        vec!["az"]
    };

    for name in candidates {
        if let Ok(output) = std::process::Command::new(name)
            .arg("--version")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // First line is typically "azure-cli 2.67.0"
                if let Some(line) = stdout.lines().next() {
                    return Some(line.to_string());
                }
            }
        }
    }

    None
}
