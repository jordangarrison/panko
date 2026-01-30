//! Application state management for the TUI.

use crate::scanner::{ClaudeScanner, SessionMeta, SessionScanner};
use crate::tui::widgets::{PreviewPanel, SessionList, SessionListState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

/// Application result type.
pub type AppResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Minimum terminal dimensions for the TUI to function properly.
pub const MIN_WIDTH: u16 = 60;
pub const MIN_HEIGHT: u16 = 10;

/// Which panel currently has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusedPanel {
    /// Session list panel (left side).
    #[default]
    SessionList,
    /// Preview panel (right side).
    Preview,
}

impl FocusedPanel {
    /// Toggle between panels.
    pub fn toggle(&mut self) {
        *self = match self {
            FocusedPanel::SessionList => FocusedPanel::Preview,
            FocusedPanel::Preview => FocusedPanel::SessionList,
        };
    }
}

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
    /// Which panel currently has focus
    focused_panel: FocusedPanel,
    /// Search query input
    search_query: String,
    /// Whether we're currently in search input mode
    search_active: bool,
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
            focused_panel: FocusedPanel::default(),
            search_query: String::new(),
            search_active: false,
        }
    }

    /// Create a new application with scanned sessions.
    pub fn with_sessions(sessions: Vec<SessionMeta>) -> Self {
        Self {
            running: true,
            width: 0,
            height: 0,
            session_list_state: SessionListState::from_sessions(sessions),
            focused_panel: FocusedPanel::default(),
            search_query: String::new(),
            search_active: false,
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
            // Tab: switch focus between panels
            KeyCode::Tab => {
                self.focused_panel.toggle();
            }
            // Navigation: j or down arrow to move down (only when session list focused)
            KeyCode::Char('j') | KeyCode::Down
                if self.focused_panel == FocusedPanel::SessionList =>
            {
                self.session_list_state.select_next();
            }
            // Navigation: k or up arrow to move up (only when session list focused)
            KeyCode::Char('k') | KeyCode::Up if self.focused_panel == FocusedPanel::SessionList => {
                self.session_list_state.select_previous();
            }
            // Navigation: h or left arrow to collapse/go to parent (only when session list focused)
            KeyCode::Char('h') | KeyCode::Left
                if self.focused_panel == FocusedPanel::SessionList =>
            {
                self.session_list_state.collapse_or_parent();
            }
            // Navigation: l or right arrow to expand (only when session list focused)
            KeyCode::Char('l') | KeyCode::Right
                if self.focused_panel == FocusedPanel::SessionList =>
            {
                self.session_list_state.expand_or_select();
            }
            // Navigation: g twice to go to first (handled by tick or state)
            // For now, single 'g' goes to first
            KeyCode::Char('g') if self.focused_panel == FocusedPanel::SessionList => {
                self.session_list_state.select_first();
            }
            // Navigation: G to go to last
            KeyCode::Char('G') if self.focused_panel == FocusedPanel::SessionList => {
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

    /// Get the currently focused panel.
    pub fn focused_panel(&self) -> FocusedPanel {
        self.focused_panel
    }

    /// Set the focused panel.
    pub fn set_focused_panel(&mut self, panel: FocusedPanel) {
        self.focused_panel = panel;
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

    /// Check if the terminal size is too small.
    fn is_too_small(&self, area: Rect) -> bool {
        area.width < MIN_WIDTH || area.height < MIN_HEIGHT
    }

    /// Render the application UI.
    pub fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();

        // Check for minimum terminal size
        if self.is_too_small(area) {
            self.render_too_small(frame, area);
            return;
        }

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

    /// Render message when terminal is too small.
    fn render_too_small(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Agent Replay ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "Terminal too small",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(format!("Minimum size: {}x{}", MIN_WIDTH, MIN_HEIGHT)),
            Line::from(format!("Current size: {}x{}", area.width, area.height)),
            Line::from(""),
            Line::from("Please resize your terminal."),
        ];

        let paragraph = Paragraph::new(text).alignment(Alignment::Center);

        // Center vertically if there's enough space
        if inner.height >= 7 {
            let vertical_center = Layout::vertical([
                Constraint::Length((inner.height - 7) / 2),
                Constraint::Length(7),
                Constraint::Min(0),
            ])
            .split(inner)[1];
            frame.render_widget(paragraph, vertical_center);
        } else {
            frame.render_widget(paragraph, inner);
        }
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

        // Create a horizontal layout for header content
        let header_chunks = Layout::horizontal([
            Constraint::Length(20), // Session count
            Constraint::Min(10),    // Search area (flexible)
            Constraint::Length(10), // Help hint
        ])
        .split(inner);

        // Left: Session count
        let session_text = Line::from(vec![
            Span::raw("Sessions: "),
            Span::styled(
                format!("{}", session_count),
                Style::default().fg(Color::Cyan),
            ),
        ]);
        frame.render_widget(
            Paragraph::new(session_text).alignment(Alignment::Left),
            header_chunks[0],
        );

        // Center: Search input area (placeholder for now)
        let search_text = if self.search_query.is_empty() {
            Line::from(vec![
                Span::styled("Search: ", Style::default().fg(Color::DarkGray)),
                Span::styled("(press / to search)", Style::default().fg(Color::DarkGray)),
            ])
        } else {
            Line::from(vec![
                Span::styled("Search: ", Style::default().fg(Color::DarkGray)),
                Span::styled(&self.search_query, Style::default().fg(Color::White)),
            ])
        };
        frame.render_widget(
            Paragraph::new(search_text).alignment(Alignment::Center),
            header_chunks[1],
        );

        // Right: Help hint
        let help_text = Line::from(vec![Span::styled(
            "[?] Help",
            Style::default().fg(Color::DarkGray),
        )]);
        frame.render_widget(
            Paragraph::new(help_text).alignment(Alignment::Right),
            header_chunks[2],
        );
    }

    /// Render the main content area (session list and preview panel).
    fn render_content(&mut self, frame: &mut Frame, area: Rect) {
        // Split into two columns: session list (left) and preview (right)
        let chunks = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        // Render session list on the left
        self.render_session_list(frame, chunks[0]);

        // Render preview panel on the right
        self.render_preview(frame, chunks[1]);
    }

    /// Render the session list panel.
    fn render_session_list(&mut self, frame: &mut Frame, area: Rect) {
        let is_focused = self.focused_panel == FocusedPanel::SessionList;
        let border_style = if is_focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let title = if is_focused {
            " Sessions [focused] "
        } else {
            " Sessions "
        };

        if self.session_list_state.is_empty() {
            // Show empty state message
            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .border_style(border_style);

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
                .title(title)
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .border_style(border_style);

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

    /// Render the preview panel.
    fn render_preview(&self, frame: &mut Frame, area: Rect) {
        let is_focused = self.focused_panel == FocusedPanel::Preview;
        let border_style = if is_focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let title = if is_focused {
            " Preview [focused] "
        } else {
            " Preview "
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(border_style);

        let selected_session = self.selected_session();

        let widget = PreviewPanel::new()
            .block(block)
            .session(selected_session)
            .label_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .value_style(Style::default().fg(Color::White))
            .prompt_style(Style::default().fg(Color::Gray));

        frame.render_widget(widget, area);
    }

    /// Render the footer with keyboard hints.
    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let hints = Line::from(vec![
            Span::styled(" j/k ", Style::default().fg(Color::Cyan)),
            Span::raw("nav  "),
            Span::styled("h/l ", Style::default().fg(Color::Cyan)),
            Span::raw("collapse/expand  "),
            Span::styled("Tab ", Style::default().fg(Color::Cyan)),
            Span::raw("switch panel  "),
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

    // New tests for focus and layout functionality

    #[test]
    fn test_focused_panel_default() {
        let app = App::new();
        assert_eq!(app.focused_panel(), FocusedPanel::SessionList);
    }

    #[test]
    fn test_focused_panel_toggle() {
        let mut panel = FocusedPanel::SessionList;
        panel.toggle();
        assert_eq!(panel, FocusedPanel::Preview);
        panel.toggle();
        assert_eq!(panel, FocusedPanel::SessionList);
    }

    #[test]
    fn test_handle_key_tab_switches_focus() {
        let mut app = App::new();
        assert_eq!(app.focused_panel(), FocusedPanel::SessionList);
        app.handle_key_event(key_event(KeyCode::Tab)).unwrap();
        assert_eq!(app.focused_panel(), FocusedPanel::Preview);
        app.handle_key_event(key_event(KeyCode::Tab)).unwrap();
        assert_eq!(app.focused_panel(), FocusedPanel::SessionList);
    }

    #[test]
    fn test_set_focused_panel() {
        let mut app = App::new();
        app.set_focused_panel(FocusedPanel::Preview);
        assert_eq!(app.focused_panel(), FocusedPanel::Preview);
        app.set_focused_panel(FocusedPanel::SessionList);
        assert_eq!(app.focused_panel(), FocusedPanel::SessionList);
    }

    #[test]
    fn test_navigation_only_works_when_session_list_focused() {
        let mut app = App::with_sessions(sample_sessions());
        assert_eq!(app.session_list_state.selected(), 0);

        // Navigation works when session list is focused
        app.handle_key_event(key_event(KeyCode::Char('j'))).unwrap();
        assert_eq!(app.session_list_state.selected(), 1);

        // Switch to preview panel
        app.set_focused_panel(FocusedPanel::Preview);

        // Navigation doesn't change selection when preview is focused
        let selection_before = app.session_list_state.selected();
        app.handle_key_event(key_event(KeyCode::Char('j'))).unwrap();
        assert_eq!(app.session_list_state.selected(), selection_before);

        app.handle_key_event(key_event(KeyCode::Char('k'))).unwrap();
        assert_eq!(app.session_list_state.selected(), selection_before);
    }

    #[test]
    fn test_is_too_small() {
        let app = App::new();

        // Too narrow
        let small_rect = Rect::new(0, 0, MIN_WIDTH - 1, MIN_HEIGHT);
        assert!(app.is_too_small(small_rect));

        // Too short
        let short_rect = Rect::new(0, 0, MIN_WIDTH, MIN_HEIGHT - 1);
        assert!(app.is_too_small(short_rect));

        // Both too small
        let tiny_rect = Rect::new(0, 0, MIN_WIDTH - 1, MIN_HEIGHT - 1);
        assert!(app.is_too_small(tiny_rect));

        // Just right
        let ok_rect = Rect::new(0, 0, MIN_WIDTH, MIN_HEIGHT);
        assert!(!app.is_too_small(ok_rect));

        // Larger is fine
        let large_rect = Rect::new(0, 0, 100, 50);
        assert!(!app.is_too_small(large_rect));
    }

    #[test]
    fn test_refresh_r_works_regardless_of_focus() {
        let mut app = App::new();

        // Works when session list is focused
        app.set_focused_panel(FocusedPanel::SessionList);
        app.handle_key_event(key_event(KeyCode::Char('r'))).unwrap();
        // Should not crash

        // Also works when preview is focused
        app.set_focused_panel(FocusedPanel::Preview);
        app.handle_key_event(key_event(KeyCode::Char('r'))).unwrap();
        // Should not crash
    }

    #[test]
    fn test_quit_works_regardless_of_focus() {
        let mut app = App::new();
        app.set_focused_panel(FocusedPanel::Preview);

        // q should still quit even when preview is focused
        app.handle_key_event(key_event(KeyCode::Char('q'))).unwrap();
        assert!(!app.is_running());
    }
}
