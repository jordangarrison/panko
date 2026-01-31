//! IPC protocol types for daemon-TUI communication
//!
//! Messages are sent as newline-delimited JSON over Unix sockets.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Unique identifier for a share session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ShareId(pub Uuid);

impl ShareId {
    /// Generate a new random share ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ShareId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ShareId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for ShareId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

/// Information about an active share
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareInfo {
    pub id: ShareId,
    pub session_path: PathBuf,
    pub session_name: String,
    pub public_url: String,
    pub provider_name: String,
    pub local_port: u16,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub status: ShareStatus,
}

/// Status of a share
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShareStatus {
    /// Share is active and serving
    Active,
    /// Share is starting up
    Starting,
    /// Share encountered an error
    Error,
    /// Share has been stopped
    Stopped,
}

/// Requests sent from TUI to daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum DaemonRequest {
    /// Start a new share for a session
    StartShare {
        session_path: PathBuf,
        provider: String,
    },
    /// Stop an existing share
    StopShare { share_id: ShareId },
    /// List all active shares
    ListShares,
    /// Check if daemon is alive
    Ping,
    /// Request daemon shutdown
    Shutdown,
}

/// Responses sent from daemon to TUI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", content = "data")]
pub enum DaemonResponse {
    /// Share started successfully
    ShareStarted(ShareInfo),
    /// Share stopped successfully
    ShareStopped { share_id: ShareId },
    /// List of all shares
    ShareList(Vec<ShareInfo>),
    /// Pong response
    Pong,
    /// Shutdown acknowledged
    ShuttingDown,
    /// Error occurred
    Error { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_id_generation() {
        let id1 = ShareId::new();
        let id2 = ShareId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn share_id_parse_roundtrip() {
        let id = ShareId::new();
        let s = id.to_string();
        let parsed: ShareId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn request_serialization() {
        let req = DaemonRequest::StartShare {
            session_path: PathBuf::from("/path/to/session.jsonl"),
            provider: "cloudflare".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("StartShare"));
        assert!(json.contains("session_path"));

        let parsed: DaemonRequest = serde_json::from_str(&json).unwrap();
        match parsed {
            DaemonRequest::StartShare {
                session_path,
                provider,
            } => {
                assert_eq!(session_path, PathBuf::from("/path/to/session.jsonl"));
                assert_eq!(provider, "cloudflare");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn response_serialization() {
        let info = ShareInfo {
            id: ShareId::new(),
            session_path: PathBuf::from("/path/to/session.jsonl"),
            session_name: "test-session".to_string(),
            public_url: "https://example.trycloudflare.com".to_string(),
            provider_name: "cloudflare".to_string(),
            local_port: 8080,
            started_at: chrono::Utc::now(),
            status: ShareStatus::Active,
        };
        let resp = DaemonResponse::ShareStarted(info);
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("ShareStarted"));

        let parsed: DaemonResponse = serde_json::from_str(&json).unwrap();
        match parsed {
            DaemonResponse::ShareStarted(share) => {
                assert_eq!(share.session_name, "test-session");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn error_response_serialization() {
        let resp = DaemonResponse::Error {
            message: "Something went wrong".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("Error"));
        assert!(json.contains("Something went wrong"));
    }

    #[test]
    fn share_status_serialization() {
        assert_eq!(
            serde_json::to_string(&ShareStatus::Active).unwrap(),
            "\"active\""
        );
        assert_eq!(
            serde_json::to_string(&ShareStatus::Starting).unwrap(),
            "\"starting\""
        );
        assert_eq!(
            serde_json::to_string(&ShareStatus::Error).unwrap(),
            "\"error\""
        );
        assert_eq!(
            serde_json::to_string(&ShareStatus::Stopped).unwrap(),
            "\"stopped\""
        );
    }
}
