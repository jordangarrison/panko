//! Application state management for the TUI.

use crate::scanner::{ClaudeScanner, SessionMeta, SessionScanner};
use crate::tui::widgets::{SessionList, SessionListState};
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
    /// Session list state for the tree view
    session_list_state: SessionListState,
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
            session_list_state: SessionListState::new(),
        }
    }

    /// Create a new application with scanned sessions.
    pub fn with_sessions(sessions: Vec<SessionMeta>) -> Self {
        Self {
            running: true,
            width: 0,
            height: 0,
            session_list_state: SessionListState::from_sessions(sessions),
        }
    }

    /// Load sessions from the default scanner locations.
    pub fn load_sessions(&mut self) -> AppResult<()> {
        let scanner = ClaudeScanner::new();
        let mut all_sessions = Vec::new();

        for root in scanner.default_roots() {
            if root.exists() {
                match scanner.scan_directory(&root) {
                    Ok(sessions) => all_sessions.extend(sessions),
                    Err(e) => {
                        // Log error but continue scanning other roots
                        eprintln!("Warning: Failed to scan {}: {}", root.display(), e);
                    }
                }
            }
        }

        self.session_list_state = SessionListState::from_sessions(all_sessions);
        Ok(())
    }

    /// Refresh the session list.
    pub fn refresh_sessions(&mut self) -> AppResult<()> {
        self.load_sessions()
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
            // Navigation: j or down arrow to move down
            KeyCode::Char('j') | KeyCode::Down => {
                self.session_list_state.select_next();
            }
            // Navigation: k or up arrow to move up
            KeyCode::Char('k') | KeyCode::Up => {
                self.session_list_state.select_previous();
            }
            // Navigation: h or left arrow to collapse/go to parent
            KeyCode::Char('h') | KeyCode::Left => {
                self.session_list_state.collapse_or_parent();
            }
            // Navigation: l or right arrow to expand
            KeyCode::Char('l') | KeyCode::Right => {
                self.session_list_state.expand_or_select();
            }
            // Navigation: g twice to go to first (handled by tick or state)
            // For now, single 'g' goes to first
            KeyCode::Char('g') => {
                self.session_list_state.select_first();
            }
            // Navigation: G to go to last
            KeyCode::Char('G') => {
                self.session_list_state.select_last();
            }
            // Refresh: r to reload sessions
            KeyCode::Char('r') => {
                let _ = self.refresh_sessions();
            }
            // Escape closes app (for now, will change later for overlays)
            KeyCode::Esc => {
                self.quit();
            }
            _ => {}
        }
        Ok(())
    }

    /// Get the session list state.
    pub fn session_list_state(&self) -> &SessionListState {
        &self.session_list_state
    }

    /// Get mutable session list state.
    pub fn session_list_state_mut(&mut self) -> &mut SessionListState {
        &mut self.session_list_state
    }

    /// Get the currently selected session.
    pub fn selected_session(&self) -> Option<&SessionMeta> {
        self.session_list_state.selected_session()
    }

    /// Render the application UI.
    pub fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();

        // Main layout with header, content, and footer
        let chunks = Layout::vertical([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Content
            Constraint::Length(1), // Footer
        ])
        .split(area);

        // Render header
        self.render_header(frame, chunks[0]);

        // Render content (session list for now)
        self.render_content(frame, chunks[1]);

        // Render footer
        self.render_footer(frame, chunks[2]);
    }

    /// Render the header section.
    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Agent Replay ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded);

        let session_count = self.session_list_state.visible_count();
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let text = Line::from(vec![
            Span::raw("Sessions: "),
            Span::styled(
                format!("{}", session_count),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw("                                    "),
            Span::styled("[?] Help", Style::default().fg(Color::DarkGray)),
        ]);

        let paragraph = Paragraph::new(text).alignment(Alignment::Left);
        frame.render_widget(paragraph, inner);
    }

    /// Render the main content area (session list).
    fn render_content(&mut self, frame: &mut Frame, area: Rect) {
        if self.session_list_state.is_empty() {
            // Show empty state message
            let block = Block::default()
                .title(" Sessions ")
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded);

            let inner = block.inner(area);
            frame.render_widget(block, area);

            let text = vec![
                Line::from(""),
                Line::from("No sessions found."),
                Line::from(""),
                Line::from(Span::styled(
                    "Sessions are stored in ~/.claude/projects/",
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(""),
                Line::from("Press 'r' to refresh or 'q' to quit."),
            ];

            let paragraph = Paragraph::new(text).alignment(Alignment::Center);
            let vertical_center = if inner.height > 6 {
                Layout::vertical([
                    Constraint::Length((inner.height - 6) / 2),
                    Constraint::Length(6),
                    Constraint::Min(0),
                ])
                .split(inner)[1]
            } else {
                inner
            };

            frame.render_widget(paragraph, vertical_center);
        } else {
            // Render session list
            let block = Block::default()
                .title(" Sessions ")
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded);

            let widget = SessionList::new()
                .block(block)
                .highlight_style(
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                )
                .project_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
                .normal_style(Style::default().fg(Color::Gray));

            frame.render_stateful_widget(widget, area, &mut self.session_list_state);
        }
    }

    /// Render the footer with keyboard hints.
    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let hints = Line::from(vec![
            Span::styled(" j/k ", Style::default().fg(Color::Cyan)),
            Span::raw("navigate  "),
            Span::styled("h/l ", Style::default().fg(Color::Cyan)),
            Span::raw("collapse/expand  "),
            Span::styled("r ", Style::default().fg(Color::Cyan)),
            Span::raw("refresh  "),
            Span::styled("q ", Style::default().fg(Color::Cyan)),
            Span::raw("quit"),
        ]);

        let paragraph = Paragraph::new(hints)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);

        frame.render_widget(paragraph, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    use std::path::PathBuf;

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

    fn test_timestamp() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn sample_sessions() -> Vec<SessionMeta> {
        vec![
            SessionMeta::new(
                "abc12345",
                PathBuf::from("/home/user/.claude/projects/api/abc12345.jsonl"),
                "~/projects/api",
                test_timestamp(),
            )
            .with_message_count(12),
            SessionMeta::new(
                "def67890",
                PathBuf::from("/home/user/.claude/projects/api/def67890.jsonl"),
                "~/projects/api",
                test_timestamp(),
            )
            .with_message_count(8),
        ]
    }

    #[test]
    fn test_app_new() {
        let app = App::new();
        assert!(app.is_running());
        assert!(app.session_list_state.is_empty());
    }

    #[test]
    fn test_app_default() {
        let app = App::default();
        assert!(app.is_running());
    }

    #[test]
    fn test_app_with_sessions() {
        let app = App::with_sessions(sample_sessions());
        assert!(app.is_running());
        assert!(!app.session_list_state.is_empty());
        assert_eq!(app.session_list_state.visible_count(), 3); // 1 project + 2 sessions
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
    fn test_handle_key_j_moves_down() {
        let mut app = App::with_sessions(sample_sessions());
        assert_eq!(app.session_list_state.selected(), 0);
        app.handle_key_event(key_event(KeyCode::Char('j'))).unwrap();
        assert_eq!(app.session_list_state.selected(), 1);
    }

    #[test]
    fn test_handle_key_k_moves_up() {
        let mut app = App::with_sessions(sample_sessions());
        app.session_list_state_mut().select_next();
        assert_eq!(app.session_list_state.selected(), 1);
        app.handle_key_event(key_event(KeyCode::Char('k'))).unwrap();
        assert_eq!(app.session_list_state.selected(), 0);
    }

    #[test]
    fn test_handle_key_down_moves_down() {
        let mut app = App::with_sessions(sample_sessions());
        app.handle_key_event(key_event(KeyCode::Down)).unwrap();
        assert_eq!(app.session_list_state.selected(), 1);
    }

    #[test]
    fn test_handle_key_up_moves_up() {
        let mut app = App::with_sessions(sample_sessions());
        app.session_list_state_mut().select_next();
        app.handle_key_event(key_event(KeyCode::Up)).unwrap();
        assert_eq!(app.session_list_state.selected(), 0);
    }

    #[test]
    fn test_handle_key_g_goes_to_first() {
        let mut app = App::with_sessions(sample_sessions());
        app.session_list_state_mut().select_last();
        app.handle_key_event(key_event(KeyCode::Char('g'))).unwrap();
        assert_eq!(app.session_list_state.selected(), 0);
    }

    #[test]
    fn test_handle_key_shift_g_goes_to_last() {
        let mut app = App::with_sessions(sample_sessions());
        app.handle_key_event(key_event(KeyCode::Char('G'))).unwrap();
        assert_eq!(app.session_list_state.selected(), 2); // 1 project + 2 sessions - 1
    }

    #[test]
    fn test_handle_key_h_collapses() {
        let mut app = App::with_sessions(sample_sessions());
        // First item is a project, should be expanded
        app.handle_key_event(key_event(KeyCode::Char('h'))).unwrap();
        // Project should now be collapsed
        assert_eq!(app.session_list_state.visible_count(), 1); // Just the project
    }

    #[test]
    fn test_handle_key_l_expands() {
        let mut app = App::with_sessions(sample_sessions());
        // Collapse first
        app.handle_key_event(key_event(KeyCode::Char('h'))).unwrap();
        assert_eq!(app.session_list_state.visible_count(), 1);
        // Expand
        app.handle_key_event(key_event(KeyCode::Char('l'))).unwrap();
        assert_eq!(app.session_list_state.visible_count(), 3);
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

    #[test]
    fn test_selected_session() {
        let mut app = App::with_sessions(sample_sessions());
        // First item is a project, not a session
        assert!(app.selected_session().is_none());
        // Move to first session
        app.session_list_state_mut().select_next();
        assert!(app.selected_session().is_some());
    }
}
