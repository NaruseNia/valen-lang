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

// --- TASK-001: fn params, return type, type annotations ---

#[test]
fn fn_with_params_and_return() {
    assert_snapshot!(check("fn add(a: Int, b: Int) -> Int { a }"));
}

#[test]
fn fn_with_generic_type() {
    assert_snapshot!(check("fn first(xs: List<Int>) -> Option<Int> { xs }"));
}

#[test]
fn let_with_type_annotation() {
    assert_snapshot!(check("fn main() { let x: Int = 42; x }"));
}

#[test]
fn nullable_type() {
    assert_snapshot!(check("fn find(id: Int) -> String? { id }"));
}

// --- if/else ---

#[test]
fn if_else_expr() {
    assert_snapshot!(check("fn abs(x: Int) -> Int { if x { 1 } else { 2 } }"));
}

#[test]
fn if_without_else() {
    assert_snapshot!(check("fn f() { if cond { do_thing(); } }"));
}

#[test]
fn if_else_if_chain() {
    assert_snapshot!(check(
        "fn f(x: Int) -> Int { if x { 1 } else if y { 2 } else { 3 } }"
    ));
}

// --- match ---

#[test]
fn match_literals() {
    assert_snapshot!(check(
        "fn f(x: Int) -> String { match x { 0 => zero, 1 => one, _ => other } }"
    ));
}

#[test]
fn match_with_guard() {
    assert_snapshot!(check(
        "fn f(x: Int) -> Int { match x { n if n > 0 => n, _ => 0 } }"
    ));
}

#[test]
fn match_or_pattern() {
    assert_snapshot!(check(
        "fn f(x: Int) -> Bool { match x { 1 | 2 | 3 => true, _ => false } }"
    ));
}

#[test]
fn match_range_pattern() {
    assert_snapshot!(check(
        "fn f(x: Int) -> String { match x { 0..=9 => small, _ => big } }"
    ));
}

#[test]
fn match_enum_destructure() {
    assert_snapshot!(check(
        "fn f() -> Int { match shape { Shape::Circle(r) => r, Shape::Point => 0 } }"
    ));
}

#[test]
fn match_at_binding() {
    assert_snapshot!(check(
        "fn f(x: Int) -> Int { match x { n @ 1 => n, _ => 0 } }"
    ));
}

// --- call / method call / field ---

#[test]
fn function_call() {
    assert_snapshot!(check("fn main() { foo(1, 2, 3) }"));
}

#[test]
fn named_args_call() {
    assert_snapshot!(check("fn main() { greet(msg = hello, count = 3) }"));
}

#[test]
fn method_call() {
    assert_snapshot!(check("fn main() { xs.map(f) }"));
}

#[test]
fn field_access() {
    assert_snapshot!(check("fn main() { user.name }"));
}

#[test]
fn chained_method_calls() {
    assert_snapshot!(check("fn main() { xs.filter(f).map(g).count() }"));
}

#[test]
fn path_with_double_colon() {
    assert_snapshot!(check("fn main() { Shape::Circle }"));
}

// --- try operator ---

#[test]
fn try_operator() {
    assert_snapshot!(check("fn f() -> Int { get_value()? }"));
}

// --- return ---

#[test]
fn return_expr() {
    assert_snapshot!(check("fn f(x: Int) -> Int { return x; }"));
}

#[test]
fn return_no_value() {
    assert_snapshot!(check("fn f() { return; }"));
}

// --- TASK-002: class ctor/body, data class, enum, trait, impl, package, import ---

#[test]
fn class_with_ctor_params() {
    assert_snapshot!(check("class User(pub name: String, mut age: Int) {}"));
}

#[test]
fn class_with_methods() {
    assert_snapshot!(check(
        "class Dog(pub name: String) { fn greet(self) -> String { self.name } }"
    ));
}

#[test]
fn class_with_superclass() {
    assert_snapshot!(check("class Dog(pub name: String) : Animal {}"));
}

#[test]
fn sealed_class_no_body() {
    assert_snapshot!(check("sealed class Payment;"));
}

#[test]
fn data_class() {
    assert_snapshot!(check("data class Point(x: Float, y: Float);"));
}

#[test]
fn enum_with_variants() {
    assert_snapshot!(check(
        "enum Shape { Circle(r: Float), Rectangle(w: Float, h: Float), Point }"
    ));
}

#[test]
fn trait_definition() {
    assert_snapshot!(check("trait Area { fn area(self) -> Float; }"));
}

#[test]
fn trait_with_default_method() {
    assert_snapshot!(check(
        "trait Display { fn display(self) -> String { self.name } }"
    ));
}

#[test]
fn sealed_trait() {
    assert_snapshot!(check("sealed trait Expr { fn eval(self) -> Int; }"));
}

#[test]
fn sealed_trait_marker() {
    assert_snapshot!(check("sealed trait Marker {}"));
}

#[test]
fn annotation_class_with_params() {
    assert_snapshot!(check("annotation class Deprecated(pub message: String)"));
}

#[test]
fn annotation_class_marker() {
    assert_snapshot!(check("annotation class Serializable"));
}

#[test]
fn annotation_applied_to_class() {
    assert_snapshot!(check("@Deprecated(message = \"old\")\npub class OldApi {}"));
}

#[test]
fn annotation_single_param_shorthand() {
    assert_snapshot!(check("@JsonName(\"user_name\")\nclass User {}"));
}

#[test]
fn annotation_meta_target() {
    assert_snapshot!(check("@Target(\"type\")\nannotation class MyAnnotation"));
}

#[test]
fn fn_with_default_args() {
    assert_snapshot!(check(
        "fn greet(msg: String = \"hi\", count: Int = 1) -> String { msg }"
    ));
}

#[test]
fn ctor_with_default_args() {
    assert_snapshot!(check("class User(pub name: String, pub age: Int = 0) {}"));
}

#[test]
fn impl_block() {
    assert_snapshot!(check(
        "impl Area for Circle { fn area(self) -> Float { self.r } }"
    ));
}

#[test]
fn inherent_impl() {
    assert_snapshot!(check(
        "impl Vec2 { fn length(self) -> Float { 1.0 } fn scale(self, factor: Float) -> Float { factor } }"
    ));
}

#[test]
fn package_decl() {
    assert_snapshot!(check("package com.example.foo;"));
}

#[test]
fn import_single() {
    assert_snapshot!(check("import java.util.List;"));
}

#[test]
fn import_with_alias() {
    assert_snapshot!(check("import java.util.HashMap as HMap;"));
}

#[test]
fn full_file() {
    assert_snapshot!(check(
        "package com.example;\nimport java.util.List;\n\npub fn main() { let x: Int = 42; x }"
    ));
}

// --- TASK-003: for/while/loop/break/continue/lambda ---

#[test]
fn for_in_loop() {
    assert_snapshot!(check("fn f() { for x in xs { println(x); } }"));
}

#[test]
fn while_loop() {
    assert_snapshot!(check("fn f() { while running { tick(); } }"));
}

#[test]
fn loop_with_break_value() {
    assert_snapshot!(check("fn f() -> Int { loop { break 42; } }"));
}

#[test]
fn loop_with_continue() {
    assert_snapshot!(check(
        "fn f() { loop { if skip { continue; } process(); } }"
    ));
}

#[test]
fn break_no_value() {
    assert_snapshot!(check("fn f() { loop { break; } }"));
}

#[test]
fn lambda_single_expr() {
    assert_snapshot!(check("fn f() { let double = |x: Int| x; }"));
}

#[test]
fn lambda_no_params() {
    assert_snapshot!(check("fn f() { let greet = || hello; }"));
}

#[test]
fn lambda_multiple_params() {
    assert_snapshot!(check("fn f() { let add = |a: Int, b: Int| -> Int { a }; }"));
}

#[test]
fn lambda_inferred_types() {
    assert_snapshot!(check("fn f() { xs.map(|x| x); }"));
}

#[test]
fn for_while_no_semicolon() {
    assert_snapshot!(check(
        "fn f() { for x in xs { process(x); }\nwhile cond { tick(); }\nloop { break; } }"
    ));
}

#[test]
fn range_exclusive() {
    assert_snapshot!(check("fn f() { for i in 0..10 { process(i); } }"));
}

#[test]
fn range_inclusive() {
    assert_snapshot!(check("fn f() { for i in 1..=9 { process(i); } }"));
}

#[test]
fn typealias_simple() {
    assert_snapshot!(check("typealias StringList = List<String>;"));
}

#[test]
fn typealias_generic() {
    assert_snapshot!(check("pub typealias Mapping<K, V> = java.util.Map<K, V>;"));
}

// --- unsafe/safe/cast/deref/ref-mut (#052) ---

#[test]
fn unsafe_block() {
    assert_snapshot!(check("fn f() { unsafe { let x = 1; x } }"));
}

#[test]
fn unsafe_expr() {
    assert_snapshot!(check("fn f() -> Int { unsafe 42 }"));
}

#[test]
fn safe_block() {
    assert_snapshot!(check("fn f() { safe { call() } }"));
}

#[test]
fn as_cast() {
    assert_snapshot!(check("fn f(x: Int) -> Long { x as Long }"));
}

#[test]
fn deref_expr() {
    assert_snapshot!(check("fn f(r: ref mut Int) -> Int { *r }"));
}

#[test]
fn ref_mut_create() {
    assert_snapshot!(check("fn f() { let r = ref mut 42; }"));
}

#[test]
fn if_let_no_trailing_semi() {
    assert_snapshot!(check(
        "fn main() {
    let x: Option<Int> = Some(42);
    if let Some(v) = x {
        v
    }
    let y = 1;
}"
    ));
}

#[test]
fn while_let_no_trailing_semi() {
    assert_snapshot!(check(
        "fn main() {
    while let Some(v) = iter.next() {
        println(v);
    }
    let done = true;
}"
    ));
}

#[test]
fn abstract_method_without_body() {
    assert_snapshot!(check(
        "abstract class Shape {
    abstract fn area() -> Float;
    fn name() -> String { \"shape\" }
}"
    ));
}

#[test]
fn abstract_method_with_body() {
    assert_snapshot!(check(
        "abstract class Shape {
    abstract fn area() -> Float { 0.0 }
}"
    ));
}
