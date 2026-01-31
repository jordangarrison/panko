//! Confirmation dialog widget.
//!
//! A modal dialog for confirming destructive actions like session deletion.

use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

/// A confirmation dialog widget for destructive actions.
#[derive(Debug)]
pub struct ConfirmationDialog<'a> {
    /// The message to display (e.g., "Delete session abc123?")
    message: &'a str,
    /// The hint text (e.g., "(y/N)")
    hint: &'a str,
    /// The block wrapping the popup
    block: Option<Block<'a>>,
    /// Style for the message text
    message_style: Style,
    /// Style for the hint text
    hint_style: Style,
}

impl<'a> Default for ConfirmationDialog<'a> {
    fn default() -> Self {
        Self::new("Are you sure?", "(y/N)")
    }
}

impl<'a> ConfirmationDialog<'a> {
    /// Create a new confirmation dialog with a message and hint.
    pub fn new(message: &'a str, hint: &'a str) -> Self {
        Self {
            message,
            hint,
            block: None,
            message_style: Style::default().fg(Color::White),
            hint_style: Style::default().fg(Color::Yellow),
        }
    }

    /// Create a confirmation dialog for deleting a session.
    pub fn delete_session(session_id: &'a str) -> Self {
        Self {
            message: session_id,
            hint: "(y/N)",
            block: None,
            message_style: Style::default().fg(Color::White),
            hint_style: Style::default().fg(Color::Yellow),
        }
    }

    /// Set the block for this widget.
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// Set the style for the message text.
    pub fn message_style(mut self, style: Style) -> Self {
        self.message_style = style;
        self
    }

    /// Set the style for the hint text.
    pub fn hint_style(mut self, style: Style) -> Self {
        self.hint_style = style;
        self
    }
}

impl Widget for ConfirmationDialog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Calculate popup size (centered, fixed width)
        let popup_width = 40.min(area.width.saturating_sub(4));
        let popup_height = 5.min(area.height.saturating_sub(4)); // Title + message + hint + borders

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
                .title(" Confirm Delete ")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Red))
        });

        let inner_area = block.inner(popup_area);
        block.render(popup_area, buf);

        // Build the content
        let lines: Vec<Line> = vec![
            Line::from(vec![
                Span::styled("Delete session ", self.message_style),
                Span::styled(self.message, Style::default().fg(Color::Cyan)),
                Span::styled("?", self.message_style),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(self.hint, self.hint_style)]),
        ];

        let paragraph = Paragraph::new(lines).alignment(Alignment::Center);
        paragraph.render(inner_area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confirmation_dialog_new() {
        let dialog = ConfirmationDialog::new("Test message", "(y/n)");
        assert_eq!(dialog.message, "Test message");
        assert_eq!(dialog.hint, "(y/n)");
        assert!(dialog.block.is_none());
    }

    #[test]
    fn test_confirmation_dialog_default() {
        let dialog = ConfirmationDialog::default();
        assert_eq!(dialog.message, "Are you sure?");
        assert_eq!(dialog.hint, "(y/N)");
    }

    #[test]
    fn test_confirmation_dialog_delete_session() {
        let dialog = ConfirmationDialog::delete_session("abc123");
        assert_eq!(dialog.message, "abc123");
        assert_eq!(dialog.hint, "(y/N)");
    }

    #[test]
    fn test_confirmation_dialog_with_block() {
        let dialog =
            ConfirmationDialog::new("Test", "(y/n)").block(Block::default().title("Custom"));
        assert!(dialog.block.is_some());
    }

    #[test]
    fn test_confirmation_dialog_with_styles() {
        let dialog = ConfirmationDialog::new("Test", "(y/n)")
            .message_style(Style::default().fg(Color::Red))
            .hint_style(Style::default().fg(Color::Green));

        assert_eq!(dialog.message_style.fg, Some(Color::Red));
        assert_eq!(dialog.hint_style.fg, Some(Color::Green));
    }

    #[test]
    fn test_confirmation_dialog_render_does_not_panic() {
        let dialog = ConfirmationDialog::new("Delete session?", "(y/N)");
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 40));
        dialog.render(Rect::new(0, 0, 80, 40), &mut buf);
        // Should not panic
    }

    #[test]
    fn test_confirmation_dialog_render_small_area() {
        let dialog = ConfirmationDialog::new("Delete session?", "(y/N)");
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 10));
        dialog.render(Rect::new(0, 0, 20, 10), &mut buf);
        // Should not panic even with small area
    }
}
