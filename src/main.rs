use anyhow::{Context, Result};
use arboard::Clipboard;
use clap::{Parser, Subcommand};
use inquire::Select;
use std::path::{Path, PathBuf};

use panko::config::{format_config, Config};
use panko::export::{format_context, ContextOptions};
use panko::logging::{init_logging, init_tui_logging, LogConfig, Verbosity};
use panko::parser::{ClaudeParser, SessionParser};
use panko::server::{
    run_server_with_source, shutdown_signal, start_server_with_source, ServerConfig,
};
use panko::tui;
use panko::tunnel::{detect_available_providers, get_provider_with_config, AvailableProvider};

#[derive(Parser)]
#[command(name = "panko")]
#[command(version)]
#[command(about = "View and share AI coding agent sessions")]
#[command(
    long_about = "A CLI tool for viewing and sharing AI coding agent sessions (Claude Code, Codex, etc.) via a local web server with optional tunnel sharing.\n\nRun without arguments to enter interactive TUI mode."
)]
struct Cli {
    /// Enable verbose logging (can be repeated: -v for debug, -vv for trace)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// View a session file in your browser
    View {
        /// Path to the session file
        file: PathBuf,

        /// Port to start the server on (default: 3000)
        #[arg(short, long, default_value = "3000")]
        port: u16,

        /// Don't open browser automatically
        #[arg(long)]
        no_browser: bool,
    },
    /// Share a session file via a public tunnel
    Share {
        /// Path to the session file
        file: PathBuf,

        /// Tunnel provider to use (cloudflare, ngrok, tailscale)
        #[arg(short = 't', long)]
        tunnel: Option<String>,

        /// Port to start the server on (default: 3000)
        #[arg(short, long, default_value = "3000")]
        port: u16,
    },
    /// Validate session files without starting a server
    Check {
        /// Path(s) to session file(s) to validate
        #[arg(required = true)]
        files: Vec<PathBuf>,

        /// Quiet mode: only output failures
        #[arg(short, long)]
        quiet: bool,
    },
    /// Manage configuration settings
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current configuration
    Show,
    /// Set a configuration value
    Set {
        /// Configuration key (default_provider, ngrok_token, default_port)
        key: String,
        /// Configuration value (use empty string to unset)
        value: String,
    },
    /// Unset a configuration value
    Unset {
        /// Configuration key to unset
        key: String,
    },
    /// Show configuration file path
    Path,
}

/// List of available parsers.
fn get_parsers() -> Vec<Box<dyn SessionParser>> {
    vec![Box::new(ClaudeParser::new())]
}

/// Parse a session file using available parsers.
fn parse_session(path: &Path) -> Result<panko::parser::Session> {
    let parsers = get_parsers();

    for parser in &parsers {
        if parser.can_parse(path) {
            return parser
                .parse(path)
                .map_err(|e| anyhow::anyhow!("Failed to parse session: {}", e));
        }
    }

    anyhow::bail!(
        "No parser available for file: {}. Supported formats: JSONL (Claude Code)",
        path.display()
    )
}

/// Copy text to the clipboard.
fn copy_to_clipboard(text: &str) -> Result<()> {
    let mut clipboard = Clipboard::new().context("Failed to access clipboard")?;
    clipboard
        .set_text(text)
        .context("Failed to copy to clipboard")?;
    Ok(())
}

/// Open a folder in the system file manager.
///
/// Cross-platform support:
/// - macOS: uses `open`
/// - Linux: uses `xdg-open`
/// - Windows: uses `explorer`
fn open_in_file_manager(path: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .context("Failed to open folder with 'open'")?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .context("Failed to open folder with 'xdg-open'")?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .spawn()
            .context("Failed to open folder with 'explorer'")?;
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        anyhow::bail!("Opening folders is not supported on this platform");
    }

    Ok(())
}

/// Prompt the user to select a tunnel provider.
fn prompt_tunnel_selection(providers: &[AvailableProvider]) -> Result<AvailableProvider> {
    let options: Vec<String> = providers
        .iter()
        .map(|p| p.display_name.to_string())
        .collect();

    let selection = Select::new("Select a tunnel provider:", options)
        .prompt()
        .context("Failed to get tunnel selection")?;

    providers
        .iter()
        .find(|p| p.display_name == selection)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Invalid selection"))
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load configuration for log file path
    let app_config = Config::load().unwrap_or_default();

    // Determine verbosity level from CLI
    let verbosity = match cli.verbose {
        0 => Verbosity::Quiet,
        1 => Verbosity::Normal,
        2 => Verbosity::Verbose,
        _ => Verbosity::Trace,
    };

    // If no subcommand is provided, enter TUI mode
    let command = match cli.command {
        Some(cmd) => cmd,
        None => {
            // For TUI mode, only log to file (if configured) since stderr is used by the TUI
            let _log_guard = init_tui_logging(app_config.log_file.as_deref());
            return run_tui();
        }
    };

    // Initialize logging for CLI commands
    let log_config = LogConfig {
        verbosity,
        log_file: app_config.log_file.clone(),
    };
    let _log_guard = init_logging(&log_config);

    match command {
        Commands::View {
            file,
            port,
            no_browser,
        } => {
            // Check file exists
            if !file.exists() {
                anyhow::bail!("File not found: {}", file.display());
            }

            // Parse the session
            let session = parse_session(&file)
                .with_context(|| format!("Failed to parse session file: {}", file.display()))?;

            println!(
                "Loaded session '{}' with {} blocks",
                session.id,
                session.blocks.len()
            );

            // Calculate effective port (CLI > config > default)
            let effective_port = app_config.effective_port(port);

            // Run the server
            let server_config = ServerConfig {
                base_port: effective_port,
                open_browser: !no_browser,
            };

            run_server_with_source(session, server_config, Some(file)).await?;
        }
        Commands::Share { file, tunnel, port } => {
            // Check file exists
            if !file.exists() {
                anyhow::bail!("File not found: {}", file.display());
            }

            // Parse the session
            let session = parse_session(&file)
                .with_context(|| format!("Failed to parse session file: {}", file.display()))?;

            println!(
                "Loaded session '{}' with {} blocks",
                session.id,
                session.blocks.len()
            );

            // Get ngrok token from config if available
            let ngrok_token = app_config.ngrok_token.as_deref();

            // Select tunnel provider
            // Priority: CLI argument > config default_provider > auto-detect
            let selected_provider = if let Some(tunnel_name) = tunnel {
                // User explicitly specified a provider on CLI
                let provider =
                    get_provider_with_config(&tunnel_name, ngrok_token).ok_or_else(|| {
                        anyhow::anyhow!(
                            "Unknown tunnel provider: {}. Available: cloudflare, ngrok, tailscale",
                            tunnel_name
                        )
                    })?;

                if !provider.is_available() {
                    anyhow::bail!(
                        "Tunnel provider '{}' is not available. Please install the required binary.",
                        tunnel_name
                    );
                }

                AvailableProvider {
                    name: provider.name(),
                    display_name: provider.display_name(),
                }
            } else if let Some(ref default_provider) = app_config.default_provider {
                // Use config default_provider
                let provider =
                    get_provider_with_config(default_provider, ngrok_token).ok_or_else(|| {
                        anyhow::anyhow!(
                            "Unknown tunnel provider in config: {}. Available: cloudflare, ngrok, tailscale",
                            default_provider
                        )
                    })?;

                if !provider.is_available() {
                    anyhow::bail!(
                        "Default tunnel provider '{}' (from config) is not available. Please install the required binary or change your config.",
                        default_provider
                    );
                }

                println!(
                    "Using {} tunnel provider (from config)",
                    provider.display_name()
                );
                AvailableProvider {
                    name: provider.name(),
                    display_name: provider.display_name(),
                }
            } else {
                // Detect available tunnel providers
                let available_providers = detect_available_providers();

                if available_providers.is_empty() {
                    anyhow::bail!(
                        "No tunnel providers available. Please install one of:\n\
                         - cloudflared (Cloudflare Tunnels): https://developers.cloudflare.com/cloudflare-one/connections/connect-apps/install-and-setup/installation/\n\
                         - ngrok: https://ngrok.com/download\n\
                         - tailscale: https://tailscale.com/download/"
                    );
                }

                // Select tunnel provider
                if available_providers.len() == 1 {
                    let provider = &available_providers[0];
                    println!("Using {} tunnel provider", provider.display_name);
                    provider.clone()
                } else {
                    prompt_tunnel_selection(&available_providers)?
                }
            };

            // Calculate effective port (CLI > config > default)
            let effective_port = app_config.effective_port(port);

            // Start the local server (don't open browser for share)
            let server_config = ServerConfig {
                base_port: effective_port,
                open_browser: false,
            };

            let server_handle =
                start_server_with_source(session, server_config, Some(file.clone())).await?;
            let actual_port = server_handle.port();

            println!("Local server running at: {}", server_handle.local_url());

            // Spawn the tunnel
            println!("Starting {} tunnel...", selected_provider.display_name);

            let provider = get_provider_with_config(selected_provider.name, ngrok_token)
                .ok_or_else(|| {
                    anyhow::anyhow!("Failed to get provider: {}", selected_provider.name)
                })?;

            let mut tunnel_handle = match provider.spawn(actual_port) {
                Ok(handle) => handle,
                Err(e) => {
                    server_handle.stop().await;
                    anyhow::bail!("Failed to start tunnel: {}", e);
                }
            };

            let public_url = tunnel_handle.url().to_string();

            // Copy URL to clipboard
            match copy_to_clipboard(&public_url) {
                Ok(()) => println!("\n✓ URL copied to clipboard!"),
                Err(e) => eprintln!("\n⚠ Could not copy to clipboard: {}", e),
            }

            // Print the public URL with clear messaging
            println!("\n{}", "=".repeat(60));
            println!("🌐 Your session is now publicly available at:");
            println!("\n   {}\n", public_url);
            println!("{}", "=".repeat(60));
            println!("\nPress Ctrl+C to stop sharing\n");

            // Wait for shutdown signal
            shutdown_signal().await;

            // Clean up
            println!("\nStopping tunnel...");
            if let Err(e) = tunnel_handle.stop() {
                eprintln!("Warning: Error stopping tunnel: {}", e);
            }

            println!("Stopping server...");
            server_handle.stop().await;

            println!("Sharing stopped");
        }
        Commands::Check { files, quiet } => {
            let exit_code = handle_check_command(&files, quiet)?;
            if exit_code != 0 {
                std::process::exit(exit_code);
            }
        }
        Commands::Config { action } => {
            handle_config_command(action)?;
        }
    }

    Ok(())
}

/// Run the TUI application.
fn run_tui() -> Result<()> {
    // Load configuration
    let mut config = Config::load().unwrap_or_default();

    // Create application state and load sessions
    let mut app = tui::App::new();
    if let Err(e) = app.load_sessions() {
        // Log warning but continue - user can still refresh
        eprintln!("Warning: Failed to load sessions: {}", e);
    }

    // Apply sort order from config
    if let Some(ref sort_str) = config.default_sort {
        if let Some(sort_order) = tui::SortOrder::from_str(sort_str) {
            app.set_sort_order(sort_order);
        }
    }

    // Apply max_shares from config (default is 5)
    app.set_max_shares(config.effective_max_shares(tui::DEFAULT_MAX_SHARES));

    // Track initial sort order to detect changes
    let initial_sort_order = app.sort_order();

    // Main loop that handles TUI and actions
    loop {
        // Initialize terminal
        let mut terminal = tui::init().context("Failed to initialize terminal")?;

        // Run the TUI until it returns
        let result = tui::run(&mut terminal, &mut app);

        // Restore terminal before handling any action
        tui::restore().context("Failed to restore terminal")?;

        // Handle the result
        match result {
            Ok(tui::RunResult::Done) => {
                // User quit - stop all active shares first
                app.stop_all_shares();

                // Save sort order if changed
                let final_sort_order = app.sort_order();
                if final_sort_order != initial_sort_order {
                    config.set_default_sort(Some(final_sort_order.as_str().to_string()));
                    if let Err(e) = config.save() {
                        eprintln!("Warning: Failed to save config: {}", e);
                    }
                }

                break;
            }
            Ok(tui::RunResult::Continue) => {
                // This shouldn't happen but just continue
                continue;
            }
            Ok(tui::RunResult::Action(action)) => {
                // Handle the action
                handle_tui_action(&action, &mut app)?;
                // Continue the TUI loop after action completes
            }
            Err(e) => {
                // Stop all shares on error
                app.stop_all_shares();
                return Err(anyhow::anyhow!("Application error: {}", e));
            }
        }
    }

    Ok(())
}

/// Handle an action triggered from the TUI.
fn handle_tui_action(action: &tui::Action, app: &mut tui::App) -> Result<()> {
    match action {
        tui::Action::ViewSession(path) => {
            // Note: We don't stop shares when viewing, user can view while sharing
            // View the session using the existing server code
            handle_view_from_tui(path)?;
        }
        tui::Action::ShareSession(path) => {
            // Check if we can add another share
            if !app.can_add_share() {
                eprintln!(
                    "Maximum number of concurrent shares reached ({}). Stop a share first.",
                    app.active_share_count()
                );
                wait_for_key("Press Enter to continue...");
                return Ok(());
            }

            // Detect available providers
            let available = detect_available_providers();
            if available.is_empty() {
                eprintln!(
                    "No tunnel providers available. Install cloudflared, ngrok, or tailscale."
                );
                wait_for_key("Press Enter to continue...");
                return Ok(());
            }

            // Convert to ProviderOption
            let providers: Vec<tui::ProviderOption> = available
                .iter()
                .map(|p| tui::ProviderOption::new(p.name, p.display_name))
                .collect();

            if providers.len() == 1 {
                // Only one provider - start sharing immediately
                let provider = &providers[0];
                let share_id = tui::ShareId::new();
                let handle = tui::SharingHandle::start(path.clone(), provider.name.clone());

                // Track pending share (waiting for Started message)
                app.set_pending_share(share_id, path.clone(), provider.name.clone());
                app.share_manager_mut().add_pending_share(
                    share_id,
                    path.clone(),
                    provider.name.clone(),
                    handle,
                );

                // Set UI state to Starting
                app.set_sharing_active("Starting...".to_string(), provider.name.clone());
            } else {
                // Multiple providers - show selection popup
                app.start_provider_selection(path.clone(), providers);
            }
        }
        tui::Action::StartSharing { path, provider } => {
            // Start sharing with the selected provider
            let share_id = tui::ShareId::new();
            let handle = tui::SharingHandle::start(path.clone(), provider.clone());

            // Track pending share
            app.set_pending_share(share_id, path.clone(), provider.clone());
            app.share_manager_mut().add_pending_share(
                share_id,
                path.clone(),
                provider.clone(),
                handle,
            );
        }
        tui::Action::StopSharing => {
            // Stop all active shares (legacy behavior for single Esc press)
            app.stop_all_shares();
        }
        tui::Action::SharingStarted { url, provider } => {
            // This is handled via the message channel now
            app.set_sharing_active(url.clone(), provider.clone());
        }
        tui::Action::CopyPath(path) => {
            // Copy the session file path to clipboard
            let path_str = path.display().to_string();
            match copy_to_clipboard(&path_str) {
                Ok(()) => {
                    app.set_status_message(format!("✓ Copied: {}", path_str));
                }
                Err(e) => {
                    app.set_status_message(format!("✗ Copy failed: {}", e));
                }
            }
        }
        tui::Action::CopyContext(path) => {
            // Copy session context to clipboard
            match handle_copy_context(path) {
                Ok((message_count, estimated_tokens)) => {
                    app.set_status_message(format!(
                        "✓ Context copied ({} messages, ~{} tokens)",
                        message_count, estimated_tokens
                    ));
                }
                Err(e) => {
                    app.set_status_message(format!("✗ Copy failed: {}", e));
                }
            }
        }
        tui::Action::OpenFolder(path) => {
            // Open the containing folder in the system file manager
            if let Some(parent) = path.parent() {
                match open_in_file_manager(parent) {
                    Ok(()) => {
                        app.set_status_message(format!("✓ Opened: {}", parent.display()));
                    }
                    Err(e) => {
                        app.set_status_message(format!("✗ Open failed: {}", e));
                    }
                }
            } else {
                app.set_status_message("✗ Cannot determine parent folder");
            }
        }
        tui::Action::DeleteSession(path) => {
            // Delete the session file
            match std::fs::remove_file(path) {
                Ok(()) => {
                    // Remove the session from the list
                    app.remove_session_by_path(path);
                    app.set_status_message("✓ Deleted session");
                }
                Err(e) => {
                    app.set_status_message(format!("✗ Delete failed: {}", e));
                }
            }
        }
        tui::Action::DownloadSession(path) => {
            // Download the session file to ~/Downloads
            match handle_download_session(path) {
                Ok(dest_path) => {
                    app.set_status_message(format!("✓ Saved to {}", dest_path.display()));
                }
                Err(e) => {
                    app.set_status_message(format!("✗ Download failed: {}", e));
                }
            }
        }
        tui::Action::CopyShareUrl(ref url) => {
            // Copy the share URL to clipboard (from share modal)
            match copy_to_clipboard(url) {
                Ok(()) => {
                    app.set_status_message("✓ URL copied to clipboard");
                }
                Err(e) => {
                    app.set_status_message(format!("✗ Copy failed: {}", e));
                }
            }
        }
        tui::Action::StopShareById(id) => {
            // Stop a specific share by its ID
            app.stop_share(*id);
            // Update the shares panel state
            if app.is_shares_panel_showing() {
                let shares = app.share_manager().shares();
                if shares.is_empty() {
                    // Close panel if no more shares
                    app.toggle_shares_panel();
                }
            }
            app.set_status_message("✓ Share stopped");
        }
        tui::Action::None => {
            // Nothing to do
        }
    }
    Ok(())
}

/// Handle viewing a session from the TUI.
///
/// This is similar to the view command but with messaging appropriate
/// for returning to the TUI afterwards.
fn handle_view_from_tui(path: &Path) -> Result<()> {
    // Check file exists
    if !path.exists() {
        eprintln!("Error: File not found: {}", path.display());
        wait_for_key("Press Enter to return to the browser...");
        return Ok(());
    }

    // Parse the session
    let session = match parse_session(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: Failed to parse session: {}", e);
            wait_for_key("Press Enter to return to the browser...");
            return Ok(());
        }
    };

    println!(
        "\nViewing session '{}' with {} blocks",
        session.id,
        session.blocks.len()
    );
    println!("Press Ctrl+C to return to the browser\n");

    // Load configuration for port
    let app_config = Config::load().unwrap_or_default();
    let effective_port = app_config.effective_port(3000);

    // Run the server
    let server_config = ServerConfig {
        base_port: effective_port,
        open_browser: true,
    };

    // Run the server using the current runtime (we're already inside #[tokio::main])
    // Use block_in_place to run async code from synchronous context within runtime
    let source_path = path.to_path_buf();
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            if let Err(e) = run_server_with_source(session, server_config, Some(source_path)).await
            {
                eprintln!("Server error: {}", e);
            }
        })
    });

    println!("\nReturning to session browser...\n");

    Ok(())
}

/// Wait for the user to press Enter.
fn wait_for_key(message: &str) {
    use std::io::{self, BufRead, Write};

    print!("{}", message);
    let _ = io::stdout().flush();

    let stdin = io::stdin();
    let _ = stdin.lock().lines().next();
}

/// Handle copying session context to clipboard.
///
/// Parses the session, formats it as markdown context, and copies to clipboard.
/// Returns the message count and estimated token count on success.
fn handle_copy_context(path: &Path) -> Result<(usize, usize)> {
    // Parse the session
    let session = parse_session(path)?;

    // Format as context
    let options = ContextOptions::for_clipboard();
    let context = format_context(&session, &options);

    // Copy to clipboard
    copy_to_clipboard(&context.content)?;

    Ok((context.message_count, context.estimated_tokens))
}

/// Handle downloading a session file to ~/Downloads.
///
/// Copies the session JSONL file to the user's Downloads directory
/// with the filename format: {session_id}.jsonl
fn handle_download_session(path: &Path) -> Result<PathBuf> {
    // Get the Downloads directory
    let downloads_dir = dirs::download_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find Downloads directory"))?;

    // Get the filename from the source path
    let filename = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Invalid source path"))?;

    // Create the destination path
    let dest_path = downloads_dir.join(filename);

    // Copy the file
    std::fs::copy(path, &dest_path).context("Failed to copy session file")?;

    Ok(dest_path)
}

/// Handle config subcommand
fn handle_config_command(action: Option<ConfigAction>) -> Result<()> {
    match action {
        None | Some(ConfigAction::Show) => {
            // Show current configuration
            let config = Config::load().context("Failed to load configuration")?;
            println!("{}", format_config(&config));

            if let Ok(path) = Config::config_path() {
                println!("\nConfig file: {}", path.display());
            }
        }
        Some(ConfigAction::Set { key, value }) => {
            let mut config = Config::load().context("Failed to load configuration")?;

            match key.as_str() {
                "default_provider" => {
                    if value.is_empty() {
                        config.set_default_provider(None);
                        println!("Unset default_provider");
                    } else {
                        // Validate provider name
                        let valid_providers = ["cloudflare", "ngrok", "tailscale"];
                        if !valid_providers.contains(&value.as_str()) {
                            anyhow::bail!(
                                "Invalid provider '{}'. Valid options: cloudflare, ngrok, tailscale",
                                value
                            );
                        }
                        config.set_default_provider(Some(value.clone()));
                        println!("Set default_provider = \"{}\"", value);
                    }
                }
                "ngrok_token" => {
                    if value.is_empty() {
                        config.set_ngrok_token(None);
                        println!("Unset ngrok_token");
                    } else {
                        config.set_ngrok_token(Some(value));
                        println!("Set ngrok_token = \"********\"");
                    }
                }
                "default_port" => {
                    if value.is_empty() {
                        config.set_default_port(None);
                        println!("Unset default_port");
                    } else {
                        let port: u16 = value
                            .parse()
                            .context("Invalid port number. Must be a valid port (1-65535)")?;
                        config.set_default_port(Some(port));
                        println!("Set default_port = {}", port);
                    }
                }
                "default_sort" => {
                    if value.is_empty() {
                        config.set_default_sort(None);
                        println!("Unset default_sort");
                    } else {
                        // Validate sort option
                        if tui::SortOrder::from_str(&value).is_none() {
                            anyhow::bail!(
                                "Invalid sort option '{}'. Valid options: date_newest, date_oldest, message_count, project_name",
                                value
                            );
                        }
                        config.set_default_sort(Some(value.clone()));
                        println!("Set default_sort = \"{}\"", value);
                    }
                }
                "max_shares" => {
                    if value.is_empty() {
                        config.set_max_shares(None);
                        println!("Unset max_shares");
                    } else {
                        let max: usize = value
                            .parse()
                            .context("Invalid max_shares value. Must be a positive integer")?;
                        if max == 0 {
                            anyhow::bail!("max_shares must be at least 1");
                        }
                        config.set_max_shares(Some(max));
                        println!("Set max_shares = {}", max);
                    }
                }
                "log_file" => {
                    if value.is_empty() {
                        config.set_log_file(None);
                        println!("Unset log_file");
                    } else {
                        config.set_log_file(Some(value.clone()));
                        println!("Set log_file = \"{}\"", value);
                    }
                }
                _ => {
                    anyhow::bail!(
                        "Unknown configuration key '{}'. Valid keys: default_provider, ngrok_token, default_port, default_sort, max_shares, log_file",
                        key
                    );
                }
            }

            config.save().context("Failed to save configuration")?;
        }
        Some(ConfigAction::Unset { key }) => {
            let mut config = Config::load().context("Failed to load configuration")?;

            match key.as_str() {
                "default_provider" => {
                    config.set_default_provider(None);
                    println!("Unset default_provider");
                }
                "ngrok_token" => {
                    config.set_ngrok_token(None);
                    println!("Unset ngrok_token");
                }
                "default_port" => {
                    config.set_default_port(None);
                    println!("Unset default_port");
                }
                "default_sort" => {
                    config.set_default_sort(None);
                    println!("Unset default_sort");
                }
                "max_shares" => {
                    config.set_max_shares(None);
                    println!("Unset max_shares");
                }
                "log_file" => {
                    config.set_log_file(None);
                    println!("Unset log_file");
                }
                _ => {
                    anyhow::bail!(
                        "Unknown configuration key '{}'. Valid keys: default_provider, ngrok_token, default_port, default_sort, max_shares, log_file",
                        key
                    );
                }
            }

            config.save().context("Failed to save configuration")?;
        }
        Some(ConfigAction::Path) => {
            let path = Config::config_path().context("Failed to determine config path")?;
            println!("{}", path.display());
        }
    }

    Ok(())
}

/// Result of checking a single file.
struct CheckResult {
    path: PathBuf,
    success: bool,
    session_id: Option<String>,
    block_count: Option<usize>,
    duration_secs: Option<i64>,
    error: Option<String>,
}

/// Handle check subcommand - validate session files without starting a server.
fn handle_check_command(files: &[PathBuf], quiet: bool) -> Result<i32> {
    let mut results: Vec<CheckResult> = Vec::new();
    let mut failure_count = 0;

    for path in files {
        let result = check_single_file(path);
        if !result.success {
            failure_count += 1;
        }
        results.push(result);
    }

    // Output results
    for result in &results {
        if result.success {
            if !quiet {
                print_success_result(result);
            }
        } else {
            print_failure_result(result);
        }
    }

    // Summary for multiple files
    if files.len() > 1 && !quiet {
        println!();
        let success_count = results.iter().filter(|r| r.success).count();
        println!(
            "Summary: {}/{} files passed validation",
            success_count,
            files.len()
        );
    }

    // Return exit code: 0 on success, 1 if any failures
    Ok(if failure_count > 0 { 1 } else { 0 })
}

/// Check a single session file.
fn check_single_file(path: &Path) -> CheckResult {
    // Check if file exists
    if !path.exists() {
        return CheckResult {
            path: path.to_path_buf(),
            success: false,
            session_id: None,
            block_count: None,
            duration_secs: None,
            error: Some(format!("File not found: {}", path.display())),
        };
    }

    // Try to parse the session
    match parse_session(path) {
        Ok(session) => {
            // Calculate session duration if we have blocks
            let duration_secs = if let (Some(first), Some(last)) =
                (session.blocks.first(), session.blocks.last())
            {
                let first_ts = first.timestamp();
                let last_ts = last.timestamp();
                Some((last_ts - first_ts).num_seconds())
            } else {
                None
            };

            CheckResult {
                path: path.to_path_buf(),
                success: true,
                session_id: Some(session.id),
                block_count: Some(session.blocks.len()),
                duration_secs,
                error: None,
            }
        }
        Err(e) => CheckResult {
            path: path.to_path_buf(),
            success: false,
            session_id: None,
            block_count: None,
            duration_secs: None,
            error: Some(e.to_string()),
        },
    }
}

/// Print success result with summary stats.
fn print_success_result(result: &CheckResult) {
    println!("✓ {}", result.path.display());

    if let Some(ref session_id) = result.session_id {
        println!("  Session ID: {}", session_id);
    }

    if let Some(block_count) = result.block_count {
        println!("  Blocks: {}", block_count);
    }

    if let Some(duration_secs) = result.duration_secs {
        let duration_str = format_duration(duration_secs);
        println!("  Duration: {}", duration_str);
    }
}

/// Print failure result with error message.
fn print_failure_result(result: &CheckResult) {
    eprintln!("✗ {}", result.path.display());
    if let Some(ref error) = result.error {
        eprintln!("  Error: {}", error);
    }
}

/// Format duration in human-readable form.
fn format_duration(total_secs: i64) -> String {
    if total_secs < 0 {
        return "0s".to_string();
    }

    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}
