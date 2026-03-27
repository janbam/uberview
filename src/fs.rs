use anyhow::{Context, Result, bail};
use ignore::WalkBuilder;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::language::{self, LanguageKind};

/// One resolved input file ready for parsing.
#[derive(Clone, Debug)]
pub struct DiscoveredFile {
    /// The absolute or cwd-resolved filesystem path.
    pub actual_path: PathBuf,
    /// The path shown in the rendered file header.
    pub display_path: String,
    /// The detected source language for the file.
    pub language: LanguageKind,
}

/// The resolved CLI target.
#[derive(Debug)]
pub enum InputTarget {
    /// A single supported source file.
    File(DiscoveredFile),
    /// A directory that should be scanned recursively.
    Directory(PathBuf),
}

/// Resolve the user-provided path into either a single file target or a directory scan root.
pub fn resolve_input(path: &Path) -> Result<InputTarget> {
    // Resolve relative inputs once so later file IO is independent of the caller's cwd.
    let actual_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .context("failed to resolve current working directory")?
            .join(path)
    };

    let metadata = fs::metadata(&actual_path)
        .with_context(|| format!("failed to inspect {}", actual_path.to_string_lossy()))?;

    if metadata.is_dir() {
        return Ok(InputTarget::Directory(actual_path));
    }

    if !metadata.is_file() {
        bail!("path is neither a regular file nor a directory");
    }

    let source = fs::read_to_string(&actual_path)
        .with_context(|| format!("failed to read {}", actual_path.to_string_lossy()))?;
    let language =
        language::require_language(&actual_path, Some(language::sniff_content_prefix(&source)))?;
    let display_path = display_single_file(path, &actual_path)?;

    Ok(InputTarget::File(DiscoveredFile {
        actual_path,
        display_path,
        language,
    }))
}

/// Recursively collect supported source files under a directory root.
pub fn discover_directory_files(root: &Path) -> Result<Vec<DiscoveredFile>> {
    // Walk without gitignore semantics so TreeBrief observes the actual tree except for the spec's defaults.
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .ignore(false)
        .filter_entry(|entry| !should_skip_path(entry.path()));

    let mut files = Vec::new();

    for entry in builder.build() {
        let entry =
            entry.with_context(|| format!("failed while walking {}", root.to_string_lossy()))?;
        let file_type = match entry.file_type() {
            Some(file_type) => file_type,
            None => continue,
        };

        if !file_type.is_file() {
            continue;
        }

        if !language::maybe_supported_path(entry.path()) {
            continue;
        }

        let source = match fs::read_to_string(entry.path()) {
            Ok(source) => source,
            Err(_) => continue,
        };

        let Some(language) =
            language::detect_language(entry.path(), Some(language::sniff_content_prefix(&source)))
        else {
            continue;
        };

        let display_path = entry
            .path()
            .strip_prefix(root)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .to_string();

        files.push(DiscoveredFile {
            actual_path: entry.into_path(),
            display_path,
            language,
        });
    }

    // Sort once after discovery so later parallel parsing can remain deterministic.
    files.sort_by(|left, right| left.display_path.cmp(&right.display_path));

    Ok(files)
}

/// Decide whether a path should be skipped by the default directory traversal behavior.
fn should_skip_path(path: &Path) -> bool {
    path.components().any(|component| match component {
        Component::Normal(name) => matches!(
            name.to_str(),
            Some(
                ".git" | "node_modules" | "dist" | "build" | "target" | ".venv" | "venv" | "vendor"
            )
        ),
        _ => false,
    })
}

/// Choose a stable display path for single-file invocations.
fn display_single_file(input: &Path, actual_path: &Path) -> Result<String> {
    if !input.is_absolute() {
        return Ok(input.to_string_lossy().to_string());
    }

    // Prefer a cwd-relative rendering when the user passed an absolute path from nearby.
    let cwd = std::env::current_dir().context("failed to resolve current working directory")?;

    Ok(actual_path
        .strip_prefix(&cwd)
        .unwrap_or(actual_path)
        .to_string_lossy()
        .to_string())
}
