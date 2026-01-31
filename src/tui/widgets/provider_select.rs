//! Provider selection popup widget.
//!
//! A modal dialog for selecting a tunnel provider when sharing a session.

use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph},
};

/// Information about an available provider for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderOption {
    /// Internal name (e.g., "cloudflare", "ngrok", "tailscale")
    pub name: String,
    /// Display name for the UI
    pub display_name: String,
}

impl ProviderOption {
    /// Create a new provider option.
    pub fn new(name: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            display_name: display_name.into(),
        }
    }
}

/// State for the provider selection popup.
#[derive(Debug, Default)]
pub struct ProviderSelectState {
    /// Available providers
    providers: Vec<ProviderOption>,
    /// Currently selected index
    selected: usize,
    /// List state for ratatui
    list_state: ListState,
}

impl ProviderSelectState {
    /// Create a new provider selection state with the given providers.
    pub fn new(providers: Vec<ProviderOption>) -> Self {
        let mut list_state = ListState::default();
        if !providers.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            providers,
            selected: 0,
            list_state,
        }
    }

    /// Get the available providers.
    pub fn providers(&self) -> &[ProviderOption] {
        &self.providers
    }

    /// Get the currently selected index.
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Get the currently selected provider.
    pub fn selected_provider(&self) -> Option<&ProviderOption> {
        self.providers.get(self.selected)
    }

    /// Move selection to the next item.
    pub fn select_next(&mut self) {
        if !self.providers.is_empty() {
            self.selected = (self.selected + 1) % self.providers.len();
            self.list_state.select(Some(self.selected));
        }
    }

    /// Move selection to the previous item.
    pub fn select_previous(&mut self) {
        if !self.providers.is_empty() {
            self.selected = if self.selected == 0 {
                self.providers.len() - 1
            } else {
                self.selected - 1
            };
            self.list_state.select(Some(self.selected));
        }
    }

    /// Check if there are any providers.
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// Get the number of providers.
    pub fn len(&self) -> usize {
        self.providers.len()
    }
}

/// A popup widget for selecting a tunnel provider.
#[derive(Debug)]
pub struct ProviderSelect<'a> {
    /// The block wrapping the popup
    block: Option<Block<'a>>,
    /// Style for highlighted/selected items
    highlight_style: Style,
    /// Normal item style
    normal_style: Style,
}

impl<'a> Default for ProviderSelect<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> ProviderSelect<'a> {
    /// Create a new provider select widget.
    pub fn new() -> Self {
        Self {
            block: None,
            highlight_style: Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
            normal_style: Style::default(),
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
}

impl StatefulWidget for ProviderSelect<'_> {
    type State = ProviderSelectState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Calculate popup size and position (centered)
        let popup_width = 40.min(area.width.saturating_sub(4));
        let popup_height = (state.len() as u16 + 6).min(area.height.saturating_sub(4));

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
                .title(" Select Tunnel Provider ")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
        });

        let inner_area = block.inner(popup_area);
        block.render(popup_area, buf);

        if state.is_empty() {
            // Show message when no providers are available
            let text = Paragraph::new(
                "No tunnel providers available.\nInstall cloudflared, ngrok, or tailscale.",
            )
            .style(Style::default().fg(Color::Red))
            .alignment(Alignment::Center);
            text.render(inner_area, buf);
            return;
        }

        // Split inner area: instructions at top, list below
        let chunks = Layout::vertical([
            Constraint::Length(2), // Instructions
            Constraint::Min(1),    // Provider list
        ])
        .split(inner_area);

        // Render instructions
        let instructions = Paragraph::new("↑/↓ or j/k to select, Enter to confirm, Esc to cancel")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        instructions.render(chunks[0], buf);

        // Create list items
        let items: Vec<ListItem> = state
            .providers
            .iter()
            .enumerate()
            .map(|(i, provider)| {
                let style = if i == state.selected {
                    self.highlight_style
                } else {
                    self.normal_style
                };
                ListItem::new(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(&provider.display_name, style),
                ]))
            })
            .collect();

        let list = List::new(items).highlight_style(self.highlight_style);

        // Render the list with state
        StatefulWidget::render(list, chunks[1], buf, &mut state.list_state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_providers() -> Vec<ProviderOption> {
        vec![
            ProviderOption::new("cloudflare", "Cloudflare Quick Tunnel"),
            ProviderOption::new("ngrok", "ngrok"),
            ProviderOption::new("tailscale", "Tailscale Serve"),
        ]
    }

    #[test]
    fn test_provider_option_new() {
        let option = ProviderOption::new("cloudflare", "Cloudflare Quick Tunnel");
        assert_eq!(option.name, "cloudflare");
        assert_eq!(option.display_name, "Cloudflare Quick Tunnel");
    }

    #[test]
    fn test_provider_select_state_new() {
        let state = ProviderSelectState::new(sample_providers());
        assert_eq!(state.len(), 3);
        assert_eq!(state.selected(), 0);
        assert!(!state.is_empty());
    }

    #[test]
    fn test_provider_select_state_empty() {
        let state = ProviderSelectState::new(vec![]);
        assert!(state.is_empty());
        assert_eq!(state.len(), 0);
        assert!(state.selected_provider().is_none());
    }

    #[test]
    fn test_provider_select_state_select_next() {
        let mut state = ProviderSelectState::new(sample_providers());
        assert_eq!(state.selected(), 0);

        state.select_next();
        assert_eq!(state.selected(), 1);

        state.select_next();
        assert_eq!(state.selected(), 2);

        // Wrap around
        state.select_next();
        assert_eq!(state.selected(), 0);
    }

    #[test]
    fn test_provider_select_state_select_previous() {
        let mut state = ProviderSelectState::new(sample_providers());
        assert_eq!(state.selected(), 0);

        // Wrap to end
        state.select_previous();
        assert_eq!(state.selected(), 2);

        state.select_previous();
        assert_eq!(state.selected(), 1);
    }

    #[test]
    fn test_provider_select_state_selected_provider() {
        let state = ProviderSelectState::new(sample_providers());
        let provider = state.selected_provider().unwrap();
        assert_eq!(provider.name, "cloudflare");
    }

    #[test]
    fn test_provider_select_default() {
        let state = ProviderSelectState::default();
        assert!(state.is_empty());
    }

    #[test]
    fn test_provider_select_widget_new() {
        let widget = ProviderSelect::new();
        assert!(widget.block.is_none());
    }

    #[test]
    fn test_provider_select_widget_with_block() {
        let widget = ProviderSelect::new().block(Block::default().title("Test"));
        assert!(widget.block.is_some());
    }

    #[test]
    fn test_select_next_on_empty_does_not_panic() {
        let mut state = ProviderSelectState::new(vec![]);
        state.select_next();
        assert_eq!(state.selected(), 0);
    }

    #[test]
    fn test_select_previous_on_empty_does_not_panic() {
        let mut state = ProviderSelectState::new(vec![]);
        state.select_previous();
        assert_eq!(state.selected(), 0);
    }
}
