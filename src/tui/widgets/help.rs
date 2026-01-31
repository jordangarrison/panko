//! Help overlay widget.
//!
//! A modal overlay that displays all keyboard shortcuts grouped by category.

use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

/// A category of keyboard shortcuts for the help overlay.
#[derive(Debug, Clone)]
pub struct ShortcutCategory {
    /// Category name (e.g., "Navigation", "Actions")
    pub name: &'static str,
    /// Shortcuts in this category: (key, description)
    pub shortcuts: Vec<(&'static str, &'static str)>,
}

/// Get all the shortcut categories for the help overlay.
pub fn get_shortcut_categories() -> Vec<ShortcutCategory> {
    vec![
        ShortcutCategory {
            name: "Navigation",
            shortcuts: vec![
                ("j / ↓", "Move down"),
                ("k / ↑", "Move up"),
                ("h / ←", "Collapse / Go to parent"),
                ("l / →", "Expand / Select child"),
                ("g", "Go to first item"),
                ("G", "Go to last item"),
                ("Tab", "Switch panel focus"),
            ],
        },
        ShortcutCategory {
            name: "Search",
            shortcuts: vec![
                ("/", "Start search"),
                ("Enter", "Confirm search"),
                ("Esc", "Clear search"),
            ],
        },
        ShortcutCategory {
            name: "Actions",
            shortcuts: vec![
                ("v / Enter", "View session"),
                ("s", "Share session"),
                ("S", "Show active shares"),
                ("c", "Copy session path"),
                ("C", "Copy context to clipboard"),
                ("D", "Download to ~/Downloads"),
                ("o", "Open folder"),
                ("d", "Delete session"),
                ("r", "Refresh list"),
            ],
        },
        ShortcutCategory {
            name: "General",
            shortcuts: vec![
                ("?", "Toggle help"),
                ("q / Esc", "Quit"),
                ("Ctrl+C", "Quit"),
            ],
        },
    ]
}

/// A modal overlay widget for displaying keyboard shortcuts.
#[derive(Debug)]
pub struct HelpOverlay<'a> {
    /// The block wrapping the popup
    block: Option<Block<'a>>,
    /// Style for category headers
    header_style: Style,
    /// Style for shortcut keys
    key_style: Style,
    /// Style for shortcut descriptions
    description_style: Style,
}

impl<'a> Default for HelpOverlay<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> HelpOverlay<'a> {
    /// Create a new help overlay widget.
    pub fn new() -> Self {
        Self {
            block: None,
            header_style: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            key_style: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            description_style: Style::default().fg(Color::White),
        }
    }

    /// Set the block for this widget.
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// Set the style for category headers.
    pub fn header_style(mut self, style: Style) -> Self {
        self.header_style = style;
        self
    }

    /// Set the style for shortcut keys.
    pub fn key_style(mut self, style: Style) -> Self {
        self.key_style = style;
        self
    }

    /// Set the style for shortcut descriptions.
    pub fn description_style(mut self, style: Style) -> Self {
        self.description_style = style;
        self
    }
}

impl Widget for HelpOverlay<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Get shortcut categories
        let categories = get_shortcut_categories();

        // Calculate content height (categories + shortcuts + spacing)
        let content_height: u16 = categories
            .iter()
            .map(|c| 2 + c.shortcuts.len() as u16) // header line + blank + shortcuts
            .sum::<u16>()
            + 3; // Instructions at bottom

        // Calculate popup size (centered, max 60 width, content height + border)
        let popup_width = 50.min(area.width.saturating_sub(4));
        let popup_height = (content_height + 2).min(area.height.saturating_sub(4)); // +2 for borders

        let popup_x = (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = (area.height.saturating_sub(popup_height)) / 2;

        let popup_area = Rect::new(
            area.x + popup_x,
            area.y + popup_y,
            popup_width,
            popup_height,
        );

        // Clear the area behind the popup (semi-transparent effect via clearing)
        Clear.render(popup_area, buf);

        // Render the block
        let block = self.block.unwrap_or_else(|| {
            Block::default()
                .title(" Keyboard Shortcuts ")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
        });

        let inner_area = block.inner(popup_area);
        block.render(popup_area, buf);

        // Build the content
        let mut lines: Vec<Line> = Vec::new();

        for category in &categories {
            // Category header
            lines.push(Line::from(vec![Span::styled(
                category.name,
                self.header_style,
            )]));

            // Shortcuts
            for (key, desc) in &category.shortcuts {
                let key_width = 14; // Fixed width for alignment
                let padded_key = format!("  {:<width$}", key, width = key_width);
                lines.push(Line::from(vec![
                    Span::styled(padded_key, self.key_style),
                    Span::styled(*desc, self.description_style),
                ]));
            }

            // Blank line between categories
            lines.push(Line::from(""));
        }

        // Add instructions at the bottom
        lines.push(Line::from(vec![Span::styled(
            "Press any key to close",
            Style::default().fg(Color::DarkGray),
        )]));

        let paragraph = Paragraph::new(lines);
        paragraph.render(inner_area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_shortcut_categories_returns_categories() {
        let categories = get_shortcut_categories();
        assert!(!categories.is_empty());

        // Should have at least Navigation, Search, Actions, and General
        let names: Vec<&str> = categories.iter().map(|c| c.name).collect();
        assert!(names.contains(&"Navigation"));
        assert!(names.contains(&"Search"));
        assert!(names.contains(&"Actions"));
        assert!(names.contains(&"General"));
    }

    #[test]
    fn test_shortcut_categories_have_shortcuts() {
        let categories = get_shortcut_categories();
        for category in &categories {
            assert!(
                !category.shortcuts.is_empty(),
                "Category '{}' should have shortcuts",
                category.name
            );
        }
    }

    #[test]
    fn test_help_overlay_new() {
        let overlay = HelpOverlay::new();
        assert!(overlay.block.is_none());
    }

    #[test]
    fn test_help_overlay_default() {
        let overlay = HelpOverlay::default();
        assert!(overlay.block.is_none());
    }

    #[test]
    fn test_help_overlay_with_block() {
        let overlay = HelpOverlay::new().block(Block::default().title("Test"));
        assert!(overlay.block.is_some());
    }

    #[test]
    fn test_help_overlay_with_styles() {
        let overlay = HelpOverlay::new()
            .header_style(Style::default().fg(Color::Red))
            .key_style(Style::default().fg(Color::Green))
            .description_style(Style::default().fg(Color::Blue));

        assert_eq!(overlay.header_style.fg, Some(Color::Red));
        assert_eq!(overlay.key_style.fg, Some(Color::Green));
        assert_eq!(overlay.description_style.fg, Some(Color::Blue));
    }

    #[test]
    fn test_help_overlay_render_does_not_panic() {
        let overlay = HelpOverlay::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 40));
        overlay.render(Rect::new(0, 0, 80, 40), &mut buf);
        // Should not panic
    }

    #[test]
    fn test_help_overlay_render_small_area() {
        let overlay = HelpOverlay::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 10));
        overlay.render(Rect::new(0, 0, 20, 10), &mut buf);
        // Should not panic even with small area
    }
}
