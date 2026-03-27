use crate::model::{
    Definition, FileOutput, FileSection, Item, LineRange, SkippedRange, Snippet, TextSpan,
};

/// The fixed indent unit used to visualize retained container nesting.
const OUTPUT_INDENT: &str = "    ";

/// The user-facing rendering toggles that shape the reduced-source text output.
#[derive(Clone, Copy, Debug, Default)]
pub struct RenderOptions {
    /// Extend line-number prefixes from definitions to every retained snippet.
    pub show_line_numbers_for_all_items: bool,
}

/// Render a complete ordered output across multiple file results.
pub fn render_outputs(outputs: &[FileOutput], options: RenderOptions) -> String {
    outputs
        .iter()
        .map(|output| render_output(output, options))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Render one file result into the human-facing output contract.
pub fn render_output(output: &FileOutput, options: RenderOptions) -> String {
    match output {
        FileOutput::Section(section) => render_section(section, options),
        FileOutput::Failure(failure) => {
            format!(
                "=== {} ===\n\n!! parse failed: {}",
                failure.display_path, failure.message
            )
        }
    }
}

/// Render one successful reduced-source file section.
pub fn render_section(section: &FileSection, options: RenderOptions) -> String {
    let mut lines = vec![format!("=== {} ===", section.display_path), String::new()];

    if section.items.is_empty() {
        lines.push("(no retained structure)".to_owned());
        return lines.join("\n");
    }

    // Stream the retained items directly so the output stays in source order.
    for item in &section.items {
        render_item(item, section, options, 0, &mut lines);
    }

    lines.join("\n")
}

/// Render one retained item, delegating recursively for definitions.
fn render_item(
    item: &Item,
    section: &FileSection,
    options: RenderOptions,
    depth: usize,
    lines: &mut Vec<String>,
) {
    match item {
        Item::Snippet(snippet) => render_snippet(snippet, section, options, depth, lines),
        Item::Definition(definition) => {
            render_definition(definition, section, options, depth, lines)
        }
        Item::SkippedRange(range) => render_skipped_range(range, depth, lines),
    }
}

/// Render a verbatim retained snippet exactly as source-like lines.
fn render_snippet(
    snippet: &Snippet,
    section: &FileSection,
    options: RenderOptions,
    depth: usize,
    lines: &mut Vec<String>,
) {
    let rendered = section.source.rendered_lines(snippet.span);

    // Keep the default output source-like, but allow opt-in numbering for every retained item.
    if !options.show_line_numbers_for_all_items {
        push_indented_lines(&rendered, depth, lines);
        return;
    }

    let Some((first, rest)) = rendered.split_first() else {
        return;
    };
    let (indentation, content) = split_leading_whitespace(first);

    lines.push(format!(
        "{}{}[{}] {}",
        render_indent(depth),
        indentation,
        format_line_range(line_range_for_span(section, snippet.span)),
        content
    ));
    push_indented_lines(rest, depth, lines);
}

/// Render a definition header with its required line range and nested items.
fn render_definition(
    definition: &Definition,
    section: &FileSection,
    options: RenderOptions,
    depth: usize,
    lines: &mut Vec<String>,
) {
    let header_lines = section.source.rendered_lines(definition.header_span);
    let signature_end = header_signature_end(&header_lines);
    let (signature_lines, trailing_header_lines) = header_lines.split_at(signature_end + 1);

    // Separate nested containers from prior body content so their headers scan as structure, not prose.
    if should_precede_nested_container_with_blank_line(definition, depth, lines) {
        lines.push(String::new());
    }

    // Add the synthetic summary line before the retained source so large files scan faster.
    lines.push(format!(
        "{}[{}] {}: {}",
        render_indent(depth),
        format_line_range(definition.line_range),
        definition.kind.label(),
        definition.name
    ));
    push_indented_lines(signature_lines, depth, lines);
    render_trailing_header_lines(
        definition,
        signature_end,
        trailing_header_lines,
        options,
        depth,
        lines,
    );

    // Keep nested retained items immediately after the header so source order stays intact.
    for item in &definition.items {
        render_item(item, section, options, depth + 1, lines);
    }

    // Separate whole definition blocks so adjacent structure does not visually collapse together.
    lines.push(String::new());
}

/// Render a synthetic placeholder for one omitted top-level symbol run.
fn render_skipped_range(range: &SkippedRange, depth: usize, lines: &mut Vec<String>) {
    let item_label = if range.count == 1 { "item" } else { "items" };

    lines.push(format!(
        "{}[{}] Skipped top-level assignments/constants ({} {})",
        render_indent(depth),
        format_line_range(range.line_range),
        range.count,
        item_label
    ));

    // Keep placeholder blocks separated just like real definition blocks.
    lines.push(String::new());
}

/// Render any retained lines that were captured between the signature and the first named child.
fn render_trailing_header_lines(
    definition: &Definition,
    signature_end: usize,
    trailing_header_lines: &[String],
    options: RenderOptions,
    depth: usize,
    lines: &mut Vec<String>,
) {
    // Keep the default output source-like, but number these retained lines when the user opts in.
    if !options.show_line_numbers_for_all_items {
        push_indented_lines(trailing_header_lines, depth, lines);
        return;
    }

    let line_offset = definition.line_range.start + signature_end;

    for (index, line) in trailing_header_lines.iter().enumerate() {
        if line.is_empty() {
            lines.push(String::new());
            continue;
        }

        let line_number = line_offset + index + 1;
        let (indentation, content) = split_leading_whitespace(line);

        lines.push(format!(
            "{}{}[{}] {}",
            render_indent(depth),
            indentation,
            format_line_range(LineRange {
                start: line_number,
                end: line_number,
            }),
            content
        ));
    }
}

/// Find the last signature line inside one retained definition header.
fn header_signature_end(header_lines: &[String]) -> usize {
    header_lines
        .iter()
        .rposition(|line| line.trim_end().ends_with(':'))
        .unwrap_or_else(|| header_lines.len().saturating_sub(1))
}

/// Render a retained line range in the compact bracketed form.
fn format_line_range(range: LineRange) -> String {
    if range.start == range.end {
        return range.start.to_string();
    }

    format!("{}-{}", range.start, range.end)
}

/// Convert a retained source span into the line range shown by snippet numbering.
fn line_range_for_span(section: &FileSection, span: TextSpan) -> LineRange {
    LineRange {
        start: section.source.line_number_at(span.start_byte),
        end: section
            .source
            .line_number_at(span.end_byte.saturating_sub(1)),
    }
}

/// Prefix a retained block with the renderer-owned indentation for one nesting depth.
fn push_indented_lines(block: &[String], depth: usize, lines: &mut Vec<String>) {
    // Reapply only retained-container indentation so omitted runtime scopes stay invisible.
    for line in block {
        if line.is_empty() {
            lines.push(String::new());
            continue;
        }

        lines.push(format!("{}{}", render_indent(depth), line));
    }
}

/// Return the whitespace-only prefix and the remaining content for one rendered line.
fn split_leading_whitespace(line: &str) -> (&str, &str) {
    let indentation_len = line
        .bytes()
        .take_while(|byte| matches!(byte, b' ' | b'\t'))
        .count();

    line.split_at(indentation_len)
}

/// Return the indentation prefix for one retained-container depth.
fn render_indent(depth: usize) -> String {
    OUTPUT_INDENT.repeat(depth)
}

/// Decide whether one nested definition should be visually separated from prior body content.
fn should_precede_nested_container_with_blank_line(
    definition: &Definition,
    depth: usize,
    lines: &[String],
) -> bool {
    // Separate only multiline nested definitions so compact member lists do not become overly sparse.
    depth > 0
        && definition.line_range.start != definition.line_range.end
        && lines.last().is_some_and(|line| !line.is_empty())
}
