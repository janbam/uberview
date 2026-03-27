use crate::model::{Definition, FileOutput, FileSection, Item, LineRange, Snippet, TextSpan};

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
        render_item(item, section, options, &mut lines);
    }

    lines.join("\n")
}

/// Render one retained item, delegating recursively for definitions.
fn render_item(
    item: &Item,
    section: &FileSection,
    options: RenderOptions,
    lines: &mut Vec<String>,
) {
    match item {
        Item::Snippet(snippet) => render_snippet(snippet, section, options, lines),
        Item::Definition(definition) => render_definition(definition, section, options, lines),
    }
}

/// Render a verbatim retained snippet exactly as source-like lines.
fn render_snippet(
    snippet: &Snippet,
    section: &FileSection,
    options: RenderOptions,
    lines: &mut Vec<String>,
) {
    let rendered = section.source.rendered_lines(snippet.span);

    // Keep the default output source-like, but allow opt-in numbering for every retained item.
    if !options.show_line_numbers_for_all_items {
        lines.extend(rendered);
        return;
    }

    let Some((first, rest)) = rendered.split_first() else {
        return;
    };
    let indentation = section
        .source
        .leading_indentation_at(snippet.span.start_byte);
    let first = first.strip_prefix(indentation).unwrap_or(first);

    lines.push(format!(
        "{indentation}[{}] {}",
        format_line_range(line_range_for_span(section, snippet.span)),
        first
    ));
    lines.extend(rest.iter().cloned());
}

/// Render a definition header with its required line range and nested items.
fn render_definition(
    definition: &Definition,
    section: &FileSection,
    options: RenderOptions,
    lines: &mut Vec<String>,
) {
    let indentation = section
        .source
        .leading_indentation_at(definition.header_span.start_byte);
    let header_lines = section.source.rendered_lines(definition.header_span);

    // Add the synthetic summary line before the retained source so large files scan faster.
    lines.push(format!(
        "{indentation}[{}] {}: {}",
        format_line_range(definition.line_range),
        definition.kind.label(),
        definition.name
    ));
    let signature_end = header_signature_end(&header_lines);
    let (signature_lines, trailing_header_lines) = header_lines.split_at(signature_end + 1);
    lines.extend(signature_lines.iter().cloned());
    render_trailing_header_lines(
        definition,
        signature_end,
        trailing_header_lines,
        options,
        lines,
    );

    // Keep nested retained items immediately after the header so source order stays intact.
    for item in &definition.items {
        render_item(item, section, options, lines);
    }

    // Separate whole definition blocks so adjacent structure does not visually collapse together.
    lines.push(String::new());
}

/// Render any retained lines that were captured between the signature and the first named child.
fn render_trailing_header_lines(
    definition: &Definition,
    signature_end: usize,
    trailing_header_lines: &[String],
    options: RenderOptions,
    lines: &mut Vec<String>,
) {
    // Keep the default output source-like, but number these retained lines when the user opts in.
    if !options.show_line_numbers_for_all_items {
        lines.extend(trailing_header_lines.iter().cloned());
        return;
    }

    let line_offset = definition.line_range.start + signature_end;

    for (index, line) in trailing_header_lines.iter().enumerate() {
        if line.is_empty() {
            lines.push(String::new());
            continue;
        }

        let line_number = line_offset + index + 1;
        let indentation = line
            .chars()
            .take_while(|character| matches!(character, ' ' | '\t'))
            .collect::<String>();
        let content = line.strip_prefix(&indentation).unwrap_or(line);

        lines.push(format!(
            "{indentation}[{}] {}",
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
