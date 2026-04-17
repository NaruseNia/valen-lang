//! Valen lexer and parser.
//!
//! Pipeline: source `&str` → tokens (logos) → AST (recursive descent).

use valen_ast::{FileId, Item};
use valen_diagnostics::Diagnostics;

pub mod lexer;
pub mod parser;

pub struct ParseResult {
    pub items: Vec<Item>,
    pub diagnostics: Diagnostics,
}

pub fn parse(_source: &str, _file_id: FileId) -> ParseResult {
    todo!("lex the source, run recursive descent parser, collect diagnostics")
}
