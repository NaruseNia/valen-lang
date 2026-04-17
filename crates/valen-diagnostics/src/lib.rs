//! Diagnostics (errors, warnings, hints) shared across the compiler.

use smol_str::SmolStr;
use valen_ast::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Hint,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: DiagCode,
    pub message: SmolStr,
    pub primary: Span,
    pub labels: Vec<Label>,
    pub notes: Vec<SmolStr>,
}

#[derive(Debug, Clone)]
pub struct Label {
    pub span: Span,
    pub message: SmolStr,
}

/// Stable diagnostic codes. Format: `V<number>`.
///
/// Groupings:
/// - `V0001`-`V0099`: lexer
/// - `V0100`-`V0199`: parser
/// - `V0200`-`V0299`: name resolution
/// - `V0300`-`V0399`: type check
/// - `V0400`-`V0499`: coherence / orphan rule
/// - `V0500`-`V0599`: match exhaustiveness
/// - `V0600`-`V0699`: Java interop
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DiagCode(pub u16);

impl DiagCode {
    pub const LEX_UNKNOWN_CHAR: DiagCode = DiagCode(1);
    pub const PARSE_EXPECTED_SEMI: DiagCode = DiagCode(100);
    pub const PARSE_EXPECTED_EXPR: DiagCode = DiagCode(101);
    pub const NAME_NOT_FOUND: DiagCode = DiagCode(200);
    pub const TYPE_MISMATCH: DiagCode = DiagCode(300);
    pub const ORPHAN_RULE_VIOLATION: DiagCode = DiagCode(400);
    pub const BLANKET_IMPL_NOT_ALLOWED: DiagCode = DiagCode(401);
    pub const IMPL_CONFLICT: DiagCode = DiagCode(402);
    pub const MATCH_NOT_EXHAUSTIVE: DiagCode = DiagCode(500);
    pub const JAVA_EXCEPTION_NOT_WRAPPED: DiagCode = DiagCode(600);
}

/// Accumulates diagnostics during a single compilation run.
#[derive(Debug, Default)]
pub struct Diagnostics {
    entries: Vec<Diagnostic>,
}

impl Diagnostics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, diag: Diagnostic) {
        self.entries.push(diag);
    }

    pub fn error(&mut self, code: DiagCode, span: Span, message: impl Into<SmolStr>) {
        self.entries.push(Diagnostic {
            severity: Severity::Error,
            code,
            message: message.into(),
            primary: span,
            labels: Vec::new(),
            notes: Vec::new(),
        });
    }

    pub fn warning(&mut self, code: DiagCode, span: Span, message: impl Into<SmolStr>) {
        self.entries.push(Diagnostic {
            severity: Severity::Warning,
            code,
            message: message.into(),
            primary: span,
            labels: Vec::new(),
            notes: Vec::new(),
        });
    }

    pub fn iter(&self) -> impl Iterator<Item = &Diagnostic> {
        self.entries.iter()
    }

    pub fn has_errors(&self) -> bool {
        self.entries.iter().any(|d| d.severity == Severity::Error)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
