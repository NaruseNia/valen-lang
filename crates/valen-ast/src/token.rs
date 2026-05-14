//! Token kinds produced by the lexer.
//!
//! The lexer is in `valen-parser`; this module defines the shared token alphabet
//! so AST consumers can reference it without depending on the parser crate.

use smol_str::SmolStr;

/// Classification of a single lexer token.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    IntLit(i64),
    LongLit(i64),
    FloatLit(f32),
    DoubleLit(f64),
    StringLit(SmolStr),
    FStringLit(SmolStr),
    CharLit(char),
    BoolLit(bool),

    // Identifiers
    Ident(SmolStr),

    // Keywords
    Fn,
    Let,
    Mut,
    SelfKw,
    Return,
    If,
    Else,
    Match,
    Class,
    Data,
    Enum,
    Trait,
    Impl,
    Pub,
    Internal,
    Private,
    Open,
    Override,
    Abstract,
    Sealed,
    Package,
    Import,
    For,
    In,
    While,
    Loop,
    Break,
    Continue,
    True,
    False,
    As,
    Safe,
    Annotation,
    // Reserved for future
    Suspend,
    Async,
    Await,
    Yield,
    TypeAlias,

    // Punctuation
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Semi,
    Colon,
    DoubleColon,
    Dot,
    DotDot,
    DotDotEq,
    Arrow,      // ->
    FatArrow,   // =>
    Question,   // ?
    Bang,       // !
    At,         // @
    Underscore, // _

    // Operators
    Eq,
    EqEq,
    EqEqEq,
    NotEq,
    NotEqEq,
    Lt,
    Le,
    Gt,
    Ge,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Amp,
    AmpAmp,
    Pipe,
    PipePipe,
    Caret,
    Shl,
    Shr,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    PercentEq,

    // Trivia
    Whitespace,
    LineComment,
    BlockComment,
    DocComment(SmolStr),

    // End of file
    Eof,

    // Error recovery
    Error(SmolStr),
}
