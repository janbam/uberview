use anyhow::Result;
use rayon::prelude::*;
use std::collections::HashSet;
use std::fs as std_fs;

use crate::cli::Cli;
use crate::extract::{self, ExtractOptions};
use crate::fs::{
    DiscoveredFile, InputTarget, ResolvedInput, discover_directory_files, resolve_input,
    resolve_inputs_lenient,
};
use crate::model::{FileFailure, FileOutput, FileSection};
use crate::render::{self, RenderOptions};

/// One ordered multi-input output slot, either ready now or waiting on file parsing.
#[derive(Debug)]
enum OutputWorkItem {
    /// A ready-to-render non-fatal failure discovered before parsing.
    Ready(FileOutput),
    /// A source file that should still flow through the lenient parse path.
    Pending(DiscoveredFile),
}

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

    // Keep the true single-file path loud so an explicit broken file never looks like a partial success.
    if let [path] = cli.paths.as_slice() {
        if let InputTarget::File(file) = resolve_input(path)? {
            // Respect language exclusions even for explicit single-file invocations.
            if should_exclude_file(&file, &cli) {
                println!("No supported files found.");
                return Ok(());
            }

            let section = parse_file_strict(&file, extract_options)?;
            println!(
                "{}",
                render::render_output(&FileOutput::Section(section), render_options)
            );
            return Ok(());
        }
    }

    let work_items = collect_output_work(resolve_inputs_lenient(&cli.paths), &cli);
    if work_items.is_empty() {
        println!("No supported files found.");
        return Ok(());
    }

    // Keep the discovery order stable while parallelizing only the file-local parse and extraction work.
    let outputs = work_items
        .into_par_iter()
        .map(|item| match item {
            OutputWorkItem::Ready(output) => output,
            OutputWorkItem::Pending(file) => parse_file_lenient(&file, extract_options),
        })
        .collect::<Vec<_>>();

    println!("{}", render::render_outputs(&outputs, render_options));

    Ok(())
}

/// Expand the resolved CLI targets into one ordered stream of rendered failures and parse work.
fn collect_output_work(inputs: Vec<ResolvedInput>, cli: &Cli) -> Vec<OutputWorkItem> {
    let mut work_items = Vec::new();
    let mut seen_paths = HashSet::new();

    // Respect the user-provided root order while keeping target-local failures inline.
    for input in inputs {
        match input {
            ResolvedInput::Failure(failure) => {
                work_items.push(OutputWorkItem::Ready(FileOutput::Failure(failure)));
            }
            ResolvedInput::Target(target) => match target {
                InputTarget::File(file) => {
                    if !should_exclude_file(&file, cli) {
                        push_unique_file(file, &mut seen_paths, &mut work_items);
                    }
                }
                InputTarget::Directory {
                    actual_path,
                    display_path,
                } => match discover_directory_files(&actual_path) {
                    Ok(files) => {
                        for file in files {
                            if !should_exclude_file(&file, cli) {
                                push_unique_file(file, &mut seen_paths, &mut work_items);
                            }
                        }
                    }
                    Err(error) => {
                        work_items.push(OutputWorkItem::Ready(FileOutput::Failure(FileFailure {
                            display_path,
                            message: error.to_string(),
                        })));
                    }
                },
            },
        }
    }

    work_items
}

/// Decide whether one discovered file should be skipped by the current CLI filters.
fn should_exclude_file(file: &DiscoveredFile, cli: &Cli) -> bool {
    // Keep Markdown opt-out centralized so single-file and directory flows agree.
    cli.exclude_markdown && file.language == crate::language::LanguageKind::Markdown
}

/// Keep only the first occurrence of each concrete file path across all resolved roots.
fn push_unique_file(
    file: DiscoveredFile,
    seen_paths: &mut HashSet<std::path::PathBuf>,
    work_items: &mut Vec<OutputWorkItem>,
) {
    if seen_paths.insert(file.actual_path.clone()) {
        work_items.push(OutputWorkItem::Pending(file));
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
