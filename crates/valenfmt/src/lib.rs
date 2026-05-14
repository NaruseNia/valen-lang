//! `valenfmt` — Valen source formatter library.
//!
//! Parses `.vln` source, pretty-prints the AST with K&R brace style,
//! 4-space indentation, and trailing-semicolon normalization, while
//! preserving comments recovered from the original source.

pub mod comment;
pub mod printer;

use valen_ast::span::FileId;

/// Format Valen source code. Returns `None` if the source contains parse errors.
pub fn format_source(source: &str) -> Option<String> {
    let result = valen_parser::parse(source, FileId(0));
    if result.diagnostics.has_errors() {
        return None;
    }
    let comments = comment::extract_comments(source);
    let printer = printer::Printer::new(source, comments);
    Some(printer.print(&result.items))
}
