//! Headless OpenCalc: `check` evaluates a document and reports problems.
//!
//! Milestone 1 skeleton: the subcommands parse but do not yet read documents.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "opencalc-cli", version = opencalc_core::version(), about = "OpenCalc headless tool")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Evaluate a worksheet and report errors and warnings. Exit 0 if clean.
    Check { file: PathBuf },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Check { file } => {
            eprintln!("check: not implemented yet ({})", file.display());
            ExitCode::from(2)
        }
    }
}
