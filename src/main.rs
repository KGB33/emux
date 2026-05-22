mod commands;
mod config;
mod lua_api;

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
    /// Verify the syntax of a Lua or Fennel file.
    Verify {
        /// Path to the Lua (.lua) or Fennel (.fnl) file to verify.
        file: PathBuf,
    },
    /// Run a Lua config file with the emux library loaded.
    Run {
        /// Path to the Lua (.lua) or Fennel (.fnl) config file to run.
        file: PathBuf,
    },
    /// Show what changes would be made without applying them.
    Diff {
        /// Path to the Lua (.lua) or Fennel (.fnl) config file.
        file: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Verify { file } => commands::verify::run(file),
        Commands::Run { file } => commands::run::run(file),
        Commands::Diff { file } => commands::diff::run(file),
    }
}
