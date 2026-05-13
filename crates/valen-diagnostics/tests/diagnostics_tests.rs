use valen_ast::{FileId, Span};
use valen_diagnostics::{DiagCode, Diagnostics, Severity};

fn dummy_span() -> Span {
    Span::new(0, 10, FileId(0))
}

#[test]
fn new_is_empty() {
    let d = Diagnostics::new();
    assert!(d.is_empty());
    assert_eq!(d.len(), 0);
    assert!(!d.has_errors());
}

#[test]
fn error_sets_has_errors() {
    let mut d = Diagnostics::new();
    d.error(DiagCode::TYPE_MISMATCH, dummy_span(), "bad type");
    assert!(d.has_errors());
    assert_eq!(d.len(), 1);
}

#[test]
fn warning_does_not_set_has_errors() {
    let mut d = Diagnostics::new();
    d.warning(DiagCode::TYPE_MISMATCH, dummy_span(), "watch out");
    assert!(!d.has_errors());
    assert_eq!(d.len(), 1);
}

#[test]
fn iter_yields_all() {
    let mut d = Diagnostics::new();
    d.error(DiagCode::NAME_NOT_FOUND, dummy_span(), "a");
    d.warning(DiagCode::TYPE_MISMATCH, dummy_span(), "b");
    let items: Vec<_> = d.iter().collect();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].severity, Severity::Error);
    assert_eq!(items[1].severity, Severity::Warning);
}

#[test]
fn severity_display() {
    assert_eq!(format!("{}", Severity::Error), "error");
    assert_eq!(format!("{}", Severity::Warning), "warning");
    assert_eq!(format!("{}", Severity::Hint), "hint");
}

#[test]
fn diagcode_display() {
    assert_eq!(format!("{}", DiagCode(1)), "V0001");
    assert_eq!(format!("{}", DiagCode(100)), "V0100");
    assert_eq!(format!("{}", DiagCode(9999)), "V9999");
}

#[test]
fn diagnostic_display() {
    let mut d = Diagnostics::new();
    d.error(
        DiagCode::TYPE_MISMATCH,
        dummy_span(),
        "expected Int, got String",
    );
    let diag = d.iter().next().unwrap();
    assert_eq!(format!("{diag}"), "error[V0300]: expected Int, got String");
}
