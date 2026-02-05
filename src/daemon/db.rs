//! SQLite persistence layer for daemon state
//!
//! Stores share information so shares can persist across daemon restarts.

use crate::daemon::protocol::{ShareId, ShareInfo, ShareStatus};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Database error types
#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Failed to create database directory: {0}")]
    CreateDir(std::io::Error),

    #[error("Invalid share status: {0}")]
    InvalidStatus(String),

    #[error("Invalid UUID: {0}")]
    InvalidUuid(#[from] uuid::Error),

    #[error("Invalid timestamp: {0}")]
    InvalidTimestamp(String),
}

/// Database handle for share persistence
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open or create a database at the default location
    /// (~/.local/share/panko/state.db)
    pub fn open_default() -> Result<Self, DatabaseError> {
        let path = default_db_path();
        Self::open(&path)
    }

    /// Open or create a database at the specified path
    pub fn open(path: &Path) -> Result<Self, DatabaseError> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(DatabaseError::CreateDir)?;
        }

        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.create_tables()?;
        Ok(db)
    }

    /// Open an in-memory database (for testing)
    pub fn open_in_memory() -> Result<Self, DatabaseError> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.create_tables()?;
        Ok(db)
    }

    /// Create database tables if they don't exist
    fn create_tables(&self) -> Result<(), DatabaseError> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS shares (
                id TEXT PRIMARY KEY,
                session_path TEXT NOT NULL,
                session_name TEXT NOT NULL,
                public_url TEXT NOT NULL,
                provider_name TEXT NOT NULL,
                local_port INTEGER NOT NULL,
                started_at TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'active'
            );

            CREATE TABLE IF NOT EXISTS daemon_state (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            "#,
        )?;
        Ok(())
    }

    /// Insert a new share into the database
    pub fn insert_share(&self, share: &ShareInfo) -> Result<(), DatabaseError> {
        self.conn.execute(
            r#"
            INSERT INTO shares (id, session_path, session_name, public_url, provider_name, local_port, started_at, status)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                share.id.to_string(),
                share.session_path.to_string_lossy(),
                share.session_name,
                share.public_url,
                share.provider_name,
                share.local_port,
                share.started_at.to_rfc3339(),
                status_to_string(share.status),
            ],
        )?;
        Ok(())
    }

    /// Update an existing share's status
    pub fn update_share_status(
        &self,
        share_id: ShareId,
        status: ShareStatus,
    ) -> Result<(), DatabaseError> {
        self.conn.execute(
            "UPDATE shares SET status = ?1 WHERE id = ?2",
            params![status_to_string(status), share_id.to_string()],
        )?;
        Ok(())
    }

    /// Update a share's public URL (useful when tunnel reconnects with new URL)
    pub fn update_share_url(
        &self,
        share_id: ShareId,
        public_url: &str,
    ) -> Result<(), DatabaseError> {
        self.conn.execute(
            "UPDATE shares SET public_url = ?1 WHERE id = ?2",
            params![public_url, share_id.to_string()],
        )?;
        Ok(())
    }

    /// Delete a share from the database
    pub fn delete_share(&self, share_id: ShareId) -> Result<(), DatabaseError> {
        self.conn.execute(
            "DELETE FROM shares WHERE id = ?1",
            params![share_id.to_string()],
        )?;
        Ok(())
    }

    /// Get a share by ID
    pub fn get_share(&self, share_id: ShareId) -> Result<Option<ShareInfo>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, session_path, session_name, public_url, provider_name, local_port, started_at, status
            FROM shares
            WHERE id = ?1
            "#,
        )?;

        let row_data: Option<ShareRowData> = stmt
            .query_row(params![share_id.to_string()], ShareRowData::from_row)
            .optional()?;

        match row_data {
            Some(data) => Ok(Some(data.into_share_info()?)),
            None => Ok(None),
        }
    }

    /// List all shares, optionally filtered by status
    pub fn list_shares(
        &self,
        status_filter: Option<ShareStatus>,
    ) -> Result<Vec<ShareInfo>, DatabaseError> {
        let mut shares = Vec::new();

        let (sql, filter_value): (&str, Option<String>) = match status_filter {
            Some(status) => (
                r#"
                SELECT id, session_path, session_name, public_url, provider_name, local_port, started_at, status
                FROM shares
                WHERE status = ?1
                ORDER BY started_at DESC
                "#,
                Some(status_to_string(status)),
            ),
            None => (
                r#"
                SELECT id, session_path, session_name, public_url, provider_name, local_port, started_at, status
                FROM shares
                ORDER BY started_at DESC
                "#,
                None,
            ),
        };

        let mut stmt = self.conn.prepare(sql)?;

        let mut rows = match filter_value {
            Some(ref val) => stmt.query([val])?,
            None => stmt.query([])?,
        };

        while let Some(row) = rows.next()? {
            let row_data = ShareRowData::from_row(row)?;
            shares.push(row_data.into_share_info()?);
        }

        Ok(shares)
    }

    /// List only active shares
    pub fn list_active_shares(&self) -> Result<Vec<ShareInfo>, DatabaseError> {
        self.list_shares(Some(ShareStatus::Active))
    }

    /// Get a daemon state value
    pub fn get_state(&self, key: &str) -> Result<Option<String>, DatabaseError> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM daemon_state WHERE key = ?1")?;

        let result: Option<String> = stmt.query_row(params![key], |row| row.get(0)).optional()?;

        Ok(result)
    }

    /// Set a daemon state value
    pub fn set_state(&self, key: &str, value: &str) -> Result<(), DatabaseError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO daemon_state (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    /// Delete a daemon state value
    pub fn delete_state(&self, key: &str) -> Result<(), DatabaseError> {
        self.conn
            .execute("DELETE FROM daemon_state WHERE key = ?1", params![key])?;
        Ok(())
    }
}

/// Get the default database path
pub fn default_db_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("panko")
        .join("state.db")
}

/// Helper struct for extracting raw row data from rusqlite
/// This avoids the error type mismatch between rusqlite::Error and DatabaseError
struct ShareRowData {
    id: String,
    session_path: String,
    session_name: String,
    public_url: String,
    provider_name: String,
    local_port: i64,
    started_at: String,
    status: String,
}

impl ShareRowData {
    fn from_row(row: &rusqlite::Row) -> Result<Self, rusqlite::Error> {
        Ok(Self {
            id: row.get(0)?,
            session_path: row.get(1)?,
            session_name: row.get(2)?,
            public_url: row.get(3)?,
            provider_name: row.get(4)?,
            local_port: row.get(5)?,
            started_at: row.get(6)?,
            status: row.get(7)?,
        })
    }

    fn into_share_info(self) -> Result<ShareInfo, DatabaseError> {
        let id: ShareId = self.id.parse()?;
        let started_at = chrono::DateTime::parse_from_rfc3339(&self.started_at)
            .map_err(|e| DatabaseError::InvalidTimestamp(e.to_string()))?
            .with_timezone(&chrono::Utc);
        let status = status_from_string(&self.status)?;

        Ok(ShareInfo {
            id,
            session_path: PathBuf::from(self.session_path),
            session_name: self.session_name,
            public_url: self.public_url,
            provider_name: self.provider_name,
            local_port: self.local_port as u16,
            started_at,
            status,
        })
    }
}

/// Convert ShareStatus to string for storage
fn status_to_string(status: ShareStatus) -> String {
    match status {
        ShareStatus::Active => "active".to_string(),
        ShareStatus::Starting => "starting".to_string(),
        ShareStatus::Error => "error".to_string(),
        ShareStatus::Stopped => "stopped".to_string(),
    }
}

/// Parse ShareStatus from string
fn status_from_string(s: &str) -> Result<ShareStatus, DatabaseError> {
    match s {
        "active" => Ok(ShareStatus::Active),
        "starting" => Ok(ShareStatus::Starting),
        "error" => Ok(ShareStatus::Error),
        "stopped" => Ok(ShareStatus::Stopped),
        other => Err(DatabaseError::InvalidStatus(other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_share(id: ShareId) -> ShareInfo {
        ShareInfo {
            id,
            session_path: PathBuf::from("/test/session.jsonl"),
            session_name: "test-session".to_string(),
            public_url: "https://test.example.com".to_string(),
            provider_name: "cloudflare".to_string(),
            local_port: 8080,
            started_at: chrono::Utc::now(),
            status: ShareStatus::Active,
        }
    }

    #[test]
    fn test_open_in_memory() {
        let db = Database::open_in_memory().expect("Failed to open in-memory database");
        assert!(db.list_shares(None).unwrap().is_empty());
    }

    #[test]
    fn test_insert_and_get_share() {
        let db = Database::open_in_memory().unwrap();
        let share = create_test_share(ShareId::new());

        db.insert_share(&share).unwrap();

        let retrieved = db.get_share(share.id).unwrap().unwrap();
        assert_eq!(retrieved.id, share.id);
        assert_eq!(retrieved.session_path, share.session_path);
        assert_eq!(retrieved.session_name, share.session_name);
        assert_eq!(retrieved.public_url, share.public_url);
        assert_eq!(retrieved.provider_name, share.provider_name);
        assert_eq!(retrieved.local_port, share.local_port);
        assert_eq!(retrieved.status, ShareStatus::Active);
    }

    #[test]
    fn test_get_nonexistent_share() {
        let db = Database::open_in_memory().unwrap();
        let result = db.get_share(ShareId::new()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_list_shares() {
        let db = Database::open_in_memory().unwrap();

        // Insert multiple shares
        let share1 = create_test_share(ShareId::new());
        let share2 = create_test_share(ShareId::new());

        db.insert_share(&share1).unwrap();
        db.insert_share(&share2).unwrap();

        let shares = db.list_shares(None).unwrap();
        assert_eq!(shares.len(), 2);
    }

    #[test]
    fn test_list_shares_with_status_filter() {
        let db = Database::open_in_memory().unwrap();

        let share1 = create_test_share(ShareId::new());
        let mut share2 = create_test_share(ShareId::new());
        share2.status = ShareStatus::Stopped;

        db.insert_share(&share1).unwrap();
        db.insert_share(&share2).unwrap();

        let active = db.list_shares(Some(ShareStatus::Active)).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, share1.id);

        let stopped = db.list_shares(Some(ShareStatus::Stopped)).unwrap();
        assert_eq!(stopped.len(), 1);
        assert_eq!(stopped[0].id, share2.id);
    }

    #[test]
    fn test_update_share_status() {
        let db = Database::open_in_memory().unwrap();
        let share = create_test_share(ShareId::new());

        db.insert_share(&share).unwrap();
        assert_eq!(
            db.get_share(share.id).unwrap().unwrap().status,
            ShareStatus::Active
        );

        db.update_share_status(share.id, ShareStatus::Stopped)
            .unwrap();
        assert_eq!(
            db.get_share(share.id).unwrap().unwrap().status,
            ShareStatus::Stopped
        );
    }

    #[test]
    fn test_update_share_url() {
        let db = Database::open_in_memory().unwrap();
        let share = create_test_share(ShareId::new());

        db.insert_share(&share).unwrap();

        db.update_share_url(share.id, "https://new-url.example.com")
            .unwrap();
        let retrieved = db.get_share(share.id).unwrap().unwrap();
        assert_eq!(retrieved.public_url, "https://new-url.example.com");
    }

    #[test]
    fn test_delete_share() {
        let db = Database::open_in_memory().unwrap();
        let share = create_test_share(ShareId::new());

        db.insert_share(&share).unwrap();
        assert!(db.get_share(share.id).unwrap().is_some());

        db.delete_share(share.id).unwrap();
        assert!(db.get_share(share.id).unwrap().is_none());
    }

    #[test]
    fn test_list_active_shares() {
        let db = Database::open_in_memory().unwrap();

        let share1 = create_test_share(ShareId::new());
        let mut share2 = create_test_share(ShareId::new());
        share2.status = ShareStatus::Error;
        let mut share3 = create_test_share(ShareId::new());
        share3.status = ShareStatus::Active;

        db.insert_share(&share1).unwrap();
        db.insert_share(&share2).unwrap();
        db.insert_share(&share3).unwrap();

        let active = db.list_active_shares().unwrap();
        assert_eq!(active.len(), 2);
    }

    #[test]
    fn test_daemon_state_operations() {
        let db = Database::open_in_memory().unwrap();

        // Initially empty
        assert!(db.get_state("test_key").unwrap().is_none());

        // Set and get
        db.set_state("test_key", "test_value").unwrap();
        assert_eq!(
            db.get_state("test_key").unwrap(),
            Some("test_value".to_string())
        );

        // Update
        db.set_state("test_key", "new_value").unwrap();
        assert_eq!(
            db.get_state("test_key").unwrap(),
            Some("new_value".to_string())
        );

        // Delete
        db.delete_state("test_key").unwrap();
        assert!(db.get_state("test_key").unwrap().is_none());
    }

    #[test]
    fn test_default_db_path() {
        let path = default_db_path();
        assert!(path.ends_with("panko/state.db"));
    }

    #[test]
    fn test_status_roundtrip() {
        let statuses = [
            ShareStatus::Active,
            ShareStatus::Starting,
            ShareStatus::Error,
            ShareStatus::Stopped,
        ];

        for status in statuses {
            let s = status_to_string(status);
            let parsed = status_from_string(&s).unwrap();
            assert_eq!(status, parsed);
        }
    }

    #[test]
    fn test_invalid_status() {
        let result = status_from_string("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_open_creates_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("nested").join("dir").join("state.db");

        // Directory doesn't exist yet
        assert!(!db_path.parent().unwrap().exists());

        // Opening should create it
        let db = Database::open(&db_path).unwrap();
        assert!(db_path.parent().unwrap().exists());

        // Should be able to use the database
        db.set_state("test", "value").unwrap();
        assert_eq!(db.get_state("test").unwrap(), Some("value".to_string()));
    }
}
