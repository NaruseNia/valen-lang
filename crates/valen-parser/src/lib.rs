//! Valen lexer and parser.
//!
//! Pipeline: source `&str` → tokens (logos) → AST (recursive descent).

use valen_ast::{FileId, Item};
use valen_diagnostics::Diagnostics;

pub mod lexer;
pub mod parser;

pub use lexer::{lex, Lexer};
pub use parser::Parser;

/// Result of parsing a single source file: the AST items and any diagnostics.
pub struct ParseResult {
    pub items: Vec<Item>,
    pub diagnostics: Diagnostics,
}

/// Lex and parse `source` into a list of top-level items, collecting diagnostics.
pub fn parse(source: &str, file_id: FileId) -> ParseResult {
    let mut parser = Parser::new(source, file_id);
    let items = parser.parse_file();
    ParseResult {
        items,
        diagnostics: parser.into_diagnostics(),
    }
}
