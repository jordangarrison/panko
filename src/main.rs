use anyhow::{Context, Result};
use arboard::Clipboard;
use clap::{Parser, Subcommand};
use inquire::Select;
use std::path::{Path, PathBuf};

use panko::config::{format_config, Config};
use panko::parser::{ClaudeParser, SessionParser};
use panko::server::{run_server, shutdown_signal, start_server, ServerConfig};
use panko::tunnel::{detect_available_providers, get_provider_with_config, AvailableProvider};

#[derive(Parser)]
#[command(name = "panko")]
#[command(version)]
#[command(about = "View and share AI coding agent sessions")]
#[command(
    long_about = "A CLI tool for viewing and sharing AI coding agent sessions (Claude Code, Codex, etc.) via a local web server with optional tunnel sharing."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
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

    match cli.command {
        Commands::View {
            file,
            port,
            no_browser,
        } => {
            // Load configuration
            let app_config = Config::load().unwrap_or_default();

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

            run_server(session, server_config).await?;
        }
        Commands::Share { file, tunnel, port } => {
            // Load configuration
            let app_config = Config::load().unwrap_or_default();

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

            let server_handle = start_server(session, server_config).await?;
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
        Commands::Config { action } => {
            handle_config_command(action)?;
        }
    }

    Ok(())
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
                _ => {
                    anyhow::bail!(
                        "Unknown configuration key '{}'. Valid keys: default_provider, ngrok_token, default_port",
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
                _ => {
                    anyhow::bail!(
                        "Unknown configuration key '{}'. Valid keys: default_provider, ngrok_token, default_port",
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
