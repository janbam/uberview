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
    /// Extend line-number prefixes from definitions to every retained item.
    #[arg(long)]
    pub show_line_numbers_for_all_items: bool,
    /// Surface actual `return` statements in retained definition bodies.
    #[arg(long)]
    pub show_returns: bool,
    /// The file or directory to analyze.
    pub path: PathBuf,
}
