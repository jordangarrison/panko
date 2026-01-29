use anyhow::{Context, Result};
use arboard::Clipboard;
use clap::{Parser, Subcommand};
use inquire::Select;
use std::path::{Path, PathBuf};

use agent_replay::parser::{ClaudeParser, SessionParser};
use agent_replay::server::{run_server, shutdown_signal, start_server, ServerConfig};
use agent_replay::tunnel::{detect_available_providers, get_provider, AvailableProvider};

#[derive(Parser)]
#[command(name = "agent-replay")]
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
}

/// List of available parsers.
fn get_parsers() -> Vec<Box<dyn SessionParser>> {
    vec![Box::new(ClaudeParser::new())]
}

/// Parse a session file using available parsers.
fn parse_session(path: &Path) -> Result<agent_replay::parser::Session> {
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

            // Run the server
            let config = ServerConfig {
                base_port: port,
                open_browser: !no_browser,
            };

            run_server(session, config).await?;
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

            // Select tunnel provider
            let selected_provider = if let Some(tunnel_name) = tunnel {
                // User explicitly specified a provider
                let provider = get_provider(&tunnel_name).ok_or_else(|| {
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

            // Start the local server (don't open browser for share)
            let config = ServerConfig {
                base_port: port,
                open_browser: false,
            };

            let server_handle = start_server(session, config).await?;
            let port = server_handle.port();

            println!("Local server running at: {}", server_handle.local_url());

            // Spawn the tunnel
            println!("Starting {} tunnel...", selected_provider.display_name);

            let provider = get_provider(selected_provider.name).ok_or_else(|| {
                anyhow::anyhow!("Failed to get provider: {}", selected_provider.name)
            })?;

            let mut tunnel_handle = match provider.spawn(port) {
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
    }

    Ok(())
}
