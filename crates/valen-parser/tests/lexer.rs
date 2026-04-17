//! Lexer snapshot tests.

use insta::assert_snapshot;
use valen_ast::token::TokenKind;
use valen_ast::FileId;
use valen_parser::lex;

fn fmt(src: &str) -> String {
    lex(src, FileId(0))
        .iter()
        .filter(|(k, _)| !matches!(k, TokenKind::Eof))
        .map(|(k, s)| format!("{:?} @ {}..{}", k, s.start, s.end))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn empty_fn() {
    assert_snapshot!(fmt("fn main() {}"));
}

#[test]
fn let_with_binop() {
    assert_snapshot!(fmt("fn main() { let x = 1 + 2; x }"));
}

#[test]
fn all_literals() {
    assert_snapshot!(fmt(
        r#"fn main() { let s = "hi\n"; let b = true; let n = 42; }"#
    ));
}

#[test]
fn operators() {
    assert_snapshot!(fmt(
        "fn main() { a + b - c * d / e % f == g != h < i <= j > k >= l && m || n }"
    ));
}

#[test]
fn line_comment_is_skipped() {
    assert_snapshot!(fmt("fn main() {\n  // comment\n  42\n}"));
}

#[test]
fn unknown_char_produces_error_token() {
    assert_snapshot!(fmt("fn main() { @ }"));
}
