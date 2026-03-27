use clap::Parser;
use std::process::ExitCode;

use treebrief::{app, cli::Cli};

/// Parse CLI arguments and delegate execution to the application layer.
fn main() -> ExitCode {
    let cli = Cli::parse();

    match app::run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}
