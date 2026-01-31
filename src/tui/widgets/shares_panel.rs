//! Active shares panel widget.
//!
//! A panel that displays all currently active shares with controls
//! for navigating, copying URLs, and stopping shares.

use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph},
};

use crate::tui::sharing::ActiveShare;

/// State for the shares panel.
#[derive(Debug, Default)]
pub struct SharesPanelState {
    /// Currently selected index in the shares list.
    selected: usize,
    /// List state for ratatui.
    list_state: ListState,
    /// Number of shares in the list (cached for bounds checking).
    share_count: usize,
}

impl SharesPanelState {
    /// Create a new shares panel state.
    pub fn new() -> Self {
        Self {
            selected: 0,
            list_state: ListState::default(),
            share_count: 0,
        }
    }

    /// Update the state with current shares.
    pub fn update(&mut self, shares: &[ActiveShare]) {
        self.share_count = shares.len();
        if shares.is_empty() {
            self.selected = 0;
            self.list_state.select(None);
        } else {
            // Clamp selection to valid range
            if self.selected >= shares.len() {
                self.selected = shares.len().saturating_sub(1);
            }
            self.list_state.select(Some(self.selected));
        }
    }

    /// Get the currently selected index.
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Set the selected index.
    pub fn set_selected(&mut self, index: usize) {
        self.selected = index;
        if self.share_count > 0 {
            self.list_state
                .select(Some(index.min(self.share_count.saturating_sub(1))));
        }
    }

    /// Check if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.share_count == 0
    }

    /// Get the number of shares.
    pub fn len(&self) -> usize {
        self.share_count
    }

    /// Move selection to the next item.
    pub fn select_next(&mut self) {
        if self.share_count > 0 {
            self.selected = (self.selected + 1) % self.share_count;
            self.list_state.select(Some(self.selected));
        }
    }

    /// Move selection to the previous item.
    pub fn select_previous(&mut self) {
        if self.share_count > 0 {
            self.selected = if self.selected == 0 {
                self.share_count - 1
            } else {
                self.selected - 1
            };
            self.list_state.select(Some(self.selected));
        }
    }
}

/// A panel widget that displays all active shares.
#[derive(Debug)]
pub struct SharesPanel<'a> {
    /// The shares to display.
    shares: &'a [ActiveShare],
    /// The block wrapping the panel.
    block: Option<Block<'a>>,
    /// Style for highlighted/selected items.
    highlight_style: Style,
    /// Normal item style.
    normal_style: Style,
    /// Style for URLs.
    url_style: Style,
    /// Style for provider names.
    provider_style: Style,
    /// Style for duration text.
    duration_style: Style,
}

impl<'a> Default for SharesPanel<'a> {
    fn default() -> Self {
        Self::new(&[])
    }
}

impl<'a> SharesPanel<'a> {
    /// Create a new shares panel widget.
    pub fn new(shares: &'a [ActiveShare]) -> Self {
        Self {
            shares,
            block: None,
            highlight_style: Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
            normal_style: Style::default(),
            url_style: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            provider_style: Style::default().fg(Color::Yellow),
            duration_style: Style::default().fg(Color::DarkGray),
        }
    }

    /// Set the block for this widget.
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// Set the style for highlighted items.
    pub fn highlight_style(mut self, style: Style) -> Self {
        self.highlight_style = style;
        self
    }

    /// Set the style for normal items.
    pub fn normal_style(mut self, style: Style) -> Self {
        self.normal_style = style;
        self
    }

    /// Set the style for URLs.
    pub fn url_style(mut self, style: Style) -> Self {
        self.url_style = style;
        self
    }

    /// Set the style for provider names.
    pub fn provider_style(mut self, style: Style) -> Self {
        self.provider_style = style;
        self
    }

    /// Set the style for duration text.
    pub fn duration_style(mut self, style: Style) -> Self {
        self.duration_style = style;
        self
    }

    /// Truncate a URL to fit within the given width.
    fn truncate_url(url: &str, max_width: usize) -> String {
        if url.len() <= max_width {
            url.to_string()
        } else if max_width > 3 {
            format!("{}...", &url[..max_width - 3])
        } else {
            url.chars().take(max_width).collect()
        }
    }
}

impl StatefulWidget for SharesPanel<'_> {
    type State = SharesPanelState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Calculate popup size and position (centered)
        let popup_width = 70.min(area.width.saturating_sub(4));
        let popup_height = (self.shares.len() as u16 * 3 + 8).min(area.height.saturating_sub(4));

        let popup_x = (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = (area.height.saturating_sub(popup_height)) / 2;

        let popup_area = Rect::new(
            area.x + popup_x,
            area.y + popup_y,
            popup_width,
            popup_height,
        );

        // Clear the area behind the popup
        Clear.render(popup_area, buf);

        // Render the block
        let block = self.block.unwrap_or_else(|| {
            Block::default()
                .title(" Active Shares ")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
        });

        let inner_area = block.inner(popup_area);
        block.render(popup_area, buf);

        if self.shares.is_empty() {
            // Show empty state message
            let text = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "No active shares",
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Press 's' on a session to start sharing",
                    Style::default().fg(Color::DarkGray),
                )),
            ];

            let paragraph = Paragraph::new(text).alignment(Alignment::Center);

            // Center vertically
            if inner_area.height >= 4 {
                let vertical_center = Layout::vertical([
                    Constraint::Length((inner_area.height.saturating_sub(4)) / 2),
                    Constraint::Length(4),
                    Constraint::Min(0),
                ])
                .split(inner_area)[1];
                paragraph.render(vertical_center, buf);
            } else {
                paragraph.render(inner_area, buf);
            }
            return;
        }

        // Split inner area: instructions at top, list in middle, hints at bottom
        let chunks = Layout::vertical([
            Constraint::Length(2), // Instructions
            Constraint::Min(1),    // Share list
            Constraint::Length(2), // Keyboard hints
        ])
        .split(inner_area);

        // Render instructions
        let count_text = if self.shares.len() == 1 {
            "1 active share".to_string()
        } else {
            format!("{} active shares", self.shares.len())
        };
        let instructions = Paragraph::new(count_text)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        instructions.render(chunks[0], buf);

        // Calculate available width for URL (account for padding and other text)
        let url_max_width = popup_width.saturating_sub(6) as usize;

        // Create list items
        let items: Vec<ListItem> = self
            .shares
            .iter()
            .enumerate()
            .map(|(i, share)| {
                let is_selected = i == state.selected;
                let base_style = if is_selected {
                    self.highlight_style
                } else {
                    self.normal_style
                };

                // First line: session name and provider
                let line1 = Line::from(vec![
                    Span::styled("  ", base_style),
                    Span::styled(share.session_name(), base_style),
                    Span::styled(" via ", self.duration_style),
                    Span::styled(&share.provider_name, self.provider_style),
                    Span::styled(
                        format!(" ({})", share.duration_string()),
                        self.duration_style,
                    ),
                ]);

                // Second line: URL (truncated if needed)
                let truncated_url = Self::truncate_url(&share.public_url, url_max_width);
                let line2 = Line::from(vec![
                    Span::styled("  ", base_style),
                    Span::styled(truncated_url, self.url_style),
                ]);

                ListItem::new(vec![line1, line2])
            })
            .collect();

        let list = List::new(items).highlight_style(self.highlight_style);

        // Render the list with state
        StatefulWidget::render(list, chunks[1], buf, &mut state.list_state);

        // Render keyboard hints
        let hints = Line::from(vec![
            Span::styled("j/k", Style::default().fg(Color::Cyan)),
            Span::styled(" navigate  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Enter", Style::default().fg(Color::Cyan)),
            Span::styled(" copy URL  ", Style::default().fg(Color::DarkGray)),
            Span::styled("d", Style::default().fg(Color::Cyan)),
            Span::styled(" stop  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Esc/S", Style::default().fg(Color::Cyan)),
            Span::styled(" close", Style::default().fg(Color::DarkGray)),
        ]);
        let hints_paragraph = Paragraph::new(hints).alignment(Alignment::Center);
        hints_paragraph.render(chunks[2], buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::sharing::ShareId;
    use std::path::PathBuf;

    fn sample_shares() -> Vec<ActiveShare> {
        vec![
            ActiveShare::new(
                ShareId::new(),
                PathBuf::from("/path/to/session1.jsonl"),
                "https://abc123.trycloudflare.com".to_string(),
                "cloudflare".to_string(),
            ),
            ActiveShare::new(
                ShareId::new(),
                PathBuf::from("/path/to/session2.jsonl"),
                "https://xyz789.ngrok.io".to_string(),
                "ngrok".to_string(),
            ),
        ]
    }

    #[test]
    fn test_shares_panel_state_new() {
        let state = SharesPanelState::new();
        assert_eq!(state.selected(), 0);
        assert!(state.is_empty());
        assert_eq!(state.len(), 0);
    }

    #[test]
    fn test_shares_panel_state_default() {
        let state = SharesPanelState::default();
        assert_eq!(state.selected(), 0);
        assert!(state.is_empty());
    }

    #[test]
    fn test_shares_panel_state_update_with_shares() {
        let mut state = SharesPanelState::new();
        let shares = sample_shares();
        state.update(&shares);

        assert!(!state.is_empty());
        assert_eq!(state.len(), 2);
        assert_eq!(state.selected(), 0);
    }

    #[test]
    fn test_shares_panel_state_update_empty() {
        let mut state = SharesPanelState::new();
        state.selected = 5; // Set to out-of-bounds value
        state.update(&[]);

        assert!(state.is_empty());
        assert_eq!(state.selected(), 0);
    }

    #[test]
    fn test_shares_panel_state_update_clamps_selection() {
        let mut state = SharesPanelState::new();
        state.selected = 10; // Out of bounds
        state.share_count = 10;

        let shares = sample_shares(); // Only 2 shares
        state.update(&shares);

        assert_eq!(state.selected(), 1); // Clamped to last valid index
    }

    #[test]
    fn test_shares_panel_state_select_next() {
        let mut state = SharesPanelState::new();
        let shares = sample_shares();
        state.update(&shares);

        assert_eq!(state.selected(), 0);
        state.select_next();
        assert_eq!(state.selected(), 1);
        state.select_next();
        assert_eq!(state.selected(), 0); // Wraps around
    }

    #[test]
    fn test_shares_panel_state_select_previous() {
        let mut state = SharesPanelState::new();
        let shares = sample_shares();
        state.update(&shares);

        assert_eq!(state.selected(), 0);
        state.select_previous();
        assert_eq!(state.selected(), 1); // Wraps to end
        state.select_previous();
        assert_eq!(state.selected(), 0);
    }

    #[test]
    fn test_shares_panel_state_select_next_empty() {
        let mut state = SharesPanelState::new();
        state.select_next();
        assert_eq!(state.selected(), 0); // Should not panic
    }

    #[test]
    fn test_shares_panel_state_select_previous_empty() {
        let mut state = SharesPanelState::new();
        state.select_previous();
        assert_eq!(state.selected(), 0); // Should not panic
    }

    #[test]
    fn test_shares_panel_widget_new() {
        let shares = sample_shares();
        let widget = SharesPanel::new(&shares);
        assert!(widget.block.is_none());
        assert_eq!(widget.shares.len(), 2);
    }

    #[test]
    fn test_shares_panel_widget_default() {
        let widget = SharesPanel::default();
        assert!(widget.shares.is_empty());
    }

    #[test]
    fn test_shares_panel_widget_with_block() {
        let shares = sample_shares();
        let widget = SharesPanel::new(&shares).block(Block::default().title("Test"));
        assert!(widget.block.is_some());
    }

    #[test]
    fn test_shares_panel_widget_with_styles() {
        let shares = sample_shares();
        let widget = SharesPanel::new(&shares)
            .highlight_style(Style::default().fg(Color::Red))
            .normal_style(Style::default().fg(Color::Green))
            .url_style(Style::default().fg(Color::Blue))
            .provider_style(Style::default().fg(Color::Yellow))
            .duration_style(Style::default().fg(Color::Magenta));

        assert_eq!(widget.highlight_style.fg, Some(Color::Red));
        assert_eq!(widget.normal_style.fg, Some(Color::Green));
        assert_eq!(widget.url_style.fg, Some(Color::Blue));
        assert_eq!(widget.provider_style.fg, Some(Color::Yellow));
        assert_eq!(widget.duration_style.fg, Some(Color::Magenta));
    }

    #[test]
    fn test_shares_panel_render_does_not_panic() {
        let shares = sample_shares();
        let widget = SharesPanel::new(&shares);
        let mut state = SharesPanelState::new();
        state.update(&shares);

        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 40));
        widget.render(Rect::new(0, 0, 80, 40), &mut buf, &mut state);
        // Should not panic
    }

    #[test]
    fn test_shares_panel_render_empty_does_not_panic() {
        let widget = SharesPanel::new(&[]);
        let mut state = SharesPanelState::new();

        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 40));
        widget.render(Rect::new(0, 0, 80, 40), &mut buf, &mut state);
        // Should not panic
    }

    #[test]
    fn test_shares_panel_render_small_area() {
        let shares = sample_shares();
        let widget = SharesPanel::new(&shares);
        let mut state = SharesPanelState::new();
        state.update(&shares);

        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 20));
        widget.render(Rect::new(0, 0, 40, 20), &mut buf, &mut state);
        // Should not panic even with small area
    }

    #[test]
    fn test_truncate_url_short() {
        let url = "https://example.com";
        let truncated = SharesPanel::truncate_url(url, 50);
        assert_eq!(truncated, url);
    }

    #[test]
    fn test_truncate_url_long() {
        let url = "https://very-long-subdomain-that-needs-truncation.trycloudflare.com/path";
        let truncated = SharesPanel::truncate_url(url, 30);
        assert_eq!(truncated.len(), 30);
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn test_truncate_url_very_short_max() {
        let url = "https://example.com";
        let truncated = SharesPanel::truncate_url(url, 3);
        assert_eq!(truncated, "htt");
    }
}
