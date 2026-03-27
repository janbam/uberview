use anyhow::{Context, Result};
use std::ffi::OsStr;
use std::path::Path;
use tree_sitter::{Language, Parser};

/// The supported source languages.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LanguageKind {
    /// Python source files.
    Python,
    /// JavaScript source files.
    JavaScript,
    /// TypeScript source files.
    TypeScript,
    /// TSX source files.
    Tsx,
    /// Rust source files.
    Rust,
}

impl LanguageKind {
    /// Return the tree-sitter grammar for this language.
    pub fn grammar(self) -> Language {
        match self {
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
        }
    }
}

/// Configure a parser for the selected language.
pub fn configure_parser(parser: &mut Parser, language: LanguageKind) -> Result<()> {
    parser
        .set_language(&language.grammar())
        .with_context(|| format!("failed to load {language:?} grammar"))
}

/// Detect the language from path metadata and optional leading file content.
pub fn detect_language(path: &Path, sniff: Option<&str>) -> Option<LanguageKind> {
    // Honor the fast extension path first so common cases stay cheap.
    if let Some(language) = detect_by_extension(path.extension()) {
        return Some(language);
    }

    // Fall back to a small filename table for well-known no-extension entrypoints.
    if let Some(language) = detect_by_filename(path.file_name()) {
        return Some(language);
    }

    // Use a lightweight shebang sniff only when the filename surface is inconclusive.
    sniff.and_then(detect_by_shebang)
}

/// Determine whether a path could plausibly be a supported source file.
pub fn maybe_supported_path(path: &Path) -> bool {
    // Avoid opening obviously unsupported extensions during directory scans.
    detect_by_extension(path.extension()).is_some()
        || detect_by_filename(path.file_name()).is_some()
        || path.extension().is_none()
}

/// Require a supported language for one concrete input file.
pub fn require_language(path: &Path, sniff: Option<&str>) -> Result<LanguageKind> {
    detect_language(path, sniff)
        .with_context(|| format!("unsupported source file: {}", path.to_string_lossy()))
}

/// Map a file extension to a supported language.
fn detect_by_extension(extension: Option<&OsStr>) -> Option<LanguageKind> {
    let extension = extension.and_then(OsStr::to_str)?;

    match extension {
        "py" => Some(LanguageKind::Python),
        "js" | "jsx" | "mjs" | "cjs" => Some(LanguageKind::JavaScript),
        "ts" | "mts" | "cts" => Some(LanguageKind::TypeScript),
        "tsx" => Some(LanguageKind::Tsx),
        "rs" => Some(LanguageKind::Rust),
        _ => None,
    }
}

/// Map well-known filenames without useful extensions to a supported language.
fn detect_by_filename(file_name: Option<&OsStr>) -> Option<LanguageKind> {
    let file_name = file_name.and_then(OsStr::to_str)?;

    match file_name {
        "Jakefile" | "jakefile" => Some(LanguageKind::JavaScript),
        _ => None,
    }
}

/// Infer a language from a shebang line when the path itself is ambiguous.
fn detect_by_shebang(sniff: &str) -> Option<LanguageKind> {
    let first_line = sniff.lines().next()?.trim();

    if !first_line.starts_with("#!") {
        return None;
    }

    if first_line.contains("python") {
        return Some(LanguageKind::Python);
    }

    if first_line.contains("node") || first_line.contains("deno") || first_line.contains("bun") {
        return Some(LanguageKind::JavaScript);
    }

    None
}

/// Read the first line-sized prefix used for lightweight content sniffing.
pub fn sniff_content_prefix(source: &str) -> &str {
    const SNIFF_LIMIT: usize = 256;

    &source[..source.len().min(SNIFF_LIMIT)]
}

/// Ensure a parser was produced after configuration.
pub fn parse_source(parser: &mut Parser, source: &str) -> Result<tree_sitter::Tree> {
    parser
        .parse(source, None)
        .context("tree-sitter did not return a syntax tree")
}

#[cfg(test)]
mod tests {
    use super::{LanguageKind, detect_language, maybe_supported_path};
    use std::path::Path;

    /// Verify that the fast directory-scan prefilter excludes obviously unsupported extensions.
    #[test]
    fn maybe_supported_path_filters_by_supported_extensions_or_extensionless_files() {
        // Keep extensionless files eligible for shebang sniffing but prune unrelated extensions early.
        assert!(maybe_supported_path(Path::new("main.py")));
        assert!(maybe_supported_path(Path::new("script")));
        assert!(!maybe_supported_path(Path::new("README.md")));
        assert!(!maybe_supported_path(Path::new("data.json")));
    }

    /// Verify that extensionless files still fall through to shebang-based detection.
    #[test]
    fn detect_language_uses_shebang_for_extensionless_files() {
        // Ensure the prefilter tightening does not regress shebang support.
        let language = detect_language(Path::new("tool"), Some("#!/usr/bin/env python3\n"));

        assert_eq!(language, Some(LanguageKind::Python));
    }
}
