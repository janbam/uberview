use serde::Serialize;
use serde_json::json;
use std::path::Path;

use crate::language::LanguageKind;
use crate::model::{Definition, DefinitionKind, FileSection, Item, LineRange, Snippet};

/// One successfully extracted file plus its canonical filesystem path.
#[derive(Clone, Copy)]
pub struct JsonRenderFile<'a> {
    /// The absolute path used to make the JSON file path relative to the document root.
    pub actual_path: &'a Path,
    /// The extracted section whose retained structure is serialized.
    pub section: &'a FileSection,
}

/// Render the schema document for `--show-json-schema`.
pub fn render_schema() -> String {
    // Keep the schema as data so the CLI prints one deterministic JSON document, just like normal JSON mode.
    serde_json::to_string_pretty(&json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "uberview/1",
        "title": "uberview JSON output",
        "type": "object",
        "additionalProperties": false,
        "required": ["schema", "root", "files"],
        "properties": {
            "schema": { "const": "uberview/1" },
            "root": { "type": "string" },
            "files": {
                "type": "array",
                "items": { "$ref": "#/$defs/file" }
            }
        },
        "$defs": {
            "range": {
                "type": "array",
                "prefixItems": [
                    { "type": "integer", "minimum": 1 },
                    { "type": "integer", "minimum": 1 }
                ],
                "items": false,
                "minItems": 2,
                "maxItems": 2
            },
            "file": {
                "type": "object",
                "additionalProperties": false,
                "required": ["path", "language", "lines"],
                "oneOf": [
                    { "required": ["symbols"], "not": { "required": ["headings"] } },
                    { "required": ["headings"], "not": { "required": ["symbols"] } }
                ],
                "properties": {
                    "path": { "type": "string" },
                    "language": { "enum": ["py", "ts", "md", "js", "tsx", "rs", "lua"] },
                    "lines": { "type": "integer", "minimum": 0 },
                    "symbols": {
                        "type": "array",
                        "items": { "$ref": "#/$defs/symbol" }
                    },
                    "headings": {
                        "type": "array",
                        "items": { "$ref": "#/$defs/heading" }
                    }
                }
            },
            "symbol": {
                "type": "object",
                "additionalProperties": false,
                "required": [
                    "kind",
                    "name",
                    "range",
                    "exported",
                    "signature",
                    "inlineComments",
                    "children"
                ],
                "properties": {
                    "kind": { "type": "string" },
                    "name": { "type": "string" },
                    "range": { "$ref": "#/$defs/range" },
                    "exported": { "type": "boolean" },
                    "signature": { "type": "string" },
                    "doc": { "type": "string" },
                    "inlineComments": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "children": {
                        "type": "array",
                        "items": { "$ref": "#/$defs/symbol" }
                    }
                }
            },
            "heading": {
                "type": "object",
                "additionalProperties": false,
                "required": ["title", "level", "range", "children"],
                "properties": {
                    "title": { "type": "string" },
                    "level": { "type": "integer", "minimum": 1, "maximum": 6 },
                    "range": { "$ref": "#/$defs/range" },
                    "children": {
                        "type": "array",
                        "items": { "$ref": "#/$defs/heading" }
                    }
                }
            }
        }
    }))
    .expect("schema document should serialize")
}

/// Render one deterministic JSON document for extracted files.
pub fn render_document(root: &Path, files: &[JsonRenderFile<'_>]) -> String {
    let mut files = files
        .iter()
        .map(|file| render_file(root, file))
        .collect::<Vec<_>>();

    // Sort by the exact JSON path so parallel extraction and input order cannot leak into machine output.
    files.sort_by(|left, right| left.path.cmp(&right.path));

    let document = JsonDocument {
        schema: "uberview/1",
        root: path_to_posix(root),
        files,
    };

    serde_json::to_string_pretty(&document).expect("JSON document should serialize")
}

#[derive(Serialize)]
struct JsonDocument {
    schema: &'static str,
    root: String,
    files: Vec<JsonFile>,
}

#[derive(Serialize)]
struct JsonFile {
    path: String,
    language: &'static str,
    lines: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    symbols: Option<Vec<JsonSymbol>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    headings: Option<Vec<JsonHeading>>,
}

#[derive(Serialize)]
struct JsonSymbol {
    kind: &'static str,
    name: String,
    range: [usize; 2],
    exported: bool,
    signature: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    doc: Option<String>,
    #[serde(rename = "inlineComments")]
    inline_comments: Vec<String>,
    children: Vec<JsonSymbol>,
}

#[derive(Serialize)]
struct JsonHeading {
    title: String,
    level: u8,
    range: [usize; 2],
    children: Vec<JsonHeading>,
}

/// Render one file entry, choosing the mutually exclusive structure field by language.
fn render_file(root: &Path, file: &JsonRenderFile<'_>) -> JsonFile {
    let path = relative_posix_path(root, file.actual_path);
    let language = file.section.language;
    let lines = file.section.source.line_count();

    if language == LanguageKind::Markdown {
        return JsonFile {
            path,
            language: language.json_code(),
            lines,
            symbols: None,
            headings: Some(render_headings(&file.section.items, file.section)),
        };
    }

    JsonFile {
        path,
        language: language.json_code(),
        lines,
        symbols: Some(render_symbols(
            &file.section.items,
            file.section,
            language,
            true,
            None,
        )),
        headings: None,
    }
}

/// Render retained definitions as source symbols, ignoring non-symbol top-level snippets.
fn render_symbols(
    items: &[Item],
    section: &FileSection,
    language: LanguageKind,
    parent_reachable: bool,
    parent_kind: Option<DefinitionKind>,
) -> Vec<JsonSymbol> {
    items
        .iter()
        .filter_map(|item| {
            let Item::Definition(definition) = item else {
                return None;
            };

            Some(render_symbol(
                definition,
                section,
                language,
                parent_reachable,
                parent_kind,
            ))
        })
        .collect()
}

/// Render one retained definition with doc text, body comments, and nested child symbols.
fn render_symbol(
    definition: &Definition,
    section: &FileSection,
    language: LanguageKind,
    parent_reachable: bool,
    parent_kind: Option<DefinitionKind>,
) -> JsonSymbol {
    let signature = section
        .source
        .rendered_lines(definition.header_span)
        .join("\n");
    let exported = symbol_is_exported(
        definition,
        language,
        parent_reachable,
        parent_kind,
        &signature,
    );
    let doc = symbol_doc(definition, section);
    let inline_comments = inline_comments(definition, section, doc.is_some());
    let child_parent_reachable = child_parent_reachability(language, definition.kind, exported);
    let children = render_symbols(
        &definition.items,
        section,
        language,
        child_parent_reachable,
        Some(definition.kind),
    );

    JsonSymbol {
        kind: definition.kind.json_kind(),
        name: definition.name.clone(),
        range: line_range_array(definition.line_range),
        exported,
        signature,
        doc,
        inline_comments,
        children,
    }
}

/// Decide whether child symbols should evaluate their own public-surface marker.
fn child_parent_reachability(
    language: LanguageKind,
    parent_kind: DefinitionKind,
    parent_exported: bool,
) -> bool {
    // Rust `impl` blocks are not themselves public declarations, but their `pub` methods can be.
    parent_exported || (language == LanguageKind::Rust && parent_kind == DefinitionKind::Impl)
}

/// Render retained Markdown heading definitions as heading nodes.
fn render_headings(items: &[Item], section: &FileSection) -> Vec<JsonHeading> {
    items
        .iter()
        .filter_map(|item| {
            let Item::Definition(definition) = item else {
                return None;
            };

            if definition.kind != DefinitionKind::Heading {
                return None;
            }

            Some(JsonHeading {
                title: definition.name.clone(),
                level: heading_level(definition, section),
                range: line_range_array(definition.line_range),
                children: render_headings(&definition.items, section),
            })
        })
        .collect()
}

/// Decide whether one symbol is reachable from the file's public surface.
fn symbol_is_exported(
    definition: &Definition,
    language: LanguageKind,
    parent_reachable: bool,
    parent_kind: Option<DefinitionKind>,
    signature: &str,
) -> bool {
    if !parent_reachable {
        return false;
    }

    // TypeScript/JavaScript exported containers expose their public members, but not arbitrary nested helpers.
    if matches!(
        language,
        LanguageKind::JavaScript | LanguageKind::TypeScript | LanguageKind::Tsx
    ) && matches!(
        definition.kind,
        DefinitionKind::Method | DefinitionKind::Property | DefinitionKind::Field
    ) {
        return definition.exported || !is_private_member(definition, signature);
    }

    // Python exposes module-level names and public members on exported classes, not function-local helpers.
    if language == LanguageKind::Python {
        return match parent_kind {
            None => definition.exported,
            Some(DefinitionKind::Class) => definition.exported,
            _ => false,
        };
    }

    definition.exported
}

/// Detect member syntax that should not be treated as a public class/interface surface.
fn is_private_member(definition: &Definition, signature: &str) -> bool {
    let trimmed = signature.trim_start();

    definition.name.starts_with('#')
        || trimmed.starts_with("private ")
        || trimmed.starts_with("protected ")
        || trimmed.starts_with("#")
}

/// Return a stripped documentation comment or docstring when the retained model exposes one.
fn symbol_doc(definition: &Definition, section: &FileSection) -> Option<String> {
    if !definition.leading_comment_snippets.is_empty() {
        let doc = definition
            .leading_comment_snippets
            .iter()
            .map(|snippet| stripped_snippet_text(*snippet, section))
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n");

        return (!doc.is_empty()).then_some(doc);
    }

    let first_snippet = definition.items.iter().find_map(|item| match item {
        Item::Snippet(snippet) => Some(*snippet),
        _ => None,
    })?;
    let raw = section.source.span_text(first_snippet.span).trim();

    is_docstring_literal(raw).then(|| strip_docstring_markers(raw))
}

/// Collect direct retained body comments, preserving source order and intentionally omitting anchors.
fn inline_comments(
    definition: &Definition,
    section: &FileSection,
    skip_first_doc_snippet: bool,
) -> Vec<String> {
    let mut skipped_doc = false;
    let mut comments = Vec::new();

    for item in &definition.items {
        let Item::Snippet(snippet) = item else {
            continue;
        };

        let raw = section.source.span_text(snippet.span).trim();
        if skip_first_doc_snippet && !skipped_doc && is_docstring_literal(raw) {
            skipped_doc = true;
            continue;
        }

        let text = stripped_snippet_text(*snippet, section);
        if !text.is_empty() {
            comments.push(text);
        }
    }

    comments
}

/// Strip language comment or docstring markers from one retained snippet.
fn stripped_snippet_text(snippet: Snippet, section: &FileSection) -> String {
    let raw = section.source.span_text(snippet.span).trim();

    if is_docstring_literal(raw) {
        return strip_docstring_markers(raw);
    }

    if raw.starts_with("/*") {
        return strip_block_comment_markers(raw);
    }

    raw.lines()
        .map(strip_line_comment_marker)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}

/// Report whether text looks like a Python string literal used as a docstring.
fn is_docstring_literal(raw: &str) -> bool {
    let trimmed = raw.trim_start_matches(['r', 'R', 'u', 'U', 'f', 'F', 'b', 'B']);

    trimmed.starts_with("\"\"\"")
        || trimmed.starts_with("'''")
        || trimmed.starts_with('"')
        || trimmed.starts_with('\'')
}

/// Strip Python string delimiters while preserving the docstring body.
fn strip_docstring_markers(raw: &str) -> String {
    let trimmed = raw
        .trim()
        .trim_start_matches(['r', 'R', 'u', 'U', 'f', 'F', 'b', 'B']);

    for marker in ["\"\"\"", "'''", "\"", "'"] {
        if let Some(body) = trimmed
            .strip_prefix(marker)
            .and_then(|text| text.strip_suffix(marker))
        {
            return body.trim().to_owned();
        }
    }

    trimmed.to_owned()
}

/// Strip JavaScript/Rust block comment delimiters and leading star margin.
fn strip_block_comment_markers(raw: &str) -> String {
    let inner = raw
        .trim_start_matches("/**")
        .trim_start_matches("/*!")
        .trim_start_matches("/*")
        .trim_end_matches("*/");

    inner
        .lines()
        .map(|line| {
            line.trim_start()
                .strip_prefix('*')
                .unwrap_or(line.trim_start())
                .trim_start()
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}

/// Strip one line-oriented comment marker while preserving the comment text.
fn strip_line_comment_marker(line: &str) -> String {
    let trimmed = line.trim_start();

    for marker in ["///", "//!", "//", "---", "--", "#"] {
        if let Some(rest) = trimmed.strip_prefix(marker) {
            return rest.trim_start().to_owned();
        }
    }

    trimmed.to_owned()
}

/// Infer the Markdown heading level from the retained heading source.
fn heading_level(definition: &Definition, section: &FileSection) -> u8 {
    let header = section.source.span_text(definition.header_span);
    let first_line = header.lines().next().unwrap_or_default().trim_start();

    if first_line.starts_with('#') {
        return first_line.bytes().take_while(|byte| *byte == b'#').count() as u8;
    }

    header
        .lines()
        .nth(1)
        .map(str::trim)
        .map(|line| if line.starts_with('=') { 1 } else { 2 })
        .unwrap_or(1)
}

/// Convert a line range into the array shape required by the JSON contract.
fn line_range_array(range: LineRange) -> [usize; 2] {
    [range.start, range.end]
}

/// Return a stable POSIX-style path relative to the JSON document root.
fn relative_posix_path(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    path_to_posix(relative)
}

/// Render any platform path with forward slashes for deterministic JSON.
fn path_to_posix(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
