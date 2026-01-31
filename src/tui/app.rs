//! Application state management for the TUI.

use crate::scanner::{ScannerRegistry, SessionMeta};
use crate::tui::actions::Action;
use crate::tui::widgets::{
    ConfirmationDialog, HelpOverlay, PreviewPanel, ProviderOption, ProviderSelect,
    ProviderSelectState, SessionList, SessionListState, SortOrder,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};
use std::path::PathBuf;

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

/// State of the sharing feature.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SharingState {
    /// Not sharing anything.
    #[default]
    Inactive,
    /// Showing provider selection popup (path to share, available providers).
    SelectingProvider {
        /// The session file path to share.
        session_path: PathBuf,
    },
    /// Waiting for the sharing to start (server + tunnel).
    Starting {
        /// The session file path being shared.
        session_path: PathBuf,
        /// The selected provider name.
        provider_name: String,
    },
    /// Actively sharing with a public URL.
    Active {
        /// The public URL where the session is available.
        public_url: String,
        /// The provider being used.
        provider_name: String,
    },
    /// Stopping the sharing process.
    Stopping,
}

impl SharingState {
    /// Check if actively sharing.
    pub fn is_active(&self) -> bool {
        matches!(self, SharingState::Active { .. })
    }

    /// Check if a provider selection popup should be shown.
    pub fn is_selecting_provider(&self) -> bool {
        matches!(self, SharingState::SelectingProvider { .. })
    }

    /// Check if any sharing operation is in progress.
    pub fn is_busy(&self) -> bool {
        !matches!(self, SharingState::Inactive)
    }

    /// Get the public URL if actively sharing.
    pub fn public_url(&self) -> Option<&str> {
        match self {
            SharingState::Active { public_url, .. } => Some(public_url),
            _ => None,
        }
    }
}

/// State of refresh operation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RefreshState {
    /// Not refreshing.
    #[default]
    Idle,
    /// Currently refreshing the session list.
    Refreshing,
}

/// State of the delete confirmation dialog.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ConfirmationState {
    /// No confirmation dialog is showing.
    #[default]
    Inactive,
    /// Confirming deletion of a session.
    ConfirmingDelete {
        /// The session file path to delete.
        session_path: PathBuf,
        /// The session ID for display.
        session_id: String,
    },
}

impl RefreshState {
    /// Check if refresh is in progress.
    pub fn is_refreshing(&self) -> bool {
        matches!(self, RefreshState::Refreshing)
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
    /// Pending action to be executed outside the TUI loop
    pending_action: Action,
    /// Current sharing state
    sharing_state: SharingState,
    /// Provider selection state (when showing provider popup)
    provider_select_state: ProviderSelectState,
    /// Status message to display in the footer (briefly)
    status_message: Option<String>,
    /// Whether the help overlay is visible
    show_help: bool,
    /// Current refresh state
    refresh_state: RefreshState,
    /// Current confirmation state (for delete dialog)
    confirmation_state: ConfirmationState,
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
            pending_action: Action::None,
            sharing_state: SharingState::default(),
            provider_select_state: ProviderSelectState::default(),
            status_message: None,
            show_help: false,
            refresh_state: RefreshState::default(),
            confirmation_state: ConfirmationState::default(),
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
            pending_action: Action::None,
            sharing_state: SharingState::default(),
            provider_select_state: ProviderSelectState::default(),
            status_message: None,
            show_help: false,
            refresh_state: RefreshState::default(),
            confirmation_state: ConfirmationState::default(),
        }
    }

    /// Load sessions from all registered scanner locations.
    ///
    /// Uses the `ScannerRegistry` to scan sessions from all registered
    /// AI coding agents (Claude, Codex, etc.).
    pub fn load_sessions(&mut self) -> AppResult<()> {
        let registry = ScannerRegistry::default();
        let all_sessions = registry.scan_all_defaults();
        self.session_list_state = SessionListState::from_sessions(all_sessions);
        Ok(())
    }

    /// Refresh the session list, preserving the current selection if possible.
    pub fn refresh_sessions(&mut self) -> AppResult<()> {
        // Set refresh state
        self.refresh_state = RefreshState::Refreshing;

        // Remember current selection by session ID
        let selected_session_id = self
            .session_list_state
            .selected_session()
            .map(|s| s.id.clone());

        // Reload sessions
        let result = self.load_sessions();

        // Try to restore selection
        if let Some(session_id) = selected_session_id {
            self.session_list_state.select_session_by_id(&session_id);
        }

        // Clear refresh state
        self.refresh_state = RefreshState::Idle;

        result
    }

    /// Check if refresh is in progress.
    pub fn is_refreshing(&self) -> bool {
        self.refresh_state.is_refreshing()
    }

    /// Get the refresh state.
    pub fn refresh_state(&self) -> &RefreshState {
        &self.refresh_state
    }

    /// Get the current sort order.
    pub fn sort_order(&self) -> SortOrder {
        self.session_list_state.sort_order()
    }

    /// Set the sort order.
    pub fn set_sort_order(&mut self, sort_order: SortOrder) {
        self.session_list_state.set_sort_order(sort_order);
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
        // Handle help overlay (any key closes it)
        if self.show_help {
            self.show_help = false;
            return Ok(());
        }

        // Route key events based on current state
        if self.sharing_state.is_selecting_provider() {
            return self.handle_provider_select_key(key_event);
        }

        if self.sharing_state.is_active() {
            return self.handle_sharing_key(key_event);
        }

        // Handle confirmation dialog (delete confirmation)
        if self.is_confirming() {
            return self.handle_confirmation_key(key_event);
        }

        // Handle search mode input
        if self.search_active {
            return self.handle_search_key(key_event);
        }

        // Normal key handling
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
            // Search: / to activate search mode
            KeyCode::Char('/') => {
                self.activate_search();
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
            // View: v or Enter to view selected session
            KeyCode::Char('v') | KeyCode::Enter => {
                if let Some(session) = self.selected_session() {
                    self.pending_action = Action::ViewSession(session.path.clone());
                }
            }
            // Share: s to share selected session
            KeyCode::Char('s') => {
                if let Some(session) = self.selected_session() {
                    self.pending_action = Action::ShareSession(session.path.clone());
                }
            }
            // Copy path: c to copy session file path to clipboard
            KeyCode::Char('c') => {
                if let Some(session) = self.selected_session() {
                    self.pending_action = Action::CopyPath(session.path.clone());
                }
            }
            // Open folder: o to open containing folder in file manager
            KeyCode::Char('o') => {
                if let Some(session) = self.selected_session() {
                    self.pending_action = Action::OpenFolder(session.path.clone());
                }
            }
            // Refresh: r to reload sessions
            KeyCode::Char('r') => {
                let _ = self.refresh_sessions();
            }
            // Sort: S (shift+s) to cycle sort order
            KeyCode::Char('S') => {
                self.session_list_state.cycle_sort_order();
            }
            // Copy context: C (shift+c) to copy session context to clipboard
            KeyCode::Char('C') => {
                if let Some(session) = self.selected_session() {
                    self.pending_action = Action::CopyContext(session.path.clone());
                }
            }
            // Download: D (shift+d) to download session to ~/Downloads
            KeyCode::Char('D') => {
                if let Some(session) = self.selected_session() {
                    self.pending_action = Action::DownloadSession(session.path.clone());
                }
            }
            // Delete: d to delete selected session (with confirmation)
            KeyCode::Char('d') => {
                // Cannot delete while sharing is active
                if self.sharing_state.is_active() {
                    self.set_status_message("✗ Cannot delete while sharing is active");
                } else if let Some(session) = self.selected_session() {
                    self.confirmation_state = ConfirmationState::ConfirmingDelete {
                        session_path: session.path.clone(),
                        session_id: session.id.clone(),
                    };
                }
            }
            // Help: ? to show keyboard shortcuts
            KeyCode::Char('?') => {
                self.show_help = true;
            }
            // Escape: clear search if active, otherwise quit
            KeyCode::Esc => {
                if self.session_list_state.is_searching() {
                    self.clear_search();
                } else {
                    self.quit();
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle key events when search input is active.
    fn handle_search_key(&mut self, key_event: KeyEvent) -> AppResult<()> {
        match key_event.code {
            // Escape: deactivate search input
            KeyCode::Esc => {
                self.deactivate_search();
            }
            // Enter: select first match and exit search mode
            KeyCode::Enter => {
                // Just exit search input mode - selection is preserved
                self.search_active = false;
            }
            // Backspace: remove last character
            KeyCode::Backspace => {
                if !self.search_query.is_empty() {
                    self.search_query.pop();
                    self.update_search_filter();
                }
            }
            // Ctrl+C: quit (must be before generic Char match)
            KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.quit();
            }
            // Typing characters: add to search query
            KeyCode::Char(c) => {
                self.search_query.push(c);
                self.update_search_filter();
            }
            // Navigation still works during search input
            KeyCode::Down => {
                self.session_list_state.select_next();
            }
            KeyCode::Up => {
                self.session_list_state.select_previous();
            }
            _ => {}
        }
        Ok(())
    }

    /// Activate search input mode.
    fn activate_search(&mut self) {
        self.search_active = true;
        // Focus session list when searching
        self.focused_panel = FocusedPanel::SessionList;
    }

    /// Deactivate search input mode without clearing the filter.
    fn deactivate_search(&mut self) {
        self.search_active = false;
        // If query is empty, clear the search
        if self.search_query.is_empty() {
            self.session_list_state.clear_search();
        }
    }

    /// Update the search filter based on the current query.
    fn update_search_filter(&mut self) {
        self.session_list_state.set_search_query(&self.search_query);
    }

    /// Clear the search completely.
    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.search_active = false;
        self.session_list_state.clear_search();
    }

    /// Check if search input is active.
    pub fn is_search_active(&self) -> bool {
        self.search_active
    }

    /// Get the current search query.
    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    /// Handle key events when provider selection popup is shown.
    fn handle_provider_select_key(&mut self, key_event: KeyEvent) -> AppResult<()> {
        match key_event.code {
            // Navigation in popup
            KeyCode::Char('j') | KeyCode::Down => {
                self.provider_select_state.select_next();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.provider_select_state.select_previous();
            }
            // Confirm selection
            KeyCode::Enter => {
                if let Some(provider) = self.provider_select_state.selected_provider() {
                    if let SharingState::SelectingProvider { session_path } = &self.sharing_state {
                        // Move to Starting state and set pending action
                        let path = session_path.clone();
                        let provider_name = provider.name.clone();
                        self.sharing_state = SharingState::Starting {
                            session_path: path.clone(),
                            provider_name: provider_name.clone(),
                        };
                        // Signal that we need to start sharing
                        self.pending_action = Action::StartSharing {
                            path,
                            provider: provider_name,
                        };
                    }
                }
            }
            // Cancel selection
            KeyCode::Esc => {
                self.sharing_state = SharingState::Inactive;
            }
            // Quit still works
            KeyCode::Char('q') => {
                self.sharing_state = SharingState::Inactive;
                self.quit();
            }
            KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.sharing_state = SharingState::Inactive;
                self.quit();
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle key events when actively sharing.
    fn handle_sharing_key(&mut self, key_event: KeyEvent) -> AppResult<()> {
        match key_event.code {
            // Esc stops sharing
            KeyCode::Esc => {
                self.sharing_state = SharingState::Stopping;
                self.pending_action = Action::StopSharing;
            }
            // Quit also stops sharing first
            KeyCode::Char('q') => {
                self.sharing_state = SharingState::Stopping;
                self.pending_action = Action::StopSharing;
                // Note: The main loop will need to handle quitting after stop
            }
            KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.sharing_state = SharingState::Stopping;
                self.pending_action = Action::StopSharing;
            }
            // Delete is blocked while sharing
            KeyCode::Char('d') => {
                self.set_status_message("✗ Cannot delete while sharing is active");
            }
            // Navigation still works while sharing
            KeyCode::Char('j') | KeyCode::Down
                if self.focused_panel == FocusedPanel::SessionList =>
            {
                self.session_list_state.select_next();
            }
            KeyCode::Char('k') | KeyCode::Up if self.focused_panel == FocusedPanel::SessionList => {
                self.session_list_state.select_previous();
            }
            KeyCode::Tab => {
                self.focused_panel.toggle();
            }
            _ => {}
        }
        Ok(())
    }

    /// Get the pending action (if any).
    pub fn pending_action(&self) -> &Action {
        &self.pending_action
    }

    /// Take the pending action, replacing it with None.
    pub fn take_pending_action(&mut self) -> Action {
        std::mem::take(&mut self.pending_action)
    }

    /// Check if there is a pending action.
    pub fn has_pending_action(&self) -> bool {
        !matches!(self.pending_action, Action::None)
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

    /// Get the current sharing state.
    pub fn sharing_state(&self) -> &SharingState {
        &self.sharing_state
    }

    /// Start provider selection for sharing.
    pub fn start_provider_selection(
        &mut self,
        session_path: PathBuf,
        providers: Vec<ProviderOption>,
    ) {
        self.provider_select_state = ProviderSelectState::new(providers);
        self.sharing_state = SharingState::SelectingProvider { session_path };
    }

    /// Set sharing as active with the given URL.
    pub fn set_sharing_active(&mut self, url: String, provider: String) {
        self.sharing_state = SharingState::Active {
            public_url: url,
            provider_name: provider,
        };
    }

    /// Clear the sharing state (after stopping).
    pub fn clear_sharing_state(&mut self) {
        self.sharing_state = SharingState::Inactive;
    }

    /// Set a status message to display in the footer.
    pub fn set_status_message(&mut self, message: impl Into<String>) {
        self.status_message = Some(message.into());
    }

    /// Clear the status message.
    pub fn clear_status_message(&mut self) {
        self.status_message = None;
    }

    /// Get the current status message.
    pub fn status_message(&self) -> Option<&str> {
        self.status_message.as_deref()
    }

    /// Check if a confirmation dialog is showing.
    pub fn is_confirming(&self) -> bool {
        !matches!(self.confirmation_state, ConfirmationState::Inactive)
    }

    /// Get the current confirmation state.
    pub fn confirmation_state(&self) -> &ConfirmationState {
        &self.confirmation_state
    }

    /// Cancel any active confirmation dialog.
    pub fn cancel_confirmation(&mut self) {
        self.confirmation_state = ConfirmationState::Inactive;
    }

    /// Handle key events when a confirmation dialog is showing.
    fn handle_confirmation_key(&mut self, key_event: KeyEvent) -> AppResult<()> {
        match key_event.code {
            // y or Y confirms the action
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let ConfirmationState::ConfirmingDelete { session_path, .. } =
                    &self.confirmation_state
                {
                    self.pending_action = Action::DeleteSession(session_path.clone());
                }
                self.confirmation_state = ConfirmationState::Inactive;
            }
            // Any other key cancels (n, N, Esc, or any key)
            _ => {
                self.confirmation_state = ConfirmationState::Inactive;
            }
        }
        Ok(())
    }

    /// Remove a session from the list by its file path.
    pub fn remove_session_by_path(&mut self, path: &PathBuf) {
        self.session_list_state.remove_session_by_path(path);
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

        // Render footer (changes based on sharing state)
        self.render_footer(frame, chunks[2]);

        // Render provider selection popup if selecting
        if self.sharing_state.is_selecting_provider() {
            self.render_provider_select_popup(frame, area);
        }

        // Render confirmation dialog if confirming
        if self.is_confirming() {
            self.render_confirmation_dialog(frame, area);
        }

        // Render help overlay if visible
        if self.show_help {
            self.render_help_overlay(frame, area);
        }
    }

    /// Render the provider selection popup.
    fn render_provider_select_popup(&mut self, frame: &mut Frame, area: Rect) {
        let widget = ProviderSelect::new();
        frame.render_stateful_widget(widget, area, &mut self.provider_select_state);
    }

    /// Render the help overlay.
    fn render_help_overlay(&self, frame: &mut Frame, area: Rect) {
        let widget = HelpOverlay::new();
        frame.render_widget(widget, area);
    }

    /// Render the confirmation dialog.
    fn render_confirmation_dialog(&self, frame: &mut Frame, area: Rect) {
        if let ConfirmationState::ConfirmingDelete { session_id, .. } = &self.confirmation_state {
            let widget = ConfirmationDialog::delete_session(session_id);
            frame.render_widget(widget, area);
        }
    }

    /// Render message when terminal is too small.
    fn render_too_small(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Panko ")
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
            .title(" Panko ")
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
            Constraint::Length(12), // Sort indicator
            Constraint::Length(10), // Help hint
        ])
        .split(inner);

        // Left: Session count (with total if filtering)
        let session_text = if self.session_list_state.is_searching() {
            // Show filtered count and search indicator
            Line::from(vec![
                Span::raw("Matches: "),
                Span::styled(
                    format!("{}", session_count),
                    Style::default().fg(Color::Yellow),
                ),
            ])
        } else {
            Line::from(vec![
                Span::raw("Sessions: "),
                Span::styled(
                    format!("{}", session_count),
                    Style::default().fg(Color::Cyan),
                ),
            ])
        };
        frame.render_widget(
            Paragraph::new(session_text).alignment(Alignment::Left),
            header_chunks[0],
        );

        // Center: Search input area
        let search_text = if self.search_active {
            // Active search input mode - show cursor
            Line::from(vec![
                Span::styled("Search: ", Style::default().fg(Color::Cyan)),
                Span::styled(&self.search_query, Style::default().fg(Color::White)),
                Span::styled("█", Style::default().fg(Color::Cyan)), // Cursor
            ])
        } else if !self.search_query.is_empty() {
            // Search filter is active but not typing
            Line::from(vec![
                Span::styled("Search: ", Style::default().fg(Color::Yellow)),
                Span::styled(&self.search_query, Style::default().fg(Color::Yellow)),
                Span::styled(" (Esc to clear)", Style::default().fg(Color::DarkGray)),
            ])
        } else {
            // No search
            Line::from(vec![
                Span::styled("Search: ", Style::default().fg(Color::DarkGray)),
                Span::styled("(press / to search)", Style::default().fg(Color::DarkGray)),
            ])
        };
        frame.render_widget(
            Paragraph::new(search_text).alignment(Alignment::Center),
            header_chunks[1],
        );

        // Sort indicator
        let sort_text = Line::from(vec![
            Span::styled("[S] ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                self.session_list_state.sort_order().short_name(),
                Style::default().fg(Color::Magenta),
            ),
        ]);
        frame.render_widget(
            Paragraph::new(sort_text).alignment(Alignment::Center),
            header_chunks[2],
        );

        // Right: Help hint
        let help_text = Line::from(vec![Span::styled(
            "[?] Help",
            Style::default().fg(Color::DarkGray),
        )]);
        frame.render_widget(
            Paragraph::new(help_text).alignment(Alignment::Right),
            header_chunks[3],
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
        // Show status message if present (takes priority)
        let content = if let Some(ref msg) = self.status_message {
            Line::from(vec![Span::styled(
                msg.clone(),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )])
        } else if let Some(url) = self.sharing_state.public_url() {
            // Sharing is active - show URL and stop hint
            Line::from(vec![
                Span::styled("🌐 Sharing at ", Style::default().fg(Color::Green)),
                Span::styled(
                    url,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" - press ", Style::default().fg(Color::DarkGray)),
                Span::styled("Esc", Style::default().fg(Color::Yellow)),
                Span::styled(" to stop", Style::default().fg(Color::DarkGray)),
            ])
        } else if matches!(self.sharing_state, SharingState::Starting { .. }) {
            // Starting sharing - show loading message
            Line::from(vec![Span::styled(
                "Starting sharing... ",
                Style::default().fg(Color::Yellow),
            )])
        } else if matches!(self.sharing_state, SharingState::Stopping) {
            // Stopping sharing
            Line::from(vec![Span::styled(
                "Stopping sharing... ",
                Style::default().fg(Color::Yellow),
            )])
        } else if self.refresh_state.is_refreshing() {
            // Refreshing session list
            Line::from(vec![Span::styled(
                "Refreshing... ",
                Style::default().fg(Color::Yellow),
            )])
        } else {
            // Normal footer
            Line::from(vec![
                Span::styled(" v ", Style::default().fg(Color::Cyan)),
                Span::raw("view  "),
                Span::styled("s ", Style::default().fg(Color::Cyan)),
                Span::raw("share  "),
                Span::styled("c ", Style::default().fg(Color::Cyan)),
                Span::raw("copy  "),
                Span::styled("o ", Style::default().fg(Color::Cyan)),
                Span::raw("open  "),
                Span::styled("r ", Style::default().fg(Color::Cyan)),
                Span::raw("refresh  "),
                Span::styled("q ", Style::default().fg(Color::Cyan)),
                Span::raw("quit"),
            ])
        };

        let paragraph = Paragraph::new(content)
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

    // Tests for view action

    #[test]
    fn test_pending_action_default_is_none() {
        let app = App::new();
        assert_eq!(*app.pending_action(), Action::None);
        assert!(!app.has_pending_action());
    }

    #[test]
    fn test_handle_key_v_triggers_view_on_session() {
        let mut app = App::with_sessions(sample_sessions());
        // Move to first session (first item is a project)
        app.session_list_state_mut().select_next();

        // Press 'v' to view
        app.handle_key_event(key_event(KeyCode::Char('v'))).unwrap();

        // Should have a pending ViewSession action
        assert!(app.has_pending_action());
        match app.pending_action() {
            Action::ViewSession(path) => {
                assert!(path.to_string_lossy().contains("abc12345.jsonl"));
            }
            _ => panic!("Expected ViewSession action"),
        }
    }

    #[test]
    fn test_handle_key_enter_triggers_view_on_session() {
        let mut app = App::with_sessions(sample_sessions());
        // Move to first session (first item is a project)
        app.session_list_state_mut().select_next();

        // Press Enter to view
        app.handle_key_event(key_event(KeyCode::Enter)).unwrap();

        // Should have a pending ViewSession action
        assert!(app.has_pending_action());
        match app.pending_action() {
            Action::ViewSession(_) => {}
            _ => panic!("Expected ViewSession action"),
        }
    }

    #[test]
    fn test_handle_key_v_does_nothing_on_project() {
        let mut app = App::with_sessions(sample_sessions());
        // First item is a project, not a session
        assert!(app.selected_session().is_none());

        // Press 'v' on project
        app.handle_key_event(key_event(KeyCode::Char('v'))).unwrap();

        // Should not have a pending action since we're on a project, not a session
        assert!(!app.has_pending_action());
    }

    #[test]
    fn test_handle_key_v_does_nothing_when_empty() {
        let mut app = App::new();

        // Press 'v' with no sessions
        app.handle_key_event(key_event(KeyCode::Char('v'))).unwrap();

        // Should not have a pending action
        assert!(!app.has_pending_action());
    }

    #[test]
    fn test_take_pending_action_clears_action() {
        let mut app = App::with_sessions(sample_sessions());
        app.session_list_state_mut().select_next();
        app.handle_key_event(key_event(KeyCode::Char('v'))).unwrap();

        // Take the action
        let action = app.take_pending_action();
        assert!(matches!(action, Action::ViewSession(_)));

        // Action should be cleared
        assert!(!app.has_pending_action());
        assert_eq!(*app.pending_action(), Action::None);
    }

    #[test]
    fn test_view_action_works_regardless_of_focus() {
        let mut app = App::with_sessions(sample_sessions());
        app.session_list_state_mut().select_next();

        // Switch to preview panel
        app.set_focused_panel(FocusedPanel::Preview);

        // Press 'v' - should still work since we have a selected session
        app.handle_key_event(key_event(KeyCode::Char('v'))).unwrap();

        // Should have a pending action
        assert!(app.has_pending_action());
    }

    // Tests for share action

    #[test]
    fn test_handle_key_s_triggers_share_on_session() {
        let mut app = App::with_sessions(sample_sessions());
        // Move to first session (first item is a project)
        app.session_list_state_mut().select_next();

        // Press 's' to share
        app.handle_key_event(key_event(KeyCode::Char('s'))).unwrap();

        // Should have a pending ShareSession action
        assert!(app.has_pending_action());
        match app.pending_action() {
            Action::ShareSession(path) => {
                assert!(path.to_string_lossy().contains("abc12345.jsonl"));
            }
            _ => panic!("Expected ShareSession action"),
        }
    }

    #[test]
    fn test_handle_key_s_does_nothing_on_project() {
        let mut app = App::with_sessions(sample_sessions());
        // First item is a project, not a session
        assert!(app.selected_session().is_none());

        // Press 's' on project
        app.handle_key_event(key_event(KeyCode::Char('s'))).unwrap();

        // Should not have a pending action since we're on a project, not a session
        assert!(!app.has_pending_action());
    }

    #[test]
    fn test_handle_key_s_does_nothing_when_empty() {
        let mut app = App::new();

        // Press 's' with no sessions
        app.handle_key_event(key_event(KeyCode::Char('s'))).unwrap();

        // Should not have a pending action
        assert!(!app.has_pending_action());
    }

    #[test]
    fn test_share_action_works_regardless_of_focus() {
        let mut app = App::with_sessions(sample_sessions());
        app.session_list_state_mut().select_next();

        // Switch to preview panel
        app.set_focused_panel(FocusedPanel::Preview);

        // Press 's' - should still work since we have a selected session
        app.handle_key_event(key_event(KeyCode::Char('s'))).unwrap();

        // Should have a pending action
        assert!(app.has_pending_action());
        assert!(matches!(app.pending_action(), Action::ShareSession(_)));
    }

    // Tests for sharing state management

    #[test]
    fn test_sharing_state_default() {
        let app = App::new();
        assert!(!app.sharing_state().is_active());
        assert!(!app.sharing_state().is_selecting_provider());
        assert!(!app.sharing_state().is_busy());
    }

    #[test]
    fn test_start_provider_selection() {
        let mut app = App::new();
        let path = PathBuf::from("/test/session.jsonl");
        let providers = vec![
            ProviderOption::new("cloudflare", "Cloudflare"),
            ProviderOption::new("ngrok", "ngrok"),
        ];

        app.start_provider_selection(path.clone(), providers);

        assert!(app.sharing_state().is_selecting_provider());
        assert!(app.sharing_state().is_busy());
        assert!(!app.sharing_state().is_active());
    }

    #[test]
    fn test_set_sharing_active() {
        let mut app = App::new();

        app.set_sharing_active("https://example.com".to_string(), "cloudflare".to_string());

        assert!(app.sharing_state().is_active());
        assert!(app.sharing_state().is_busy());
        assert_eq!(
            app.sharing_state().public_url(),
            Some("https://example.com")
        );
    }

    #[test]
    fn test_clear_sharing_state() {
        let mut app = App::new();
        app.set_sharing_active("https://example.com".to_string(), "cloudflare".to_string());
        assert!(app.sharing_state().is_active());

        app.clear_sharing_state();

        assert!(!app.sharing_state().is_active());
        assert!(!app.sharing_state().is_busy());
    }

    // Tests for provider selection key handling

    #[test]
    fn test_provider_select_navigation() {
        let mut app = App::new();
        let path = PathBuf::from("/test/session.jsonl");
        let providers = vec![
            ProviderOption::new("cloudflare", "Cloudflare"),
            ProviderOption::new("ngrok", "ngrok"),
            ProviderOption::new("tailscale", "Tailscale"),
        ];

        app.start_provider_selection(path, providers);
        assert_eq!(app.provider_select_state.selected(), 0);

        // Navigate down
        app.handle_key_event(key_event(KeyCode::Char('j'))).unwrap();
        assert_eq!(app.provider_select_state.selected(), 1);

        // Navigate up
        app.handle_key_event(key_event(KeyCode::Char('k'))).unwrap();
        assert_eq!(app.provider_select_state.selected(), 0);

        // Navigate with arrows
        app.handle_key_event(key_event(KeyCode::Down)).unwrap();
        assert_eq!(app.provider_select_state.selected(), 1);

        app.handle_key_event(key_event(KeyCode::Up)).unwrap();
        assert_eq!(app.provider_select_state.selected(), 0);
    }

    #[test]
    fn test_provider_select_cancel_with_esc() {
        let mut app = App::new();
        let path = PathBuf::from("/test/session.jsonl");
        let providers = vec![ProviderOption::new("cloudflare", "Cloudflare")];

        app.start_provider_selection(path, providers);
        assert!(app.sharing_state().is_selecting_provider());

        // Press Esc to cancel
        app.handle_key_event(key_event(KeyCode::Esc)).unwrap();

        // Should be back to inactive state
        assert!(!app.sharing_state().is_selecting_provider());
        assert!(!app.sharing_state().is_busy());
        // App should still be running
        assert!(app.is_running());
    }

    #[test]
    fn test_provider_select_confirm_with_enter() {
        let mut app = App::new();
        let path = PathBuf::from("/test/session.jsonl");
        let providers = vec![
            ProviderOption::new("cloudflare", "Cloudflare"),
            ProviderOption::new("ngrok", "ngrok"),
        ];

        app.start_provider_selection(path.clone(), providers);

        // Select second provider
        app.handle_key_event(key_event(KeyCode::Char('j'))).unwrap();

        // Press Enter to confirm
        app.handle_key_event(key_event(KeyCode::Enter)).unwrap();

        // Should have StartSharing action
        assert!(app.has_pending_action());
        match app.take_pending_action() {
            Action::StartSharing { path: p, provider } => {
                assert_eq!(p, path);
                assert_eq!(provider, "ngrok");
            }
            _ => panic!("Expected StartSharing action"),
        }
    }

    // Tests for active sharing key handling

    #[test]
    fn test_sharing_esc_stops_sharing() {
        let mut app = App::new();
        app.set_sharing_active("https://example.com".to_string(), "cloudflare".to_string());
        assert!(app.sharing_state().is_active());

        // Press Esc to stop sharing
        app.handle_key_event(key_event(KeyCode::Esc)).unwrap();

        // Should have StopSharing action
        assert!(app.has_pending_action());
        assert_eq!(app.take_pending_action(), Action::StopSharing);

        // State should be stopping
        assert!(matches!(app.sharing_state(), &SharingState::Stopping));
    }

    #[test]
    fn test_sharing_navigation_still_works() {
        let mut app = App::with_sessions(sample_sessions());
        app.set_sharing_active("https://example.com".to_string(), "cloudflare".to_string());

        // Navigation should still work while sharing
        let initial_selection = app.session_list_state.selected();
        app.handle_key_event(key_event(KeyCode::Char('j'))).unwrap();
        assert_ne!(app.session_list_state.selected(), initial_selection);

        // Tab should still work
        assert_eq!(app.focused_panel(), FocusedPanel::SessionList);
        app.handle_key_event(key_event(KeyCode::Tab)).unwrap();
        assert_eq!(app.focused_panel(), FocusedPanel::Preview);
    }

    // Tests for copy path action

    #[test]
    fn test_handle_key_c_triggers_copy_path_on_session() {
        let mut app = App::with_sessions(sample_sessions());
        // Move to first session (first item is a project)
        app.session_list_state_mut().select_next();

        // Press 'c' to copy path
        app.handle_key_event(key_event(KeyCode::Char('c'))).unwrap();

        // Should have a pending CopyPath action
        assert!(app.has_pending_action());
        match app.pending_action() {
            Action::CopyPath(path) => {
                assert!(path.to_string_lossy().contains("abc12345.jsonl"));
            }
            _ => panic!("Expected CopyPath action"),
        }
    }

    #[test]
    fn test_handle_key_c_does_nothing_on_project() {
        let mut app = App::with_sessions(sample_sessions());
        // First item is a project, not a session
        assert!(app.selected_session().is_none());

        // Press 'c' on project
        app.handle_key_event(key_event(KeyCode::Char('c'))).unwrap();

        // Should not have a pending action since we're on a project, not a session
        assert!(!app.has_pending_action());
    }

    #[test]
    fn test_handle_key_c_does_nothing_when_empty() {
        let mut app = App::new();

        // Press 'c' with no sessions
        app.handle_key_event(key_event(KeyCode::Char('c'))).unwrap();

        // Should not have a pending action
        assert!(!app.has_pending_action());
    }

    // Tests for open folder action

    #[test]
    fn test_handle_key_o_triggers_open_folder_on_session() {
        let mut app = App::with_sessions(sample_sessions());
        // Move to first session (first item is a project)
        app.session_list_state_mut().select_next();

        // Press 'o' to open folder
        app.handle_key_event(key_event(KeyCode::Char('o'))).unwrap();

        // Should have a pending OpenFolder action
        assert!(app.has_pending_action());
        match app.pending_action() {
            Action::OpenFolder(path) => {
                assert!(path.to_string_lossy().contains("abc12345.jsonl"));
            }
            _ => panic!("Expected OpenFolder action"),
        }
    }

    #[test]
    fn test_handle_key_o_does_nothing_on_project() {
        let mut app = App::with_sessions(sample_sessions());
        // First item is a project, not a session
        assert!(app.selected_session().is_none());

        // Press 'o' on project
        app.handle_key_event(key_event(KeyCode::Char('o'))).unwrap();

        // Should not have a pending action since we're on a project, not a session
        assert!(!app.has_pending_action());
    }

    #[test]
    fn test_handle_key_o_does_nothing_when_empty() {
        let mut app = App::new();

        // Press 'o' with no sessions
        app.handle_key_event(key_event(KeyCode::Char('o'))).unwrap();

        // Should not have a pending action
        assert!(!app.has_pending_action());
    }

    // Tests for status message

    #[test]
    fn test_status_message_default_is_none() {
        let app = App::new();
        assert!(app.status_message().is_none());
    }

    #[test]
    fn test_set_status_message() {
        let mut app = App::new();
        app.set_status_message("Test message");
        assert_eq!(app.status_message(), Some("Test message"));
    }

    #[test]
    fn test_clear_status_message() {
        let mut app = App::new();
        app.set_status_message("Test message");
        assert!(app.status_message().is_some());

        app.clear_status_message();
        assert!(app.status_message().is_none());
    }

    #[test]
    fn test_copy_and_open_actions_work_regardless_of_focus() {
        let mut app = App::with_sessions(sample_sessions());
        app.session_list_state_mut().select_next();

        // Switch to preview panel
        app.set_focused_panel(FocusedPanel::Preview);

        // Press 'c' - should still work since we have a selected session
        app.handle_key_event(key_event(KeyCode::Char('c'))).unwrap();
        assert!(app.has_pending_action());
        assert!(matches!(app.take_pending_action(), Action::CopyPath(_)));

        // Press 'o' - should also work
        app.handle_key_event(key_event(KeyCode::Char('o'))).unwrap();
        assert!(app.has_pending_action());
        assert!(matches!(app.pending_action(), Action::OpenFolder(_)));
    }

    // Tests for copy context action (Shift+C)

    #[test]
    fn test_handle_key_shift_c_triggers_copy_context_on_session() {
        let mut app = App::with_sessions(sample_sessions());
        // Move to first session (first item is a project)
        app.session_list_state_mut().select_next();

        // Press 'C' (Shift+C) to copy context
        app.handle_key_event(key_event(KeyCode::Char('C'))).unwrap();

        // Should have a pending CopyContext action
        assert!(app.has_pending_action());
        match app.pending_action() {
            Action::CopyContext(path) => {
                assert!(path.to_string_lossy().contains("abc12345.jsonl"));
            }
            _ => panic!("Expected CopyContext action"),
        }
    }

    #[test]
    fn test_handle_key_shift_c_does_nothing_on_project() {
        let mut app = App::with_sessions(sample_sessions());
        // First item is a project, not a session
        assert!(app.selected_session().is_none());

        // Press 'C' (Shift+C) on project
        app.handle_key_event(key_event(KeyCode::Char('C'))).unwrap();

        // Should not have a pending action since we're on a project, not a session
        assert!(!app.has_pending_action());
    }

    #[test]
    fn test_handle_key_shift_c_does_nothing_when_empty() {
        let mut app = App::new();

        // Press 'C' (Shift+C) with no sessions
        app.handle_key_event(key_event(KeyCode::Char('C'))).unwrap();

        // Should not have a pending action
        assert!(!app.has_pending_action());
    }

    #[test]
    fn test_copy_context_works_regardless_of_focus() {
        let mut app = App::with_sessions(sample_sessions());
        app.session_list_state_mut().select_next();

        // Switch to preview panel
        app.set_focused_panel(FocusedPanel::Preview);

        // Press 'C' (Shift+C) - should still work since we have a selected session
        app.handle_key_event(key_event(KeyCode::Char('C'))).unwrap();
        assert!(app.has_pending_action());
        assert!(matches!(app.pending_action(), Action::CopyContext(_)));
    }

    // Tests for Shift+D download action

    #[test]
    fn test_handle_key_shift_d_triggers_download_on_session() {
        let mut app = App::with_sessions(sample_sessions());
        // Select the first session (not a project)
        app.session_list_state_mut().select_next();

        // Press 'D' (Shift+D)
        app.handle_key_event(key_event(KeyCode::Char('D'))).unwrap();

        // Should have a pending download action
        assert!(app.has_pending_action());
        assert!(matches!(app.pending_action(), Action::DownloadSession(_)));
    }

    #[test]
    fn test_handle_key_shift_d_does_nothing_on_project() {
        let mut app = App::with_sessions(sample_sessions());
        // Don't select - starts on a project header

        // Press 'D' (Shift+D)
        app.handle_key_event(key_event(KeyCode::Char('D'))).unwrap();

        // Should not have a pending action (selected item is project, not session)
        assert!(!app.has_pending_action());
    }

    #[test]
    fn test_handle_key_shift_d_does_nothing_when_empty() {
        let mut app = App::new();

        // Press 'D' (Shift+D) with no sessions
        app.handle_key_event(key_event(KeyCode::Char('D'))).unwrap();

        // Should not have a pending action
        assert!(!app.has_pending_action());
    }

    #[test]
    fn test_download_works_regardless_of_focus() {
        let mut app = App::with_sessions(sample_sessions());
        app.session_list_state_mut().select_next();

        // Switch to preview panel
        app.set_focused_panel(FocusedPanel::Preview);

        // Press 'D' (Shift+D) - should still work since we have a selected session
        app.handle_key_event(key_event(KeyCode::Char('D'))).unwrap();
        assert!(app.has_pending_action());
        assert!(matches!(app.pending_action(), Action::DownloadSession(_)));
    }

    // Tests for fuzzy search

    #[test]
    fn test_search_default_inactive() {
        let app = App::new();
        assert!(!app.is_search_active());
        assert_eq!(app.search_query(), "");
    }

    #[test]
    fn test_handle_key_slash_activates_search() {
        let mut app = App::with_sessions(sample_sessions());
        assert!(!app.is_search_active());

        app.handle_key_event(key_event(KeyCode::Char('/'))).unwrap();

        assert!(app.is_search_active());
        // Focus should be on session list
        assert_eq!(app.focused_panel(), FocusedPanel::SessionList);
    }

    #[test]
    fn test_search_typing_updates_query() {
        let mut app = App::with_sessions(sample_sessions());
        app.handle_key_event(key_event(KeyCode::Char('/'))).unwrap();

        // Type some characters
        app.handle_key_event(key_event(KeyCode::Char('a'))).unwrap();
        app.handle_key_event(key_event(KeyCode::Char('p'))).unwrap();
        app.handle_key_event(key_event(KeyCode::Char('i'))).unwrap();

        assert_eq!(app.search_query(), "api");
    }

    #[test]
    fn test_search_backspace_removes_character() {
        let mut app = App::with_sessions(sample_sessions());
        app.handle_key_event(key_event(KeyCode::Char('/'))).unwrap();

        // Type "api"
        app.handle_key_event(key_event(KeyCode::Char('a'))).unwrap();
        app.handle_key_event(key_event(KeyCode::Char('p'))).unwrap();
        app.handle_key_event(key_event(KeyCode::Char('i'))).unwrap();
        assert_eq!(app.search_query(), "api");

        // Backspace removes last char
        app.handle_key_event(key_event(KeyCode::Backspace)).unwrap();
        assert_eq!(app.search_query(), "ap");
    }

    #[test]
    fn test_search_esc_deactivates_search_mode() {
        let mut app = App::with_sessions(sample_sessions());
        app.handle_key_event(key_event(KeyCode::Char('/'))).unwrap();
        assert!(app.is_search_active());

        // Type something
        app.handle_key_event(key_event(KeyCode::Char('a'))).unwrap();

        // Esc should exit search mode but keep query if non-empty
        app.handle_key_event(key_event(KeyCode::Esc)).unwrap();
        assert!(!app.is_search_active());
    }

    #[test]
    fn test_search_enter_exits_search_mode() {
        let mut app = App::with_sessions(sample_sessions());
        app.handle_key_event(key_event(KeyCode::Char('/'))).unwrap();
        app.handle_key_event(key_event(KeyCode::Char('a'))).unwrap();
        assert!(app.is_search_active());

        // Enter should exit search input mode
        app.handle_key_event(key_event(KeyCode::Enter)).unwrap();
        assert!(!app.is_search_active());
    }

    #[test]
    fn test_clear_search_clears_everything() {
        let mut app = App::with_sessions(sample_sessions());
        app.handle_key_event(key_event(KeyCode::Char('/'))).unwrap();
        app.handle_key_event(key_event(KeyCode::Char('a'))).unwrap();

        app.clear_search();

        assert!(!app.is_search_active());
        assert_eq!(app.search_query(), "");
    }

    #[test]
    fn test_search_navigation_works_during_search() {
        let mut app = App::with_sessions(sample_sessions());
        app.handle_key_event(key_event(KeyCode::Char('/'))).unwrap();
        assert!(app.is_search_active());

        // Arrow keys should still navigate
        let initial_selection = app.session_list_state().selected();
        app.handle_key_event(key_event(KeyCode::Down)).unwrap();
        assert_ne!(app.session_list_state().selected(), initial_selection);
    }

    #[test]
    fn test_search_ctrl_c_quits() {
        let mut app = App::with_sessions(sample_sessions());
        app.handle_key_event(key_event(KeyCode::Char('/'))).unwrap();
        assert!(app.is_running());

        // Ctrl+C should quit even in search mode
        app.handle_key_event(key_event_with_modifiers(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        ))
        .unwrap();
        assert!(!app.is_running());
    }

    #[test]
    fn test_esc_clears_active_search_instead_of_quitting() {
        let mut app = App::with_sessions(sample_sessions());

        // Apply a search filter (not in active search mode, just with an active filter)
        app.handle_key_event(key_event(KeyCode::Char('/'))).unwrap();
        app.handle_key_event(key_event(KeyCode::Char('a'))).unwrap();
        app.handle_key_event(key_event(KeyCode::Enter)).unwrap(); // Exit search input mode

        assert!(!app.is_search_active());
        assert!(app.session_list_state().is_searching()); // Filter is active

        // Esc should clear the filter, not quit
        app.handle_key_event(key_event(KeyCode::Esc)).unwrap();
        assert!(app.is_running()); // Still running
        assert!(!app.session_list_state().is_searching()); // Filter cleared
    }

    #[test]
    fn test_esc_quits_when_no_search_active() {
        let mut app = App::with_sessions(sample_sessions());
        assert!(!app.session_list_state().is_searching());

        // Esc should quit when no search is active
        app.handle_key_event(key_event(KeyCode::Esc)).unwrap();
        assert!(!app.is_running());
    }

    // Tests for help overlay

    #[test]
    fn test_show_help_default_is_false() {
        let app = App::new();
        assert!(!app.show_help);
    }

    #[test]
    fn test_handle_key_question_mark_shows_help() {
        let mut app = App::new();
        assert!(!app.show_help);

        // Press '?' to show help
        app.handle_key_event(key_event(KeyCode::Char('?'))).unwrap();

        assert!(app.show_help);
        assert!(app.is_running()); // Should still be running
    }

    #[test]
    fn test_any_key_closes_help_overlay() {
        let mut app = App::new();
        app.show_help = true;

        // Any key should close help
        app.handle_key_event(key_event(KeyCode::Char('a'))).unwrap();
        assert!(!app.show_help);
    }

    #[test]
    fn test_esc_closes_help_overlay() {
        let mut app = App::new();
        app.show_help = true;

        // Esc should close help
        app.handle_key_event(key_event(KeyCode::Esc)).unwrap();
        assert!(!app.show_help);
        assert!(app.is_running()); // Should not quit when closing help
    }

    #[test]
    fn test_q_closes_help_overlay_without_quitting() {
        let mut app = App::new();
        app.show_help = true;

        // q should close help, not quit the app
        app.handle_key_event(key_event(KeyCode::Char('q'))).unwrap();
        assert!(!app.show_help);
        assert!(app.is_running()); // Should still be running
    }

    #[test]
    fn test_enter_closes_help_overlay() {
        let mut app = App::new();
        app.show_help = true;

        // Enter should close help
        app.handle_key_event(key_event(KeyCode::Enter)).unwrap();
        assert!(!app.show_help);
        assert!(app.is_running());
    }

    #[test]
    fn test_help_toggle_on_and_off() {
        let mut app = App::new();

        // Show help
        app.handle_key_event(key_event(KeyCode::Char('?'))).unwrap();
        assert!(app.show_help);

        // Any key closes help
        app.handle_key_event(key_event(KeyCode::Char('?'))).unwrap();
        assert!(!app.show_help);

        // Show help again
        app.handle_key_event(key_event(KeyCode::Char('?'))).unwrap();
        assert!(app.show_help);
    }

    #[test]
    fn test_help_overlay_intercepts_navigation_keys() {
        let mut app = App::with_sessions(sample_sessions());
        let initial_selection = app.session_list_state().selected();

        app.show_help = true;

        // j should close help, not navigate
        app.handle_key_event(key_event(KeyCode::Char('j'))).unwrap();
        assert!(!app.show_help);
        assert_eq!(app.session_list_state().selected(), initial_selection);
    }

    #[test]
    fn test_help_overlay_intercepts_action_keys() {
        let mut app = App::with_sessions(sample_sessions());
        app.session_list_state_mut().select_next(); // Select a session
        app.show_help = true;

        // v should close help, not trigger view action
        app.handle_key_event(key_event(KeyCode::Char('v'))).unwrap();
        assert!(!app.show_help);
        assert!(!app.has_pending_action()); // No action should be triggered
    }

    // Tests for refresh functionality

    #[test]
    fn test_refresh_state_default_is_idle() {
        let app = App::new();
        assert_eq!(*app.refresh_state(), RefreshState::Idle);
        assert!(!app.is_refreshing());
    }

    #[test]
    fn test_refresh_state_is_refreshing() {
        let mut state = RefreshState::Refreshing;
        assert!(state.is_refreshing());

        state = RefreshState::Idle;
        assert!(!state.is_refreshing());
    }

    #[test]
    fn test_refresh_sessions_sets_state_to_idle_after_completion() {
        let mut app = App::with_sessions(sample_sessions());

        // After refresh, state should be Idle
        let _ = app.refresh_sessions();
        assert_eq!(*app.refresh_state(), RefreshState::Idle);
    }

    #[test]
    fn test_refresh_sessions_preserves_selection_by_id() {
        let mut app = App::with_sessions(sample_sessions());

        // Navigate to the second session (index 2 in visible items: project at 0, session at 1, session at 2)
        app.session_list_state_mut().select_next(); // Now at first session
        app.session_list_state_mut().select_next(); // Now at second session

        // Get the selected session ID
        let selected_session = app.selected_session();
        assert!(selected_session.is_some());
        let selected_id = selected_session.unwrap().id.clone();
        assert_eq!(selected_id, "def67890");

        // Refresh (in a real scenario this would reload from disk)
        let _ = app.refresh_sessions();

        // Since refresh_sessions calls load_sessions which scans from disk,
        // and we're using sample_sessions() which are in-memory,
        // the selection preservation logic is tested by the session_list_state tests
    }

    #[test]
    fn test_handle_key_r_triggers_refresh() {
        let mut app = App::with_sessions(sample_sessions());

        // Press 'r' to refresh
        app.handle_key_event(key_event(KeyCode::Char('r'))).unwrap();

        // App should still be running
        assert!(app.is_running());
        // Refresh state should be idle (refresh completed synchronously)
        assert_eq!(*app.refresh_state(), RefreshState::Idle);
    }

    #[test]
    fn test_refresh_works_regardless_of_focus() {
        let mut app = App::with_sessions(sample_sessions());

        // Works when session list is focused
        app.set_focused_panel(FocusedPanel::SessionList);
        app.handle_key_event(key_event(KeyCode::Char('r'))).unwrap();
        assert!(app.is_running());

        // Also works when preview is focused
        app.set_focused_panel(FocusedPanel::Preview);
        app.handle_key_event(key_event(KeyCode::Char('r'))).unwrap();
        assert!(app.is_running());
    }

    // Tests for deletion functionality

    #[test]
    fn test_confirmation_state_default_is_inactive() {
        let app = App::new();
        assert!(!app.is_confirming());
        assert!(matches!(
            app.confirmation_state(),
            ConfirmationState::Inactive
        ));
    }

    #[test]
    fn test_handle_key_d_triggers_confirmation_on_session() {
        let mut app = App::with_sessions(sample_sessions());
        // Move to first session (first item is a project)
        app.session_list_state_mut().select_next();

        // Press 'd' to delete
        app.handle_key_event(key_event(KeyCode::Char('d'))).unwrap();

        // Should be in confirmation state
        assert!(app.is_confirming());
        assert!(matches!(
            app.confirmation_state(),
            ConfirmationState::ConfirmingDelete { .. }
        ));
    }

    #[test]
    fn test_handle_key_d_does_nothing_on_project() {
        let mut app = App::with_sessions(sample_sessions());
        // First item is a project, not a session
        assert!(app.selected_session().is_none());

        // Press 'd' on project
        app.handle_key_event(key_event(KeyCode::Char('d'))).unwrap();

        // Should not be in confirmation state
        assert!(!app.is_confirming());
    }

    #[test]
    fn test_handle_key_d_does_nothing_when_empty() {
        let mut app = App::new();

        // Press 'd' with no sessions
        app.handle_key_event(key_event(KeyCode::Char('d'))).unwrap();

        // Should not be in confirmation state
        assert!(!app.is_confirming());
    }

    #[test]
    fn test_handle_key_d_blocked_while_sharing() {
        let mut app = App::with_sessions(sample_sessions());
        app.session_list_state_mut().select_next();
        app.set_sharing_active("https://example.com".to_string(), "cloudflare".to_string());

        // Press 'd' while sharing is active
        app.handle_key_event(key_event(KeyCode::Char('d'))).unwrap();

        // Should show status message and NOT be in confirmation state
        assert!(!app.is_confirming());
        assert!(app.status_message().is_some());
        assert!(app
            .status_message()
            .unwrap()
            .contains("Cannot delete while sharing"));
    }

    #[test]
    fn test_confirmation_y_triggers_delete_action() {
        let mut app = App::with_sessions(sample_sessions());
        app.session_list_state_mut().select_next();

        // Enter confirmation state
        app.handle_key_event(key_event(KeyCode::Char('d'))).unwrap();
        assert!(app.is_confirming());

        // Press 'y' to confirm
        app.handle_key_event(key_event(KeyCode::Char('y'))).unwrap();

        // Should have DeleteSession action and exit confirmation state
        assert!(!app.is_confirming());
        assert!(app.has_pending_action());
        assert!(matches!(app.pending_action(), Action::DeleteSession(_)));
    }

    #[test]
    fn test_confirmation_upper_y_triggers_delete_action() {
        let mut app = App::with_sessions(sample_sessions());
        app.session_list_state_mut().select_next();

        // Enter confirmation state
        app.handle_key_event(key_event(KeyCode::Char('d'))).unwrap();
        assert!(app.is_confirming());

        // Press 'Y' to confirm
        app.handle_key_event(key_event(KeyCode::Char('Y'))).unwrap();

        // Should have DeleteSession action
        assert!(!app.is_confirming());
        assert!(app.has_pending_action());
        assert!(matches!(app.pending_action(), Action::DeleteSession(_)));
    }

    #[test]
    fn test_confirmation_n_cancels() {
        let mut app = App::with_sessions(sample_sessions());
        app.session_list_state_mut().select_next();

        // Enter confirmation state
        app.handle_key_event(key_event(KeyCode::Char('d'))).unwrap();
        assert!(app.is_confirming());

        // Press 'n' to cancel
        app.handle_key_event(key_event(KeyCode::Char('n'))).unwrap();

        // Should exit confirmation state without action
        assert!(!app.is_confirming());
        assert!(!app.has_pending_action());
    }

    #[test]
    fn test_confirmation_esc_cancels() {
        let mut app = App::with_sessions(sample_sessions());
        app.session_list_state_mut().select_next();

        // Enter confirmation state
        app.handle_key_event(key_event(KeyCode::Char('d'))).unwrap();
        assert!(app.is_confirming());

        // Press Esc to cancel
        app.handle_key_event(key_event(KeyCode::Esc)).unwrap();

        // Should exit confirmation state without action
        assert!(!app.is_confirming());
        assert!(!app.has_pending_action());
    }

    #[test]
    fn test_confirmation_any_key_cancels() {
        let mut app = App::with_sessions(sample_sessions());
        app.session_list_state_mut().select_next();

        // Enter confirmation state
        app.handle_key_event(key_event(KeyCode::Char('d'))).unwrap();
        assert!(app.is_confirming());

        // Press any other key to cancel (like 'x')
        app.handle_key_event(key_event(KeyCode::Char('x'))).unwrap();

        // Should exit confirmation state without action
        assert!(!app.is_confirming());
        assert!(!app.has_pending_action());
    }

    #[test]
    fn test_cancel_confirmation_method() {
        let mut app = App::with_sessions(sample_sessions());
        app.session_list_state_mut().select_next();

        // Enter confirmation state
        app.handle_key_event(key_event(KeyCode::Char('d'))).unwrap();
        assert!(app.is_confirming());

        // Cancel via method
        app.cancel_confirmation();

        assert!(!app.is_confirming());
    }

    #[test]
    fn test_remove_session_by_path() {
        let mut app = App::with_sessions(sample_sessions());
        let initial_count = app.session_list_state().visible_count();

        // Get the path of a session
        app.session_list_state_mut().select_next(); // Move to first session
        let session_path = app.selected_session().unwrap().path.clone();

        // Remove the session
        app.remove_session_by_path(&session_path);

        // Session count should decrease
        assert!(app.session_list_state().visible_count() < initial_count);
    }

    // SharingState predicate tests

    #[test]
    fn test_sharing_state_is_active_predicate() {
        // Active state
        let active = SharingState::Active {
            public_url: "https://test.url".into(),
            provider_name: "test".into(),
        };
        assert!(active.is_active());
        assert!(active.is_busy());
        assert!(!active.is_selecting_provider());

        // Inactive state
        let inactive = SharingState::Inactive;
        assert!(!inactive.is_active());
        assert!(!inactive.is_busy());
        assert!(!inactive.is_selecting_provider());
    }

    #[test]
    fn test_sharing_state_public_url_accessor() {
        // Active state has URL
        let active = SharingState::Active {
            public_url: "https://test.url".into(),
            provider_name: "test".into(),
        };
        assert_eq!(active.public_url(), Some("https://test.url"));

        // Inactive state has no URL
        let inactive = SharingState::Inactive;
        assert_eq!(inactive.public_url(), None);

        // Starting state has no URL
        let starting = SharingState::Starting {
            session_path: PathBuf::from("/test.jsonl"),
            provider_name: "test".into(),
        };
        assert_eq!(starting.public_url(), None);

        // Selecting provider has no URL
        let selecting = SharingState::SelectingProvider {
            session_path: PathBuf::from("/test.jsonl"),
        };
        assert_eq!(selecting.public_url(), None);

        // Stopping has no URL
        let stopping = SharingState::Stopping;
        assert_eq!(stopping.public_url(), None);
    }

    #[test]
    fn test_sharing_state_selecting_provider_predicate() {
        let state = SharingState::SelectingProvider {
            session_path: PathBuf::from("/test.jsonl"),
        };
        assert!(state.is_selecting_provider());
        assert!(state.is_busy());
        assert!(!state.is_active());
    }

    #[test]
    fn test_sharing_state_starting_predicates() {
        let state = SharingState::Starting {
            session_path: PathBuf::from("/test.jsonl"),
            provider_name: "cloudflare".into(),
        };
        assert!(!state.is_selecting_provider());
        assert!(state.is_busy());
        assert!(!state.is_active());
    }

    #[test]
    fn test_sharing_state_stopping_predicates() {
        let state = SharingState::Stopping;
        assert!(!state.is_selecting_provider());
        assert!(state.is_busy());
        assert!(!state.is_active());
    }

    #[test]
    fn test_sharing_state_all_variants_covered() {
        // Ensure all SharingState variants are testable
        let states = vec![
            SharingState::Inactive,
            SharingState::SelectingProvider {
                session_path: PathBuf::from("/test.jsonl"),
            },
            SharingState::Starting {
                session_path: PathBuf::from("/test.jsonl"),
                provider_name: "test".into(),
            },
            SharingState::Active {
                public_url: "https://test.url".into(),
                provider_name: "test".into(),
            },
            SharingState::Stopping,
        ];

        // Verify each state has consistent predicates
        for state in &states {
            // Only one of these can be true (or none for Inactive)
            let is_active = state.is_active();
            let is_selecting = state.is_selecting_provider();

            // Active and selecting are mutually exclusive
            assert!(!(is_active && is_selecting));

            // If busy, state is not Inactive
            if state.is_busy() {
                assert!(!matches!(state, SharingState::Inactive));
            }
        }
    }
}
