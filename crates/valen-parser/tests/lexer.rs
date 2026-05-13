//! Lexer snapshot tests.

use insta::assert_snapshot;
use valen_ast::token::TokenKind;
use valen_ast::FileId;
use valen_parser::lex;

fn fmt(src: &str) -> String {
    let (tokens, _diagnostics) = lex(src, FileId(0));
    tokens
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
fn at_sign_is_token() {
    assert_snapshot!(fmt("fn main() { @ }"));
}

#[test]
fn unknown_char_produces_error_token() {
    assert_snapshot!(fmt("fn main() { $ }"));
}

#[test]
fn class_keywords() {
    assert_snapshot!(fmt("pub sealed class Foo {}"));
}

#[test]
fn all_keywords_reserved() {
    assert_snapshot!(fmt(
        "fn let mut self return if else match class data enum trait impl pub internal private open override abstract sealed package import for in while loop as"
    ));
}

#[test]
fn float_literal() {
    assert_snapshot!(fmt("fn main() { let x = 3.14; }"));
}

#[test]
fn dot_and_double_colon() {
    assert_snapshot!(fmt("foo.bar::baz"));
}
