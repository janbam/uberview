use anyhow::Result;
use rayon::prelude::*;
use std::collections::HashSet;
use std::fs as std_fs;

use crate::cli::Cli;
use crate::extract::{self, ExtractOptions};
use crate::fs::{DiscoveredFile, InputTarget, discover_directory_files, resolve_inputs};
use crate::model::{FileFailure, FileOutput, FileSection};
use crate::render::{self, RenderOptions};

/// Run TreeBrief for the parsed CLI input.
pub fn run(cli: Cli) -> Result<()> {
    // Carry the CLI toggles once so both single-file and directory modes render consistently.
    let render_options = RenderOptions {
        show_line_numbers_for_all_items: cli.show_line_numbers_for_all_items,
    };
    let extract_options = ExtractOptions {
        show_returns: cli.show_returns,
        show_top_level_symbols: cli.show_top_level_symbols,
    };

    let targets = resolve_inputs(&cli.paths)?;

    match targets.as_slice() {
        [InputTarget::File(file)] => {
            // Fail the whole run for a single-file invocation so an empty output never hides an error.
            let section = parse_file_strict(file, extract_options)?;
            println!(
                "{}",
                render::render_output(&FileOutput::Section(section), render_options)
            );
        }
        _ => {
            let files = collect_input_files(targets)?;
            if files.is_empty() {
                println!("No supported files found.");
                return Ok(());
            }

            // Keep the discovery order stable while parallelizing the parse and extraction work.
            let outputs = files
                .par_iter()
                .map(|file| parse_file_lenient(file, extract_options))
                .collect::<Vec<_>>();

            println!("{}", render::render_outputs(&outputs, render_options));
        }
    }

    Ok(())
}

/// Expand the resolved CLI targets into one deduplicated, source-ordered file list.
fn collect_input_files(targets: Vec<InputTarget>) -> Result<Vec<DiscoveredFile>> {
    let mut files = Vec::new();
    let mut seen_paths = HashSet::new();

    // Respect the user-provided root order while deduplicating concrete files across overlaps.
    for target in targets {
        match target {
            InputTarget::File(file) => push_unique_file(file, &mut seen_paths, &mut files),
            InputTarget::Directory(root) => {
                for file in discover_directory_files(&root)? {
                    push_unique_file(file, &mut seen_paths, &mut files);
                }
            }
        }
    }

    Ok(files)
}

/// Keep only the first occurrence of each concrete file path across all resolved roots.
fn push_unique_file(
    file: DiscoveredFile,
    seen_paths: &mut HashSet<std::path::PathBuf>,
    files: &mut Vec<DiscoveredFile>,
) {
    if seen_paths.insert(file.actual_path.clone()) {
        files.push(file);
    }
}

/// Parse one file strictly so a true single-file invocation still fails loudly on errors.
fn parse_file_strict(
    file: &DiscoveredFile,
    extract_options: ExtractOptions,
) -> Result<FileSection> {
    let source = std_fs::read_to_string(&file.actual_path)?;
    extract::extract_file(
        file.display_path.clone(),
        source,
        file.language,
        extract_options,
    )
}

/// Parse one file leniently so multi-root runs can keep going after individual failures.
fn parse_file_lenient(file: &DiscoveredFile, extract_options: ExtractOptions) -> FileOutput {
    match std_fs::read_to_string(&file.actual_path) {
        Ok(source) => match extract::extract_file(
            file.display_path.clone(),
            source,
            file.language,
            extract_options,
        ) {
            Ok(section) => FileOutput::Section(section),
            Err(error) => FileOutput::Failure(FileFailure {
                display_path: file.display_path.clone(),
                message: error.to_string(),
            }),
        },
        Err(error) => FileOutput::Failure(FileFailure {
            display_path: file.display_path.clone(),
            message: error.to_string(),
        }),
    }
}
