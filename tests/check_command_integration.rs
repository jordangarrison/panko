//! Integration tests for the `panko check` command.

use std::process::Command;

/// Get path to the panko binary.
fn panko_bin() -> std::path::PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // Remove test binary name
    path.pop(); // Remove deps
    path.push("panko");
    path
}

#[test]
fn test_check_valid_file() {
    let output = Command::new(panko_bin())
        .args(["check", "tests/fixtures/sample_claude_session.jsonl"])
        .output()
        .expect("Failed to execute panko check");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "Expected success exit code");
    assert!(stdout.contains("✓"), "Expected checkmark in output");
    assert!(stdout.contains("Session ID:"), "Expected session ID");
    assert!(stdout.contains("Blocks:"), "Expected block count");
    assert!(stdout.contains("Duration:"), "Expected duration");
}

#[test]
fn test_check_nonexistent_file() {
    let output = Command::new(panko_bin())
        .args(["check", "nonexistent_file.jsonl"])
        .output()
        .expect("Failed to execute panko check");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "Expected failure exit code");
    assert!(stderr.contains("✗"), "Expected X mark in error output");
    assert!(
        stderr.contains("File not found"),
        "Expected file not found error"
    );
}

#[test]
fn test_check_multiple_files_all_valid() {
    // Use the same valid file twice to test multiple file handling
    let output = Command::new(panko_bin())
        .args([
            "check",
            "tests/fixtures/sample_claude_session.jsonl",
            "tests/fixtures/sample_claude_session.jsonl",
        ])
        .output()
        .expect("Failed to execute panko check");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "Expected success exit code");
    assert!(
        stdout.contains("2/2 files passed"),
        "Expected summary for multiple files"
    );
}

#[test]
fn test_check_multiple_files_with_failure() {
    let output = Command::new(panko_bin())
        .args([
            "check",
            "tests/fixtures/sample_claude_session.jsonl",
            "nonexistent.jsonl",
        ])
        .output()
        .expect("Failed to execute panko check");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "Expected failure exit code");
    assert!(
        stdout.contains("✓"),
        "Expected success marker for valid file"
    );
    assert!(
        stderr.contains("✗"),
        "Expected failure marker for invalid file"
    );
    assert!(
        stdout.contains("1/2 files passed"),
        "Expected summary showing partial failure"
    );
}

#[test]
fn test_check_quiet_mode_hides_success() {
    let output = Command::new(panko_bin())
        .args(["check", "-q", "tests/fixtures/sample_claude_session.jsonl"])
        .output()
        .expect("Failed to execute panko check");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "Expected success exit code");
    assert!(
        stdout.is_empty() || !stdout.contains("✓"),
        "Quiet mode should not show success markers"
    );
    assert!(
        !stdout.contains("Session ID:"),
        "Quiet mode should not show session details"
    );
}

#[test]
fn test_check_quiet_mode_shows_failures() {
    let output = Command::new(panko_bin())
        .args(["check", "-q", "nonexistent.jsonl"])
        .output()
        .expect("Failed to execute panko check");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "Expected failure exit code");
    assert!(
        stderr.contains("✗"),
        "Quiet mode should still show failure markers"
    );
    assert!(
        stderr.contains("Error:"),
        "Quiet mode should still show error messages"
    );
}

#[test]
fn test_check_exit_code_zero_on_success() {
    let output = Command::new(panko_bin())
        .args(["check", "tests/fixtures/sample_claude_session.jsonl"])
        .output()
        .expect("Failed to execute panko check");

    assert_eq!(
        output.status.code(),
        Some(0),
        "Exit code should be 0 on success"
    );
}

#[test]
fn test_check_exit_code_nonzero_on_failure() {
    let output = Command::new(panko_bin())
        .args(["check", "nonexistent.jsonl"])
        .output()
        .expect("Failed to execute panko check");

    assert!(
        output.status.code().unwrap_or(1) != 0,
        "Exit code should be non-zero on failure"
    );
}

#[test]
fn test_check_exit_code_nonzero_on_any_failure() {
    // Even if some files succeed, exit code should be non-zero if any fail
    let output = Command::new(panko_bin())
        .args([
            "check",
            "tests/fixtures/sample_claude_session.jsonl",
            "nonexistent.jsonl",
        ])
        .output()
        .expect("Failed to execute panko check");

    assert!(
        output.status.code().unwrap_or(1) != 0,
        "Exit code should be non-zero when any file fails"
    );
}
