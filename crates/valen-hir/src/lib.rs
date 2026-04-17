//! High-level IR: after AST, before bytecode.
//!
//! Responsibilities:
//! - Name resolution (imports, visibility, path lookup)
//! - Type checking (including `Option`/`Result` + `?` propagation rules)
//! - Coherence / orphan rule enforcement
//!   - trait or outermost nominal constructor must be locally owned
//!   - typealias does not grant ownership
//!   - blanket impls rejected (std only)
//!   - impl uniqueness globally
//! - Exhaustiveness check for `match` over Valen enums and `@closed` Java types
//! - Lowering to a typed IR suitable for codegen

pub mod coherence;
pub mod exhaustive;
pub mod resolve;
pub mod ty;

pub struct Hir {
    // TODO: resolved items, type-annotated bodies, impl table, etc.
}

pub fn lower(_items: &[valen_ast::Item]) -> Hir {
    todo!("run resolve → typeck → coherence → exhaustiveness → lower to HIR")
}
