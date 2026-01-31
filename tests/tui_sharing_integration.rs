//! Integration tests for TUI sharing functionality.

use chrono::{TimeZone, Utc};
use panko::scanner::SessionMeta;
use panko::tui::widgets::ProviderOption;
use panko::tui::{App, FocusedPanel, SharingState};
use std::path::PathBuf;

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
        SessionMeta::new(
            "xyz99999",
            PathBuf::from("/home/user/other/.claude/xyz99999.jsonl"),
            "/home/user/other",
            test_timestamp(),
        ),
    ]
}

#[test]
fn test_full_sharing_state_flow() {
    let mut app = App::with_sessions(sample_sessions());
    let path = PathBuf::from("/test/session.jsonl");
    let providers = vec![
        ProviderOption::new("cloudflare", "Cloudflare"),
        ProviderOption::new("ngrok", "ngrok"),
    ];

    // 1. Start with inactive state
    assert!(!app.sharing_state().is_busy());
    assert!(matches!(app.sharing_state(), &SharingState::Inactive));

    // 2. Show provider selection
    app.start_provider_selection(path.clone(), providers);
    assert!(app.sharing_state().is_selecting_provider());
    assert!(app.sharing_state().is_busy());
    assert!(!app.sharing_state().is_active());

    // 3. Simulate starting (would happen when provider selected)
    app.clear_sharing_state();
    assert!(!app.sharing_state().is_busy());

    // 4. Simulate active sharing
    app.set_sharing_active(
        "https://tunnel.example.com".to_string(),
        "cloudflare".to_string(),
    );
    assert!(app.sharing_state().is_active());
    assert!(app.sharing_state().is_busy());
    assert!(!app.sharing_state().is_selecting_provider());
    assert_eq!(
        app.sharing_state().public_url(),
        Some("https://tunnel.example.com")
    );

    // 5. Clear sharing
    app.clear_sharing_state();
    assert!(!app.sharing_state().is_busy());
    assert!(!app.sharing_state().is_active());
    assert_eq!(app.sharing_state().public_url(), None);
}

#[test]
fn test_sharing_operations_dont_block_navigation() {
    let mut app = App::with_sessions(sample_sessions());

    // Set sharing active
    app.set_sharing_active(
        "https://tunnel.example.com".to_string(),
        "cloudflare".to_string(),
    );

    // Get initial selection
    let initial = app.session_list_state().selected();

    // Navigation should still work
    app.session_list_state_mut().select_next();
    let after_next = app.session_list_state().selected();
    assert_ne!(initial, after_next, "Navigation down should work");

    app.session_list_state_mut().select_previous();
    let after_prev = app.session_list_state().selected();
    assert_ne!(after_next, after_prev, "Navigation up should work");

    // Sharing should still be active
    assert!(app.sharing_state().is_active());
}

#[test]
fn test_provider_selection_flow() {
    let mut app = App::with_sessions(sample_sessions());
    let path = PathBuf::from("/test/session.jsonl");
    let providers = vec![
        ProviderOption::new("cloudflare", "Cloudflare"),
        ProviderOption::new("ngrok", "ngrok"),
        ProviderOption::new("localtunnel", "LocalTunnel"),
    ];

    // Start provider selection
    app.start_provider_selection(path, providers);

    // Should be in selecting provider state
    assert!(app.sharing_state().is_selecting_provider());
    assert!(app.sharing_state().is_busy());
    assert!(!app.sharing_state().is_active());

    // Cancel and verify state clears
    app.clear_sharing_state();
    assert!(!app.sharing_state().is_selecting_provider());
    assert!(!app.sharing_state().is_busy());
}

#[test]
fn test_sharing_with_fixture_file() {
    // This test verifies that we can construct a sharing scenario
    // with a real session file path (though we don't actually parse it)
    let fixture_path = PathBuf::from("tests/fixtures/sample_claude_session.jsonl");

    let mut app = App::new();
    let providers = vec![ProviderOption::new("cloudflare", "Cloudflare")];

    // Start provider selection with fixture path
    app.start_provider_selection(fixture_path.clone(), providers);
    assert!(app.sharing_state().is_selecting_provider());

    // Simulate what would happen after provider selection
    // In real scenario, this would be done by the main loop after StartSharing action
    app.clear_sharing_state();
    app.set_sharing_active(
        "https://example.cloudflare.dev/abc123".to_string(),
        "cloudflare".to_string(),
    );

    assert!(app.sharing_state().is_active());
    assert!(app
        .sharing_state()
        .public_url()
        .unwrap()
        .contains("cloudflare.dev"));
}

#[test]
fn test_panel_focus_during_sharing() {
    let mut app = App::with_sessions(sample_sessions());

    // Start sharing
    app.set_sharing_active("https://test.url".to_string(), "test".to_string());

    // Panel focus should work while sharing
    assert_eq!(app.focused_panel(), FocusedPanel::SessionList);

    app.set_focused_panel(FocusedPanel::Preview);
    assert_eq!(app.focused_panel(), FocusedPanel::Preview);

    app.set_focused_panel(FocusedPanel::SessionList);
    assert_eq!(app.focused_panel(), FocusedPanel::SessionList);

    // Sharing should still be active
    assert!(app.sharing_state().is_active());
}

#[test]
fn test_multiple_sharing_state_transitions() {
    let mut app = App::new();
    let path = PathBuf::from("/test.jsonl");
    let providers = vec![ProviderOption::new("cloudflare", "Cloudflare")];

    // Rapid state transitions (simulating multiple operations)
    for _ in 0..5 {
        // Start selection
        app.start_provider_selection(path.clone(), providers.clone());
        assert!(app.sharing_state().is_selecting_provider());

        // Cancel selection
        app.clear_sharing_state();
        assert!(!app.sharing_state().is_busy());

        // Start sharing directly
        app.set_sharing_active("https://test.url".to_string(), "test".to_string());
        assert!(app.sharing_state().is_active());

        // Stop sharing
        app.clear_sharing_state();
        assert!(!app.sharing_state().is_busy());
    }

    // App should be in clean state after all transitions
    assert!(matches!(app.sharing_state(), &SharingState::Inactive));
}

#[test]
fn test_sharing_state_does_not_affect_running() {
    let mut app = App::with_sessions(sample_sessions());

    // App should be running
    assert!(app.is_running());

    // Set sharing active
    app.set_sharing_active("https://test.url".to_string(), "test".to_string());
    assert!(app.is_running());

    // Clear sharing
    app.clear_sharing_state();
    assert!(app.is_running());

    // Only quit() should affect running state
    app.quit();
    assert!(!app.is_running());
}
