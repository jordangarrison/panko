use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

use agent_replay::parser::{ClaudeParser, SessionParser};
use agent_replay::server::{run_server, ServerConfig};

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
        Commands::Share { file } => {
            println!("Sharing session: {}", file.display());
            // TODO: Implement share command
        }
    }

    Ok(())
}
