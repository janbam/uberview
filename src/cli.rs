use clap::Parser;
use std::path::PathBuf;

/// The command-line interface for TreeBrief.
#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about = "Print a compact, source-ordered structural overview of a source file or codebase."
)]
pub struct Cli {
    /// The file or directory to analyze.
    pub path: PathBuf,
}
