use async_lsp::lsp_types::DiagnosticSeverity;
use valen_lsp::server::{analyze_document, extract_word_at};

// -- diagnostics --

#[test]
fn valid_source_no_diagnostics() {
    let (_, diags) = analyze_document("fn main() -> Int { 42 }");
    assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
}

#[test]
fn parse_error_produces_diagnostic() {
    let (_, diags) = analyze_document("fn main( { }");
    assert!(!diags.is_empty(), "expected parse error diagnostics");
    assert!(diags
        .iter()
        .any(|d| d.severity == Some(DiagnosticSeverity::ERROR)));
}

#[test]
fn type_error_produces_diagnostic() {
    let (_, diags) = analyze_document("fn main() -> Int { true }");
    assert!(
        diags
            .iter()
            .any(|d| d.severity == Some(DiagnosticSeverity::ERROR)),
        "expected type mismatch diagnostic, got: {diags:?}"
    );
}

#[test]
fn diagnostic_code_is_valen_format() {
    let (_, diags) = analyze_document("fn main() -> Int { true }");
    for d in &diags {
        if let Some(async_lsp::lsp_types::NumberOrString::String(code)) = &d.code {
            assert!(code.starts_with('V'), "code should start with V: {code}");
        }
    }
}

#[test]
fn diagnostic_range_is_valid() {
    let (_, diags) = analyze_document("fn main( { }");
    for d in &diags {
        assert!(d.range.start.line <= d.range.end.line);
    }
}

// -- goto definition (via HIR lookup) --

#[test]
fn goto_def_finds_function() {
    let (doc, _) = analyze_document("fn greet() -> String { \"hi\" }\nfn main() -> Int { 42 }");
    let hir = doc.hir.as_ref().unwrap();
    let def = hir.defs.values().find(|d| d.name == "greet");
    assert!(def.is_some(), "greet should be in HIR defs");
}

#[test]
fn goto_def_finds_class() {
    let (doc, _) = analyze_document("class Dog(pub name: String) {}");
    let hir = doc.hir.as_ref().unwrap();
    let def = hir.defs.values().find(|d| d.name == "Dog");
    assert!(def.is_some(), "Dog should be in HIR defs");
}

#[test]
fn goto_def_finds_enum() {
    let (doc, _) = analyze_document("enum Color { Red, Green, Blue }");
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
