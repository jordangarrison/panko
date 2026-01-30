//! Actions that can be triggered from the TUI.
//!
//! Actions represent operations that need to be handled outside the TUI event loop,
//! such as launching the web viewer which requires async I/O.

use std::path::PathBuf;

/// Actions that can be triggered from the TUI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Action {
    /// View a session in the web browser.
    /// The path is the session file to view.
    ViewSession(PathBuf),
    /// Share a session via a public tunnel.
    /// The path is the session file to share.
    ShareSession(PathBuf),
    /// Copy the session file path to clipboard.
    CopyPath(PathBuf),
    /// Open the containing folder in the file manager.
    OpenFolder(PathBuf),
    /// No action to perform.
    #[default]
    None,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_default() {
        let action = Action::default();
        assert_eq!(action, Action::None);
    }

    #[test]
    fn test_action_view_session() {
        let path = PathBuf::from("/path/to/session.jsonl");
        let action = Action::ViewSession(path.clone());
        match action {
            Action::ViewSession(p) => assert_eq!(p, path),
            _ => panic!("Expected ViewSession"),
        }
    }

    #[test]
    fn test_action_share_session() {
        let path = PathBuf::from("/path/to/session.jsonl");
        let action = Action::ShareSession(path.clone());
        match action {
            Action::ShareSession(p) => assert_eq!(p, path),
            _ => panic!("Expected ShareSession"),
        }
    }

    #[test]
    fn test_action_copy_path() {
        let path = PathBuf::from("/path/to/session.jsonl");
        let action = Action::CopyPath(path.clone());
        match action {
            Action::CopyPath(p) => assert_eq!(p, path),
            _ => panic!("Expected CopyPath"),
        }
    }

    #[test]
    fn test_action_open_folder() {
        let path = PathBuf::from("/path/to/session.jsonl");
        let action = Action::OpenFolder(path.clone());
        match action {
            Action::OpenFolder(p) => assert_eq!(p, path),
            _ => panic!("Expected OpenFolder"),
        }
    }

    #[test]
    fn test_action_debug() {
        let action = Action::None;
        let debug_str = format!("{:?}", action);
        assert!(debug_str.contains("None"));
    }
}
