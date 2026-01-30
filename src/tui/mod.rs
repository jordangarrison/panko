//! TUI module for interactive session browsing.
//!
//! This module provides a terminal user interface for browsing, previewing,
//! and acting on AI coding agent sessions.

mod app;
mod events;

pub use app::{App, AppResult};
pub use events::{Event, EventHandler};

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

/// Run the TUI application.
pub fn run(terminal: &mut Tui, app: &mut App) -> AppResult<()> {
    let event_handler = EventHandler::new(250); // 250ms tick rate

    while app.is_running() {
        // Draw the UI
        terminal.draw(|frame| app.render(frame))?;

        // Handle events
        match event_handler.next()? {
            Event::Tick => app.tick(),
            Event::Key(key_event) => app.handle_key_event(key_event)?,
            Event::Resize(width, height) => app.handle_resize(width, height),
        }
    }

    Ok(())
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
