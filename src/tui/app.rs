//! Application state management for the TUI.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

/// Application result type.
pub type AppResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Application state.
#[derive(Debug)]
pub struct App {
    /// Is the application running?
    running: bool,
    /// Terminal width
    width: u16,
    /// Terminal height
    height: u16,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    /// Create a new application instance.
    pub fn new() -> Self {
        Self {
            running: true,
            width: 0,
            height: 0,
        }
    }

    /// Returns true if the application is running.
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Set running status to false to quit the application.
    pub fn quit(&mut self) {
        self.running = false;
    }

    /// Handle a tick event.
    pub fn tick(&mut self) {
        // Update any time-based state here
    }

    /// Handle terminal resize.
    pub fn handle_resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
    }

    /// Handle key events.
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> AppResult<()> {
        match key_event.code {
            // Quit on 'q'
            KeyCode::Char('q') => {
                self.quit();
            }
            // Quit on Ctrl+C
            KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.quit();
            }
            // Quit on Escape
            KeyCode::Esc => {
                self.quit();
            }
            _ => {}
        }
        Ok(())
    }

    /// Render the application UI.
    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();

        // For now, just render a simple placeholder
        let block = Block::default()
            .title(" Agent Replay ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded);

        let inner = block.inner(area);

        frame.render_widget(block, area);

        // Render placeholder text
        let text = vec![
            Line::from("Welcome to Agent Replay!"),
            Line::from(""),
            Line::from("Press 'q' or Esc to quit"),
        ];

        let paragraph = Paragraph::new(text)
            .alignment(Alignment::Center)
            .block(Block::default());

        // Center the paragraph vertically
        let vertical_center = if inner.height > 3 {
            Layout::vertical([
                Constraint::Length((inner.height - 3) / 2),
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(inner)[1]
        } else {
            inner
        };

        frame.render_widget(paragraph, vertical_center);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn key_event(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn key_event_with_modifiers(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn test_app_new() {
        let app = App::new();
        assert!(app.is_running());
    }

    #[test]
    fn test_app_default() {
        let app = App::default();
        assert!(app.is_running());
    }

    #[test]
    fn test_app_quit() {
        let mut app = App::new();
        assert!(app.is_running());
        app.quit();
        assert!(!app.is_running());
    }

    #[test]
    fn test_handle_key_q_quits() {
        let mut app = App::new();
        app.handle_key_event(key_event(KeyCode::Char('q'))).unwrap();
        assert!(!app.is_running());
    }

    #[test]
    fn test_handle_key_esc_quits() {
        let mut app = App::new();
        app.handle_key_event(key_event(KeyCode::Esc)).unwrap();
        assert!(!app.is_running());
    }

    #[test]
    fn test_handle_key_ctrl_c_quits() {
        let mut app = App::new();
        app.handle_key_event(key_event_with_modifiers(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        ))
        .unwrap();
        assert!(!app.is_running());
    }

    #[test]
    fn test_handle_key_other_does_not_quit() {
        let mut app = App::new();
        app.handle_key_event(key_event(KeyCode::Char('j'))).unwrap();
        assert!(app.is_running());
    }

    #[test]
    fn test_handle_resize() {
        let mut app = App::new();
        app.handle_resize(100, 50);
        assert_eq!(app.width, 100);
        assert_eq!(app.height, 50);
    }

    #[test]
    fn test_tick_does_not_crash() {
        let mut app = App::new();
        app.tick(); // Should not panic
    }
}
