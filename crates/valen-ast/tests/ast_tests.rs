use valen_ast::{FileId, Span};

#[test]
fn span_new() {
    let s = Span::new(10, 20, FileId(1));
    assert_eq!(s.start, 10);
    assert_eq!(s.end, 20);
    assert_eq!(s.file_id, FileId(1));
}

#[test]
fn span_len() {
    assert_eq!(Span::new(5, 15, FileId(0)).len(), 10);
    assert_eq!(Span::new(0, 0, FileId(0)).len(), 0);
}

#[test]
fn span_is_empty() {
    assert!(Span::new(5, 5, FileId(0)).is_empty());
    assert!(!Span::new(5, 6, FileId(0)).is_empty());
}

#[test]
fn span_merge_same_file() {
    let a = Span::new(10, 20, FileId(0));
    let b = Span::new(15, 30, FileId(0));
    let merged = a.merge(b);
    assert_eq!(merged.start, 10);
    assert_eq!(merged.end, 30);
    assert_eq!(merged.file_id, FileId(0));
}

#[test]
#[should_panic(expected = "cannot merge spans across files")]
fn span_merge_different_files_panics() {
    let a = Span::new(0, 10, FileId(0));
    let b = Span::new(0, 10, FileId(1));
    a.merge(b);
}

#[test]
fn span_display() {
    let s = Span::new(42, 55, FileId(0));
    assert_eq!(format!("{s}"), "42..55");
}

#[test]
fn span_dummy() {
    let d = Span::DUMMY;
    assert_eq!(d.start, 0);
    assert_eq!(d.end, 0);
    assert!(d.is_empty());
}
