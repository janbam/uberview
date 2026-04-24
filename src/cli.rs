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
    /// Restore explicit top-level assignment and constant-style symbol definitions.
    #[arg(long)]
    pub show_top_level_symbols: bool,
    /// Hide plain inline comments while keeping docstrings and documentation comments.
    #[arg(long)]
    pub hide_comments: bool,
    /// Skip Markdown files during analysis, including recursive directory scans.
    #[arg(long)]
    pub exclude_markdown: bool,
    /// One or more files or directories to analyze.
    /// Directory inputs render file headers relative to each provided root.
    #[arg(required = true)]
    pub paths: Vec<PathBuf>,
}
