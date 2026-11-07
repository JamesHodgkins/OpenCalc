//! Headless OpenCalc: `check` loads worksheets, evaluates them and reports problems.

mod report;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "opencalc-cli", version = opencalc_core::version(), about = "OpenCalc headless tool")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Load worksheets, evaluate them and report errors and warnings.
    ///
    /// Exit status: 0 if every file is clean, 1 if any has errors or warnings,
    /// 2 if any file cannot be opened.
    Check {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Print findings only, not the summary line for each file.
        #[arg(long, short)]
        quiet: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Check { files, quiet } => {
            let (mut out, mut err) = (std::io::stdout().lock(), std::io::stderr().lock());
            let mut worst = report::CLEAN;
            for file in &files {
                match report::check_file(file, quiet, &mut out, &mut err) {
                    Ok(code) => worst = worst.max(code),
                    // Output closed (a broken pipe): nothing more can be said.
                    Err(_) => return ExitCode::from(report::UNREADABLE),
                }
            }
            ExitCode::from(worst)
        }
    }
}
