//! Daemon module for persistent share management
//!
//! The daemon runs as a separate process, managing share lifecycles independently
//! of the TUI. This allows shares to persist across TUI restarts.

pub mod db;
pub mod protocol;

// Future modules (will be added in subsequent stories):
// pub mod server;
// pub mod client;
// pub mod share_service;
