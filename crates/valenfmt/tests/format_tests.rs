//! Integration tests for `valenfmt` — formatting correctness and idempotency.

use std::path::Path;

fn format(src: &str) -> String {
    valenfmt::format_source(src).expect("parse should succeed")
}

fn assert_format(input: &str, expected: &str) {
    let got = format(input);
    assert_eq!(
        got, expected,
        "\n--- input ---\n{input}\n--- expected ---\n{expected}\n--- got ---\n{got}"
    );
}

fn assert_idempotent(src: &str) {
    let first = format(src);
    let second = format(&first);
    assert_eq!(first, second, "formatter is not idempotent");
}

// ── Package & Import ────────────────────────────────────────────────

#[test]
fn package_decl() {
    assert_format("package   foo.bar ;", "package foo.bar;\n");
}

#[test]
fn imports() {
    assert_format(
        "package x;\nimport java.util.List;\nimport java.util.Map;\n",
        "package x;\n\nimport java.util.List;\nimport java.util.Map;\n",
    );
}

#[test]
fn import_alias() {
    assert_format(
        "package x;\nimport java.io.File as JavaFile;\n",
        "package x;\n\nimport java.io.File as JavaFile;\n",
    );
}

// ── Class ───────────────────────────────────────────────────────────

#[test]
fn empty_class() {
    assert_format(
        "package x;\npub class Foo {}",
        "package x;\n\npub class Foo {}\n",
    );
}

#[test]
fn class_with_methods() {
    assert_format(
        "package x;\npub class Math {\n  fn add(self,a:Int,b:Int)->Int{a + b}\n}",
        "package x;\n\npub class Math {\n    fn add(self, a: Int, b: Int) -> Int {\n        a + b\n    }\n}\n",
    );
}

#[test]
fn sealed_class() {
    assert_format(
        "package x;\npub sealed class Base{}\n",
        "package x;\n\npub sealed class Base {}\n",
    );
}

// ── Data Class ──────────────────────────────────────────────────────

#[test]
fn data_class() {
    assert_format(
        "package x;\npub data class Point(pub x:Int,pub y:Int);",
        "package x;\n\npub data class Point(pub x: Int, pub y: Int);\n",
    );
}

// ── Enum ────────────────────────────────────────────────────────────

#[test]
fn enum_with_variants() {
    assert_format(
        "package x;\npub enum Shape{Circle(r:Float),Point,}",
        "package x;\n\npub enum Shape {\n    Circle(r: Float),\n    Point,\n}\n",
    );
}

// ── Trait & Impl ────────────────────────────────────────────────────

#[test]
fn trait_and_impl() {
    let input = "\
package x;
pub trait Greeter{fn greet(self)->String;}
pub class Dog{}
impl Greeter for Dog{fn greet(self)->String{\"Woof\"}}
";
    let expected = "\
package x;

pub trait Greeter {
    fn greet(self) -> String;
}

pub class Dog {}

impl Greeter for Dog {
    fn greet(self) -> String {
        \"Woof\"
    }
}
";
    assert_format(input, expected);
}

// ── Expressions ─────────────────────────────────────────────────────

#[test]
fn match_expr() {
    assert_format(
        "package x;\npub class C{fn f(self,x:Int)->Int{match x{0=>100,1=>200,_=>0,}}}",
        "package x;\n\npub class C {\n    fn f(self, x: Int) -> Int {\n        match x {\n            0 => 100,\n            1 => 200,\n            _ => 0,\n        }\n    }\n}\n",
    );
}

#[test]
fn if_else_multiline() {
    assert_format(
        "package x;\npub class C{fn f(self,a:Int,b:Int)->Int{if a > b{a}else{b}}}",
        "package x;\n\npub class C {\n    fn f(self, a: Int, b: Int) -> Int {\n        if a > b {\n            a\n        } else {\n            b\n        }\n    }\n}\n",
    );
}

#[test]
fn for_loop_no_trailing_semi() {
    let expected = "\
package x;

pub class C {
    fn f(self) -> Int {
        let mut s = 0;
        for i in 0..10 {
            s = s + i;
        }
        s
    }
}
";
    assert_format(expected, expected);
}

#[test]
fn lambda_compact_block() {
    let expected = "\
package x;

pub class C {
    fn f(self, x: Int) -> Int {
        let g = |a: Int| -> Int { a + 1 };
        g(x)
    }
}
";
    assert_format(expected, expected);
}

// ── Comments ────────────────────────────────────────────────────────

#[test]
fn comments_preserved() {
    let input = "\
package x;

// Section comment
pub class Foo {}
";
    assert_format(input, input);
}

#[test]
fn comments_between_items() {
    let input = "\
package x;

// A
pub class A {}

// B
pub class B {}
";
    assert_format(input, input);
}

#[test]
fn trailing_comments_after_items() {
    let input = "\
package x;

// only comments below
// another line
";
    assert_format(input, input);
}

// ── Visibility ──────────────────────────────────────────────────────

#[test]
fn private_visibility() {
    assert_format(
        "package x;\nprivate class Secret{}",
        "package x;\n\nprivate class Secret {}\n",
    );
}

// ── Formatting rules ────────────────────────────────────────────────

#[test]
fn trailing_semicolon_on_let() {
    let input = "package x;\npub class C {\n    fn f(self) -> Int {\n        let x = 42;\n        x\n    }\n}\n";
    let expected = "package x;\n\npub class C {\n    fn f(self) -> Int {\n        let x = 42;\n        x\n    }\n}\n";
    assert_format(input, expected);
}

// ── Self shorthand ──────────────────────────────────────────────────

#[test]
fn self_param_shorthand() {
    let input = "\
package x;

pub trait T {
    fn method(self) -> Int;
}
";
    assert_format(input, input);
}

#[test]
fn mut_self_param_shorthand() {
    let input = "\
package x;

pub trait T {
    fn method(mut self) -> Int;
}
";
    assert_format(input, input);
}

// ── Idempotency ─────────────────────────────────────────────────────

#[test]
fn idempotent_all_fixtures() {
    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures/codegen");
    for entry in std::fs::read_dir(&fixtures_dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "vln") {
            let src = std::fs::read_to_string(&path).unwrap();
            assert_idempotent(&src);
        }
    }
}

#[test]
fn idempotent_stdlib() {
    let stdlib_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("stdlib");
    for entry in walkdir(&stdlib_dir) {
        if entry.extension().is_some_and(|e| e == "vln") {
            let src = std::fs::read_to_string(&entry).unwrap();
            assert_idempotent(&src);
        }
    }
}

fn walkdir(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                files.extend(walkdir(&p));
            } else {
                files.push(p);
            }
        }
    }
    files
}

// ── Parse error handling ────────────────────────────────────────────

#[test]
fn parse_error_returns_none() {
    assert!(valenfmt::format_source("fn {{{").is_none());
}
