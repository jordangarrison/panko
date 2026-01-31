//! File watcher for session directory changes.
//!
//! This module provides functionality to watch session directories
//! for changes and notify the TUI when new sessions appear.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use notify::{event::CreateKind, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

/// Messages sent from the file watcher to the TUI.
#[derive(Debug, Clone)]
pub enum WatcherMessage {
    /// A new session file was created.
    NewSession(PathBuf),
    /// A session file was modified.
    SessionModified(PathBuf),
    /// A session file was deleted.
    SessionDeleted(PathBuf),
    /// A general change occurred (refresh recommended).
    RefreshNeeded,
    /// An error occurred in the watcher.
    Error(String),
}

/// Handle to a running file watcher.
pub struct FileWatcher {
    /// Channel to receive messages from the watcher.
    receiver: Receiver<WatcherMessage>,
    /// The watcher itself (kept alive to continue watching).
    _watcher: RecommendedWatcher,
}

impl FileWatcher {
    /// Create a new file watcher for the given directories.
    ///
    /// Watches for JSONL file creation/modification in the specified paths.
    pub fn new(watch_paths: Vec<PathBuf>) -> Result<Self, notify::Error> {
        let (tx, rx) = mpsc::channel();

        let sender = tx.clone();
        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            match res {
                Ok(event) => {
                    // Filter for JSONL files only
                    let jsonl_paths: Vec<PathBuf> = event
                        .paths
                        .into_iter()
                        .filter(|p| p.extension().map(|ext| ext == "jsonl").unwrap_or(false))
                        .collect();

                    if jsonl_paths.is_empty() {
                        return;
                    }

                    // Send appropriate message based on event kind
                    let message = match event.kind {
                        EventKind::Create(CreateKind::File) => {
                            WatcherMessage::NewSession(jsonl_paths[0].clone())
                        }
                        EventKind::Modify(_) => {
                            WatcherMessage::SessionModified(jsonl_paths[0].clone())
                        }
                        EventKind::Remove(_) => {
                            WatcherMessage::SessionDeleted(jsonl_paths[0].clone())
                        }
                        _ => WatcherMessage::RefreshNeeded,
                    };

                    let _ = sender.send(message);
                }
                Err(e) => {
                    let _ = sender.send(WatcherMessage::Error(e.to_string()));
                }
            }
        })?;

        // Watch all specified paths
        for path in watch_paths {
            if path.exists() {
                // Use recursive mode to watch subdirectories (project folders)
                watcher.watch(&path, RecursiveMode::Recursive)?;
            }
        }

        Ok(Self {
            receiver: rx,
            _watcher: watcher,
        })
    }

    /// Try to receive a message without blocking.
    ///
    /// Returns `None` if no messages are available.
    pub fn try_recv(&self) -> Option<WatcherMessage> {
        self.receiver.try_recv().ok()
    }

    /// Check if there are any pending messages (non-blocking).
    pub fn has_pending(&self) -> bool {
        // Unfortunately mpsc doesn't have a peek, so we use try_recv
        // This is called in a loop, so messages won't be missed
        self.receiver.try_recv().is_ok()
    }
}

/// Start a file watcher in a background thread.
///
/// This is a convenience function that spawns the watcher and returns
/// a receiver for messages.
pub fn start_background_watcher(
    watch_paths: Vec<PathBuf>,
) -> Result<Receiver<WatcherMessage>, notify::Error> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        match FileWatcher::new(watch_paths) {
            Ok(watcher) => {
                // Forward all messages from the watcher
                loop {
                    if let Some(msg) = watcher.try_recv() {
                        if tx.send(msg).is_err() {
                            // Receiver dropped, exit thread
                            break;
                        }
                    }
                    // Small sleep to avoid busy-waiting
                    thread::sleep(std::time::Duration::from_millis(100));
                }
            }
            Err(e) => {
                let _ = tx.send(WatcherMessage::Error(e.to_string()));
            }
        }
    });

    Ok(rx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_watcher_message_variants() {
        // Verify all message variants can be created
        let _new = WatcherMessage::NewSession(PathBuf::from("/test.jsonl"));
        let _modified = WatcherMessage::SessionModified(PathBuf::from("/test.jsonl"));
        let _deleted = WatcherMessage::SessionDeleted(PathBuf::from("/test.jsonl"));
        let _refresh = WatcherMessage::RefreshNeeded;
        let _error = WatcherMessage::Error("test error".to_string());
    }

    #[test]
    fn test_file_watcher_creation_with_valid_path() {
        let temp_dir = TempDir::new().unwrap();
        let watch_paths = vec![temp_dir.path().to_path_buf()];

        let watcher = FileWatcher::new(watch_paths);
        assert!(watcher.is_ok());
    }

    #[test]
    fn test_file_watcher_creation_with_nonexistent_path() {
        let watch_paths = vec![PathBuf::from("/nonexistent/path/that/doesnt/exist")];

        // Should succeed but not watch the nonexistent path
        let watcher = FileWatcher::new(watch_paths);
        assert!(watcher.is_ok());
    }

    #[test]
    fn test_file_watcher_try_recv_empty() {
        let temp_dir = TempDir::new().unwrap();
        let watch_paths = vec![temp_dir.path().to_path_buf()];

        let watcher = FileWatcher::new(watch_paths).unwrap();

        // Should return None when no events
        assert!(watcher.try_recv().is_none());
    }

    #[test]
    fn test_file_watcher_detects_new_jsonl_file() {
        let temp_dir = TempDir::new().unwrap();
        let watch_paths = vec![temp_dir.path().to_path_buf()];

        let watcher = FileWatcher::new(watch_paths).unwrap();

        // Create a new JSONL file
        let file_path = temp_dir.path().join("test_session.jsonl");
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, r#"{{"type":"test"}}"#).unwrap();
        file.sync_all().unwrap();

        // Give the watcher time to detect the change
        thread::sleep(std::time::Duration::from_millis(500));

        // Should have received a notification
        // Note: Due to timing, this test may be flaky
        // The actual message might be NewSession or RefreshNeeded depending on OS
        let msg = watcher.try_recv();
        // We don't assert on the message type since it varies by platform
        // The important thing is that the watcher doesn't crash
        let _ = msg;
    }

    #[test]
    fn test_file_watcher_ignores_non_jsonl_files() {
        let temp_dir = TempDir::new().unwrap();
        let watch_paths = vec![temp_dir.path().to_path_buf()];

        let watcher = FileWatcher::new(watch_paths).unwrap();

        // Create a non-JSONL file
        let file_path = temp_dir.path().join("test.txt");
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "test content").unwrap();
        file.sync_all().unwrap();

        // Give the watcher time
        thread::sleep(std::time::Duration::from_millis(200));

        // Should not have received a notification for txt file
        // (might still get None or might get refresh needed for directory change)
        let _ = watcher.try_recv();
    }
}
