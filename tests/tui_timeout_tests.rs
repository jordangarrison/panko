//! Timeout and hang detection tests for TUI operations.
//!
//! These tests verify that TUI operations complete quickly and don't hang.

use chrono::{TimeZone, Utc};
use panko::scanner::SessionMeta;
use panko::tui::widgets::ProviderOption;
use panko::tui::{App, SharingState};
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn test_timestamp() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).single().unwrap()
}

fn sample_sessions() -> Vec<SessionMeta> {
    vec![
        SessionMeta::new(
            "abc12345",
            PathBuf::from("/home/user/project/.claude/abc12345.jsonl"),
            "/home/user/project",
            test_timestamp(),
        ),
        SessionMeta::new(
            "def67890",
            PathBuf::from("/home/user/project/.claude/def67890.jsonl"),
            "/home/user/project",
            test_timestamp(),
        ),
    ]
}

/// Maximum time allowed for operations (in milliseconds).
const MAX_OPERATION_TIME_MS: u128 = 100;

#[test]
fn test_app_state_operations_complete_quickly() {
    let start = Instant::now();

    // Create app with sessions
    let mut app = App::with_sessions(sample_sessions());
    let creation_time = start.elapsed();
    assert!(
        creation_time.as_millis() < MAX_OPERATION_TIME_MS,
        "App creation took too long: {:?}",
        creation_time
    );

    // Test sharing state transitions
    let start = Instant::now();
    for _ in 0..100 {
        app.set_sharing_active("https://test.url".to_string(), "test".to_string());
        app.clear_sharing_state();
    }
    let transition_time = start.elapsed();
    assert!(
        transition_time.as_millis() < MAX_OPERATION_TIME_MS,
        "100 sharing state transitions took too long: {:?}",
        transition_time
    );

    // Test session list navigation
    let start = Instant::now();
    for _ in 0..100 {
        app.session_list_state_mut().select_next();
        app.session_list_state_mut().select_previous();
    }
    let navigation_time = start.elapsed();
    assert!(
        navigation_time.as_millis() < MAX_OPERATION_TIME_MS,
        "100 navigation operations took too long: {:?}",
        navigation_time
    );
}

#[test]
fn test_channel_operations_non_blocking() {
    use std::sync::mpsc;

    // Test that channel try_recv is non-blocking
    let (_tx, rx) = mpsc::channel::<String>();

    let start = Instant::now();
    for _ in 0..10000 {
        let _ = rx.try_recv();
    }
    let elapsed = start.elapsed();

    // 10000 try_recv calls should complete in under 50ms
    assert!(
        elapsed.as_millis() < 50,
        "10000 try_recv calls took too long: {:?}",
        elapsed
    );
}

#[test]
fn test_sharing_state_changes_fast() {
    let mut app = App::new();
    let path = PathBuf::from("/test.jsonl");
    let providers = vec![ProviderOption::new("cloudflare", "Cloudflare")];

    let start = Instant::now();

    // Full state machine cycle
    for _ in 0..100 {
        // Inactive -> SelectingProvider
        app.start_provider_selection(path.clone(), providers.clone());
        assert!(app.sharing_state().is_selecting_provider());

        // SelectingProvider -> Inactive (cancel)
        app.clear_sharing_state();
        assert!(!app.sharing_state().is_busy());

        // Inactive -> Active
        app.set_sharing_active("https://test.url".to_string(), "test".to_string());
        assert!(app.sharing_state().is_active());

        // Active -> Inactive
        app.clear_sharing_state();
        assert!(matches!(app.sharing_state(), &SharingState::Inactive));
    }

    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < MAX_OPERATION_TIME_MS,
        "100 full state machine cycles took too long: {:?}",
        elapsed
    );
}

#[test]
fn test_session_list_operations_fast() {
    let mut app = App::with_sessions(sample_sessions());

    let start = Instant::now();

    // Repeated session list operations
    for _ in 0..100 {
        // Selection operations
        app.session_list_state_mut().select_next();
        app.session_list_state_mut().select_previous();
        app.session_list_state_mut().select_first();
        app.session_list_state_mut().select_last();

        // State queries
        let _ = app.session_list_state().selected();
        let _ = app.selected_session();
    }

    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < MAX_OPERATION_TIME_MS,
        "Session list operations took too long: {:?}",
        elapsed
    );
}

#[test]
fn test_status_message_operations_fast() {
    let mut app = App::new();

    let start = Instant::now();

    for _ in 0..1000 {
        app.set_status_message("Test status message with some content");
        let _ = app.status_message();
        app.clear_status_message();
    }

    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < MAX_OPERATION_TIME_MS,
        "1000 status message operations took too long: {:?}",
        elapsed
    );
}

#[test]
fn test_provider_options_creation_fast() {
    let start = Instant::now();

    // Create many provider options
    let _providers: Vec<ProviderOption> = (0..1000)
        .map(|i| ProviderOption::new(&format!("provider{}", i), &format!("Provider {}", i)))
        .collect();

    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < MAX_OPERATION_TIME_MS,
        "Creating 1000 provider options took too long: {:?}",
        elapsed
    );
}

#[test]
fn test_timeout_with_duration() {
    use std::sync::mpsc;

    // Test that recv_timeout actually times out
    let (_tx, rx) = mpsc::channel::<String>();

    let start = Instant::now();
    let result = rx.recv_timeout(Duration::from_millis(10));
    let elapsed = start.elapsed();

    // Should have timed out
    assert!(result.is_err());

    // Should have taken approximately 10ms (allow some margin)
    assert!(
        elapsed.as_millis() >= 5 && elapsed.as_millis() < 50,
        "Timeout took unexpected time: {:?}",
        elapsed
    );
}
