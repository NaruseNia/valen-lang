//! Embedded standard library parsing.
//!
//! Embeds `stdlib/valen/core/core.vln` at compile time and provides a function
//! to parse it into AST items for prelude injection. The parse result is cached
//! via `OnceLock` so subsequent calls are zero-cost.

use std::sync::OnceLock;
use valen_ast::{FileId, Item};

const CORE_VLN: &str = include_str!("../../../stdlib/valen/core/core.vln");

static CORE_ITEMS: OnceLock<Vec<Item>> = OnceLock::new();

/// Return the parsed AST items from the embedded `core.vln`.
///
/// Uses a synthetic `FileId(u32::MAX)` to distinguish stdlib spans from user code.
/// The result is cached so parsing only happens once per process.
pub fn parse_core_stdlib() -> &'static [Item] {
    CORE_ITEMS.get_or_init(|| {
        let result = valen_parser::parse(CORE_VLN, FileId(u32::MAX));
        assert!(
            !result.diagnostics.has_errors(),
            "embedded stdlib core.vln has parse errors: {:?}",
            result.diagnostics
        );
        result.items
    })
}
