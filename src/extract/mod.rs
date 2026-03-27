mod javascript;
mod python;
mod rust;
use anyhow::{Result, bail};
use tree_sitter::{Node, Parser, Point};

use crate::language::{self, LanguageKind};
use crate::model::{
    Definition, DefinitionKind, FileSection, Item, LineRange, SkippedRange, Snippet, TextSpan,
};
use crate::source::SourceText;

/// The extraction toggles that control which retained source surfaces are emitted.
#[derive(Clone, Copy, Debug, Default)]
pub struct ExtractOptions {
    /// Surface actual `return` statements when the caller explicitly asks for them.
    pub show_returns: bool,
    /// Restore explicit top-level symbol definitions instead of collapsing them into placeholders.
    pub show_top_level_symbols: bool,
}

/// Extract a rendered file section from one source file.
pub fn extract_file(
    display_path: String,
    source: String,
    language: LanguageKind,
    options: ExtractOptions,
) -> Result<FileSection> {
    // Parse once up front so the extraction pass can stay purely structural.
    let source = SourceText::new(source);
    let mut parser = Parser::new();
    language::configure_parser(&mut parser, language)?;
    let tree = language::parse_source(&mut parser, source.as_str())?;
    let root = tree.root_node();
    ensure_tree_has_no_syntax_errors(root, &display_path)?;
    let items = collect_items(root, language, &source, options);
    let items = if options.show_top_level_symbols {
        items
    } else {
        collapse_top_level_symbols(items, &source)
    };

    Ok(FileSection {
        display_path,
        source,
        items,
    })
}

/// Reject syntax trees that required error recovery so callers can report clear file-level failures.
fn ensure_tree_has_no_syntax_errors(root: Node<'_>, display_path: &str) -> Result<()> {
    if !root.has_error() {
        return Ok(());
    }

    let (kind, point) = first_syntax_issue(root)
        .map(|node| syntax_issue_details(node))
        .unwrap_or(("syntax error".to_owned(), root.start_position()));

    bail!(
        "parse failed for {display_path}: {kind} near line {}, column {}",
        point.row + 1,
        point.column + 1
    );
}

/// The extracted definition metadata needed to build the reduced internal model.
#[derive(Clone)]
struct DefinitionCapture<'tree> {
    /// The user-facing structural kind attached to the definition.
    kind: DefinitionKind,
    /// The user-facing name attached to the definition.
    name: String,
    /// The full source extent of the definition.
    span: TextSpan,
    /// The retained header slice emitted with the line-range prefix.
    header_span: TextSpan,
    /// The body scope walked for nested retained items.
    body: Option<Node<'tree>>,
}

/// Collect retained items under a scope node in strict source order.
fn collect_items(
    scope: Node<'_>,
    language: LanguageKind,
    source: &SourceText,
    options: ExtractOptions,
) -> Vec<Item> {
    let mut items = Vec::new();
    walk_children(scope, language, source, options, &mut items);
    dedupe_items(items)
}

/// Walk a scope recursively, pruning once definitions or retained snippets are captured.
fn walk_children(
    scope: Node<'_>,
    language: LanguageKind,
    source: &SourceText,
    options: ExtractOptions,
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
                capture, language, source, options,
            )));
            continue;
        }

        if let Some(span) = capture_snippet(language, child, options) {
            items.push(Item::Snippet(Snippet { span }));
            continue;
        }

        // Keep descending through omitted scaffolding so nested definitions, comments, and opt-in returns survive.
        walk_children(child, language, source, options, items);
    }
}

/// Materialize one captured definition and recurse into its retained body.
fn build_definition(
    capture: DefinitionCapture<'_>,
    language: LanguageKind,
    source: &SourceText,
    options: ExtractOptions,
) -> Definition {
    let items = capture
        .body
        .map(|body| collect_items(body, language, source, options))
        .unwrap_or_default();
    let start = source.line_number_at(capture.header_span.start_byte);
    let end = source.line_number_at(capture.span.end_byte.saturating_sub(1));

    Definition {
        // Keep the header metadata fully source-derived so later rendering stays deterministic.
        kind: capture.kind,
        name: capture.name,
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

/// Collapse adjacent top-level assignment-like definitions into one synthetic placeholder run.
fn collapse_top_level_symbols(items: Vec<Item>, source: &SourceText) -> Vec<Item> {
    let mut collapsed = Vec::new();
    let mut items = items.into_iter().map(Some).collect::<Vec<_>>();
    let mut index = 0;

    while index < items.len() {
        if !belongs_to_skipped_symbol_run(&items, index, source) {
            if let Some(item) = items[index].take() {
                collapsed.push(item);
            }

            index += 1;
            continue;
        }

        let mut run: Option<SkippedSymbolRun> = None;

        while index < items.len() && belongs_to_skipped_symbol_run(&items, index, source) {
            let Some(item) = items[index].take() else {
                index += 1;
                continue;
            };

            if let Item::Definition(definition) = item {
                run = Some(match run {
                    Some(mut run) => {
                        run.extend(definition, source);
                        run
                    }
                    None => SkippedSymbolRun::new(definition, source),
                });
            }

            index += 1;
        }

        if let Some(run) = run {
            collapsed.push(Item::SkippedRange(run.finish()));
        }
    }

    collapsed
}

/// Decide whether a top-level definition should be hidden behind a symbol placeholder by default.
fn should_skip_top_level_symbol(definition: &Definition) -> bool {
    matches!(
        definition.kind,
        DefinitionKind::Assignment | DefinitionKind::Constant | DefinitionKind::Variable
    )
}

/// Decide whether an item should be absorbed into the current skipped top-level symbol run.
fn belongs_to_skipped_symbol_run(
    items: &[Option<Item>],
    index: usize,
    source: &SourceText,
) -> bool {
    let Some(item) = items.get(index).and_then(|item| item.as_ref()) else {
        return false;
    };

    match item {
        Item::Definition(definition) => should_skip_top_level_symbol(definition),
        Item::Snippet(snippet) => {
            is_comment_like_snippet(source, *snippet)
                && (comment_adjoins_skipped_symbol(items, index, source, -1)
                    || comment_adjoins_skipped_symbol(items, index, source, 1))
        }
        Item::SkippedRange(_) => false,
    }
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
fn capture_snippet(
    language: LanguageKind,
    node: Node<'_>,
    options: ExtractOptions,
) -> Option<TextSpan> {
    match language {
        LanguageKind::Python => python::capture_snippet(node, options),
        LanguageKind::JavaScript | LanguageKind::TypeScript | LanguageKind::Tsx => {
            javascript::capture_snippet(node, options)
        }
        LanguageKind::Rust => rust::capture_snippet(node, options),
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

/// Borrow one named child field as trimmed source text when available.
fn child_field_text(node: Node<'_>, field_name: &str, source: &SourceText) -> Option<String> {
    node.child_by_field_name(field_name)
        .map(|child| trimmed_node_text(source, child))
}

/// Borrow one syntax node as trimmed source text.
fn trimmed_node_text(source: &SourceText, node: Node<'_>) -> String {
    source.span_text(node_span(node)).trim().to_owned()
}

/// Decide whether a top-level snippet line reads like a comment that can be dropped with a symbol run.
fn is_comment_like_snippet(source: &SourceText, snippet: Snippet) -> bool {
    let text = source.span_text(source.trim_trailing_line_breaks(snippet.span));
    let trimmed = text.trim_start();

    trimmed.starts_with('#')
        || trimmed.starts_with("//")
        || trimmed.starts_with("/*")
        || trimmed.starts_with("/**")
}

/// Decide whether a comment snippet sits flush against a skipped symbol definition on one side.
fn comment_adjoins_skipped_symbol(
    items: &[Option<Item>],
    index: usize,
    source: &SourceText,
    direction: isize,
) -> bool {
    let Some(Item::Snippet(snippet)) = items.get(index).and_then(|item| item.as_ref()) else {
        return false;
    };
    let comment_lines = snippet_line_range(source, *snippet);
    let Some(other_index) = offset_index(index, direction) else {
        return false;
    };
    let Some(Item::Definition(definition)) = items.get(other_index).and_then(|item| item.as_ref())
    else {
        return false;
    };

    if !should_skip_top_level_symbol(definition) {
        return false;
    }

    if direction < 0 {
        return definition.line_range.end + 1 == comment_lines.start;
    }

    comment_lines.end + 1 == definition.line_range.start
}

/// Convert one snippet into the line range used for adjacency checks.
fn snippet_line_range(source: &SourceText, snippet: Snippet) -> LineRange {
    LineRange {
        start: source.line_number_at(snippet.span.start_byte),
        end: source.line_number_at(snippet.span.end_byte.saturating_sub(1)),
    }
}

/// Offset one index by a signed step, returning `None` when it would underflow or overflow.
fn offset_index(index: usize, direction: isize) -> Option<usize> {
    if direction < 0 {
        return index.checked_sub(direction.unsigned_abs());
    }

    index.checked_add(direction as usize)
}

/// The mutable state used while consolidating one skipped top-level symbol run.
struct SkippedSymbolRun {
    /// The first retained byte covered by the run.
    start_byte: usize,
    /// The first line covered by the run.
    start_line: usize,
    /// The latest retained end byte covered by the run.
    end_byte: usize,
    /// The latest line covered by the run.
    end_line: usize,
    /// The number of omitted symbol definitions in the run.
    count: usize,
}

impl SkippedSymbolRun {
    /// Start a new skipped run from one top-level symbol definition.
    fn new(definition: Definition, source: &SourceText) -> Self {
        Self {
            start_byte: definition.header_span.start_byte,
            start_line: definition.line_range.start,
            end_byte: definition.span.end_byte,
            end_line: source.line_number_at(definition.span.end_byte.saturating_sub(1)),
            count: 1,
        }
    }

    /// Extend the run across one more adjacent top-level symbol definition.
    fn extend(&mut self, definition: Definition, source: &SourceText) {
        self.end_byte = definition.span.end_byte;
        self.end_line = source.line_number_at(definition.span.end_byte.saturating_sub(1));
        self.count += 1;
    }

    /// Materialize the final placeholder item for the accumulated run.
    fn finish(self) -> SkippedRange {
        SkippedRange {
            span: explicit_span(self.start_byte, self.end_byte),
            line_range: LineRange {
                start: self.start_line,
                end: self.end_line,
            },
            count: self.count,
        }
    }
}

/// Find the first concrete syntax issue inside a recovered tree.
fn first_syntax_issue(node: Node<'_>) -> Option<Node<'_>> {
    if node.is_error() || node.is_missing() {
        return Some(node);
    }

    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if let Some(issue) = first_syntax_issue(child) {
            return Some(issue);
        }
    }

    None
}

/// Describe one concrete syntax issue in user-facing terms.
fn syntax_issue_details(node: Node<'_>) -> (String, Point) {
    if node.is_missing() {
        return ("missing syntax".to_owned(), node.start_position());
    }

    if node.is_error() {
        return ("syntax error".to_owned(), node.start_position());
    }

    ("recovered syntax error".to_owned(), node.start_position())
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
