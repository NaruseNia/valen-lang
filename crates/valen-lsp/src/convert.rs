//! Span-to-LSP position conversion and diagnostic mapping.

use async_lsp::lsp_types::{self, DiagnosticSeverity, NumberOrString};
use valen_ast::Span;
use valen_diagnostics::{Diagnostics, Severity};

/// Pre-computed line offset table for converting byte offsets to LSP positions.
/// Handles UTF-16 code unit conversion as required by the LSP specification.
pub struct LineIndex {
    source: String,
    line_starts: Vec<u32>,
}

impl LineIndex {
    pub fn new(source: &str) -> Self {
        let mut line_starts = vec![0u32];
        for (i, b) in source.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push((i + 1) as u32);
            }
        }
        Self {
            source: source.to_string(),
            line_starts,
        }
    }

    pub fn offset_to_position(&self, offset: u32) -> lsp_types::Position {
        let line = match self.line_starts.binary_search(&offset) {
            Ok(exact) => exact,
            Err(next) => next.saturating_sub(1),
        };
        let line_start = self.line_starts[line] as usize;
        let byte_col = (offset as usize).saturating_sub(line_start);
        let line_text = &self.source[line_start..line_start + byte_col];
        let utf16_col = line_text.encode_utf16().count() as u32;
        lsp_types::Position::new(line as u32, utf16_col)
    }

    pub fn position_to_offset(&self, pos: lsp_types::Position) -> u32 {
        let line = pos.line as usize;
        if line >= self.line_starts.len() {
            return *self.line_starts.last().unwrap_or(&0);
        }
        let line_start = self.line_starts[line] as usize;
        let line_end = self
            .line_starts
            .get(line + 1)
            .map(|&s| s as usize)
            .unwrap_or(self.source.len());
        let line_text = &self.source[line_start..line_end];
        let target_utf16 = pos.character as usize;
        let mut utf16_count = 0;
        for (byte_idx, ch) in line_text.char_indices() {
            if utf16_count >= target_utf16 {
                return (line_start + byte_idx) as u32;
            }
            utf16_count += ch.len_utf16();
        }
        line_end as u32
    }

    pub fn span_to_range(&self, span: Span) -> lsp_types::Range {
        lsp_types::Range::new(
            self.offset_to_position(span.start),
            self.offset_to_position(span.end),
        )
    }
}

/// Convert Valen diagnostics to LSP diagnostic format.
pub fn to_lsp_diagnostics(
    diags: &Diagnostics,
    line_index: &LineIndex,
) -> Vec<lsp_types::Diagnostic> {
    diags
        .iter()
        .map(|d| lsp_types::Diagnostic {
            range: line_index.span_to_range(d.primary),
            severity: Some(match d.severity {
                Severity::Error => DiagnosticSeverity::ERROR,
                Severity::Warning => DiagnosticSeverity::WARNING,
                Severity::Hint => DiagnosticSeverity::HINT,
            }),
            code: Some(NumberOrString::String(format!("V{:04}", d.code.0))),
            source: Some("valen".to_string()),
            message: d.message.to_string(),
            ..Default::default()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use valen_ast::FileId;

    #[test]
    fn line_index_basic() {
        let li = LineIndex::new("hello\nworld\n");
        assert_eq!(li.offset_to_position(0), lsp_types::Position::new(0, 0));
        assert_eq!(li.offset_to_position(5), lsp_types::Position::new(0, 5));
        assert_eq!(li.offset_to_position(6), lsp_types::Position::new(1, 0));
        assert_eq!(li.offset_to_position(11), lsp_types::Position::new(1, 5));
    }

    #[test]
    fn position_roundtrip() {
        let li = LineIndex::new("fn main() {\n  42\n}\n");
        let pos = lsp_types::Position::new(1, 2);
        let offset = li.position_to_offset(pos);
        let back = li.offset_to_position(offset);
        assert_eq!(back, pos);
    }

    #[test]
    fn span_to_range() {
        let li = LineIndex::new("let x = 42;\n");
        let span = Span::new(4, 5, FileId(0));
        let range = li.span_to_range(span);
        assert_eq!(range.start, lsp_types::Position::new(0, 4));
        assert_eq!(range.end, lsp_types::Position::new(0, 5));
    }
}
