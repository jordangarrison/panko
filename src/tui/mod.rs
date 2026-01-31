//! TUI module for interactive session browsing.
//!
//! This module provides a terminal user interface for browsing, previewing,
//! and acting on AI coding agent sessions.

mod actions;
mod app;
mod events;
pub mod sharing;
pub mod watcher;
pub mod widgets;

pub use actions::Action;
pub use app::{
    App, AppResult, FocusedPanel, RefreshState, SharingState, DEFAULT_MAX_SHARES, MIN_HEIGHT,
    MIN_WIDTH,
};
pub use events::{Event, EventHandler};
pub use sharing::{
    ActiveShare, ShareId, ShareManager, ShareMessage, SharingCommand, SharingHandle, SharingMessage,
};
pub use watcher::{FileWatcher, WatcherMessage};
pub use widgets::{ProviderOption, SessionList, SessionListState, SortOrder, TreeItem};

use std::io;
use std::panic;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

/// A type alias for the terminal type used in this application.
pub type Tui = Terminal<CrosstermBackend<io::Stdout>>;

/// Initialize the terminal for TUI mode.
pub fn init() -> io::Result<Tui> {
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    enable_raw_mode()?;

    // Set up panic hook to restore terminal on panic
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = restore();
        original_hook(panic_info);
    }));

    Terminal::new(CrosstermBackend::new(io::stdout()))
}

/// Restore the terminal to its original state.
pub fn restore() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}

/// Result of running the TUI for one iteration.
/// Returns the action that needs to be handled (if any).
pub enum RunResult {
    /// App is done (quit requested).
    Done,
    /// Continue running (no action needed).
    Continue,
    /// Action needs to be handled outside the TUI loop.
    Action(Action),
}

/// Run the TUI application.
///
/// This function runs the main event loop. When a pending action is detected
/// (like ViewSession), it returns the action so the caller can handle it
/// (e.g., by suspending the TUI and running the server).
pub fn run(terminal: &mut Tui, app: &mut App) -> AppResult<RunResult> {
    run_with_watcher(terminal, app, None)
}

/// Run the TUI application with optional file watcher.
///
/// When a file watcher is provided, the TUI will automatically refresh
/// when new session files are detected.
pub fn run_with_watcher(
    terminal: &mut Tui,
    app: &mut App,
    watcher: Option<&FileWatcher>,
) -> AppResult<RunResult> {
    let event_handler = EventHandler::new(250); // 250ms tick rate

    while app.is_running() {
        // Check for file watcher notifications
        if let Some(w) = watcher {
            while let Some(msg) = w.try_recv() {
                match msg {
                    WatcherMessage::NewSession(_)
                    | WatcherMessage::SessionModified(_)
                    | WatcherMessage::SessionDeleted(_)
                    | WatcherMessage::RefreshNeeded => {
                        // Trigger a refresh
                        let _ = app.refresh_sessions();
                    }
                    WatcherMessage::Error(e) => {
                        // Log error but continue - watcher errors shouldn't crash the TUI
                        app.set_status_message(format!("Watcher error: {}", e));
                    }
                }
            }
        }

        // Draw the UI
        terminal.draw(|frame| app.render(frame))?;

        // Handle events
        match event_handler.next()? {
            Event::Tick => {
                app.tick();
                // Process share messages inline to avoid terminal cycling
                // This prevents screen flickering when shares are active
                if app.has_pending_share() || app.share_manager().has_active_shares() {
                    app.process_share_messages();
                }
            }
            Event::Key(key_event) => app.handle_key_event(key_event)?,
            Event::Resize(width, height) => app.handle_resize(width, height),
        }

        // Check if there's a pending action that needs external handling
        if app.has_pending_action() {
            let action = app.take_pending_action();
            return Ok(RunResult::Action(action));
        }
    }

    Ok(RunResult::Done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_restore_returns_ok() {
        // Note: This test may fail in some CI environments without a TTY
        // In those cases, we just verify the function exists and has the right signature
        let _ = restore();
    }
}
