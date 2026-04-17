//! Type system.
//!
//! - Primitive nominal types (`Int` etc.) are defined here
//! - `T?` is desugared to `Option<T>` at this layer, not in the parser
//! - `T!` (platform type) is internal-only; never surfaces to user code
//! - Declaration-site variance (`in` / `out`) enforced at trait/type param sites
//! - `?` operator checks: Result context strict, Option context requires return type `Option<U>`

pub fn check() {
    todo!()
}
