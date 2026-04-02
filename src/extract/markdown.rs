use anyhow::Result;
use tree_sitter::{Node, Parser};

use crate::language;
use crate::model::{Definition, DefinitionKind, FileSection, Item, LineRange, TextSpan};
use crate::source::SourceText;

/// Extract a heading-only reduced section from one Markdown file.
pub fn extract_file(display_path: String, source: String) -> Result<FileSection> {
    // Parse the Markdown block structure once so heading ranges and nesting stay source-derived.
    let source = SourceText::new(source);
    let mut parser = Parser::new();
    language::configure_parser(&mut parser, crate::language::LanguageKind::Markdown)?;
    let tree = language::parse_source(&mut parser, source.as_str())?;
    let root = tree.root_node();

    // Keep the Markdown surface deliberately narrow: only headings participate in structure.
    let headings = collect_heading_captures(root, &source);
    let mut index = 0;
    let items = build_heading_items(&headings, &mut index, 0, &source);

    Ok(FileSection {
        display_path,
        source,
        items,
    })
}

/// One flat Markdown heading captured before it is nested into section structure.
#[derive(Clone, Debug)]
struct HeadingCapture {
    /// The 1-based heading depth implied by the Markdown marker or underline.
    level: u8,
    /// The user-facing heading label shown in the synthetic summary line.
    name: String,
    /// The exact source span of the heading line or lines.
    header_span: TextSpan,
    /// The full source span of the section opened by this heading.
    section_span: TextSpan,
}

/// Collect every heading in source order and pre-compute its section span.
fn collect_heading_captures(root: Node<'_>, source: &SourceText) -> Vec<HeadingCapture> {
    let mut headings = Vec::new();
    walk_heading_nodes(root, source, &mut headings);

    // Extend each heading until the next sibling-or-higher heading so line ranges describe whole sections.
    for index in 0..headings.len() {
        let section_end = headings
            .iter()
            .skip(index + 1)
            .find(|candidate| candidate.level <= headings[index].level)
            .map_or(source.as_str().len(), |candidate| {
                candidate.header_span.start_byte
            });

        headings[index].section_span =
            TextSpan::new(headings[index].header_span.start_byte, section_end);
    }

    headings
}

/// Walk the block tree recursively and retain only heading nodes.
fn walk_heading_nodes(node: Node<'_>, source: &SourceText, headings: &mut Vec<HeadingCapture>) {
    if let Some(capture) = capture_heading(node, source) {
        headings.push(capture);
        return;
    }

    let mut cursor = node.walk();

    // Descend through the full block tree so headings inside parser-owned section wrappers still surface.
    for child in node.children(&mut cursor).filter(|child| child.is_named()) {
        walk_heading_nodes(child, source, headings);
    }
}

/// Build the nested heading tree implied by Markdown heading levels.
fn build_heading_items(
    headings: &[HeadingCapture],
    index: &mut usize,
    parent_level: u8,
    source: &SourceText,
) -> Vec<Item> {
    let mut items = Vec::new();

    while let Some(heading) = headings.get(*index) {
        // Stop once the next heading belongs to an ancestor or sibling section.
        if heading.level <= parent_level {
            break;
        }

        let level = heading.level;
        *index += 1;

        // Consume any deeper headings as children of the current section.
        let items_in_section = build_heading_items(headings, index, level, source);

        items.push(Item::Definition(Definition {
            // Keep Markdown aligned with the existing reduced-source model instead of inventing a second renderer.
            kind: DefinitionKind::Heading,
            name: heading.name.clone(),
            leading_comment_snippets: Vec::new(),
            span: heading.section_span,
            header_span: source.trim_trailing_line_breaks(heading.header_span),
            render_header_source: false,
            line_range: LineRange {
                start: source.line_number_at(heading.header_span.start_byte),
                end: source.line_number_at(heading.section_span.end_byte.saturating_sub(1)),
            },
            items: items_in_section,
        }));
    }

    items
}

/// Capture one heading's visible label and raw header span.
fn capture_heading(node: Node<'_>, source: &SourceText) -> Option<HeadingCapture> {
    if !matches!(node.kind(), "atx_heading" | "setext_heading") {
        return None;
    }

    let header_span = TextSpan::new(node.start_byte(), node.end_byte());

    Some(HeadingCapture {
        level: heading_level(node)?,
        name: heading_name(node, source)?,
        header_span,
        section_span: header_span,
    })
}

/// Derive the semantic depth of a Markdown heading from its marker shape.
fn heading_level(node: Node<'_>) -> Option<u8> {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor).filter(|child| child.is_named()) {
        match child.kind() {
            "atx_h1_marker" | "setext_h1_underline" => return Some(1),
            "atx_h2_marker" | "setext_h2_underline" => return Some(2),
            "atx_h3_marker" => return Some(3),
            "atx_h4_marker" => return Some(4),
            "atx_h5_marker" => return Some(5),
            "atx_h6_marker" => return Some(6),
            _ => continue,
        }
    }

    None
}

/// Derive the user-facing heading name from the heading content node.
fn heading_name(node: Node<'_>, source: &SourceText) -> Option<String> {
    // Read the parser's dedicated heading-content field so Markdown markers stay out of the label.
    let content = node.child_by_field_name("heading_content")?;
    let name = source
        .span_text(TextSpan::new(content.start_byte(), content.end_byte()))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    Some(name)
}
