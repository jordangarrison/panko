//! Preview panel widget for displaying session details.
//!
//! This widget shows detailed information about the currently selected session
//! in a right-side panel, including session metadata, first prompt preview,
//! and tool usage statistics.

use crate::scanner::SessionMeta;
use chrono::{DateTime, Local, Utc};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget, Wrap},
};

/// Preview panel widget for displaying selected session details.
#[derive(Debug)]
pub struct PreviewPanel<'a> {
    /// The session to display, if any.
    session: Option<&'a SessionMeta>,
    /// Block to wrap this widget in.
    block: Option<Block<'a>>,
    /// Style for labels.
    label_style: Style,
    /// Style for values.
    value_style: Style,
    /// Style for the prompt preview section.
    prompt_style: Style,
}

impl<'a> Default for PreviewPanel<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> PreviewPanel<'a> {
    /// Create a new preview panel.
    pub fn new() -> Self {
        Self {
            session: None,
            block: None,
            label_style: Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
            value_style: Style::default().fg(Color::White),
            prompt_style: Style::default().fg(Color::Gray),
        }
    }

    /// Set the session to display.
    pub fn session(mut self, session: Option<&'a SessionMeta>) -> Self {
        self.session = session;
        self
    }

    /// Set the block to wrap this widget in.
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// Set the style for labels.
    pub fn label_style(mut self, style: Style) -> Self {
        self.label_style = style;
        self
    }

    /// Set the style for values.
    pub fn value_style(mut self, style: Style) -> Self {
        self.value_style = style;
        self
    }

    /// Set the style for the prompt preview.
    pub fn prompt_style(mut self, style: Style) -> Self {
        self.prompt_style = style;
        self
    }

    /// Build the lines for displaying a session.
    fn build_session_lines(&self, session: &SessionMeta, width: u16) -> Vec<Line<'a>> {
        let mut lines = vec![
            // Session ID
            Line::from(vec![
                Span::styled("Session: ", self.label_style),
                Span::styled(session.id.clone(), self.value_style),
            ]),
            // Started timestamp (using updated_at since we don't track start separately)
            Line::from(vec![
                Span::styled("Updated: ", self.label_style),
                Span::styled(format_datetime(session.updated_at), self.value_style),
            ]),
            // Blank line
            Line::from(""),
            // Full path
            Line::from(vec![Span::styled("Path:", self.label_style)]),
        ];
        let path_str = session.path.display().to_string();
        // Wrap long paths
        let wrapped_path = wrap_text(&path_str, width.saturating_sub(2) as usize);
        for path_line in wrapped_path {
            lines.push(Line::from(vec![Span::styled(
                format!("  {}", path_line),
                Style::default().fg(Color::Gray),
            )]));
        }

        // Blank line
        lines.push(Line::from(""));

        // Message count
        lines.push(Line::from(vec![
            Span::styled("Messages: ", self.label_style),
            Span::styled(session.message_count.to_string(), self.value_style),
        ]));

        // Tool usage summary (if available)
        if let Some(ref tool_usage) = session.tool_usage {
            if !tool_usage.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::from(vec![Span::styled(
                    "Tools used:",
                    self.label_style,
                )]));

                // Sort tools by count descending
                let mut tools: Vec<(&String, &usize)> = tool_usage.iter().collect();
                tools.sort_by(|a, b| b.1.cmp(a.1));

                for (tool, count) in tools.iter().take(8) {
                    // Show top 8 tools
                    lines.push(Line::from(vec![Span::styled(
                        format!("  {} ({}x)", tool, count),
                        Style::default().fg(Color::Cyan),
                    )]));
                }

                if tools.len() > 8 {
                    lines.push(Line::from(vec![Span::styled(
                        format!("  ...and {} more", tools.len() - 8),
                        Style::default().fg(Color::DarkGray),
                    )]));
                }
            }
        }

        // First prompt preview
        if let Some(ref prompt) = session.first_prompt_preview {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "First prompt:",
                self.label_style,
            )]));

            // Wrap the prompt to fit the panel
            let wrapped_prompt = wrap_text(prompt, width.saturating_sub(2) as usize);
            for prompt_line in wrapped_prompt.iter().take(10) {
                // Limit to 10 lines
                lines.push(Line::from(vec![Span::styled(
                    format!("  {}", prompt_line),
                    self.prompt_style,
                )]));
            }

            if wrapped_prompt.len() > 10 {
                lines.push(Line::from(vec![Span::styled(
                    "  ...",
                    Style::default().fg(Color::DarkGray),
                )]));
            }
        }

        lines
    }

    /// Build lines for when no session is selected.
    fn build_empty_lines(&self) -> Vec<Line<'a>> {
        vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "No session selected",
                Style::default().fg(Color::DarkGray),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Navigate to a session and",
                Style::default().fg(Color::DarkGray),
            )]),
            Line::from(vec![Span::styled(
                "it will be previewed here.",
                Style::default().fg(Color::DarkGray),
            )]),
        ]
    }
}

impl Widget for PreviewPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Calculate inner area (accounting for block borders)
        let inner_area = if let Some(ref block) = self.block {
            let inner = block.inner(area);
            block.clone().render(area, buf);
            inner
        } else {
            area
        };

        if inner_area.height == 0 || inner_area.width == 0 {
            return;
        }

        let lines = if let Some(session) = self.session {
            self.build_session_lines(session, inner_area.width)
        } else {
            self.build_empty_lines()
        };

        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });

        paragraph.render(inner_area, buf);
    }
}

/// Format a datetime for display.
fn format_datetime(dt: DateTime<Utc>) -> String {
    let local: DateTime<Local> = dt.into();
    local.format("%Y-%m-%d %H:%M").to_string()
}

/// Wrap text to fit within a specified width.
fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        if current_line.is_empty() {
            if word.len() > max_width {
                // Word is longer than max width, split it
                let mut remaining = word;
                while remaining.len() > max_width {
                    lines.push(remaining[..max_width].to_string());
                    remaining = &remaining[max_width..];
                }
                current_line = remaining.to_string();
            } else {
                current_line = word.to_string();
            }
        } else if current_line.len() + 1 + word.len() <= max_width {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(current_line);
            if word.len() > max_width {
                // Word is longer than max width, split it
                let mut remaining = word;
                while remaining.len() > max_width {
                    lines.push(remaining[..max_width].to_string());
                    remaining = &remaining[max_width..];
                }
                current_line = remaining.to_string();
            } else {
                current_line = word.to_string();
            }
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn test_timestamp() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn sample_session() -> SessionMeta {
        SessionMeta::new(
            "abc12345-def6-7890-ghij-klmnopqrstuv",
            PathBuf::from("/home/user/.claude/projects/api-server/abc12345.jsonl"),
            "~/projects/api-server",
            test_timestamp(),
        )
        .with_message_count(42)
        .with_first_prompt_preview("Help me refactor the authentication module to use JWT tokens")
    }

    fn sample_session_with_tools() -> SessionMeta {
        let mut tool_usage = HashMap::new();
        tool_usage.insert("Edit".to_string(), 15);
        tool_usage.insert("Read".to_string(), 23);
        tool_usage.insert("Bash".to_string(), 8);
        tool_usage.insert("Write".to_string(), 3);

        SessionMeta::new(
            "session-with-tools",
            PathBuf::from("/path/to/session.jsonl"),
            "~/project",
            test_timestamp(),
        )
        .with_message_count(50)
        .with_first_prompt_preview("Test prompt")
        .with_tool_usage(tool_usage)
    }

    fn minimal_session() -> SessionMeta {
        SessionMeta::new(
            "minimal",
            PathBuf::from("/path"),
            "~/proj",
            test_timestamp(),
        )
    }

    #[test]
    fn test_preview_panel_new() {
        let panel = PreviewPanel::new();
        assert!(panel.session.is_none());
        assert!(panel.block.is_none());
    }

    #[test]
    fn test_preview_panel_with_session() {
        let session = sample_session();
        let panel = PreviewPanel::new().session(Some(&session));
        assert!(panel.session.is_some());
    }

    #[test]
    fn test_preview_panel_with_block() {
        let block = Block::bordered().title("Preview");
        let panel = PreviewPanel::new().block(block);
        assert!(panel.block.is_some());
    }

    #[test]
    fn test_build_session_lines_basic() {
        let session = sample_session();
        let panel = PreviewPanel::new();
        let lines = panel.build_session_lines(&session, 50);

        // Should have session ID line
        let session_line = lines.iter().find(|l| {
            l.spans
                .iter()
                .any(|s| s.content.contains("abc12345-def6-7890-ghij-klmnopqrstuv"))
        });
        assert!(session_line.is_some());

        // Should have message count
        let msg_line = lines
            .iter()
            .find(|l| l.spans.iter().any(|s| s.content.contains("42")));
        assert!(msg_line.is_some());
    }

    #[test]
    fn test_build_session_lines_with_tools() {
        let session = sample_session_with_tools();
        let panel = PreviewPanel::new();
        let lines = panel.build_session_lines(&session, 50);

        // Should have "Tools used:" label
        let tools_label = lines
            .iter()
            .find(|l| l.spans.iter().any(|s| s.content.contains("Tools used")));
        assert!(tools_label.is_some());

        // Should have Read tool entry
        let read_line = lines.iter().find(|l| {
            l.spans
                .iter()
                .any(|s| s.content.contains("Read") && s.content.contains("23x"))
        });
        assert!(read_line.is_some());
    }

    #[test]
    fn test_build_session_lines_minimal() {
        let session = minimal_session();
        let panel = PreviewPanel::new();
        let lines = panel.build_session_lines(&session, 50);

        // Should still have session ID
        let session_line = lines
            .iter()
            .find(|l| l.spans.iter().any(|s| s.content.contains("minimal")));
        assert!(session_line.is_some());

        // Should have message count of 0
        let msg_line = lines
            .iter()
            .find(|l| l.spans.iter().any(|s| s.content == "0"));
        assert!(msg_line.is_some());
    }

    #[test]
    fn test_build_empty_lines() {
        let panel = PreviewPanel::new();
        let lines = panel.build_empty_lines();

        // Should have "No session selected" message
        let empty_msg = lines.iter().find(|l| {
            l.spans
                .iter()
                .any(|s| s.content.contains("No session selected"))
        });
        assert!(empty_msg.is_some());
    }

    #[test]
    fn test_format_datetime() {
        let dt = test_timestamp();
        let formatted = format_datetime(dt);

        // Should contain date and time
        assert!(formatted.contains("2024"));
        assert!(formatted.contains("01"));
        assert!(formatted.contains("15"));
    }

    #[test]
    fn test_wrap_text_short() {
        let text = "Hello world";
        let wrapped = wrap_text(text, 50);
        assert_eq!(wrapped.len(), 1);
        assert_eq!(wrapped[0], "Hello world");
    }

    #[test]
    fn test_wrap_text_long() {
        let text = "This is a longer piece of text that should be wrapped across multiple lines";
        let wrapped = wrap_text(text, 20);
        assert!(wrapped.len() > 1);
        for line in &wrapped {
            assert!(line.len() <= 20);
        }
    }

    #[test]
    fn test_wrap_text_very_long_word() {
        let text = "Supercalifragilisticexpialidocious";
        let wrapped = wrap_text(text, 10);
        assert!(wrapped.len() > 1);
        for line in &wrapped {
            assert!(line.len() <= 10);
        }
    }

    #[test]
    fn test_wrap_text_empty() {
        let text = "";
        let wrapped = wrap_text(text, 50);
        assert_eq!(wrapped.len(), 1);
        assert_eq!(wrapped[0], "");
    }

    #[test]
    fn test_wrap_text_zero_width() {
        let text = "Some text";
        let wrapped = wrap_text(text, 0);
        assert_eq!(wrapped.len(), 1);
        assert_eq!(wrapped[0], "Some text");
    }
}
