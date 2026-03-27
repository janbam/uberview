use crate::source::SourceText;

/// A half-open byte span within the original source text.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct TextSpan {
    /// The first byte retained in the span.
    pub start_byte: usize,
    /// The first byte after the retained span.
    pub end_byte: usize,
}

impl TextSpan {
    /// Build a span from a start and end byte offset.
    pub const fn new(start_byte: usize, end_byte: usize) -> Self {
        Self {
            start_byte,
            end_byte,
        }
    }

    /// Report whether the span contains no text.
    pub const fn is_empty(self) -> bool {
        self.start_byte >= self.end_byte
    }
}

/// The line-range coordinate attached to retained definitions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LineRange {
    /// The first 1-based line covered by the definition.
    pub start: usize,
    /// The last 1-based line covered by the definition.
    pub end: usize,
}

/// A source-ordered file section ready for rendering.
#[derive(Debug)]
pub struct FileSection {
    /// The path shown in the section header.
    pub display_path: String,
    /// The original source text used for rendering retained slices.
    pub source: SourceText,
    /// The retained reduced-source items in source order.
    pub items: Vec<Item>,
}

/// A source-ordered retained item inside a file or definition scope.
#[derive(Debug)]
pub enum Item {
    /// A verbatim snippet such as a comment, docstring, or exit statement.
    Snippet(Snippet),
    /// A retained definition with its nested reduced-source body.
    Definition(Definition),
}

impl Item {
    /// Return the first byte retained by the item for ordering and deduplication.
    pub const fn start_byte(&self) -> usize {
        match self {
            Self::Snippet(snippet) => snippet.span.start_byte,
            Self::Definition(definition) => definition.header_span.start_byte,
        }
    }
}

/// A verbatim retained fragment from the original source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Snippet {
    /// The source span to keep verbatim.
    pub span: TextSpan,
}

/// A retained definition with its line range, header, and nested items.
#[derive(Debug)]
pub struct Definition {
    /// The full source extent of the definition.
    pub span: TextSpan,
    /// The source slice used for the retained header line(s).
    pub header_span: TextSpan,
    /// The line coordinates printed alongside the definition header.
    pub line_range: LineRange,
    /// The retained nested items inside the definition body.
    pub items: Vec<Item>,
}

/// A rendered file result, including directory-scan failures that should not abort the run.
#[derive(Debug)]
pub enum FileOutput {
    /// A successfully extracted file section.
    Section(FileSection),
    /// A file-level failure rendered inline during directory scans.
    Failure(FileFailure),
}

/// A non-fatal per-file failure for directory scans.
#[derive(Debug)]
pub struct FileFailure {
    /// The path shown in the section header.
    pub display_path: String,
    /// The message rendered under the file header.
    pub message: String,
}
