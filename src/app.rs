use anyhow::Result;
use rayon::prelude::*;
use std::collections::HashSet;
use std::fs as std_fs;
use std::path::PathBuf;

use crate::cli::Cli;
use crate::extract::{self, ExtractOptions};
use crate::fs::{
    DiscoveredFile, InputTarget, ResolvedInput, discover_directory_files, resolve_input,
    resolve_inputs_lenient,
};
use crate::json::{self as json_render, JsonRenderFile};
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

/// One parsed file paired with the path metadata needed by JSON rendering.
#[derive(Debug)]
struct JsonOutputItem {
    /// The canonical source path used for root-relative JSON paths.
    actual_path: PathBuf,
    /// The extracted file output, or a parse/read failure to report on stderr.
    output: FileOutput,
}

/// Run TreeBrief for the parsed CLI input.
pub fn run(cli: Cli) -> Result<()> {
    if cli.show_json_schema {
        println!("{}", json_render::render_schema());
        return Ok(());
    }

    if cli.json {
        return run_json(cli);
    }

    // Carry the CLI toggles once so both single-file and directory modes render consistently.
    let render_options = RenderOptions {
        show_line_numbers_for_all_items: cli.show_line_numbers_for_all_items,
    };
    let extract_options = ExtractOptions {
        show_returns: cli.show_returns,
        show_top_level_symbols: cli.show_top_level_symbols,
        hide_comments: cli.hide_comments,
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

/// Run Uberview in deterministic JSON mode, keeping diagnostics off stdout.
fn run_json(cli: Cli) -> Result<()> {
    let extract_options = ExtractOptions {
        show_returns: cli.show_returns,
        show_top_level_symbols: cli.show_top_level_symbols,
        hide_comments: cli.hide_comments,
    };

    // Preserve the existing strict behavior for a true explicit single-file parse failure.
    if let [path] = cli.paths.as_slice() {
        if let InputTarget::File(file) = resolve_input(path)? {
            let root = file
                .actual_path
                .parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| file.actual_path.clone());

            if should_exclude_file(&file, &cli) {
                println!("{}", json_render::render_document(&root, &[]));
                return Ok(());
            }

            let section = parse_file_strict(&file, extract_options)?;
            println!(
                "{}",
                json_render::render_document(
                    &root,
                    &[JsonRenderFile {
                        actual_path: &file.actual_path,
                        section: &section,
                    }],
                )
            );
            return Ok(());
        }
    }

    let inputs = resolve_inputs_lenient(&cli.paths);
    let root = json_root_for_inputs(&inputs)?;
    let work_items = collect_output_work(inputs, &cli);

    if work_items.is_empty() {
        println!("{}", json_render::render_document(&root, &[]));
        return Ok(());
    }

    // Parse files in parallel, but materialize diagnostics before rendering the single stdout document.
    let outputs = work_items
        .into_par_iter()
        .filter_map(|item| match item {
            OutputWorkItem::Ready(FileOutput::Failure(failure)) => Some(JsonOutputItem {
                actual_path: root.join(&failure.display_path),
                output: FileOutput::Failure(failure),
            }),
            OutputWorkItem::Ready(FileOutput::Section(_)) => None,
            OutputWorkItem::Pending(file) => Some(parse_file_json_lenient(&file, extract_options)),
        })
        .collect::<Vec<_>>();

    for item in &outputs {
        if let FileOutput::Failure(failure) = &item.output {
            eprintln!("{}: {}", failure.display_path, failure.message);
        }
    }

    let files = outputs
        .iter()
        .filter_map(|item| match &item.output {
            FileOutput::Section(section) => Some(JsonRenderFile {
                actual_path: &item.actual_path,
                section,
            }),
            FileOutput::Failure(_) => None,
        })
        .collect::<Vec<_>>();

    println!("{}", json_render::render_document(&root, &files));

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

/// Parse one file for JSON mode while preserving its canonical path for later serialization.
fn parse_file_json_lenient(
    file: &DiscoveredFile,
    extract_options: ExtractOptions,
) -> JsonOutputItem {
    JsonOutputItem {
        actual_path: file.actual_path.clone(),
        output: parse_file_lenient(file, extract_options),
    }
}

/// Choose the single absolute root used for JSON-relative file paths.
fn json_root_for_inputs(inputs: &[ResolvedInput]) -> Result<PathBuf> {
    let roots = inputs
        .iter()
        .filter_map(|input| match input {
            ResolvedInput::Target(InputTarget::File(file)) => {
                file.actual_path.parent().map(PathBuf::from)
            }
            ResolvedInput::Target(InputTarget::Directory { actual_path, .. }) => {
                Some(actual_path.clone())
            }
            ResolvedInput::Failure(_) => None,
        })
        .collect::<Vec<_>>();

    // Fall back to cwd when every input failed before a filesystem root could be resolved.
    Ok(common_ancestor(&roots).unwrap_or(std::env::current_dir()?))
}

/// Return the deepest path prefix shared by every candidate root.
fn common_ancestor(paths: &[PathBuf]) -> Option<PathBuf> {
    let mut ancestor = paths.first()?.clone();

    // Walk upward until every root lives under the candidate ancestor.
    while !paths.iter().all(|path| path.starts_with(&ancestor)) {
        if !ancestor.pop() {
            return None;
        }
    }

    Some(ancestor)
}
