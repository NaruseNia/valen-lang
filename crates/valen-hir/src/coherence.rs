//! Orphan rule + blanket impl ban + global uniqueness for trait impls.
//!
//! Rules (MVP):
//! - `impl Trait for Type` requires the trait OR the outermost nominal type
//!   constructor of `Type` to be owned by the current compile unit.
//! - Typealias does not grant ownership.
//! - `impl<T: Foo> Bar for T` (blanket) is rejected outside of std.
//! - Same `(trait, type)` pair may have only one impl globally.
//! - All Java types are foreign (including arrays, SAM, auto-generated proxies).
//! - inherent impl wins over trait impl when both provide the same method name;
//!   ambiguous trait impls require explicit `as Trait` disambiguation.

pub fn check_coherence() {
    todo!()
}
