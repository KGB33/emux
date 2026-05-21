mod commands;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// Environment Multiplexer — manage per-worktree configuration overrides.
#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Verify the syntax of a Lua file.
    Verify {
        /// Path to the Lua file to verify.
        file: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Verify { file } => commands::verify::run(file),
    }
}
