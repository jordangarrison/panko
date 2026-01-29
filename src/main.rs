use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

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
    },
    /// Share a session file via a public tunnel
    Share {
        /// Path to the session file
        file: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::View { file } => {
            println!("Viewing session: {}", file.display());
            // TODO: Implement view command
        }
        Commands::Share { file } => {
            println!("Sharing session: {}", file.display());
            // TODO: Implement share command
        }
    }

    Ok(())
}
