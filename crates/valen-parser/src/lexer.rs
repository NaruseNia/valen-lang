//! Token lexer. Backed by logos.
//!
//! Notes:
//! - f-string literals (`f"..."`) need custom handling because interpolation
//!   expressions cross token boundaries; handle in a second pass.
//! - Shebang (`#!/...`) at file start should be skipped before lexing.

use valen_ast::token::TokenKind;
use valen_ast::Span;

pub struct Lexer<'src> {
    _source: &'src str,
}

impl<'src> Lexer<'src> {
    pub fn new(source: &'src str) -> Self {
        Self { _source: source }
    }

    pub fn next_token(&mut self) -> Option<(TokenKind, Span)> {
        todo!("drive logos-generated lexer, map to TokenKind, compute spans")
    }
}
