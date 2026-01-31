//! Share started modal widget.
//!
//! A modal popup that displays when a share successfully starts, showing
//! the session name, public URL, and provider. Auto-dismisses after a timeout
//! or on keypress.

use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};
use std::time::{Duration, Instant};

/// Default auto-dismiss timeout for the share modal.
pub const SHARE_MODAL_TIMEOUT: Duration = Duration::from_secs(5);

/// State for the share started modal.
#[derive(Debug, Clone)]
pub struct ShareModalState {
    /// The session name being shared.
    pub session_name: String,
    /// The public URL where the session is available.
    pub public_url: String,
    /// The name of the provider being used.
    pub provider_name: String,
    /// When the modal was shown.
    pub shown_at: Instant,
    /// How long until auto-dismiss (default: 5 seconds).
    pub timeout: Duration,
}

impl ShareModalState {
    /// Create a new share modal state.
    pub fn new(session_name: String, public_url: String, provider_name: String) -> Self {
        Self {
            session_name,
            public_url,
            provider_name,
            shown_at: Instant::now(),
            timeout: SHARE_MODAL_TIMEOUT,
        }
    }

    /// Create a new share modal state with a custom timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Check if the modal should auto-dismiss (timeout elapsed).
    pub fn should_dismiss(&self) -> bool {
        self.shown_at.elapsed() >= self.timeout
    }

    /// Get the remaining time until auto-dismiss.
    pub fn remaining_time(&self) -> Duration {
        self.timeout.saturating_sub(self.shown_at.elapsed())
    }

    /// Get the remaining seconds as a whole number.
    pub fn remaining_seconds(&self) -> u64 {
        self.remaining_time().as_secs()
    }
}

/// A modal popup widget that displays when sharing starts successfully.
#[derive(Debug)]
pub struct ShareModal<'a> {
    /// The block wrapping the popup
    block: Option<Block<'a>>,
    /// Style for the URL text
    url_style: Style,
    /// Style for the session name
    session_style: Style,
    /// Style for the provider name
    provider_style: Style,
    /// Style for hint text
    hint_style: Style,
}

impl<'a> Default for ShareModal<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> ShareModal<'a> {
    /// Create a new share modal widget.
    pub fn new() -> Self {
        Self {
            block: None,
            url_style: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            session_style: Style::default().fg(Color::White),
            provider_style: Style::default().fg(Color::Yellow),
            hint_style: Style::default().fg(Color::DarkGray),
        }
    }

    /// Set the block for this widget.
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// Set the style for the URL text.
    pub fn url_style(mut self, style: Style) -> Self {
        self.url_style = style;
        self
    }

    /// Set the style for the session name.
    pub fn session_style(mut self, style: Style) -> Self {
        self.session_style = style;
        self
    }

    /// Set the style for the provider name.
    pub fn provider_style(mut self, style: Style) -> Self {
        self.provider_style = style;
        self
    }

    /// Set the style for hint text.
    pub fn hint_style(mut self, style: Style) -> Self {
        self.hint_style = style;
        self
    }
}

impl StatefulWidget for ShareModal<'_> {
    type State = ShareModalState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Calculate popup size (centered)
        let popup_width = 60.min(area.width.saturating_sub(4));
        let popup_height = 10.min(area.height.saturating_sub(4));

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
                .title(" Share Started ")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Green))
        });

        let inner_area = block.inner(popup_area);
        block.render(popup_area, buf);

        // Truncate URL if needed for display
        let display_url = if state.public_url.len() > (popup_width as usize - 4) {
            format!(
                "{}...",
                &state.public_url[..popup_width as usize - 7.min(state.public_url.len())]
            )
        } else {
            state.public_url.clone()
        };

        // Build the content
        let remaining = state.remaining_seconds();
        let auto_dismiss_text = if remaining > 0 {
            format!("(auto-close in {}s)", remaining)
        } else {
            "(closing...)".to_string()
        };

        let lines: Vec<Line> = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("Session: ", Style::default().fg(Color::DarkGray)),
                Span::styled(&state.session_name, self.session_style),
            ]),
            Line::from(vec![
                Span::styled("Provider: ", Style::default().fg(Color::DarkGray)),
                Span::styled(&state.provider_name, self.provider_style),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(display_url, self.url_style)]),
            Line::from(""),
            Line::from(vec![
                Span::styled("c", Style::default().fg(Color::Cyan)),
                Span::styled(" copy URL  ", self.hint_style),
                Span::styled("Enter/Esc", Style::default().fg(Color::Cyan)),
                Span::styled(" close  ", self.hint_style),
                Span::styled(&auto_dismiss_text, Style::default().fg(Color::DarkGray)),
            ]),
        ];

        let paragraph = Paragraph::new(lines).alignment(Alignment::Center);
        paragraph.render(inner_area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_share_modal_state_new() {
        let state = ShareModalState::new(
            "my_session".to_string(),
            "https://example.trycloudflare.com".to_string(),
            "cloudflare".to_string(),
        );

        assert_eq!(state.session_name, "my_session");
        assert_eq!(state.public_url, "https://example.trycloudflare.com");
        assert_eq!(state.provider_name, "cloudflare");
        assert_eq!(state.timeout, SHARE_MODAL_TIMEOUT);
    }

    #[test]
    fn test_share_modal_state_with_timeout() {
        let state = ShareModalState::new(
            "test".to_string(),
            "https://example.com".to_string(),
            "ngrok".to_string(),
        )
        .with_timeout(Duration::from_secs(10));

        assert_eq!(state.timeout, Duration::from_secs(10));
    }

    #[test]
    fn test_share_modal_state_should_dismiss_false_initially() {
        let state = ShareModalState::new(
            "test".to_string(),
            "https://example.com".to_string(),
            "ngrok".to_string(),
        );

        // Just created, should not dismiss yet
        assert!(!state.should_dismiss());
    }

    #[test]
    fn test_share_modal_state_should_dismiss_with_zero_timeout() {
        let state = ShareModalState::new(
            "test".to_string(),
            "https://example.com".to_string(),
            "ngrok".to_string(),
        )
        .with_timeout(Duration::from_secs(0));

        // With zero timeout, should dismiss immediately
        assert!(state.should_dismiss());
    }

    #[test]
    fn test_share_modal_state_remaining_time() {
        let state = ShareModalState::new(
            "test".to_string(),
            "https://example.com".to_string(),
            "ngrok".to_string(),
        );

        // Remaining time should be close to timeout (within 1 second)
        let remaining = state.remaining_time();
        assert!(remaining <= SHARE_MODAL_TIMEOUT);
        assert!(remaining > Duration::from_secs(4)); // Should be > 4s (just created)
    }

    #[test]
    fn test_share_modal_state_remaining_seconds() {
        let state = ShareModalState::new(
            "test".to_string(),
            "https://example.com".to_string(),
            "ngrok".to_string(),
        );

        // Just created, should be close to 5
        let secs = state.remaining_seconds();
        assert!(secs >= 4 && secs <= 5);
    }

    #[test]
    fn test_share_modal_widget_new() {
        let widget = ShareModal::new();
        assert!(widget.block.is_none());
    }

    #[test]
    fn test_share_modal_widget_default() {
        let widget = ShareModal::default();
        assert!(widget.block.is_none());
    }

    #[test]
    fn test_share_modal_widget_with_block() {
        let widget = ShareModal::new().block(Block::default().title("Custom"));
        assert!(widget.block.is_some());
    }

    #[test]
    fn test_share_modal_widget_with_styles() {
        let widget = ShareModal::new()
            .url_style(Style::default().fg(Color::Red))
            .session_style(Style::default().fg(Color::Green))
            .provider_style(Style::default().fg(Color::Blue))
            .hint_style(Style::default().fg(Color::Yellow));

        assert_eq!(widget.url_style.fg, Some(Color::Red));
        assert_eq!(widget.session_style.fg, Some(Color::Green));
        assert_eq!(widget.provider_style.fg, Some(Color::Blue));
        assert_eq!(widget.hint_style.fg, Some(Color::Yellow));
    }

    #[test]
    fn test_share_modal_render_does_not_panic() {
        let widget = ShareModal::new();
        let mut state = ShareModalState::new(
            "test".to_string(),
            "https://example.com".to_string(),
            "ngrok".to_string(),
        );
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 40));
        widget.render(Rect::new(0, 0, 80, 40), &mut buf, &mut state);
        // Should not panic
    }

    #[test]
    fn test_share_modal_render_small_area() {
        let widget = ShareModal::new();
        let mut state = ShareModalState::new(
            "test".to_string(),
            "https://example.com".to_string(),
            "ngrok".to_string(),
        );
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 15));
        widget.render(Rect::new(0, 0, 30, 15), &mut buf, &mut state);
        // Should not panic even with small area
    }

    #[test]
    fn test_share_modal_render_long_url_truncated() {
        let widget = ShareModal::new();
        let mut state = ShareModalState::new(
            "test".to_string(),
            "https://very-long-subdomain-that-will-definitely-need-truncation.trycloudflare.com/path/to/resource".to_string(),
            "cloudflare".to_string(),
        );
        let mut buf = Buffer::empty(Rect::new(0, 0, 50, 20));
        widget.render(Rect::new(0, 0, 50, 20), &mut buf, &mut state);
        // Should not panic with long URL
    }
}
