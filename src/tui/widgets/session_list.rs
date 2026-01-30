//! Session list widget with project grouping.
//!
//! This widget displays sessions in a tree view, grouped by project path.
//! Projects can be expanded/collapsed, and sessions show truncated info.

use crate::scanner::SessionMeta;
use chrono::{DateTime, Utc};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, StatefulWidget, Widget},
};
use std::collections::BTreeMap;

/// A tree item representing either a project folder or a session.
#[derive(Debug, Clone, PartialEq)]
pub enum TreeItem {
    /// A project folder that contains sessions.
    Project {
        path: String,
        expanded: bool,
        session_count: usize,
    },
    /// A session within a project.
    Session(SessionMeta),
}

impl TreeItem {
    /// Create a new project tree item.
    pub fn project(path: impl Into<String>, session_count: usize) -> Self {
        TreeItem::Project {
            path: path.into(),
            expanded: true,
            session_count,
        }
    }

    /// Create a new session tree item.
    pub fn session(meta: SessionMeta) -> Self {
        TreeItem::Session(meta)
    }

    /// Check if this item is a project.
    pub fn is_project(&self) -> bool {
        matches!(self, TreeItem::Project { .. })
    }

    /// Check if this item is expanded (only relevant for projects).
    pub fn is_expanded(&self) -> bool {
        match self {
            TreeItem::Project { expanded, .. } => *expanded,
            TreeItem::Session(_) => false,
        }
    }

    /// Toggle expansion state (only relevant for projects).
    pub fn toggle_expanded(&mut self) {
        if let TreeItem::Project { expanded, .. } = self {
            *expanded = !*expanded;
        }
    }

    /// Set expansion state (only relevant for projects).
    pub fn set_expanded(&mut self, value: bool) {
        if let TreeItem::Project { expanded, .. } = self {
            *expanded = value;
        }
    }

    /// Get the display text for this item.
    pub fn display_text(&self, now: DateTime<Utc>) -> String {
        match self {
            TreeItem::Project {
                path,
                expanded,
                session_count,
            } => {
                let icon = if *expanded { "⏷" } else { "⏵" };
                format!("{} {} ({} sessions)", icon, path, session_count)
            }
            TreeItem::Session(meta) => {
                let time_ago = format_relative_time(meta.updated_at, now);
                let msg_count = format!("{} msgs", meta.message_count);
                let id_truncated = truncate_id(&meta.id, 8);
                format!("  {} {} {}", id_truncated, time_ago, msg_count)
            }
        }
    }
}

/// Truncate a session ID to the specified length.
fn truncate_id(id: &str, max_len: usize) -> String {
    if id.len() <= max_len {
        id.to_string()
    } else {
        format!("{}...", &id[..max_len.saturating_sub(3)])
    }
}

/// Format a timestamp as relative time (e.g., "2h ago", "1d ago").
fn format_relative_time(time: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let duration = now.signed_duration_since(time);

    if duration.num_seconds() < 60 {
        "just now".to_string()
    } else if duration.num_minutes() < 60 {
        format!("{}m ago", duration.num_minutes())
    } else if duration.num_hours() < 24 {
        format!("{}h ago", duration.num_hours())
    } else if duration.num_days() < 30 {
        format!("{}d ago", duration.num_days())
    } else if duration.num_days() < 365 {
        format!("{}mo ago", duration.num_days() / 30)
    } else {
        format!("{}y ago", duration.num_days() / 365)
    }
}

/// State for the session list widget.
#[derive(Debug, Default)]
pub struct SessionListState {
    /// All items in the tree (projects and sessions).
    items: Vec<TreeItem>,
    /// Index of the currently selected item.
    selected: usize,
    /// Scroll offset for the viewport.
    offset: usize,
    /// Cached list of visible indices (for navigation).
    visible_indices: Vec<usize>,
}

impl SessionListState {
    /// Create a new session list state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build the tree from a list of session metadata.
    pub fn from_sessions(sessions: Vec<SessionMeta>) -> Self {
        // Group sessions by project path
        let mut by_project: BTreeMap<String, Vec<SessionMeta>> = BTreeMap::new();
        for session in sessions {
            by_project
                .entry(session.project_path.clone())
                .or_default()
                .push(session);
        }

        // Build tree items
        let mut items = Vec::new();
        for (project_path, mut project_sessions) in by_project {
            // Sort sessions by updated_at (newest first)
            project_sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

            // Add project header
            let session_count = project_sessions.len();
            items.push(TreeItem::project(project_path, session_count));

            // Add sessions under this project
            for session in project_sessions {
                items.push(TreeItem::session(session));
            }
        }

        let mut state = Self {
            items,
            selected: 0,
            offset: 0,
            visible_indices: Vec::new(),
        };
        state.rebuild_visible_indices();
        state
    }

    /// Rebuild the list of visible item indices.
    fn rebuild_visible_indices(&mut self) {
        self.visible_indices.clear();
        let mut current_project_expanded = true;

        for (i, item) in self.items.iter().enumerate() {
            match item {
                TreeItem::Project { expanded, .. } => {
                    self.visible_indices.push(i);
                    current_project_expanded = *expanded;
                }
                TreeItem::Session(_) => {
                    if current_project_expanded {
                        self.visible_indices.push(i);
                    }
                }
            }
        }
    }

    /// Get the number of visible items.
    pub fn visible_count(&self) -> usize {
        self.visible_indices.len()
    }

    /// Get the currently selected item.
    pub fn selected_item(&self) -> Option<&TreeItem> {
        self.visible_indices
            .get(self.selected)
            .and_then(|&idx| self.items.get(idx))
    }

    /// Get the currently selected session metadata (if a session is selected).
    pub fn selected_session(&self) -> Option<&SessionMeta> {
        match self.selected_item() {
            Some(TreeItem::Session(meta)) => Some(meta),
            _ => None,
        }
    }

    /// Get the selected index.
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Get the scroll offset.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Move selection up.
    pub fn select_previous(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        if !self.visible_indices.is_empty() && self.selected < self.visible_indices.len() - 1 {
            self.selected += 1;
        }
    }

    /// Move selection to the first item.
    pub fn select_first(&mut self) {
        self.selected = 0;
    }

    /// Move selection to the last item.
    pub fn select_last(&mut self) {
        if !self.visible_indices.is_empty() {
            self.selected = self.visible_indices.len() - 1;
        }
    }

    /// Expand the selected item (if it's a project).
    pub fn expand_selected(&mut self) {
        if let Some(&idx) = self.visible_indices.get(self.selected) {
            if let Some(item) = self.items.get_mut(idx) {
                if item.is_project() && !item.is_expanded() {
                    item.set_expanded(true);
                    self.rebuild_visible_indices();
                }
            }
        }
    }

    /// Collapse the selected item (if it's a project).
    pub fn collapse_selected(&mut self) {
        if let Some(&idx) = self.visible_indices.get(self.selected) {
            if let Some(item) = self.items.get_mut(idx) {
                if item.is_project() && item.is_expanded() {
                    item.set_expanded(false);
                    self.rebuild_visible_indices();
                }
            }
        }
    }

    /// Toggle expansion of the selected item (if it's a project).
    pub fn toggle_selected(&mut self) {
        if let Some(&idx) = self.visible_indices.get(self.selected) {
            if let Some(item) = self.items.get_mut(idx) {
                if item.is_project() {
                    item.toggle_expanded();
                    self.rebuild_visible_indices();
                }
            }
        }
    }

    /// Collapse if on a session, go to parent project.
    /// If on an expanded project, collapse it.
    /// If on a collapsed project, do nothing.
    pub fn collapse_or_parent(&mut self) {
        if let Some(&idx) = self.visible_indices.get(self.selected) {
            if let Some(item) = self.items.get(idx) {
                match item {
                    TreeItem::Project { expanded: true, .. } => {
                        self.collapse_selected();
                    }
                    TreeItem::Session(_) => {
                        // Find parent project and select it
                        for (visible_idx, &item_idx) in self.visible_indices.iter().enumerate() {
                            if item_idx == idx {
                                // Search backwards for the parent project
                                for i in (0..visible_idx).rev() {
                                    if let Some(&parent_idx) = self.visible_indices.get(i) {
                                        if let Some(TreeItem::Project { .. }) =
                                            self.items.get(parent_idx)
                                        {
                                            self.selected = i;
                                            return;
                                        }
                                    }
                                }
                                break;
                            }
                        }
                    }
                    TreeItem::Project {
                        expanded: false, ..
                    } => {
                        // Already collapsed, do nothing
                    }
                }
            }
        }
    }

    /// Expand if on a project, or do nothing if on a session.
    pub fn expand_or_select(&mut self) {
        if let Some(&idx) = self.visible_indices.get(self.selected) {
            if let Some(item) = self.items.get(idx) {
                if item.is_project() && !item.is_expanded() {
                    self.expand_selected();
                }
            }
        }
    }

    /// Adjust the scroll offset to keep the selected item visible.
    pub fn adjust_scroll(&mut self, viewport_height: usize) {
        if viewport_height == 0 {
            return;
        }

        // Ensure selected item is within viewport
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + viewport_height {
            self.offset = self.selected.saturating_sub(viewport_height - 1);
        }
    }

    /// Check if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Get all items (for testing).
    #[cfg(test)]
    pub fn items(&self) -> &[TreeItem] {
        &self.items
    }
}

/// Session list widget for rendering the tree view.
#[derive(Debug)]
pub struct SessionList<'a> {
    block: Option<Block<'a>>,
    now: DateTime<Utc>,
    highlight_style: Style,
    normal_style: Style,
    project_style: Style,
}

impl<'a> Default for SessionList<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> SessionList<'a> {
    /// Create a new session list widget.
    pub fn new() -> Self {
        Self {
            block: None,
            now: Utc::now(),
            highlight_style: Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
            normal_style: Style::default().fg(Color::Gray),
            project_style: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        }
    }

    /// Set the block to wrap this widget in.
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// Set the current time (for relative timestamps).
    pub fn now(mut self, now: DateTime<Utc>) -> Self {
        self.now = now;
        self
    }

    /// Set the highlight style for the selected item.
    pub fn highlight_style(mut self, style: Style) -> Self {
        self.highlight_style = style;
        self
    }

    /// Set the normal style for non-selected items.
    pub fn normal_style(mut self, style: Style) -> Self {
        self.normal_style = style;
        self
    }

    /// Set the style for project folder headers.
    pub fn project_style(mut self, style: Style) -> Self {
        self.project_style = style;
        self
    }
}

impl StatefulWidget for SessionList<'_> {
    type State = SessionListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Calculate inner area (accounting for block borders)
        let inner_area = if let Some(ref block) = self.block {
            let inner = block.inner(area);
            block.clone().render(area, buf);
            inner
        } else {
            area
        };

        if inner_area.height == 0 || state.visible_indices.is_empty() {
            return;
        }

        // Adjust scroll to keep selection visible
        state.adjust_scroll(inner_area.height as usize);

        // Render visible items
        let viewport_height = inner_area.height as usize;
        let start_idx = state.offset;
        let end_idx = (start_idx + viewport_height).min(state.visible_indices.len());

        for (row, visible_idx) in (start_idx..end_idx).enumerate() {
            if let Some(&item_idx) = state.visible_indices.get(visible_idx) {
                if let Some(item) = state.items.get(item_idx) {
                    let y = inner_area.y + row as u16;
                    let line_area = Rect::new(inner_area.x, y, inner_area.width, 1);

                    // Determine style based on selection and item type
                    let is_selected = visible_idx == state.selected;
                    let base_style = if item.is_project() {
                        self.project_style
                    } else {
                        self.normal_style
                    };

                    let style = if is_selected {
                        base_style.patch(self.highlight_style)
                    } else {
                        base_style
                    };

                    // Render the item text
                    let text = item.display_text(self.now);
                    let line = Line::from(Span::styled(text, style));
                    buf.set_line(line_area.x, line_area.y, &line, line_area.width);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_timestamp() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn test_now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2024-01-15T12:30:00Z")
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
            SessionMeta::new(
                "ghi11111",
                PathBuf::from("/home/user/.claude/projects/frontend/ghi11111.jsonl"),
                "~/projects/frontend",
                test_timestamp(),
            )
            .with_message_count(25),
        ]
    }

    #[test]
    fn test_tree_item_project() {
        let item = TreeItem::project("~/projects/api", 5);
        assert!(item.is_project());
        assert!(item.is_expanded());
    }

    #[test]
    fn test_tree_item_session() {
        let meta = SessionMeta::new("session-123", PathBuf::from("/path"), "~/proj", test_now());
        let item = TreeItem::session(meta);
        assert!(!item.is_project());
        assert!(!item.is_expanded());
    }

    #[test]
    fn test_tree_item_toggle_expanded() {
        let mut item = TreeItem::project("~/projects/api", 5);
        assert!(item.is_expanded());
        item.toggle_expanded();
        assert!(!item.is_expanded());
        item.toggle_expanded();
        assert!(item.is_expanded());
    }

    #[test]
    fn test_tree_item_display_text_project() {
        let item = TreeItem::project("~/projects/api", 5);
        let text = item.display_text(test_now());
        assert!(text.contains("~/projects/api"));
        assert!(text.contains("5 sessions"));
        assert!(text.contains("⏷")); // Expanded icon
    }

    #[test]
    fn test_tree_item_display_text_collapsed_project() {
        let mut item = TreeItem::project("~/projects/api", 5);
        item.set_expanded(false);
        let text = item.display_text(test_now());
        assert!(text.contains("⏵")); // Collapsed icon
    }

    #[test]
    fn test_tree_item_display_text_session() {
        let meta = SessionMeta::new(
            "abc123456789",
            PathBuf::from("/path"),
            "~/proj",
            test_timestamp(),
        )
        .with_message_count(12);
        let item = TreeItem::session(meta);
        let text = item.display_text(test_now());
        assert!(text.contains("abc12...")); // Truncated ID (longer than 8 chars)
        assert!(text.contains("12 msgs"));
        assert!(text.contains("2h ago"));
    }

    #[test]
    fn test_truncate_id_short() {
        assert_eq!(truncate_id("abc123", 8), "abc123");
    }

    #[test]
    fn test_truncate_id_long() {
        assert_eq!(truncate_id("abc123456789", 8), "abc12...");
    }

    #[test]
    fn test_format_relative_time_just_now() {
        let now = test_now();
        let time = now - chrono::Duration::seconds(30);
        assert_eq!(format_relative_time(time, now), "just now");
    }

    #[test]
    fn test_format_relative_time_minutes() {
        let now = test_now();
        let time = now - chrono::Duration::minutes(45);
        assert_eq!(format_relative_time(time, now), "45m ago");
    }

    #[test]
    fn test_format_relative_time_hours() {
        let now = test_now();
        let time = now - chrono::Duration::hours(5);
        assert_eq!(format_relative_time(time, now), "5h ago");
    }

    #[test]
    fn test_format_relative_time_days() {
        let now = test_now();
        let time = now - chrono::Duration::days(3);
        assert_eq!(format_relative_time(time, now), "3d ago");
    }

    #[test]
    fn test_session_list_state_from_sessions() {
        let state = SessionListState::from_sessions(sample_sessions());

        // Should have 2 projects + 3 sessions = 5 items
        assert_eq!(state.items.len(), 5);
        // Should have 2 project headers
        let project_count = state.items.iter().filter(|i| i.is_project()).count();
        assert_eq!(project_count, 2);
    }

    #[test]
    fn test_session_list_state_visible_count() {
        let state = SessionListState::from_sessions(sample_sessions());
        // All items visible by default (projects expanded)
        assert_eq!(state.visible_count(), 5);
    }

    #[test]
    fn test_session_list_state_select_next() {
        let mut state = SessionListState::from_sessions(sample_sessions());
        assert_eq!(state.selected(), 0);
        state.select_next();
        assert_eq!(state.selected(), 1);
    }

    #[test]
    fn test_session_list_state_select_previous() {
        let mut state = SessionListState::from_sessions(sample_sessions());
        state.selected = 2;
        state.select_previous();
        assert_eq!(state.selected(), 1);
    }

    #[test]
    fn test_session_list_state_select_previous_at_start() {
        let mut state = SessionListState::from_sessions(sample_sessions());
        assert_eq!(state.selected(), 0);
        state.select_previous();
        assert_eq!(state.selected(), 0); // Stays at 0
    }

    #[test]
    fn test_session_list_state_select_next_at_end() {
        let mut state = SessionListState::from_sessions(sample_sessions());
        state.select_last();
        let last = state.selected();
        state.select_next();
        assert_eq!(state.selected(), last); // Stays at end
    }

    #[test]
    fn test_session_list_state_collapse_project() {
        let mut state = SessionListState::from_sessions(sample_sessions());
        // First item should be a project (~/projects/api)
        assert!(state.items[0].is_project());
        assert!(state.items[0].is_expanded());

        state.collapse_selected();

        assert!(!state.items[0].is_expanded());
        // Visible count should decrease (project + 2 sessions hidden -> visible count drops by 2)
        assert_eq!(state.visible_count(), 3); // 2 projects + 1 session under second project
    }

    #[test]
    fn test_session_list_state_expand_project() {
        let mut state = SessionListState::from_sessions(sample_sessions());
        state.collapse_selected();
        assert!(!state.items[0].is_expanded());

        state.expand_selected();

        assert!(state.items[0].is_expanded());
        assert_eq!(state.visible_count(), 5);
    }

    #[test]
    fn test_session_list_state_selected_item() {
        let state = SessionListState::from_sessions(sample_sessions());
        let item = state.selected_item().unwrap();
        assert!(item.is_project()); // First item is a project
    }

    #[test]
    fn test_session_list_state_selected_session() {
        let mut state = SessionListState::from_sessions(sample_sessions());
        state.select_next(); // Move to first session under first project
        let session = state.selected_session().unwrap();
        assert!(session.id.starts_with("abc") || session.id.starts_with("def"));
    }

    #[test]
    fn test_session_list_state_adjust_scroll() {
        let mut state = SessionListState::from_sessions(sample_sessions());
        state.selected = 4; // Last item
        state.offset = 0;
        state.adjust_scroll(3); // Viewport of 3 lines
        assert!(state.offset > 0); // Should have scrolled
    }

    #[test]
    fn test_session_list_state_collapse_or_parent_on_session() {
        let mut state = SessionListState::from_sessions(sample_sessions());
        state.select_next(); // Move to first session
        assert!(!state.selected_item().unwrap().is_project());

        state.collapse_or_parent();

        // Should have moved back to parent project
        assert!(state.selected_item().unwrap().is_project());
    }

    #[test]
    fn test_session_list_state_collapse_or_parent_on_expanded_project() {
        let mut state = SessionListState::from_sessions(sample_sessions());
        // First item is an expanded project
        assert!(state.selected_item().unwrap().is_project());
        assert!(state.selected_item().unwrap().is_expanded());

        state.collapse_or_parent();

        // Project should now be collapsed
        assert!(!state.selected_item().unwrap().is_expanded());
    }

    #[test]
    fn test_session_list_widget_new() {
        let widget = SessionList::new();
        assert!(widget.block.is_none());
    }

    #[test]
    fn test_session_list_widget_block() {
        let block = Block::bordered().title("Sessions");
        let widget = SessionList::new().block(block);
        assert!(widget.block.is_some());
    }

    #[test]
    fn test_session_list_state_empty() {
        let state = SessionListState::from_sessions(vec![]);
        assert!(state.is_empty());
        assert_eq!(state.visible_count(), 0);
        assert!(state.selected_item().is_none());
    }

    #[test]
    fn test_session_list_state_select_first_and_last() {
        let mut state = SessionListState::from_sessions(sample_sessions());
        state.select_last();
        assert_eq!(state.selected(), 4);
        state.select_first();
        assert_eq!(state.selected(), 0);
    }
}
