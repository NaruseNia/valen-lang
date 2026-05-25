use async_lsp::lsp_types::DiagnosticSeverity;
use valen_ast::FileId;
use valen_lsp::server::{analyze_document, extract_word_at};

fn analyze_document_test(
    text: &str,
    file_id: FileId,
) -> (
    valen_lsp::server::DocumentState,
    Vec<async_lsp::lsp_types::Diagnostic>,
) {
    analyze_document(text, file_id, &[])
}

// -- diagnostics --

#[test]
fn valid_source_no_diagnostics() {
    let (_, diags) = analyze_document_test("fn main() -> Int { 42 }", FileId(0));
    assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
}

#[test]
fn parse_error_produces_diagnostic() {
    let (_, diags) = analyze_document_test("fn main( { }", FileId(0));
    assert!(!diags.is_empty(), "expected parse error diagnostics");
    assert!(diags
        .iter()
        .any(|d| d.severity == Some(DiagnosticSeverity::ERROR)));
}

#[test]
fn type_error_produces_diagnostic() {
    let (_, diags) = analyze_document_test("fn main() -> Int { true }", FileId(0));
    assert!(
        diags
            .iter()
            .any(|d| d.severity == Some(DiagnosticSeverity::ERROR)),
        "expected type mismatch diagnostic, got: {diags:?}"
    );
}

#[test]
fn diagnostic_code_is_valen_format() {
    let (_, diags) = analyze_document_test("fn main() -> Int { true }", FileId(0));
    for d in &diags {
        if let Some(async_lsp::lsp_types::NumberOrString::String(code)) = &d.code {
            assert!(code.starts_with('V'), "code should start with V: {code}");
        }
    }
}

#[test]
fn diagnostic_range_is_valid() {
    let (_, diags) = analyze_document_test("fn main( { }", FileId(0));
    for d in &diags {
        assert!(d.range.start.line <= d.range.end.line);
    }
}

// -- goto definition (via HIR lookup) --

#[test]
fn goto_def_finds_function() {
    let (doc, _) = analyze_document_test(
        "fn greet() -> String { \"hi\" }\nfn main() -> Int { 42 }",
        FileId(0),
    );
    let hir = doc.hir.as_ref().unwrap();
    let def = hir.defs.values().find(|d| d.name == "greet");
    assert!(def.is_some(), "greet should be in HIR defs");
}

#[test]
fn goto_def_finds_class() {
    let (doc, _) = analyze_document_test("class Dog(pub name: String) {}", FileId(0));
    let hir = doc.hir.as_ref().unwrap();
    let def = hir.defs.values().find(|d| d.name == "Dog");
    assert!(def.is_some(), "Dog should be in HIR defs");
}

#[test]
fn goto_def_finds_enum() {
    let (doc, _) = analyze_document_test("enum Color { Red, Green, Blue }", FileId(0));
    let hir = doc.hir.as_ref().unwrap();
    let def = hir.defs.values().find(|d| d.name == "Color");
    assert!(def.is_some(), "Color should be in HIR defs");
}

// -- extract_word_at --

#[test]
fn extract_word_simple() {
    assert_eq!(extract_word_at("fn main() {}", 3), Some("main"));
}

#[test]
fn extract_word_at_start() {
    assert_eq!(extract_word_at("hello world", 0), Some("hello"));
}

#[test]
fn extract_word_underscore() {
    assert_eq!(extract_word_at("my_var = 42", 0), Some("my_var"));
    assert_eq!(extract_word_at("my_var = 42", 3), Some("my_var"));
}

#[test]
fn extract_word_on_space() {
    assert_eq!(extract_word_at("fn main", 2), None);
}

#[test]
fn extract_word_past_end() {
    assert_eq!(extract_word_at("hello", 10), None);
}

#[test]
fn local_variable_completion_bodies_present() {
    let src = r#"package one.nxeu;

class Circle(r: Float) {}

fn main() {
    let circle = Circle(10.0f);
    let x = circle;
}
"#;
    let (doc, _diags) = analyze_document_test(src, FileId(0));
    assert!(doc.bodies.is_some(), "typed bodies should be present");
}

#[test]
fn local_var_and_dot_completion_full_repro() {
    use valen_lsp::server::collect_local_variables_pub;

    let src = r#"package one.nxeu;

trait Shape {
    fn area(self) -> Float;
}

class Rectangle(w: Float, h: Float) {}
class Circle(r: Float) {}

impl Shape for Rectangle {
    fn area(self) -> Float {
        self.w * self.h
    }
}

impl Shape for Circle {
    fn area(self) -> Float {
        self.r * self.r * 3.14f
    }
}

fn main() {
    let circle = Circle(10.0f);
    let rect = Rectangle(10.0f, 10.0f);
    let circleArea = circle.area();
}
"#;
    let (doc, diags) = analyze_document_test(src, FileId(0));
    eprintln!(
        "diags: {:?}",
        diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );

    // Bodies must be present
    assert!(doc.bodies.is_some(), "typed bodies must be present");
    let bodies = doc.bodies.as_ref().unwrap();
    assert!(!bodies.is_empty(), "should have at least one body");

    // Find offset inside main() after `let rect = ...;\n    `
    let marker = "let circleArea";
    let offset = src.find(marker).unwrap() as u32;

    let locals = collect_local_variables_pub(bodies, offset, doc.hir.as_ref());
    let local_names: Vec<&str> = locals.iter().map(|(n, _)| n.as_str()).collect();
    eprintln!("locals at offset {offset}: {local_names:?}");
    assert!(
        local_names.contains(&"circle"),
        "circle should be visible, got: {local_names:?}"
    );
    assert!(
        local_names.contains(&"rect"),
        "rect should be visible, got: {local_names:?}"
    );

    // Check that circle's type resolves to Circle
    let circle_ty = locals.iter().find(|(n, _)| n == "circle").map(|(_, t)| t);
    eprintln!("circle type: {circle_ty:?}");
    assert!(
        matches!(circle_ty, Some(valen_hir::Ty::Named(n)) if n == "Circle"),
        "circle should be typed as Circle, got: {circle_ty:?}"
    );

    // Check trait_impls contain Circle -> area
    let hir = doc.hir.as_ref().unwrap();
    eprintln!("trait_impls count: {}", hir.trait_impls.len());
    for entry in &hir.trait_impls {
        eprintln!(
            "  impl {} for {} ({} methods)",
            entry.trait_name,
            entry.target_name,
            entry.methods.len()
        );
        for &mid in &entry.methods {
            if let Some(mdef) = hir.defs.get(&mid) {
                eprintln!("    method: {}", mdef.name);
            }
        }
    }

    let circle_impls: Vec<_> = hir
        .trait_impls
        .iter()
        .filter(|e| e.target_name.as_str() == "Circle")
        .collect();
    assert!(
        !circle_impls.is_empty(),
        "should have trait impl for Circle"
    );
    let has_area = circle_impls.iter().any(|e| {
        e.methods.iter().any(|&mid| {
            hir.defs
                .get(&mid)
                .map(|d| d.name.as_str() == "area")
                .unwrap_or(false)
        })
    });
    assert!(has_area, "Circle should have area() from trait impl");

    // Check type_methods
    eprintln!(
        "type_methods keys: {:?}",
        hir.type_methods.keys().collect::<Vec<_>>()
    );
    if let Some(methods) = hir.type_methods.get("Circle") {
        eprintln!("Circle type_methods: {} entries", methods.len());
    } else {
        eprintln!("Circle has no type_methods entry");
    }

    // Check fields: class Circle(r: Float)
    for def in hir.defs.values() {
        if let valen_hir::DefKind::Class(c) = &def.kind {
            if def.name == "Circle" {
                eprintln!(
                    "Circle ctor_params: {:?}",
                    c.ctor_params.iter().map(|p| &p.name).collect::<Vec<_>>()
                );
            }
        }
    }
}

#[test]
fn dot_completion_with_incomplete_code() {
    use valen_lsp::server::find_let_type_annotation_pub;

    // Simulate typing "circle." — incomplete expression causes parse error,
    // fn main() is dropped from AST. The text-based heuristic must still
    // resolve the receiver type from `let circle = Circle(...)`.
    let src = r#"package one.nxeu;

trait Shape {
    fn area(self) -> Float;
}

class Circle(r: Float) {}

impl Shape for Circle {
    fn area(self) -> Float {
        self.r * self.r * 3.14f
    }
}

fn main() {
    let circle = Circle(10.0f);
    circle.
}
"#;
    let (doc, _diags) = analyze_document_test(src, FileId(0));

    let hir = doc
        .hir
        .as_ref()
        .expect("HIR must exist even with parse errors");
    let circle_impl_count = hir
        .trait_impls
        .iter()
        .filter(|e| e.target_name == "Circle")
        .count();
    assert!(
        circle_impl_count > 0,
        "Circle trait impl must survive parse errors"
    );

    let inferred = find_let_type_annotation_pub(src, "circle");
    assert_eq!(
        inferred.as_deref(),
        Some("Circle"),
        "should infer Circle from constructor call"
    );
}

#[test]
fn complex_generics_file_does_not_break_analysis() {
    let src = r#"package one.nxeu;

enum Color {
    Red,
    Green,
    Blue,
}

trait Shape {
    fn area(self) -> Float;
    fn color(col: Color) -> Unit;
}

trait Satisfied<T> {
    fn satisfied(self) -> Option<T>;
}

class SatisfiedShape<T: Shape + Satisfied> {
    fn area(shape: T) -> Option<Float> {
        Option::Some(shape.area())
    }
}

impl<T: Shape + Satisfied> Satisfied<String> for SatisfiedShape<T> {
    fn satisfied(self) -> Option<String> {
        Some("Satisfied")
    }
}

class Rectangle(w: Float, h: Float) {
    fn isSatisfied(self) -> Option<Rectangle> {
        if self.w > 0.0f && self.h > 0.0f {
            Some(self)
        } else {
            None
        }
    }
}

class Circle(r: Float) {}

impl Shape for Rectangle {
    fn area(self) -> Float {
        self.w * self.h
    }
    fn color(col: Color) -> Unit {
    }
}

impl Shape for Circle {
    fn area(self) -> Float {
        self.r * self.r * 3.14f
    }
    fn color(col: Color) -> Unit {
    }
}

fn main() {
    let circle = Circle(10.0f);
    let rect = Rectangle(10.0f, 10.0f);
    let circleArea = circle.area();
    let mut rectArea = rect.area();
    let margedArea = rectArea + circleArea;
}
"#;
    let (doc, _diags) = analyze_document_test(src, FileId(0));
    assert!(doc.hir.is_some(), "HIR should be present");
    assert!(doc.bodies.is_some(), "typed bodies should be present");

    let hir = doc.hir.as_ref().unwrap();
    assert!(
        hir.defs.values().any(|d| d.name == "Circle"),
        "Circle should be in defs"
    );
    assert!(
        hir.defs.values().any(|d| d.name == "Rectangle"),
        "Rectangle should be in defs"
    );
}

// -- basic analysis: locals, return types, HIR defs (#042) -----------------

#[test]
fn analysis_finds_local_variables() {
    use valen_lsp::server::collect_local_variables_pub;

    let src = r#"
fn main() {
    let x = 42;
    let y = "hello";
    let z = x;
}
"#;
    let (doc, _) = analyze_document_test(src, FileId(0));
    assert!(doc.bodies.is_some());
    let bodies = doc.bodies.as_ref().unwrap();

    let marker = "let z";
    let offset = src.find(marker).unwrap() as u32;
    let locals = collect_local_variables_pub(bodies, offset, doc.hir.as_ref());
    let names: Vec<&str> = locals.iter().map(|(n, _)| n.as_str()).collect();
    assert!(names.contains(&"x"), "x should be visible, got: {names:?}");
    assert!(names.contains(&"y"), "y should be visible, got: {names:?}");
}

#[test]
fn analysis_fn_def_has_return_type() {
    let src = "fn add(a: Int, b: Int) -> Int { a + b }";
    let (doc, diags) = analyze_document_test(src, FileId(0));
    assert!(diags.is_empty(), "no errors expected: {diags:?}");
    let hir = doc.hir.as_ref().unwrap();
    let add_def = hir
        .defs
        .values()
        .find(|d| d.name == "add")
        .expect("add should be in HIR defs");
    if let valen_hir::DefKind::Fn(f) = &add_def.kind {
        assert!(
            f.return_ty != Some(valen_hir::TyRef::Prim(valen_hir::PrimTy::Unit)),
            "add() should have a non-Unit return type"
        );
    } else {
        panic!("add should be a function def");
    }
}

#[test]
fn analysis_trait_def_discovered() {
    let src = r#"
trait Greetable {
    fn greet(self) -> String;
}
"#;
    let (doc, _) = analyze_document_test(src, FileId(0));
    let hir = doc.hir.as_ref().unwrap();
    assert!(
        hir.defs.values().any(|d| d.name == "Greetable"),
        "Greetable trait should be in defs"
    );
}

#[test]
fn analysis_enum_variants_in_hir() {
    let src = r#"
enum Direction {
    North,
    South,
    East,
    West,
}
"#;
    let (doc, _) = analyze_document_test(src, FileId(0));
    let hir = doc.hir.as_ref().unwrap();
    let dir_def = hir
        .defs
        .values()
        .find(|d| d.name == "Direction")
        .expect("Direction should be in defs");
    if let valen_hir::DefKind::Enum(e) = &dir_def.kind {
        assert_eq!(e.variants.len(), 4, "should have 4 variants");
    } else {
        panic!("Direction should be an enum def");
    }
}

#[test]
fn analysis_data_class_fields() {
    let src = "data class Point(pub x: Int, pub y: Int);";
    let (doc, _) = analyze_document_test(src, FileId(0));
    let hir = doc.hir.as_ref().unwrap();
    let point_def = hir
        .defs
        .values()
        .find(|d| d.name == "Point")
        .expect("Point should be in defs");
    if let valen_hir::DefKind::DataClass(dc) = &point_def.kind {
        assert_eq!(dc.ctor_params.len(), 2, "should have 2 ctor params");
    } else {
        panic!("Point should be a data class def");
    }
}

#[test]
fn analysis_impl_methods_in_type_methods() {
    let src = r#"
class Dog(pub name: String) {}
impl Dog {
    fn bark(self) -> String { self.name }
}
"#;
    let (doc, _) = analyze_document_test(src, FileId(0));
    let hir = doc.hir.as_ref().unwrap();
    assert!(
        hir.type_methods.contains_key("Dog"),
        "Dog should have type_methods entry"
    );
}
