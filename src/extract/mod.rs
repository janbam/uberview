mod javascript;
mod python;
mod rust;

use anyhow::Result;
use tree_sitter::{Node, Parser};

use crate::language::{self, LanguageKind};
use crate::model::{Definition, FileSection, Item, LineRange, Snippet, TextSpan};
use crate::source::SourceText;

/// Extract a rendered file section from one source file.
pub fn extract_file(
    display_path: String,
    source: String,
    language: LanguageKind,
) -> Result<FileSection> {
    // Parse once up front so the extraction pass can stay purely structural.
    let source = SourceText::new(source);
    let mut parser = Parser::new();
    language::configure_parser(&mut parser, language)?;
    let tree = language::parse_source(&mut parser, source.as_str())?;
    let root = tree.root_node();
    let items = collect_items(root, language, &source);

    Ok(FileSection {
        display_path,
        source,
        items,
    })
}

/// The extracted definition metadata needed to build the reduced internal model.
#[derive(Clone, Copy)]
struct DefinitionCapture<'tree> {
    /// The full source extent of the definition.
    span: TextSpan,
    /// The retained header slice emitted with the line-range prefix.
    header_span: TextSpan,
    /// The body scope walked for nested retained items.
    body: Option<Node<'tree>>,
}

/// Collect retained items under a scope node in strict source order.
fn collect_items(scope: Node<'_>, language: LanguageKind, source: &SourceText) -> Vec<Item> {
    let mut items = Vec::new();
    walk_children(scope, language, source, &mut items);
    dedupe_items(items)
}

/// Walk a scope recursively, pruning once definitions or retained snippets are captured.
fn walk_children(
    scope: Node<'_>,
    language: LanguageKind,
    source: &SourceText,
    items: &mut Vec<Item>,
) {
    let mut cursor = scope.walk();

    for child in scope.children(&mut cursor) {
        if !child.is_named() && !is_comment_node(language, child) {
            continue;
        }

        if should_skip_node(language, child) {
            continue;
        }

        if let Some(capture) = capture_definition(language, child, source) {
            items.push(Item::Definition(build_definition(
                capture, language, source,
            )));
            continue;
        }

        if let Some(span) = capture_snippet(language, child) {
            items.push(Item::Snippet(Snippet { span }));
            continue;
        }

        // Keep descending through omitted scaffolding so nested definitions, comments, and exits survive.
        walk_children(child, language, source, items);
    }
}

/// Materialize one captured definition and recurse into its retained body.
fn build_definition(
    capture: DefinitionCapture<'_>,
    language: LanguageKind,
    source: &SourceText,
) -> Definition {
    let items = capture
        .body
        .map(|body| collect_items(body, language, source))
        .unwrap_or_default();
    let start = source.line_number_at(capture.header_span.start_byte);
    let end = source.line_number_at(capture.span.end_byte.saturating_sub(1));

    Definition {
        span: capture.span,
        header_span: source.trim_trailing_line_breaks(capture.header_span),
        line_range: LineRange { start, end },
        items,
    }
}

/// Drop exact duplicate spans that can arise from recursive traversal through recovery nodes.
fn dedupe_items(items: Vec<Item>) -> Vec<Item> {
    let mut deduped = Vec::new();
    let mut last_start = None;

    for item in items {
        let start = item.start_byte();

        if last_start == Some(start) {
            continue;
        }

        last_start = Some(start);
        deduped.push(item);
    }

    deduped
}

/// Dispatch definition capture to the active language adapter.
fn capture_definition<'tree>(
    language: LanguageKind,
    node: Node<'tree>,
    source: &SourceText,
) -> Option<DefinitionCapture<'tree>> {
    match language {
        LanguageKind::Python => python::capture_definition(node, source),
        LanguageKind::JavaScript | LanguageKind::TypeScript | LanguageKind::Tsx => {
            javascript::capture_definition(language, node, source)
        }
        LanguageKind::Rust => rust::capture_definition(node, source),
    }
}

/// Dispatch retained snippet capture to the active language adapter.
fn capture_snippet(language: LanguageKind, node: Node<'_>) -> Option<TextSpan> {
    match language {
        LanguageKind::Python => python::capture_snippet(node),
        LanguageKind::JavaScript | LanguageKind::TypeScript | LanguageKind::Tsx => {
            javascript::capture_snippet(node)
        }
        LanguageKind::Rust => rust::capture_snippet(node),
    }
}

/// Dispatch comment-node detection to the active language adapter.
fn is_comment_node(language: LanguageKind, node: Node<'_>) -> bool {
    match language {
        LanguageKind::Python => python::is_comment_node(node),
        LanguageKind::JavaScript | LanguageKind::TypeScript | LanguageKind::Tsx => {
            javascript::is_comment_node(node)
        }
        LanguageKind::Rust => rust::is_comment_node(node),
    }
}

/// Skip helper nodes that are retained as part of a surrounding definition header.
fn should_skip_node(language: LanguageKind, node: Node<'_>) -> bool {
    match language {
        LanguageKind::Python => python::should_skip_node(node),
        LanguageKind::JavaScript | LanguageKind::TypeScript | LanguageKind::Tsx => {
            javascript::should_skip_node(node)
        }
        LanguageKind::Rust => rust::should_skip_node(node),
    }
}

/// Build a span directly from one syntax node.
fn node_span(node: Node<'_>) -> TextSpan {
    TextSpan::new(node.start_byte(), node.end_byte())
}

/// Build a span from an explicit byte range.
fn explicit_span(start_byte: usize, end_byte: usize) -> TextSpan {
    TextSpan::new(start_byte, end_byte)
}

/// Extend a header slice to include the opening delimiter of a body node when present.
fn body_header_end(source: &SourceText, body: Node<'_>) -> usize {
    match source.as_str().as_bytes().get(body.start_byte()) {
        Some(b'{' | b'(' | b'[') => body.start_byte() + 1,
        _ => body.start_byte(),
    }
}

/// Expand a node's start to include adjacent prefix siblings such as Rust attributes.
fn expand_start_with_prefix_siblings(
    node: Node<'_>,
    should_attach: impl Fn(Node<'_>) -> bool,
) -> usize {
    let mut start = node.start_byte();
    let mut sibling = node.prev_sibling();

    // Walk backward only across directly attached prefix nodes.
    while let Some(current) = sibling {
        if !should_attach(current) {
            break;
        }

        start = current.start_byte();
        sibling = current.prev_sibling();
    }

    start
}
