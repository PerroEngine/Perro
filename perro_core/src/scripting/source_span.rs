// Source span tracking for multi-language source mapping
use serde::{Deserialize, Serialize};

/// Represents a location in the original source code
/// This is language-agnostic and used by all Perro scripting languages
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    /// Source file identifier (e.g., "res://player.pup", "res://Player.ts")
    pub file: String,
    /// Line number (1-indexed)
    pub line: u32,
    /// Column number (1-indexed, UTF-8 character offset)
    pub column: u32,
    /// Length of the span in characters
    pub length: u32,
    /// Language identifier (e.g., "pup", "typescript", "csharp")
    pub language: String,
}

impl SourceSpan {
    /// Create a new source span
    pub fn new(file: String, line: u32, column: u32, length: u32, language: String) -> Self {
        Self {
            file,
            line,
            column,
            length,
            language,
        }
    }

    /// Create a source span for a single character
    pub fn single_char(file: String, line: u32, column: u32, language: String) -> Self {
        Self::new(file, line, column, 1, language)
    }

    /// Get the end column (exclusive)
    pub fn end_column(&self) -> u32 {
        self.column + self.length
    }

    /// Check if this span contains a given line and column
    pub fn contains(&self, line: u32, column: u32) -> bool {
        self.line == line && column >= self.column && column < self.end_column()
    }

    /// Merge two spans into a span that covers both
    pub fn merge(&self, other: &SourceSpan) -> Option<SourceSpan> {
        if self.file != other.file || self.language != other.language {
            return None;
        }

        let start_line = self.line.min(other.line);
        let end_line = self.line.max(other.line);

        let start_col = if self.line < other.line {
            self.column
        } else if other.line < self.line {
            other.column
        } else {
            self.column.min(other.column)
        };

        let end_col = if self.line > other.line {
            self.end_column()
        } else if other.line > self.line {
            other.end_column()
        } else {
            self.end_column().max(other.end_column())
        };

        Some(SourceSpan {
            file: self.file.clone(),
            line: start_line,
            column: start_col,
            length: if start_line == end_line {
                end_col - start_col
            } else {
                // Approximate for multi-line spans
                end_col
            },
            language: self.language.clone(),
        })
    }
}

/// Helper to extract source span from a string position
/// Given a source string and a byte offset, compute line and column
pub fn position_to_span(
    source: &str,
    byte_offset: usize,
    file: String,
    language: String,
) -> SourceSpan {
    let mut line = 1u32;
    let mut column = 1u32;

    for (idx, ch) in source.char_indices() {
        if idx >= byte_offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }

    SourceSpan::single_char(file, line, column, language)
}

/// Helper to extract source span from a range in a string
pub fn range_to_span(
    source: &str,
    start_byte: usize,
    end_byte: usize,
    file: String,
    language: String,
) -> SourceSpan {
    let start_span = position_to_span(source, start_byte, file.clone(), language.clone());
    let end_span = position_to_span(source, end_byte, file, language.clone());

    // Calculate length
    let length = if start_span.line == end_span.line {
        end_span.column - start_span.column
    } else {
        // Multi-line span - approximate
        end_span.column
    };

    SourceSpan {
        line: start_span.line,
        column: start_span.column,
        length,
        file: start_span.file,
        language,
    }
}
