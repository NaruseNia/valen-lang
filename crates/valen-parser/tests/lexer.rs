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
        "fn let mut self return if else match class data enum trait impl pub internal private open override abstract sealed package import for in while loop as static void new this super null throw try catch finally extends implements"
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

#[test]
fn bom_prefixed_source() {
    // U+FEFF BOM should be silently stripped; tokens identical to non-BOM input
    let with_bom = "\u{FEFF}fn main() {}";
    let without_bom = "fn main() {}";
    let (tok_bom, diag_bom) = lex(with_bom, FileId(0));
    let (tok_no, diag_no) = lex(without_bom, FileId(0));
    // Same token kinds
    let kinds_bom: Vec<_> = tok_bom.iter().map(|(k, _)| k.clone()).collect();
    let kinds_no: Vec<_> = tok_no.iter().map(|(k, _)| k.clone()).collect();
    assert_eq!(kinds_bom, kinds_no);
    // Spans are offset by BOM length (3 bytes for UTF-8 BOM)
    for ((_, s_bom), (_, s_no)) in tok_bom.iter().zip(tok_no.iter()) {
        assert_eq!(s_bom.start, s_no.start + 3, "start offset mismatch");
        assert_eq!(s_bom.end, s_no.end + 3, "end offset mismatch");
    }
    // No diagnostics from BOM
    assert!(!diag_bom.has_errors());
    assert!(!diag_no.has_errors());
}

#[test]
fn bom_only_source_produces_eof() {
    // A source consisting of only a BOM should produce just an EOF token
    let (tokens, diags) = lex("\u{FEFF}", FileId(0));
    assert_eq!(tokens.len(), 1);
    assert!(matches!(tokens[0].0, TokenKind::Eof));
    assert!(!diags.has_errors());
}

// -- char literals (#051) ---------------------------------------------------

#[test]
fn char_literal_simple() {
    assert_snapshot!(fmt("fn main() { let c = 'A'; }"));
}

#[test]
fn char_literal_escape() {
    assert_snapshot!(fmt("fn main() { let c = '\\n'; }"));
}

// -- long literals (#051) ---------------------------------------------------

#[test]
fn long_literal() {
    assert_snapshot!(fmt("fn main() { let x = 42L; }"));
}

#[test]
fn long_literal_negative() {
    assert_snapshot!(fmt("fn main() { let x = -100L; }"));
}

// -- float literal with suffix (#051) ---------------------------------------

#[test]
fn float_literal_with_suffix() {
    assert_snapshot!(fmt("fn main() { let x = 3.14f; }"));
}

#[test]
fn float_literal_no_suffix() {
    assert_snapshot!(fmt("fn main() { let x = 2.718; }"));
}

// -- f-string literals (#051) -----------------------------------------------

#[test]
fn fstring_simple() {
    assert_snapshot!(fmt(r#"fn main() { let s = f"hello {name}"; }"#));
}

#[test]
fn fstring_multiple_interp() {
    assert_snapshot!(fmt(r#"fn main() { let s = f"{a} + {b} = {c}"; }"#));
}

// -- underscore separators (#051) -------------------------------------------

#[test]
fn int_underscore_separator() {
    assert_snapshot!(fmt("fn main() { let x = 1_000_000; }"));
}
