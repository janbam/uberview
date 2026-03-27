use crate::model::{Definition, FileOutput, FileSection, Item, Snippet};

/// Render a complete ordered output across multiple file results.
pub fn render_outputs(outputs: &[FileOutput]) -> String {
    outputs
        .iter()
        .map(render_output)
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Render one file result into the human-facing output contract.
pub fn render_output(output: &FileOutput) -> String {
    match output {
        FileOutput::Section(section) => render_section(section),
        FileOutput::Failure(failure) => {
            format!(
                "=== {} ===\n\n!! parse failed: {}",
                failure.display_path, failure.message
            )
        }
    }
}

/// Render one successful reduced-source file section.
pub fn render_section(section: &FileSection) -> String {
    let mut lines = vec![format!("=== {} ===", section.display_path), String::new()];

    if section.items.is_empty() {
        lines.push("(no retained structure)".to_owned());
        return lines.join("\n");
    }

    // Stream the retained items directly so the output stays in source order.
    for item in &section.items {
        render_item(item, section, &mut lines);
    }

    lines.join("\n")
}

/// Render one retained item, delegating recursively for definitions.
fn render_item(item: &Item, section: &FileSection, lines: &mut Vec<String>) {
    match item {
        Item::Snippet(snippet) => render_snippet(snippet, section, lines),
        Item::Definition(definition) => render_definition(definition, section, lines),
    }
}

/// Render a verbatim retained snippet exactly as source-like lines.
fn render_snippet(snippet: &Snippet, section: &FileSection, lines: &mut Vec<String>) {
    lines.extend(section.source.rendered_lines(snippet.span));
}

/// Render a definition header with its required line range and nested items.
fn render_definition(definition: &Definition, section: &FileSection, lines: &mut Vec<String>) {
    let header_lines = section.source.rendered_lines(definition.header_span);

    if let Some((first, rest)) = header_lines.split_first() {
        lines.push(format!(
            "{}-{}\t{}",
            definition.line_range.start, definition.line_range.end, first
        ));
        lines.extend(rest.iter().cloned());
    }

    // Keep nested retained items immediately after the header so source order stays intact.
    for item in &definition.items {
        render_item(item, section, lines);
    }
}
