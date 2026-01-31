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
    /// Copy session context to clipboard for reuse.
    /// The path is the session file to export context from.
    CopyContext(PathBuf),
    /// Share a session via a public tunnel.
    /// This triggers provider detection and selection.
    ShareSession(PathBuf),
    /// Start sharing with a specific provider.
    /// Used after the user selects a provider from the popup.
    StartSharing {
        /// Path to the session file to share.
        path: PathBuf,
        /// Provider name to use (e.g., "cloudflare", "ngrok", "tailscale").
        provider: String,
    },
    /// Stop the current sharing session.
    StopSharing,
    /// Sharing has started successfully.
    /// The main loop should call `set_sharing_active` with the URL.
    SharingStarted {
        /// The public URL where the session is available.
        url: String,
        /// The provider name.
        provider: String,
    },
    /// Copy the session file path to clipboard.
    CopyPath(PathBuf),
    /// Open the containing folder in the file manager.
    OpenFolder(PathBuf),
    /// Delete a session file.
    DeleteSession(PathBuf),
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

    #[test]
    fn test_action_start_sharing() {
        let path = PathBuf::from("/path/to/session.jsonl");
        let action = Action::StartSharing {
            path: path.clone(),
            provider: "cloudflare".to_string(),
        };
        match action {
            Action::StartSharing {
                path: p,
                provider: prov,
            } => {
                assert_eq!(p, path);
                assert_eq!(prov, "cloudflare");
            }
            _ => panic!("Expected StartSharing"),
        }
    }

    #[test]
    fn test_action_stop_sharing() {
        let action = Action::StopSharing;
        assert_eq!(action, Action::StopSharing);
    }

    #[test]
    fn test_action_sharing_started() {
        let action = Action::SharingStarted {
            url: "https://example.com".to_string(),
            provider: "cloudflare".to_string(),
        };
        match action {
            Action::SharingStarted { url, provider } => {
                assert_eq!(url, "https://example.com");
                assert_eq!(provider, "cloudflare");
            }
            _ => panic!("Expected SharingStarted"),
        }
    }

    #[test]
    fn test_action_delete_session() {
        let path = PathBuf::from("/path/to/session.jsonl");
        let action = Action::DeleteSession(path.clone());
        match action {
            Action::DeleteSession(p) => assert_eq!(p, path),
            _ => panic!("Expected DeleteSession"),
        }
    }

    #[test]
    fn test_action_copy_context() {
        let path = PathBuf::from("/path/to/session.jsonl");
        let action = Action::CopyContext(path.clone());
        match action {
            Action::CopyContext(p) => assert_eq!(p, path),
            _ => panic!("Expected CopyContext"),
        }
    }
}
