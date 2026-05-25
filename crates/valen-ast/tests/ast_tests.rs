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
#[cfg_attr(
    debug_assertions,
    should_panic(expected = "cannot merge spans across files")
)]
fn span_merge_different_files() {
    let a = Span::new(0, 10, FileId(0));
    let b = Span::new(0, 10, FileId(1));
    let merged = a.merge(b);
    // In release builds, merge returns `self` as a graceful fallback
    #[cfg(not(debug_assertions))]
    {
        assert_eq!(merged.start, a.start);
        assert_eq!(merged.end, a.end);
        assert_eq!(merged.file_id, a.file_id);
    }
    let _ = merged;
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

// -- Literal::span() -------------------------------------------------------

use smol_str::SmolStr;
use valen_ast::{BindingPattern, RangePattern};
use valen_ast::{Expr, FnType, Literal, Pattern, Type, TypePath, TypePathSegment};

#[test]
fn literal_int_span() {
    let lit = Literal::Int(42, Span::new(0, 2, FileId(0)));
    assert_eq!(lit.span(), Span::new(0, 2, FileId(0)));
}

#[test]
fn literal_long_span() {
    let lit = Literal::Long(100, Span::new(5, 9, FileId(0)));
    assert_eq!(lit.span(), Span::new(5, 9, FileId(0)));
}

#[test]
fn literal_float_span() {
    let lit = Literal::Float(1.5, Span::new(0, 5, FileId(0)));
    assert_eq!(lit.span(), Span::new(0, 5, FileId(0)));
}

#[test]
fn literal_double_span() {
    let lit = Literal::Double(9.81, Span::new(10, 15, FileId(1)));
    assert_eq!(lit.span(), Span::new(10, 15, FileId(1)));
}

#[test]
fn literal_char_span() {
    let lit = Literal::Char('A', Span::new(0, 3, FileId(0)));
    assert_eq!(lit.span(), Span::new(0, 3, FileId(0)));
}

#[test]
fn literal_string_span() {
    let lit = Literal::String(SmolStr::from("hello"), Span::new(0, 7, FileId(0)));
    assert_eq!(lit.span(), Span::new(0, 7, FileId(0)));
}

#[test]
fn literal_bool_span() {
    let lit = Literal::Bool(true, Span::new(0, 4, FileId(0)));
    assert_eq!(lit.span(), Span::new(0, 4, FileId(0)));
}

#[test]
fn literal_unit_span() {
    let lit = Literal::Unit(Span::new(0, 2, FileId(0)));
    assert_eq!(lit.span(), Span::new(0, 2, FileId(0)));
}

// -- Expr::span() -----------------------------------------------------------

#[test]
fn expr_literal_span() {
    let expr = Expr::Literal(Literal::Int(1, Span::new(0, 1, FileId(0))));
    assert_eq!(expr.span(), Span::new(0, 1, FileId(0)));
}

#[test]
fn expr_break_span() {
    let expr = Expr::Break(valen_ast::BreakExpr {
        value: None,
        span: Span::new(10, 15, FileId(0)),
    });
    assert_eq!(expr.span(), Span::new(10, 15, FileId(0)));
}

#[test]
fn expr_continue_span() {
    let expr = Expr::Continue(valen_ast::ContinueExpr {
        span: Span::new(20, 28, FileId(0)),
    });
    assert_eq!(expr.span(), Span::new(20, 28, FileId(0)));
}

#[test]
fn expr_return_span() {
    let expr = Expr::Return(valen_ast::ReturnExpr {
        value: None,
        span: Span::new(5, 11, FileId(0)),
    });
    assert_eq!(expr.span(), Span::new(5, 11, FileId(0)));
}

#[test]
fn expr_unsafe_span() {
    let inner = Box::new(Expr::Literal(Literal::Int(0, Span::new(8, 9, FileId(0)))));
    let expr = Expr::Unsafe(valen_ast::UnsafeExpr {
        body: inner,
        span: Span::new(0, 10, FileId(0)),
    });
    assert_eq!(expr.span(), Span::new(0, 10, FileId(0)));
}

#[test]
fn expr_cast_span() {
    let inner = Box::new(Expr::Literal(Literal::Int(0, Span::new(0, 1, FileId(0)))));
    let target_ty = Type::Path(TypePath {
        segments: vec![TypePathSegment {
            name: SmolStr::from("Long"),
            generics: vec![],
            span: Span::new(5, 9, FileId(0)),
        }],
        span: Span::new(5, 9, FileId(0)),
    });
    let expr = Expr::Cast(valen_ast::CastExpr {
        expr: inner,
        target_ty,
        span: Span::new(0, 9, FileId(0)),
    });
    assert_eq!(expr.span(), Span::new(0, 9, FileId(0)));
}

#[test]
fn expr_deref_span() {
    let inner = Box::new(Expr::Literal(Literal::Int(0, Span::new(1, 2, FileId(0)))));
    let expr = Expr::Deref(valen_ast::DerefExpr {
        expr: inner,
        span: Span::new(0, 2, FileId(0)),
    });
    assert_eq!(expr.span(), Span::new(0, 2, FileId(0)));
}

#[test]
fn expr_refmut_span() {
    let inner = Box::new(Expr::Literal(Literal::Int(0, Span::new(8, 9, FileId(0)))));
    let expr = Expr::RefMutCreate(valen_ast::RefMutExpr {
        expr: inner,
        span: Span::new(0, 9, FileId(0)),
    });
    assert_eq!(expr.span(), Span::new(0, 9, FileId(0)));
}

// -- Pattern::span() --------------------------------------------------------

#[test]
fn pattern_wildcard_span() {
    let pat = Pattern::Wildcard(Span::new(0, 1, FileId(0)));
    assert_eq!(pat.span(), Span::new(0, 1, FileId(0)));
}

#[test]
fn pattern_literal_span() {
    let pat = Pattern::Literal(Literal::Int(42, Span::new(5, 7, FileId(0))));
    assert_eq!(pat.span(), Span::new(5, 7, FileId(0)));
}

#[test]
fn pattern_binding_span() {
    let pat = Pattern::Binding(BindingPattern {
        name: SmolStr::from("x"),
        mutable: false,
        span: Span::new(10, 11, FileId(0)),
    });
    assert_eq!(pat.span(), Span::new(10, 11, FileId(0)));
}

#[test]
fn pattern_tuple_span() {
    let pat = Pattern::Tuple(vec![], Span::new(3, 5, FileId(0)));
    assert_eq!(pat.span(), Span::new(3, 5, FileId(0)));
}

#[test]
fn pattern_range_span() {
    let pat = Pattern::Range(RangePattern {
        start: Some(Literal::Int(0, Span::new(0, 1, FileId(0)))),
        end: Some(Literal::Int(9, Span::new(4, 5, FileId(0)))),
        inclusive: true,
        span: Span::new(0, 5, FileId(0)),
    });
    assert_eq!(pat.span(), Span::new(0, 5, FileId(0)));
}

#[test]
fn pattern_or_span() {
    let pat = Pattern::Or(vec![], Span::new(0, 10, FileId(0)));
    assert_eq!(pat.span(), Span::new(0, 10, FileId(0)));
}

// -- Type::span() -----------------------------------------------------------

#[test]
fn type_path_span() {
    let ty = Type::Path(TypePath {
        segments: vec![TypePathSegment {
            name: SmolStr::from("Int"),
            generics: vec![],
            span: Span::new(0, 3, FileId(0)),
        }],
        span: Span::new(0, 3, FileId(0)),
    });
    assert_eq!(ty.span(), Span::new(0, 3, FileId(0)));
}

#[test]
fn type_nullable_span() {
    let inner = Box::new(Type::Path(TypePath {
        segments: vec![TypePathSegment {
            name: SmolStr::from("String"),
            generics: vec![],
            span: Span::new(0, 6, FileId(0)),
        }],
        span: Span::new(0, 6, FileId(0)),
    }));
    let ty = Type::Nullable {
        inner,
        span: Span::new(0, 7, FileId(0)),
    };
    assert_eq!(ty.span(), Span::new(0, 7, FileId(0)));
}

#[test]
fn type_fn_span() {
    let ret = Box::new(Type::Path(TypePath {
        segments: vec![TypePathSegment {
            name: SmolStr::from("Int"),
            generics: vec![],
            span: Span::new(12, 15, FileId(0)),
        }],
        span: Span::new(12, 15, FileId(0)),
    }));
    let ty = Type::Fn(FnType {
        params: vec![],
        return_type: ret,
        span: Span::new(0, 15, FileId(0)),
    });
    assert_eq!(ty.span(), Span::new(0, 15, FileId(0)));
}

#[test]
fn type_tuple_span() {
    let ty = Type::Tuple(vec![], Span::new(0, 2, FileId(0)));
    assert_eq!(ty.span(), Span::new(0, 2, FileId(0)));
}

#[test]
fn type_refmut_span() {
    let inner = Box::new(Type::Path(TypePath {
        segments: vec![TypePathSegment {
            name: SmolStr::from("Int"),
            generics: vec![],
            span: Span::new(8, 11, FileId(0)),
        }],
        span: Span::new(8, 11, FileId(0)),
    }));
    let ty = Type::RefMut {
        inner,
        span: Span::new(0, 11, FileId(0)),
    };
    assert_eq!(ty.span(), Span::new(0, 11, FileId(0)));
}
