use std::io;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use crossterm::event::{self, Event as CrosstermEvent, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::{mpsc, RwLock};

use aztui::app::{dispatch_command, handle_event, AppState, PasswordMode};
use aztui::az::SubprocessCliExecutor;
use aztui::cache::{DiskCache, DiskCacheData, CacheStore};
use aztui::command::Command;
use aztui::config::AppConfig;
use aztui::errors::AppError;
use aztui::event::Event;
use aztui::providers::{AzAuthProvider, AzCostProvider};
use aztui::security::SecurityManager;
use aztui::ui::{handle_input, render, Theme};

/* ============================================================================================== */
#[derive(Parser, Debug)]
#[command(name = "aztui", version, about = "A TUI wrapper for Azure CLI operations")]
struct Cli {
    /// Path to config file (default: ~/.aztui/config.toml)
    #[arg(short, long)]
    config: Option<std::path::PathBuf>,

    /// Reset the master password (re-run setup flow).
    #[arg(long)]
    reset_password: bool,

    #[command(subcommand)]
    command: Option<SubCommand>,
}

#[derive(clap::Subcommand, Debug)]
enum SubCommand {
    /// Initialize aztui: create config directory, install binary to PATH, write default config
    Init {
        /// Overwrite existing config.toml
        #[arg(long)]
        force: bool,
    },
    /// Update aztui to the latest GitHub release
    Update {
        /// Only check for an update; do not install it
        #[arg(long)]
        check: bool,
    }
}

/* ============================================================================================== */
#[tokio::main]
async fn main() -> Result<(), AppError> {
    let cli = Cli::parse();
    aztui::update::cleanup_leftover();

    // Install panic hook to restore terminal before printing the panic.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stderr(), LeaveAlternateScreen);
        original_hook(info);
    }));

    // Handle subcommands that run headlessly (no TUI).
    if let Some(subcmd) = &cli.command {
        match subcmd {
            SubCommand::Init { force } => {
                match aztui::setup::run_init(*force) {
                    Ok(_) => return Ok(()),
                    Err(e) => {
                        eprintln!("Setup failed: {}", e);
                        return Err(e);
                    }
                }
            }
            SubCommand::Update { check } => {
                let check = *check;
                let outcome =
                    tokio::task::spawn_blocking(move || aztui::update::run_update(check)).await;
                match outcome {
                    Ok(Ok(())) => return Ok(()),
                    Ok(Err(e)) => {
                        eprintln!("Update failed: {}", e);
                        return Err(e);
                    }
                    Err(join_err) => {
                        eprintln!("Update failed: {}", join_err);
                        return Err(AppError::unknown(format!(
                            "update task panicked: {}",
                            join_err
                        )));
                    }
                }
            }
        }
    }

    // First-run detection: if ~/.aztui/ doesn't exist, suggest init.
    if aztui::setup::is_first_run() {
        eprintln!("Welcome to aztui! It looks like this is your first run.");
        eprintln!("Run `aztui init` to set up the config directory and add aztui to your PATH.");
        eprintln!();
        eprintln!("Continuing with default settings...");
        eprintln!();
    }

    let config = AppConfig::load(cli.config)?;
    let theme = Theme::detect();

    // Security
    let mut security =SecurityManager::new(&config.security, & config.general.data_dir)?;

    if cli.reset_password {
        security.reset()?;
        eprintln!("Master password has been reset. You will be prompted to set a new one.");
    }

    // Try OS keyring unlock before starting the TUI.
    let _keyring_unlocked = security.try_keyring_unlock().unwrap_or(false);

    // Disk cache
    let disk_cache = DiskCache::new(&config.general.data_dir);
    let preloaded = if security.is_unlocked() || !security.is_enabled() {
        disk_cache.load(&security, config.cache.context_hard_ttl).unwrap_or(None)
    } else {
        None // Can't decrypt yet - will load after unlock.
    };

    // Infrastructure
    let executor: Arc<dyn aztui::az::AzCliExecutor> = 
        Arc::new(SubprocessCliExecutor::new(config.cli.az_path.clone())?);
    let cache = Arc::new(RwLock::new(CacheStore::new()));
    let auth: Arc<dyn aztui::domain::AuthProvider> = Arc::new(AzAuthProvider::new(
        Arc::clone(&executor),
        Arc::clone(&cache),
        config.cache.clone(),
    ));

    let resources: Arc<dyn aztui::domain::ResourceProvider> = Arc::new(
        aztui::providers::AzResourceProvider::new(
            Arc::clone(&executor),
            Arc::clone(&cache),
            config.cache.clone(),
        ),
    );

    let cost: Arc<dyn aztui::domain::CostProvider> = Arc::new(
        AzCostProvider::new(
            Arc::clone(&executor),
            Arc::clone(&cache),
            config.cache.clone(),
        ),
    );

    let vm: Arc<dyn aztui::domain::VmProvider> = Arc::new(
        aztui::providers::AzVmProvider::new(
            Arc::clone(&executor),
            Duration::from_secs(180),
        ),
    );

    let activity: Arc<dyn aztui::domain::ActivityLogProvider> = Arc::new(
        aztui::providers::AzActivityLogProvider::new(
            Arc::clone(&executor),
            config.cli.default_timeout,
        ),
    );

    let graph: Arc<dyn aztui::domain::GraphProvider> = Arc::new(
        aztui::providers::AzGraphProvider::new(
            Arc::clone(&executor),
            Arc::clone(&cache),
            config.cache.clone(),
        ),
    );

    let mut state = AppState::new(config.clone(), security);

    // Populate state from disk cache if available.
    if let Some(data) = preloaded {
        state.tenants = data.tenants;
        state.subscriptions_by_tenant = data.subscription_by_tenant;
        state.recent_contexts = data.recent_contexts;
    }

    // If security is enabled and not yet unlocked, show the appropriate modal.
    if state.security.is_enabled() && !state.security.is_unlocked() {
        let mode = if state.security.needs_setup() {
            PasswordMode::Setup
        } else {
            PasswordMode::Unlock
        };
        state.modal = Some(aztui::app::Modal::PasswordPrompt {
            input: String::new(),
            error: None,
            mode,
        });
        state.locked = true;
    }

     // Channels
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<Command>(64);
    let (event_tx, mut event_rx) = mpsc::channel::<Event>(64);

    // Terminal setup
    enable_raw_mode().map_err(|e| AppError::unknown(e.to_string()))?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).map_err(|e| AppError::unknown(e.to_string()))?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal =
        Terminal::new(backend).map_err(|e| AppError::unknown(e.to_string()))?;

    // Bootstrap: only load context list if not locked (otherwise, will load after unlock).
    if !state.locked {
        cmd_tx
            .send(Command::RefreshContextList)
            .await
            .map_err(|e| AppError::unknown(e.to_string()))?;
    }

    // Background, non-blocking update check (throttled to once per 24h).
    {
        let cmd_tx = cmd_tx.clone();
        tokio::spawn(async move {
            if let Ok(Some(version)) =
                tokio::task::spawn_blocking(aztui::update::background_check).await
            {
                let _ = cmd_tx.send(Command::NotifyUpdateAvailable(version)).await;
            }
        });
    }
    
    let tick_duration = Duration::from_millis(100);
    let result = run_loop(
        &mut terminal,
        &mut state,
        &mut cmd_rx,
        &mut event_rx,
        &cmd_tx,
        &event_tx,
        Arc::clone(&auth),
        Arc::clone(&resources),
        Arc::clone(&cost),
        Arc::clone(&vm),
        Arc::clone(&activity),
        Arc::clone(&graph),
        &theme,
        tick_duration,
    )
    .await;

    // Save cache to disk on graceful shutdown.
    let cache_data = DiskCacheData::from_state(
        &state.tenants,
        &state.subscriptions_by_tenant,
        &state.recent_contexts,
    );
    if let Err(e) = disk_cache.save(&cache_data, &state.security) {
        eprintln!("Warning: failed to save cache: {}", e);
    }

    // Cleanup terminal regardless of result.
    disable_raw_mode().ok();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();

    result
}

/* ============================================================================================== */
async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut AppState,
    cmd_rx: &mut mpsc::Receiver<Command>,
    event_rx: &mut mpsc::Receiver<Event>,
    cmd_tx: &mpsc::Sender<Command>,
    event_tx: &mpsc::Sender<Event>,
    auth: Arc<dyn aztui::domain::AuthProvider>,
    resources: Arc<dyn aztui::domain::ResourceProvider>,
    cost: Arc<dyn aztui::domain::CostProvider>,
    vm: Arc<dyn aztui::domain::VmProvider>,
    activity: Arc<dyn aztui::domain::ActivityLogProvider>,
    graph: Arc<dyn aztui::domain::GraphProvider>,
    theme: &Theme,
    tick: Duration,
) -> Result<(), AppError> {
    loop {
        // 1. 
        terminal
            .draw(|frame| render(frame, state,theme))
            .map_err(|e| AppError::unknown(e.to_string()))?;

        // 2. Poll input (non-block with tick timeout)
        if event::poll(tick).map_err(|e| AppError::unknown(e.to_string()))? {
            if let Ok(CrosstermEvent::Key(key)) = event::read() {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if let Some(cmd) = handle_input(key, state) {
                    // Handle Esc-in-search sentinel
                    if matches!(&cmd, Command::UpdateSearch(s) if s == "\x1B") {
                        state.search_query.clear();
                        state.search_focused = false;
                    } else if matches!(&cmd, Command::UpdateSearch(_)) && !state.search_focused {
                        state.search_focused = true;
                        let _ = cmd_tx.send(cmd).await;
                    } else {
                        let _ = cmd_tx.send(cmd).await;
                    }
                }
            }
        }

        // 3. Process pending commands
        while let Ok(cmd) = cmd_rx.try_recv() {
            let emitted = dispatch_command(
                state,
                cmd,
                cmd_tx,
                Arc::clone(&auth),
                Arc::clone(&resources),
                Arc::clone(&cost),
                Arc::clone(&vm),
                Arc::clone(&activity),
                Arc::clone(&graph),
            )
            .await;
            for ev in emitted {
                let _ = event_tx.send(ev).await;
            }
        }

        // 4. Process pending events
        while let Ok(ev) = event_rx.try_recv() {
            handle_event(state, &ev);
        }

        // 5. Inactivity lock check
        if state.should_lock() {
            let _ = cmd_tx.send(Command::Lock).await;
        }

        // 6. Animate spinner
        state.tick_spinner();

        // 7. Quit
        if state.should_quit {
            break;
        }
    }

    Ok(())
}