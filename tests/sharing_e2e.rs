//! End-to-end tests for multi-share functionality.
//!
//! These tests validate the full sharing flow using MockTunnelProvider,
//! including concurrent shares, unique IDs, limit enforcement, and graceful shutdown.

use std::collections::HashSet;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use panko::tui::sharing::{ShareId, ShareManager};
use panko::tunnel::mock::{MockConfig, MockTunnelProvider};
use panko::tunnel::TunnelProvider;

/// Create a mock session path for testing.
fn mock_session_path(name: &str) -> PathBuf {
    PathBuf::from(format!("/mock/sessions/{}.jsonl", name))
}

// ============================================================================
// MockTunnelProvider Tests
// ============================================================================

#[test]
fn test_mock_provider_basic_spawn() {
    let provider = MockTunnelProvider::new();

    assert!(provider.is_available());
    assert_eq!(provider.name(), "mock");

    let handle = provider.spawn(8080).expect("spawn should succeed");
    assert!(!handle.url().is_empty());
    assert_eq!(handle.provider_name(), "mock");
}

#[test]
fn test_mock_provider_unique_urls_across_spawns() {
    let provider = MockTunnelProvider::new();
    let mut urls = HashSet::new();

    // Spawn multiple times and verify URLs are unique
    for _ in 0..10 {
        let handle = provider.spawn(8080).expect("spawn should succeed");
        let url = handle.url().to_string();
        assert!(urls.insert(url.clone()), "URL should be unique: {}", url);
    }

    assert_eq!(urls.len(), 10);
}

#[test]
fn test_mock_provider_with_custom_url_template() {
    let config = MockConfig::with_url("https://custom-tunnel-{n}.test.dev");
    let provider = MockTunnelProvider::with_config(config);

    let handle = provider.spawn(3000).expect("spawn should succeed");
    assert!(handle.url().starts_with("https://custom-tunnel-"));
    assert!(handle.url().ends_with(".test.dev"));
}

#[test]
fn test_mock_provider_with_simulated_delay() {
    let delay = Duration::from_millis(100);
    let config = MockConfig::with_delay(delay);
    let provider = MockTunnelProvider::with_config(config);

    let start = std::time::Instant::now();
    let _handle = provider.spawn(8080).expect("spawn should succeed");
    let elapsed = start.elapsed();

    assert!(
        elapsed >= delay,
        "Should have waited at least {:?}, but only waited {:?}",
        delay,
        elapsed
    );
}

#[test]
fn test_mock_provider_with_simulated_error() {
    let config = MockConfig::with_error(panko::tunnel::mock::MockError::NotAvailable(
        "simulated failure".to_string(),
    ));
    let provider = MockTunnelProvider::with_config(config);

    let result = provider.spawn(8080);
    assert!(result.is_err());
}

#[test]
fn test_mock_provider_unavailable() {
    let config = MockConfig::unavailable();
    let provider = MockTunnelProvider::with_config(config);

    assert!(!provider.is_available());
}

// ============================================================================
// ShareManager Multi-Share Tests
// ============================================================================

#[test]
fn test_share_manager_start_multiple_shares() {
    let mut manager = ShareManager::new(5);

    // Start 3 shares
    let id1 = ShareId::new();
    let id2 = ShareId::new();
    let id3 = ShareId::new();

    manager.mark_started(
        id1,
        mock_session_path("session1"),
        "https://mock-1.example.com".to_string(),
        "mock".to_string(),
    );

    manager.mark_started(
        id2,
        mock_session_path("session2"),
        "https://mock-2.example.com".to_string(),
        "mock".to_string(),
    );

    manager.mark_started(
        id3,
        mock_session_path("session3"),
        "https://mock-3.example.com".to_string(),
        "mock".to_string(),
    );

    // Verify all shares are tracked
    assert_eq!(manager.active_count(), 3);
    assert!(manager.has_active_shares());

    // Verify each share has unique ID
    let share1 = manager.get_share(id1).expect("share 1 should exist");
    let share2 = manager.get_share(id2).expect("share 2 should exist");
    let share3 = manager.get_share(id3).expect("share 3 should exist");

    assert_ne!(share1.id, share2.id);
    assert_ne!(share2.id, share3.id);
    assert_ne!(share1.id, share3.id);

    // Verify URLs are correct
    assert_eq!(share1.public_url, "https://mock-1.example.com");
    assert_eq!(share2.public_url, "https://mock-2.example.com");
    assert_eq!(share3.public_url, "https://mock-3.example.com");
}

#[test]
fn test_share_manager_unique_share_ids() {
    let mut ids = HashSet::new();

    // Generate many share IDs and verify uniqueness
    for _ in 0..100 {
        let id = ShareId::new();
        assert!(ids.insert(id), "ShareId should be unique");
    }

    assert_eq!(ids.len(), 100);
}

#[test]
fn test_share_manager_max_shares_limit() {
    let max_shares = 3;
    let mut manager = ShareManager::new(max_shares);

    // Should be able to add up to max_shares
    for i in 0..max_shares {
        assert!(
            manager.can_add_share(),
            "Should be able to add share {} of {}",
            i + 1,
            max_shares
        );

        let id = ShareId::new();
        manager.mark_started(
            id,
            mock_session_path(&format!("session{}", i)),
            format!("https://mock-{}.example.com", i),
            "mock".to_string(),
        );
    }

    // Now at capacity
    assert_eq!(manager.active_count(), max_shares);
    assert!(
        !manager.can_add_share(),
        "Should not be able to add more shares"
    );
}

#[test]
fn test_share_manager_stop_individual_share() {
    let mut manager = ShareManager::new(5);

    // Start 3 shares
    let id1 = ShareId::new();
    let id2 = ShareId::new();
    let id3 = ShareId::new();

    manager.mark_started(
        id1,
        mock_session_path("session1"),
        "https://mock-1.example.com".to_string(),
        "mock".to_string(),
    );
    manager.mark_started(
        id2,
        mock_session_path("session2"),
        "https://mock-2.example.com".to_string(),
        "mock".to_string(),
    );
    manager.mark_started(
        id3,
        mock_session_path("session3"),
        "https://mock-3.example.com".to_string(),
        "mock".to_string(),
    );

    assert_eq!(manager.active_count(), 3);

    // Stop the middle share
    manager.stop_share(id2);

    // Verify only 2 shares remain
    assert_eq!(manager.active_count(), 2);
    assert!(manager.get_share(id1).is_some());
    assert!(manager.get_share(id2).is_none());
    assert!(manager.get_share(id3).is_some());
}

#[test]
fn test_share_manager_graceful_shutdown() {
    let mut manager = ShareManager::new(5);

    // Start multiple shares
    for i in 0..3 {
        let id = ShareId::new();
        manager.mark_started(
            id,
            mock_session_path(&format!("session{}", i)),
            format!("https://mock-{}.example.com", i),
            "mock".to_string(),
        );
    }

    assert_eq!(manager.active_count(), 3);
    assert!(manager.has_active_shares());

    // Stop all shares
    manager.stop_all();

    // Verify all shares are stopped
    assert_eq!(manager.active_count(), 0);
    assert!(!manager.has_active_shares());
    assert!(manager.shares().is_empty());
}

#[test]
fn test_share_manager_stop_all_clears_handles() {
    let mut manager = ShareManager::new(5);

    // Mark some shares as started (without real handles for this test)
    for i in 0..3 {
        let id = ShareId::new();
        manager.mark_started(
            id,
            mock_session_path(&format!("session{}", i)),
            format!("https://mock-{}.example.com", i),
            "mock".to_string(),
        );
    }

    // Stop all
    manager.stop_all();

    // Manager should be completely clean
    assert_eq!(manager.active_count(), 0);
    assert!(!manager.has_active_shares());

    // Should be able to add shares again
    assert!(manager.can_add_share());
}

#[test]
fn test_share_manager_respects_limit_after_stop() {
    let max_shares = 2;
    let mut manager = ShareManager::new(max_shares);

    // Fill to capacity
    let id1 = ShareId::new();
    let id2 = ShareId::new();

    manager.mark_started(
        id1,
        mock_session_path("session1"),
        "https://mock-1.example.com".to_string(),
        "mock".to_string(),
    );
    manager.mark_started(
        id2,
        mock_session_path("session2"),
        "https://mock-2.example.com".to_string(),
        "mock".to_string(),
    );

    assert!(!manager.can_add_share());

    // Stop one share
    manager.stop_share(id1);

    // Should be able to add again
    assert!(manager.can_add_share());

    // Add new share
    let id3 = ShareId::new();
    manager.mark_started(
        id3,
        mock_session_path("session3"),
        "https://mock-3.example.com".to_string(),
        "mock".to_string(),
    );

    assert_eq!(manager.active_count(), 2);
    assert!(!manager.can_add_share());
}

#[test]
fn test_share_manager_session_names() {
    let mut manager = ShareManager::new(5);

    let id = ShareId::new();
    manager.mark_started(
        id,
        PathBuf::from("/home/user/project/.claude/abc12345.jsonl"),
        "https://mock.example.com".to_string(),
        "cloudflare".to_string(),
    );

    let share = manager.get_share(id).unwrap();
    assert_eq!(share.session_name(), "abc12345");
}

#[test]
fn test_share_manager_duration_tracking() {
    let mut manager = ShareManager::new(5);

    let id = ShareId::new();
    manager.mark_started(
        id,
        mock_session_path("session"),
        "https://mock.example.com".to_string(),
        "mock".to_string(),
    );

    let share = manager.get_share(id).unwrap();

    // Duration should be very small (just created)
    assert!(share.duration().as_millis() < 100);

    // Wait a bit
    thread::sleep(Duration::from_millis(50));

    // Duration should have increased
    let share = manager.get_share(id).unwrap();
    assert!(share.duration().as_millis() >= 50);
}

#[test]
fn test_share_id_display_format() {
    let id = ShareId::new();
    let display = format!("{}", id);

    assert!(display.starts_with("share-"));
    // The numeric part should be a valid number
    let num_str = display.strip_prefix("share-").unwrap();
    assert!(num_str.parse::<u64>().is_ok());
}

// ============================================================================
// Integration: MockTunnelProvider + ShareManager
// ============================================================================

#[test]
fn test_concurrent_shares_with_mock_provider() {
    let provider = MockTunnelProvider::new();
    let mut manager = ShareManager::new(5);

    // Simulate starting 3 concurrent shares
    let mut share_ids = Vec::new();
    let mut urls = Vec::new();

    for i in 0..3 {
        // In real code, this would be in a separate thread
        let handle = provider
            .spawn(3000 + i as u16)
            .expect("spawn should succeed");
        let url = handle.url().to_string();

        let id = ShareId::new();
        manager.mark_started(
            id,
            mock_session_path(&format!("session{}", i)),
            url.clone(),
            provider.name().to_string(),
        );

        share_ids.push(id);
        urls.push(url);
    }

    // Verify all shares are active
    assert_eq!(manager.active_count(), 3);

    // Verify all URLs are unique
    let url_set: HashSet<_> = urls.iter().collect();
    assert_eq!(url_set.len(), 3, "All URLs should be unique");

    // Verify each share can be retrieved
    for id in &share_ids {
        let share = manager.get_share(*id);
        assert!(share.is_some(), "Share {:?} should exist", id);
    }
}

#[test]
fn test_shares_cleanup_on_error() {
    let mut manager = ShareManager::new(5);

    // Start a share
    let id = ShareId::new();
    manager.mark_started(
        id,
        mock_session_path("session"),
        "https://mock.example.com".to_string(),
        "mock".to_string(),
    );

    assert_eq!(manager.active_count(), 1);

    // Simulate error by removing the share
    manager.remove_handle(id);

    // Share should be cleaned up
    assert_eq!(manager.active_count(), 0);
    assert!(manager.get_share(id).is_none());
}

#[test]
fn test_default_max_shares() {
    // Default ShareManager should have reasonable default
    let manager = ShareManager::default();
    assert_eq!(manager.max_shares, 0); // Default is 0, must be explicitly set

    // With explicit value
    let manager = ShareManager::new(5);
    assert_eq!(manager.max_shares, 5);
}
