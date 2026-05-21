//! Source code location tracking.

use std::fmt;

/// Byte offsets into a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: u32,
    pub end: u32,
    pub file_id: FileId,
}

impl Span {
    pub const DUMMY: Span = Span {
        start: 0,
        end: 0,
        file_id: FileId(0),
    };

    /// Creates a new span from byte offsets within the given file.
    pub fn new(start: u32, end: u32, file_id: FileId) -> Self {
        Self {
            start,
            end,
            file_id,
        }
    }

    /// Returns the length of this span in bytes.
    pub fn len(&self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    /// Returns `true` if this span covers zero bytes.
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Returns the smallest span covering both `self` and `other`.
    ///
    /// If the two spans belong to different files, a debug assertion fires
    /// (panics in debug builds) and `self` is returned as a graceful fallback
    /// in release builds.
    pub fn merge(self, other: Span) -> Span {
        debug_assert_eq!(
            self.file_id, other.file_id,
            "cannot merge spans across files"
        );
        if self.file_id != other.file_id {
            return self;
        }
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
            file_id: self.file_id,
        }
    }
}

/// Opaque identifier for a source file in the compilation session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(pub u32);

/// AST node paired with its source location.
#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    /// Wraps a value with its source span.
    pub fn new(node: T, span: Span) -> Self {
        Self { node, span }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}
