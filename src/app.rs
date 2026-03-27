use anyhow::Result;
use rayon::prelude::*;
use std::fs as std_fs;

use crate::cli::Cli;
use crate::extract;
use crate::fs::{InputTarget, discover_directory_files, resolve_input};
use crate::model::{FileFailure, FileOutput};
use crate::render;

/// Run TreeBrief for the parsed CLI input.
pub fn run(cli: Cli) -> Result<()> {
    match resolve_input(&cli.path)? {
        InputTarget::File(file) => {
            // Fail the whole run for a single-file invocation so an empty output never hides an error.
            let source = std_fs::read_to_string(&file.actual_path)?;
            let section = extract::extract_file(file.display_path, source, file.language)?;
            println!("{}", render::render_output(&FileOutput::Section(section)));
        }
        InputTarget::Directory(root) => {
            let files = discover_directory_files(&root)?;

            if files.is_empty() {
                println!("No supported files found.");
                return Ok(());
            }

            // Keep the discovery order stable while parallelizing the parse and extraction work.
            let outputs = files
                .par_iter()
                .map(|file| match std_fs::read_to_string(&file.actual_path) {
                    Ok(source) => match extract::extract_file(
                        file.display_path.clone(),
                        source,
                        file.language,
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
                })
                .collect::<Vec<_>>();

            println!("{}", render::render_outputs(&outputs));
        }
    }

    Ok(())
}
