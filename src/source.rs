use crate::model::TextSpan;

/// The original source text plus line-start indexes for fast slicing and line lookup.
#[derive(Debug)]
pub struct SourceText {
    text: String,
    line_starts: Vec<usize>,
}

impl SourceText {
    /// Build the indexed source representation used throughout extraction and rendering.
    pub fn new(text: String) -> Self {
        // Capture every line start once so later byte-to-line lookups stay cheap.
        let mut line_starts = vec![0];

        for (index, byte) in text.bytes().enumerate() {
            if byte == b'\n' && index + 1 < text.len() {
                line_starts.push(index + 1);
            }
        }

        Self { text, line_starts }
    }

    /// Borrow the full source text.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Borrow the exact source text covered by a span.
    pub fn span_text(&self, span: TextSpan) -> &str {
        &self.text[span.start_byte..span.end_byte]
    }

    /// Trim trailing line endings from a span while preserving the underlying text.
    pub fn trim_trailing_line_breaks(&self, span: TextSpan) -> TextSpan {
        // Drop only terminal newline bytes so line slices stay source-like without extra blank output.
        let bytes = self.text.as_bytes();
        let mut end = span.end_byte;

        while end > span.start_byte && matches!(bytes[end - 1], b'\n' | b'\r') {
            end -= 1;
        }

        TextSpan::new(span.start_byte, end)
    }

    /// Return normalized output lines for a retained span with any shared left margin removed.
    pub fn rendered_lines(&self, span: TextSpan) -> Vec<String> {
        // Normalize only the trailing line breaks and trailing whitespace allowed by the spec.
        let trimmed = self.trim_trailing_line_breaks(span);
        let text = self.span_text(trimmed);

        if text.is_empty() {
            return Vec::new();
        }

        let mut lines = text
            .lines()
            .map(|line| line.trim_end().to_owned())
            .collect::<Vec<_>>();

        // Reconstruct line-leading indentation because syntax spans start at the first token.
        if let Some(first_line) = lines.first_mut() {
            first_line.insert_str(0, self.leading_indentation(trimmed.start_byte));
        }

        // Drop blank trailer lines created by spans that stop at the next block's indentation.
        while lines.last().is_some_and(|line| line.is_empty()) {
            lines.pop();
        }

        // Drop the shared source-side margin so rendering can re-indent purely by retained depth.
        strip_shared_indentation(&mut lines);

        lines
    }

    /// Convert a byte offset into a 1-based line number.
    pub fn line_number_at(&self, byte_offset: usize) -> usize {
        let bounded = byte_offset.min(self.text.len());
        let line_index = self
            .line_starts
            .partition_point(|offset| *offset <= bounded);
        line_index.max(1)
    }

    /// Return the whitespace indentation that precedes the provided byte offset.
    pub fn leading_indentation_at(&self, byte_offset: usize) -> &str {
        self.leading_indentation(byte_offset)
    }

    /// Return the whitespace indentation that precedes the span on its original line.
    fn leading_indentation(&self, byte_offset: usize) -> &str {
        let line_start = self.line_start(byte_offset);
        let prefix = &self.text[line_start..byte_offset];

        if prefix
            .chars()
            .all(|character| matches!(character, ' ' | '\t'))
        {
            prefix
        } else {
            ""
        }
    }

    /// Return the first byte of the line containing the provided offset.
    fn line_start(&self, byte_offset: usize) -> usize {
        let bounded = byte_offset.min(self.text.len());
        let line_index = self
            .line_starts
            .partition_point(|offset| *offset <= bounded);
        self.line_starts[line_index.saturating_sub(1)]
    }
}

/// Remove the common leading whitespace prefix from every non-empty line in one retained block.
fn strip_shared_indentation(lines: &mut [String]) {
    let Some(prefix) = shared_indentation_prefix(lines).filter(|prefix| !prefix.is_empty()) else {
        return;
    };

    // Strip only the block-wide margin so internal multiline layout remains intact.
    for line in lines.iter_mut().filter(|line| !line.is_empty()) {
        if let Some(stripped) = line.strip_prefix(&prefix) {
            *line = stripped.to_owned();
        }
    }
}

/// Return the whitespace-only prefix shared by every non-empty line in one retained block.
fn shared_indentation_prefix(lines: &[String]) -> Option<String> {
    let mut shared: Option<String> = None;

    // Intersect the line-leading indentation so omitted runtime scopes stop affecting output depth.
    for line in lines.iter().filter(|line| !line.is_empty()) {
        let indentation = line
            .chars()
            .take_while(|character| matches!(character, ' ' | '\t'))
            .collect::<String>();

        shared = Some(match shared {
            Some(current) => common_prefix(&current, &indentation),
            None => indentation,
        });

        if shared.as_ref().is_some_and(|prefix| prefix.is_empty()) {
            break;
        }
    }

    shared
}

/// Return the longest shared byte-prefix between two whitespace strings.
fn common_prefix(left: &str, right: &str) -> String {
    let prefix_len = left
        .bytes()
        .zip(right.bytes())
        .take_while(|(left, right)| left == right)
        .count();

    left[..prefix_len].to_owned()
}
