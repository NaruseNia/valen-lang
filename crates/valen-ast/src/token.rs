//! Token kinds produced by the lexer.
//!
//! The lexer is in `valen-parser`; this module defines the shared token alphabet
//! so AST consumers can reference it without depending on the parser crate.

use smol_str::SmolStr;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    IntLit(i64),
    FloatLit(f64),
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
    Abstract,
    Sealed,
    Package,
    Import,
    For,
    In,
    While,
    Loop,
    True,
    False,
    As,
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
    NotEq,
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
