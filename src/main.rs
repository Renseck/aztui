use std::io;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use crossterm::event::{self, Event as CrosstermEvent, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::{mpsc, RwLock};

use aztui::app::{dispatch_command, handle_event, AppState, Modal};
use aztui::az::SubprocessCliExecutor;
use aztui::cache::CacheStore;
use aztui::command::Command;
use aztui::config::AppConfig;
use aztui::errors::AppError;
use aztui::event::Event;
use aztui::providers::AzAuthProvider;
use aztui::ui::{handle_input, render, Theme};

/* ============================================================================================== */
#[derive(Parser, Debug)]
#[command(name = "aztui", version, about = "A TUI wrapper for Azure CLI operations")]
struct Cli {
    /// Path to config file (default: ~/.aztui/config.toml)
    #[arg(short, long)]
    config: Option<std::path::PathBuf>,
}

/* ============================================================================================== */
#[tokio::main]
async fn main() -> Result<(), AppError> {
    let cli = Cli::parse();

    // Install panic hook to restore terminal before printing the panic.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stderr(), LeaveAlternateScreen);
    }));

    let config = AppConfig::load(cli.config)?;
    let theme = Theme::detect();

    // Infrastructure
    let executor: Arc<dyn aztui::az::AzCliExecutor> = Arc::new(SubprocessCliExecutor::new(config.cli.az_path.clone())?);
    let cache = Arc::new(RwLock::new(CacheStore::new()));
    let auth: Arc<dyn aztui::domain::AuthProvider> = Arc::new(AzAuthProvider::new(
        Arc::clone(&executor),
        Arc::clone(&cache),
        config.cache.clone(),
    ));

    let mut state = AppState::new(config);

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

    // Bootstrap: load context list and active context on startup.
    cmd_tx
        .send(Command::RefreshContextList)
        .await
        .map_err(|e| AppError::unknown(e.to_string()))?;

    let tick_duration = Duration::from_millis(100);
    let result = run_loop(
        &mut terminal,
        &mut state,
        &mut cmd_rx,
        &mut event_rx,
        &cmd_tx,
        &event_tx,
        Arc::clone(&auth),
        &theme,
        tick_duration,
    )
    .await;

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
            let emitted = dispatch_command(state, cmd, cmd_tx, Arc::clone(&auth)).await;
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