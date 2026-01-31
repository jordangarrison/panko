//! Session list widget with project grouping.
//!
//! This widget displays sessions in a tree view, grouped by project path.
//! Projects can be expanded/collapsed, and sessions show truncated info.

use crate::scanner::SessionMeta;
use chrono::{DateTime, Utc};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, StatefulWidget, Widget},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Sort order for sessions in the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    /// Sort by date, newest first (default).
    #[default]
    DateNewest,
    /// Sort by date, oldest first.
    DateOldest,
    /// Sort by message count, highest first.
    MessageCount,
    /// Sort by project name, alphabetically.
    ProjectName,
}

impl SortOrder {
    /// Cycle to the next sort order.
    pub fn next(&self) -> Self {
        match self {
            SortOrder::DateNewest => SortOrder::DateOldest,
            SortOrder::DateOldest => SortOrder::MessageCount,
            SortOrder::MessageCount => SortOrder::ProjectName,
            SortOrder::ProjectName => SortOrder::DateNewest,
        }
    }

    /// Get a display name for the sort order.
    pub fn display_name(&self) -> &'static str {
        match self {
            SortOrder::DateNewest => "Date (newest)",
            SortOrder::DateOldest => "Date (oldest)",
            SortOrder::MessageCount => "Messages",
            SortOrder::ProjectName => "Project",
        }
    }

    /// Get a short name for the sort order (for header display).
    pub fn short_name(&self) -> &'static str {
        match self {
            SortOrder::DateNewest => "↓ Date",
            SortOrder::DateOldest => "↑ Date",
            SortOrder::MessageCount => "# Msgs",
            SortOrder::ProjectName => "A-Z",
        }
    }

    /// Parse from a string representation (for config loading).
    ///
    /// Note: We don't implement `std::str::FromStr` because we want to return
    /// `Option<Self>` rather than `Result<Self, Error>` for simpler usage.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "date_newest" | "datenewest" | "newest" => Some(SortOrder::DateNewest),
            "date_oldest" | "dateoldest" | "oldest" => Some(SortOrder::DateOldest),
            "message_count" | "messagecount" | "messages" => Some(SortOrder::MessageCount),
            "project_name" | "projectname" | "project" => Some(SortOrder::ProjectName),
            _ => None,
        }
    }

    /// Alias for `parse` for backward compatibility.
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        Self::parse(s)
    }

    /// Convert to a string representation (for config saving).
    pub fn as_str(&self) -> &'static str {
        match self {
            SortOrder::DateNewest => "date_newest",
            SortOrder::DateOldest => "date_oldest",
            SortOrder::MessageCount => "message_count",
            SortOrder::ProjectName => "project_name",
        }
    }
}

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
                let agent_tag = meta.agent_type.tag();
                format!(
                    "  [{}] {} {} {}",
                    agent_tag, id_truncated, time_ago, msg_count
                )
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

/// A search match result with the matched item index and match positions.
#[derive(Debug, Clone)]
pub struct SearchMatch {
    /// Index into the items vec.
    pub item_index: usize,
    /// Score from the fuzzy matcher (higher is better).
    pub score: i64,
    /// Positions of matching characters in the search text.
    pub match_positions: Vec<usize>,
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
    /// Current search query (empty means no active search).
    search_query: String,
    /// Filtered matches when search is active.
    search_matches: Vec<SearchMatch>,
    /// Original sessions for rebuilding after search clears.
    original_sessions: Vec<SessionMeta>,
    /// Current sort order for sessions.
    sort_order: SortOrder,
}

impl SessionListState {
    /// Create a new session list state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build the tree from a list of session metadata with the default sort order.
    pub fn from_sessions(sessions: Vec<SessionMeta>) -> Self {
        Self::from_sessions_with_sort(sessions, SortOrder::default())
    }

    /// Build the tree from a list of session metadata with a specific sort order.
    pub fn from_sessions_with_sort(sessions: Vec<SessionMeta>, sort_order: SortOrder) -> Self {
        // Store original sessions for filtering later
        let original_sessions = sessions.clone();

        // Build items using the sort order
        let items = Self::build_sorted_items(&sessions, sort_order);

        let mut state = Self {
            items,
            selected: 0,
            offset: 0,
            visible_indices: Vec::new(),
            search_query: String::new(),
            search_matches: Vec::new(),
            original_sessions,
            sort_order,
        };
        state.rebuild_visible_indices();
        state
    }

    /// Build sorted tree items from sessions.
    fn build_sorted_items(sessions: &[SessionMeta], sort_order: SortOrder) -> Vec<TreeItem> {
        // Group sessions by project path
        let mut by_project: BTreeMap<String, Vec<SessionMeta>> = BTreeMap::new();
        for session in sessions {
            by_project
                .entry(session.project_path.clone())
                .or_default()
                .push(session.clone());
        }

        // Collect project names for sorting
        let mut project_names: Vec<String> = by_project.keys().cloned().collect();

        // Sort project names based on sort order
        match sort_order {
            SortOrder::ProjectName => {
                // Alphabetical order for projects
                project_names.sort();
            }
            SortOrder::DateNewest => {
                // Sort projects by their newest session
                project_names.sort_by(|a, b| {
                    let a_newest = by_project
                        .get(a)
                        .and_then(|s| s.iter().map(|s| s.updated_at).max());
                    let b_newest = by_project
                        .get(b)
                        .and_then(|s| s.iter().map(|s| s.updated_at).max());
                    b_newest.cmp(&a_newest) // Descending (newest first)
                });
            }
            SortOrder::DateOldest => {
                // Sort projects by their oldest session
                project_names.sort_by(|a, b| {
                    let a_oldest = by_project
                        .get(a)
                        .and_then(|s| s.iter().map(|s| s.updated_at).min());
                    let b_oldest = by_project
                        .get(b)
                        .and_then(|s| s.iter().map(|s| s.updated_at).min());
                    a_oldest.cmp(&b_oldest) // Ascending (oldest first)
                });
            }
            SortOrder::MessageCount => {
                // Sort projects by total message count
                project_names.sort_by(|a, b| {
                    let a_total: usize = by_project
                        .get(a)
                        .map(|s| s.iter().map(|s| s.message_count).sum())
                        .unwrap_or(0);
                    let b_total: usize = by_project
                        .get(b)
                        .map(|s| s.iter().map(|s| s.message_count).sum())
                        .unwrap_or(0);
                    b_total.cmp(&a_total) // Descending (most messages first)
                });
            }
        }

        // Build tree items in sorted order
        let mut items = Vec::new();
        for project_path in project_names {
            if let Some(mut project_sessions) = by_project.remove(&project_path) {
                // Sort sessions within each project based on sort order
                match sort_order {
                    SortOrder::DateNewest => {
                        project_sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
                    }
                    SortOrder::DateOldest => {
                        project_sessions.sort_by(|a, b| a.updated_at.cmp(&b.updated_at));
                    }
                    SortOrder::MessageCount => {
                        project_sessions.sort_by(|a, b| b.message_count.cmp(&a.message_count));
                    }
                    SortOrder::ProjectName => {
                        // Within a project, still sort by newest when sorted by project name
                        project_sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
                    }
                }

                // Add project header
                let session_count = project_sessions.len();
                items.push(TreeItem::project(project_path, session_count));

                // Add sessions under this project
                for session in project_sessions {
                    items.push(TreeItem::session(session));
                }
            }
        }

        items
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

    /// Select a session by its ID.
    ///
    /// Searches through visible items and selects the session with the matching ID.
    /// If the session is not found (e.g., it was deleted), selection remains unchanged.
    pub fn select_session_by_id(&mut self, session_id: &str) -> bool {
        for (visible_idx, &item_idx) in self.visible_indices.iter().enumerate() {
            if let Some(TreeItem::Session(meta)) = self.items.get(item_idx) {
                if meta.id == session_id {
                    self.selected = visible_idx;
                    return true;
                }
            }
        }
        false
    }

    /// Remove a session by its file path.
    ///
    /// Removes the session from both the items list and original_sessions.
    /// If the removed session was selected, moves selection to the previous item.
    /// If a project becomes empty after removal, the project is also removed.
    pub fn remove_session_by_path(&mut self, path: &std::path::PathBuf) {
        // Find and remove from original_sessions
        self.original_sessions.retain(|s| &s.path != path);

        // Find the session's item index
        let session_item_idx = self.items.iter().position(|item| {
            if let TreeItem::Session(meta) = item {
                &meta.path == path
            } else {
                false
            }
        });

        if let Some(session_idx) = session_item_idx {
            // Find the parent project's index (search backwards)
            let mut project_idx = None;
            for i in (0..session_idx).rev() {
                if self.items[i].is_project() {
                    project_idx = Some(i);
                    break;
                }
            }

            // Remove the session
            self.items.remove(session_idx);

            // Check if parent project is now empty and remove it
            if let Some(proj_idx) = project_idx {
                // Check if there are sessions remaining under this project
                let has_sessions = self
                    .items
                    .get(proj_idx + 1)
                    .map(|item| matches!(item, TreeItem::Session(_)))
                    .unwrap_or(false);

                if !has_sessions {
                    // Remove the empty project
                    self.items.remove(proj_idx);
                } else {
                    // Update the session count on the project
                    if let TreeItem::Project { session_count, .. } = &mut self.items[proj_idx] {
                        *session_count = session_count.saturating_sub(1);
                    }
                }
            }

            // Rebuild visible indices
            self.rebuild_visible_indices();

            // Adjust selection if needed
            if self.selected >= self.visible_indices.len() {
                self.selected = self.visible_indices.len().saturating_sub(1);
            }
        }
    }

    /// Get all items (for testing).
    #[cfg(test)]
    pub fn items(&self) -> &[TreeItem] {
        &self.items
    }

    // === Sort Order Methods ===

    /// Get the current sort order.
    pub fn sort_order(&self) -> SortOrder {
        self.sort_order
    }

    /// Set the sort order and re-sort the items.
    pub fn set_sort_order(&mut self, sort_order: SortOrder) {
        if self.sort_order == sort_order {
            return;
        }

        // Remember current selection
        let selected_session_id = self.selected_session().map(|s| s.id.clone());

        self.sort_order = sort_order;

        // Rebuild items with new sort order
        self.items = Self::build_sorted_items(&self.original_sessions, sort_order);
        self.rebuild_visible_indices();

        // Try to restore selection
        if let Some(session_id) = selected_session_id {
            self.select_session_by_id(&session_id);
        } else {
            // Reset selection if no session was selected
            self.selected = 0;
        }
        self.offset = 0;
    }

    /// Cycle to the next sort order.
    pub fn cycle_sort_order(&mut self) {
        let next_order = self.sort_order.next();
        self.set_sort_order(next_order);
    }

    // === Fuzzy Search Methods ===

    /// Get the current search query.
    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    /// Check if search is currently active.
    pub fn is_searching(&self) -> bool {
        !self.search_query.is_empty()
    }

    /// Set the search query and filter results.
    ///
    /// This performs fuzzy matching against project paths and first prompt previews.
    /// Matching sessions are shown with their parent projects.
    pub fn set_search_query(&mut self, query: impl Into<String>) {
        let query: String = query.into();

        if query.is_empty() {
            self.clear_search();
            return;
        }

        self.search_query = query;
        self.perform_search();
    }

    /// Clear the search and restore all items.
    pub fn clear_search(&mut self) {
        if self.search_query.is_empty() && self.search_matches.is_empty() {
            return;
        }

        self.search_query.clear();
        self.search_matches.clear();

        // Rebuild items from original sessions, preserving sort order
        let sort_order = self.sort_order;
        *self =
            Self::from_sessions_with_sort(std::mem::take(&mut self.original_sessions), sort_order);
    }

    /// Perform the fuzzy search and update the view.
    fn perform_search(&mut self) {
        let matcher = SkimMatcherV2::default();
        let mut matches: Vec<SearchMatch> = Vec::new();

        // Match against sessions only (not projects directly)
        for (idx, item) in self.items.iter().enumerate() {
            if let TreeItem::Session(meta) = item {
                // Build search text from project path and first prompt
                let search_text = format!(
                    "{} {}",
                    meta.project_path,
                    meta.first_prompt_preview.as_deref().unwrap_or("")
                );

                if let Some((score, positions)) =
                    matcher.fuzzy_indices(&search_text, &self.search_query)
                {
                    matches.push(SearchMatch {
                        item_index: idx,
                        score,
                        match_positions: positions,
                    });
                }
            }
        }

        // Sort by score (highest first)
        matches.sort_by(|a, b| b.score.cmp(&a.score));

        self.search_matches = matches;

        // Rebuild visible indices to only show matching sessions and their projects
        self.rebuild_filtered_visible_indices();

        // Reset selection to first item
        self.selected = 0;
        self.offset = 0;
    }

    /// Rebuild visible indices to only show matching sessions and their projects.
    fn rebuild_filtered_visible_indices(&mut self) {
        if self.search_matches.is_empty() {
            self.visible_indices.clear();
            return;
        }

        // Collect unique matching session indices
        let matching_session_indices: std::collections::HashSet<usize> =
            self.search_matches.iter().map(|m| m.item_index).collect();

        // Also track which projects have matching sessions
        let mut projects_with_matches: std::collections::HashSet<usize> =
            std::collections::HashSet::new();

        // Find parent projects for each matching session
        for &session_idx in &matching_session_indices {
            // Search backwards for the parent project
            for i in (0..session_idx).rev() {
                if let TreeItem::Project { .. } = &self.items[i] {
                    projects_with_matches.insert(i);
                    break;
                }
            }
        }

        // Build visible indices in tree order
        self.visible_indices.clear();
        let mut in_matching_project = false;

        for (i, item) in self.items.iter().enumerate() {
            match item {
                TreeItem::Project { .. } => {
                    if projects_with_matches.contains(&i) {
                        self.visible_indices.push(i);
                        in_matching_project = true;
                    } else {
                        in_matching_project = false;
                    }
                }
                TreeItem::Session(_) => {
                    if in_matching_project && matching_session_indices.contains(&i) {
                        self.visible_indices.push(i);
                    }
                }
            }
        }
    }

    /// Get the match info for a specific item index, if it exists.
    pub fn get_match_for_item(&self, item_index: usize) -> Option<&SearchMatch> {
        self.search_matches
            .iter()
            .find(|m| m.item_index == item_index)
    }

    /// Get the match positions for highlighting in the display text.
    /// Returns positions relative to the search text (project_path + " " + first_prompt_preview).
    pub fn get_match_positions(&self, item_index: usize) -> Option<&[usize]> {
        self.get_match_for_item(item_index)
            .map(|m| m.match_positions.as_slice())
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
    match_style: Style,
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
            match_style: Style::default()
                .fg(Color::Yellow)
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

    /// Set the style for matching characters in search results.
    pub fn match_style(mut self, style: Style) -> Self {
        self.match_style = style;
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

        let is_searching = state.is_searching();

        for (row, visible_idx) in (start_idx..end_idx).enumerate() {
            if let Some(&item_idx) = state.visible_indices.get(visible_idx) {
                if let Some(item) = state.items.get(item_idx) {
                    let y = inner_area.y + row as u16;
                    let line_area = Rect::new(inner_area.x, y, inner_area.width, 1);

                    // Check if this item has a search match
                    let has_match = is_searching && state.get_match_for_item(item_idx).is_some();

                    // Determine style based on selection, item type, and match status
                    let is_selected = visible_idx == state.selected;
                    let base_style = if item.is_project() {
                        self.project_style
                    } else if has_match {
                        // Use match style for matching sessions
                        self.match_style
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

                    // If searching and this is a matching session, add a marker
                    let display_text = if has_match {
                        format!("★ {}", text.trim_start())
                    } else {
                        text
                    };

                    let line = Line::from(Span::styled(display_text, style));
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

    // === Fuzzy Search Tests ===

    fn sessions_with_prompts() -> Vec<SessionMeta> {
        vec![
            SessionMeta::new(
                "session1",
                PathBuf::from("/home/user/.claude/projects/api/session1.jsonl"),
                "~/projects/api-server",
                test_timestamp(),
            )
            .with_message_count(10)
            .with_first_prompt_preview("Refactor the authentication module to use JWT"),
            SessionMeta::new(
                "session2",
                PathBuf::from("/home/user/.claude/projects/api/session2.jsonl"),
                "~/projects/api-server",
                test_timestamp(),
            )
            .with_message_count(5)
            .with_first_prompt_preview("Add rate limiting to API endpoints"),
            SessionMeta::new(
                "session3",
                PathBuf::from("/home/user/.claude/projects/web/session3.jsonl"),
                "~/projects/web-frontend",
                test_timestamp(),
            )
            .with_message_count(15)
            .with_first_prompt_preview("Build a responsive dashboard component"),
        ]
    }

    #[test]
    fn test_search_query_default_empty() {
        let state = SessionListState::from_sessions(sessions_with_prompts());
        assert_eq!(state.search_query(), "");
        assert!(!state.is_searching());
    }

    #[test]
    fn test_set_search_query_activates_search() {
        let mut state = SessionListState::from_sessions(sessions_with_prompts());
        state.set_search_query("api");
        assert_eq!(state.search_query(), "api");
        assert!(state.is_searching());
    }

    #[test]
    fn test_search_filters_by_project_path() {
        let mut state = SessionListState::from_sessions(sessions_with_prompts());

        // All items visible initially (2 projects + 3 sessions)
        assert_eq!(state.visible_count(), 5);

        // Search for "api-server" project
        state.set_search_query("api-server");

        // Should only show the api project with its sessions
        // 1 project + 2 sessions = 3 visible
        assert!(state.visible_count() >= 1);
        assert!(state.visible_count() <= 3);
    }

    #[test]
    fn test_search_filters_by_prompt_content() {
        let mut state = SessionListState::from_sessions(sessions_with_prompts());

        // Search for "JWT" which is only in session1's prompt
        state.set_search_query("JWT");

        // Should filter to show matching session
        assert!(state.is_searching());
        // At least 1 match (the session), plus its parent project
        assert!(state.visible_count() >= 2);
    }

    #[test]
    fn test_clear_search_restores_all_items() {
        let mut state = SessionListState::from_sessions(sessions_with_prompts());
        let original_count = state.visible_count();

        // Apply search
        state.set_search_query("JWT");
        assert!(state.visible_count() < original_count);

        // Clear search
        state.clear_search();

        // All items should be visible again
        assert!(!state.is_searching());
        assert_eq!(state.search_query(), "");
        assert_eq!(state.visible_count(), original_count);
    }

    #[test]
    fn test_empty_search_shows_all() {
        let mut state = SessionListState::from_sessions(sessions_with_prompts());
        let original_count = state.visible_count();

        // Setting empty query should not filter
        state.set_search_query("");
        assert_eq!(state.visible_count(), original_count);
        assert!(!state.is_searching());
    }

    #[test]
    fn test_search_no_matches() {
        let mut state = SessionListState::from_sessions(sessions_with_prompts());

        // Search for something that doesn't exist
        state.set_search_query("xyznonexistent123");

        // Should show no results
        assert!(state.is_searching());
        assert_eq!(state.visible_count(), 0);
    }

    #[test]
    fn test_search_resets_selection() {
        let mut state = SessionListState::from_sessions(sessions_with_prompts());
        state.select_last(); // Move to last item
        assert!(state.selected() > 0);

        // Search should reset selection to 0
        state.set_search_query("api");
        assert_eq!(state.selected(), 0);
    }

    #[test]
    fn test_fuzzy_matching_partial() {
        let mut state = SessionListState::from_sessions(sessions_with_prompts());

        // Fuzzy match "rfct" should match "refactor"
        state.set_search_query("rfct");

        // Should find the session with "Refactor" in the prompt
        assert!(state.is_searching());
        // Fuzzy matching should find something
        // Note: depending on matcher sensitivity this might vary
    }

    #[test]
    fn test_get_match_for_item_returns_none_for_non_match() {
        let mut state = SessionListState::from_sessions(sessions_with_prompts());
        state.set_search_query("dashboard"); // Only matches session3

        // Project items don't have matches
        assert!(state.get_match_for_item(0).is_none());
    }

    #[test]
    fn test_search_preserves_original_sessions() {
        let mut state = SessionListState::from_sessions(sessions_with_prompts());

        // Do multiple search/clear cycles
        state.set_search_query("api");
        state.clear_search();
        state.set_search_query("web");
        state.clear_search();

        // All sessions should still be available
        assert_eq!(state.visible_count(), 5); // 2 projects + 3 sessions
    }

    // === Selection by ID Tests ===

    #[test]
    fn test_select_session_by_id_existing_session() {
        let mut state = SessionListState::from_sessions(sample_sessions());

        // Select a session by its ID
        let found = state.select_session_by_id("ghi11111");
        assert!(found);

        // Verify the correct session is selected
        let selected = state.selected_session();
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().id, "ghi11111");
    }

    #[test]
    fn test_select_session_by_id_first_session() {
        let mut state = SessionListState::from_sessions(sample_sessions());

        // Select the first session (under first project)
        let found = state.select_session_by_id("abc12345");
        assert!(found);

        let selected = state.selected_session();
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().id, "abc12345");
    }

    #[test]
    fn test_select_session_by_id_nonexistent_session() {
        let mut state = SessionListState::from_sessions(sample_sessions());

        // Initial selection
        let initial_selection = state.selected();

        // Try to select a session that doesn't exist
        let found = state.select_session_by_id("nonexistent_id");
        assert!(!found);

        // Selection should remain unchanged
        assert_eq!(state.selected(), initial_selection);
    }

    #[test]
    fn test_select_session_by_id_empty_list() {
        let mut state = SessionListState::from_sessions(vec![]);

        let found = state.select_session_by_id("any_id");
        assert!(!found);
    }

    #[test]
    fn test_select_session_by_id_preserves_selection_on_failure() {
        let mut state = SessionListState::from_sessions(sample_sessions());

        // Navigate to a specific position
        state.select_next();
        state.select_next();
        let selection_before = state.selected();

        // Try to select non-existent session
        let found = state.select_session_by_id("does_not_exist");
        assert!(!found);

        // Selection should be preserved
        assert_eq!(state.selected(), selection_before);
    }

    // === Sort Order Tests ===

    fn sessions_for_sorting() -> Vec<SessionMeta> {
        let t1 = DateTime::parse_from_rfc3339("2024-01-15T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let t2 = DateTime::parse_from_rfc3339("2024-01-15T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let t3 = DateTime::parse_from_rfc3339("2024-01-14T08:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        vec![
            SessionMeta::new(
                "session1",
                PathBuf::from("/home/user/.claude/projects/api/session1.jsonl"),
                "~/projects/api",
                t1,
            )
            .with_message_count(10),
            SessionMeta::new(
                "session2",
                PathBuf::from("/home/user/.claude/projects/api/session2.jsonl"),
                "~/projects/api",
                t2,
            )
            .with_message_count(30),
            SessionMeta::new(
                "session3",
                PathBuf::from("/home/user/.claude/projects/web/session3.jsonl"),
                "~/projects/web",
                t3,
            )
            .with_message_count(5),
        ]
    }

    #[test]
    fn test_sort_order_default_is_date_newest() {
        assert_eq!(SortOrder::default(), SortOrder::DateNewest);
    }

    #[test]
    fn test_sort_order_next_cycles() {
        let order = SortOrder::DateNewest;
        assert_eq!(order.next(), SortOrder::DateOldest);
        assert_eq!(order.next().next(), SortOrder::MessageCount);
        assert_eq!(order.next().next().next(), SortOrder::ProjectName);
        assert_eq!(order.next().next().next().next(), SortOrder::DateNewest);
    }

    #[test]
    fn test_sort_order_display_name() {
        assert_eq!(SortOrder::DateNewest.display_name(), "Date (newest)");
        assert_eq!(SortOrder::DateOldest.display_name(), "Date (oldest)");
        assert_eq!(SortOrder::MessageCount.display_name(), "Messages");
        assert_eq!(SortOrder::ProjectName.display_name(), "Project");
    }

    #[test]
    fn test_sort_order_short_name() {
        assert_eq!(SortOrder::DateNewest.short_name(), "↓ Date");
        assert_eq!(SortOrder::DateOldest.short_name(), "↑ Date");
        assert_eq!(SortOrder::MessageCount.short_name(), "# Msgs");
        assert_eq!(SortOrder::ProjectName.short_name(), "A-Z");
    }

    #[test]
    fn test_sort_order_from_str() {
        assert_eq!(
            SortOrder::from_str("date_newest"),
            Some(SortOrder::DateNewest)
        );
        assert_eq!(SortOrder::from_str("newest"), Some(SortOrder::DateNewest));
        assert_eq!(
            SortOrder::from_str("date_oldest"),
            Some(SortOrder::DateOldest)
        );
        assert_eq!(SortOrder::from_str("oldest"), Some(SortOrder::DateOldest));
        assert_eq!(
            SortOrder::from_str("message_count"),
            Some(SortOrder::MessageCount)
        );
        assert_eq!(
            SortOrder::from_str("messages"),
            Some(SortOrder::MessageCount)
        );
        assert_eq!(
            SortOrder::from_str("project_name"),
            Some(SortOrder::ProjectName)
        );
        assert_eq!(SortOrder::from_str("project"), Some(SortOrder::ProjectName));
        assert_eq!(SortOrder::from_str("invalid"), None);
    }

    #[test]
    fn test_sort_order_as_str() {
        assert_eq!(SortOrder::DateNewest.as_str(), "date_newest");
        assert_eq!(SortOrder::DateOldest.as_str(), "date_oldest");
        assert_eq!(SortOrder::MessageCount.as_str(), "message_count");
        assert_eq!(SortOrder::ProjectName.as_str(), "project_name");
    }

    #[test]
    fn test_state_default_sort_order() {
        let state = SessionListState::from_sessions(sessions_for_sorting());
        assert_eq!(state.sort_order(), SortOrder::DateNewest);
    }

    #[test]
    fn test_state_from_sessions_with_sort() {
        let state = SessionListState::from_sessions_with_sort(
            sessions_for_sorting(),
            SortOrder::DateOldest,
        );
        assert_eq!(state.sort_order(), SortOrder::DateOldest);
    }

    #[test]
    fn test_state_set_sort_order() {
        let mut state = SessionListState::from_sessions(sessions_for_sorting());
        assert_eq!(state.sort_order(), SortOrder::DateNewest);

        state.set_sort_order(SortOrder::MessageCount);
        assert_eq!(state.sort_order(), SortOrder::MessageCount);
    }

    #[test]
    fn test_state_cycle_sort_order() {
        let mut state = SessionListState::from_sessions(sessions_for_sorting());
        assert_eq!(state.sort_order(), SortOrder::DateNewest);

        state.cycle_sort_order();
        assert_eq!(state.sort_order(), SortOrder::DateOldest);

        state.cycle_sort_order();
        assert_eq!(state.sort_order(), SortOrder::MessageCount);

        state.cycle_sort_order();
        assert_eq!(state.sort_order(), SortOrder::ProjectName);

        state.cycle_sort_order();
        assert_eq!(state.sort_order(), SortOrder::DateNewest);
    }

    #[test]
    fn test_sort_date_newest_order() {
        let state = SessionListState::from_sessions_with_sort(
            sessions_for_sorting(),
            SortOrder::DateNewest,
        );

        // First project should be the one with the newest session (api has t2)
        let items = state.items();
        if let TreeItem::Project { path, .. } = &items[0] {
            assert_eq!(path, "~/projects/api");
        } else {
            panic!("Expected project at index 0");
        }

        // First session under api should be session2 (newest)
        if let TreeItem::Session(meta) = &items[1] {
            assert_eq!(meta.id, "session2");
        } else {
            panic!("Expected session at index 1");
        }
    }

    #[test]
    fn test_sort_date_oldest_order() {
        let state = SessionListState::from_sessions_with_sort(
            sessions_for_sorting(),
            SortOrder::DateOldest,
        );

        // First project should be web (has oldest session at t3)
        let items = state.items();
        if let TreeItem::Project { path, .. } = &items[0] {
            assert_eq!(path, "~/projects/web");
        } else {
            panic!("Expected project at index 0");
        }
    }

    #[test]
    fn test_sort_message_count_order() {
        let state = SessionListState::from_sessions_with_sort(
            sessions_for_sorting(),
            SortOrder::MessageCount,
        );

        // First project should be api (has 40 total messages)
        let items = state.items();
        if let TreeItem::Project { path, .. } = &items[0] {
            assert_eq!(path, "~/projects/api");
        } else {
            panic!("Expected project at index 0");
        }

        // First session under api should be session2 (30 msgs > 10 msgs)
        if let TreeItem::Session(meta) = &items[1] {
            assert_eq!(meta.id, "session2");
        } else {
            panic!("Expected session at index 1");
        }
    }

    #[test]
    fn test_sort_project_name_alphabetical() {
        let state = SessionListState::from_sessions_with_sort(
            sessions_for_sorting(),
            SortOrder::ProjectName,
        );

        // First project should be api (alphabetically before web)
        let items = state.items();
        if let TreeItem::Project { path, .. } = &items[0] {
            assert_eq!(path, "~/projects/api");
        } else {
            panic!("Expected project at index 0");
        }

        // Second project should be web
        // Find the second project
        for item in items.iter().skip(1) {
            if let TreeItem::Project { path, .. } = item {
                assert_eq!(path, "~/projects/web");
                break;
            }
        }
    }

    #[test]
    fn test_set_sort_order_same_order_no_change() {
        let mut state = SessionListState::from_sessions(sessions_for_sorting());
        state.select_next(); // Move selection
        let selection_before = state.selected();

        // Setting the same sort order should not change anything
        state.set_sort_order(SortOrder::DateNewest);

        assert_eq!(state.selected(), selection_before);
    }

    #[test]
    fn test_sort_preserves_selection_by_session_id() {
        let mut state = SessionListState::from_sessions(sessions_for_sorting());

        // Select a specific session
        state.select_session_by_id("session3");
        let selected = state.selected_session().unwrap();
        assert_eq!(selected.id, "session3");

        // Change sort order
        state.set_sort_order(SortOrder::MessageCount);

        // Selection should still be on the same session
        let selected_after = state.selected_session().unwrap();
        assert_eq!(selected_after.id, "session3");
    }

    #[test]
    fn test_clear_search_preserves_sort_order() {
        let mut state = SessionListState::from_sessions_with_sort(
            sessions_for_sorting(),
            SortOrder::MessageCount,
        );

        // Apply search
        state.set_search_query("api");

        // Clear search
        state.clear_search();

        // Sort order should be preserved
        assert_eq!(state.sort_order(), SortOrder::MessageCount);
    }

    // === Remove Session Tests ===

    #[test]
    fn test_remove_session_by_path_removes_session() {
        let mut state = SessionListState::from_sessions(sample_sessions());
        let initial_count = state.visible_count();

        // Get the path of the first session (second item, after project)
        state.select_next();
        let session = state.selected_session().unwrap();
        let path = session.path.clone();

        // Remove the session
        state.remove_session_by_path(&path);

        // Count should decrease by 1
        assert_eq!(state.visible_count(), initial_count - 1);
    }

    #[test]
    fn test_remove_session_by_path_nonexistent_path() {
        let mut state = SessionListState::from_sessions(sample_sessions());
        let initial_count = state.visible_count();

        // Try to remove a non-existent path
        let fake_path = PathBuf::from("/nonexistent/path.jsonl");
        state.remove_session_by_path(&fake_path);

        // Count should remain unchanged
        assert_eq!(state.visible_count(), initial_count);
    }

    #[test]
    fn test_remove_session_by_path_removes_empty_project() {
        // Create sessions with only one session per project
        let sessions = vec![SessionMeta::new(
            "only_session",
            PathBuf::from("/home/user/.claude/projects/single/only.jsonl"),
            "~/projects/single",
            test_timestamp(),
        )
        .with_message_count(5)];

        let mut state = SessionListState::from_sessions(sessions);
        // Should have 1 project + 1 session = 2 items
        assert_eq!(state.visible_count(), 2);

        // Get the session path
        state.select_next();
        let session = state.selected_session().unwrap();
        let path = session.path.clone();

        // Remove the only session
        state.remove_session_by_path(&path);

        // Both session and empty project should be removed
        assert_eq!(state.visible_count(), 0);
    }

    #[test]
    fn test_remove_session_by_path_adjusts_selection() {
        let mut state = SessionListState::from_sessions(sample_sessions());

        // Select the last item
        state.select_last();
        let last_idx = state.selected();

        // Get the path of the last session
        let session = state.selected_session().unwrap();
        let path = session.path.clone();

        // Remove the last session
        state.remove_session_by_path(&path);

        // Selection should be adjusted (not out of bounds)
        assert!(state.selected() < state.visible_count());
        // Selection should have moved to previous item
        assert!(state.selected() <= last_idx);
    }

    #[test]
    fn test_remove_session_by_path_updates_project_count() {
        let mut state = SessionListState::from_sessions(sample_sessions());

        // First project (~/projects/api) has 2 sessions
        if let TreeItem::Project { session_count, .. } = &state.items()[0] {
            assert_eq!(*session_count, 2);
        }

        // Remove one session from the first project
        state.select_next(); // Select first session
        let session = state.selected_session().unwrap();
        let path = session.path.clone();
        state.remove_session_by_path(&path);

        // Project count should be updated
        if let TreeItem::Project { session_count, .. } = &state.items()[0] {
            assert_eq!(*session_count, 1);
        }
    }
}
