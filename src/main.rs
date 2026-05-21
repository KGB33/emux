use std::{fs, path::PathBuf, process};

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
    /// Read a file and print its contents to stdout.
    Verify {
        /// Path to the file to verify.
        file: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Verify { file } => {
            let contents = fs::read_to_string(&file).unwrap_or_else(|err| {
                eprintln!("error: could not read `{}`: {err}", file.display());
                process::exit(1);
            });
            print!("{contents}");
        }
    }
}
