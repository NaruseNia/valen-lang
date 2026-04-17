//! Recursive descent parser producing `valen_ast::Item` nodes.
//!
//! Key decisions:
//! - Hand-written RD parser (not chumsky) for control over error recovery.
//! - `;` is **statement terminator**; block-tail expressions without `;` become the block value.
//! - `::` appears only in enum variant / associated item paths; `.` is used for
//!   package paths, type paths, and member access.
//! - `?` operator binds tightly to the preceding expression.

use valen_ast::{FileId, Item};
use valen_diagnostics::Diagnostics;

pub struct Parser<'src> {
    _source: &'src str,
    _file_id: FileId,
    _diagnostics: Diagnostics,
}

impl<'src> Parser<'src> {
    pub fn new(source: &'src str, file_id: FileId) -> Self {
        Self {
            _source: source,
            _file_id: file_id,
            _diagnostics: Diagnostics::new(),
        }
    }

    pub fn parse_file(&mut self) -> Vec<Item> {
        todo!("parse package decl + imports + top-level items")
    }

    pub fn into_diagnostics(self) -> Diagnostics {
        self._diagnostics
    }
}
