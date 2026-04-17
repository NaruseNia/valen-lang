//! Match exhaustiveness check.
//!
//! - Valen `enum` / `sealed` hierarchy → strict exhaustive (non-exhaustive = error)
//! - Java types → exhaustive only if `@closed` metadata present; otherwise no check
//! - Range patterns, or-patterns, `@` bindings handled by a usefulness algorithm
//!   (see Rust's `rustc_mir_build::thir::pattern::usefulness` for inspiration)

pub fn check_exhaustive() {
    todo!()
}
