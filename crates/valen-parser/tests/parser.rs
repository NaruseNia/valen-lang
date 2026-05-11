//! Parser snapshot tests.

use insta::assert_snapshot;
use valen_ast::FileId;
use valen_parser::parse;

fn check(src: &str) -> String {
    let result = parse(src, FileId(0));
    let diags: Vec<String> = result
        .diagnostics
        .iter()
        .map(|d| {
            format!(
                "{:?} V{:04} {}..{}: {}",
                d.severity, d.code.0, d.primary.start, d.primary.end, d.message
            )
        })
        .collect();
    format!(
        "=== AST ===\n{:#?}\n=== diagnostics ===\n{}",
        result.items,
        if diags.is_empty() {
            "(none)".to_string()
        } else {
            diags.join("\n")
        }
    )
}

#[test]
fn empty_fn() {
    assert_snapshot!(check("fn main() {}"));
}

#[test]
fn let_then_tail() {
    assert_snapshot!(check("fn main() { let x = 1 + 2; x }"));
}

#[test]
fn let_mut() {
    assert_snapshot!(check("fn main() { let mut count = 0; count }"));
}

#[test]
fn precedence_mul_binds_tighter_than_add() {
    assert_snapshot!(check("fn main() { 1 + 2 * 3 }"));
}

#[test]
fn parentheses_override_precedence() {
    assert_snapshot!(check("fn main() { (1 + 2) * 3 }"));
}

#[test]
fn unary_prefix() {
    assert_snapshot!(check("fn main() { -1 + !false }"));
}

#[test]
fn comparison_and_logical_chain() {
    assert_snapshot!(check("fn main() { a < b && b < c || d == e }"));
}

#[test]
fn error_missing_semicolon() {
    assert_snapshot!(check("fn main() { let x = 1 x }"));
}

#[test]
fn error_unexpected_top_level() {
    assert_snapshot!(check("let x = 1;"));
}

#[test]
fn empty_class() {
    assert_snapshot!(check("class Foo {}"));
}

#[test]
fn pub_class() {
    assert_snapshot!(check("pub class Foo {}"));
}

#[test]
fn sealed_class() {
    assert_snapshot!(check("sealed class Foo {}"));
}

#[test]
fn pub_abstract_class() {
    assert_snapshot!(check("pub abstract class Bar {}"));
}

#[test]
fn pub_fn() {
    assert_snapshot!(check("pub fn main() {}"));
}

#[test]
fn class_and_fn() {
    assert_snapshot!(check("class Foo {}\nfn main() {}"));
}

#[test]
fn error_class_missing_name() {
    assert_snapshot!(check("class {}"));
}

#[test]
fn error_class_missing_brace() {
    assert_snapshot!(check("class Foo"));
}
