//! Token lexer. Backed by logos.
//!
//! MVP scope: keywords, identifiers, integer/string/bool literals, punctuation
//! and arithmetic/comparison operators needed by the minimal parser.
//!
//! Deferred (Phase 0 follow-ups):
//! - f-string literals (`f"..."`): interpolation spans multiple tokens.
//! - block / doc comments: single-line `//` only for now.
//! - float / char literals.
//! - shebang line handling.

use logos::Logos;
use smol_str::SmolStr;
use valen_ast::token::TokenKind;
use valen_ast::{FileId, Span};
use valen_diagnostics::{DiagCode, Diagnostics};

#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\r\n\f]+")]
#[logos(skip r"//[^\n]*")]
#[logos(skip r"/\*([^*]|\*[^/])*\*/")]
enum RawTok {
    // Keywords
    #[token("fn")]
    Fn,
    #[token("let")]
    Let,
    #[token("mut")]
    Mut,
    #[token("self")]
    SelfKw,
    #[token("return")]
    Return,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("match")]
    Match,
    #[token("class")]
    Class,
    #[token("data")]
    Data,
    #[token("enum")]
    Enum,
    #[token("trait")]
    Trait,
    #[token("impl")]
    Impl,
    #[token("pub")]
    Pub,
    #[token("internal")]
    Internal,
    #[token("private")]
    Private,
    #[token("open")]
    Open,
    #[token("override")]
    Override,
    #[token("abstract")]
    Abstract,
    #[token("sealed")]
    Sealed,
    #[token("package")]
    Package,
    #[token("import")]
    Import,
    #[token("for")]
    For,
    #[token("in")]
    In,
    #[token("while")]
    While,
    #[token("break")]
    Break,
    #[token("continue")]
    Continue,
    #[token("loop")]
    Loop,
    #[token("true")]
    True,
    #[token("false")]
    False,
    #[token("as")]
    As,
    #[token("safe")]
    Safe,
    #[token("suspend")]
    Suspend,
    #[token("async")]
    Async,
    #[token("await")]
    Await,
    #[token("yield")]
    Yield,
    #[token("typealias")]
    TypeAlias,
    #[token("type")]
    Type,
    #[token("annotation")]
    Annotation,
    // JVM reserved words — cannot be used as identifiers (`new` excluded: see map_token)
    #[token("static")]
    Static,
    #[token("void")]
    Void,
    #[token("new")]
    New,
    #[token("this")]
    This,
    #[token("super")]
    Super,
    #[token("null")]
    Null,
    #[token("throw")]
    Throw,
    #[token("try")]
    Try,
    #[token("catch")]
    Catch,
    #[token("finally")]
    Finally,
    #[token("extends")]
    Extends,
    #[token("implements")]
    Implements,

    // Punctuation
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token(",")]
    Comma,
    #[token(";")]
    Semi,
    #[token("::")]
    DoubleColon,
    #[token(":")]
    Colon,
    #[token("..=")]
    DotDotEq,
    #[token("..")]
    DotDot,
    #[token(".")]
    Dot,
    #[token("->")]
    Arrow,
    #[token("=>")]
    FatArrow,
    #[token("?")]
    Question,
    #[token("@")]
    At,
    #[token("_", priority = 3)]
    Underscore,

    // Operators — triple-char operators must appear before their two-char prefixes
    // so logos matches the longer token first.
    #[token("===")]
    EqEqEq,
    #[token("!==")]
    NotEqEq,
    #[token("==")]
    EqEq,
    #[token("!=")]
    NotEq,
    #[token("<=")]
    Le,
    #[token(">=")]
    Ge,
    #[token("<<")]
    Shl,
    #[token(">>")]
    Shr,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,
    #[token("=")]
    Eq,
    #[token("&&")]
    AmpAmp,
    #[token("||")]
    PipePipe,
    #[token("&")]
    Amp,
    #[token("|")]
    Pipe,
    #[token("^")]
    Caret,
    #[token("!")]
    Bang,
    #[token("+=")]
    PlusEq,
    #[token("-=")]
    MinusEq,
    #[token("*=")]
    StarEq,
    #[token("/=")]
    SlashEq,
    #[token("%=")]
    PercentEq,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,

    // Literals — longer/suffixed patterns must appear before shorter ones
    // so logos prefers the more specific match.
    #[regex(r"[0-9][0-9_]*[lL]", parse_long)]
    LongLit(i64),

    #[regex(r"[0-9][0-9_]*\.[0-9][0-9_]*([eE][+-]?[0-9]+)?[fF]", parse_float32)]
    Float32Lit(f32),

    #[regex(r"[0-9][0-9_]*\.[0-9][0-9_]*([eE][+-]?[0-9]+)?", parse_float)]
    FloatLit(f64),

    #[regex(r"[0-9][0-9_]*", parse_int)]
    IntLit(i64),

    #[regex(r"'([^'\\]|\\.)'", parse_char)]
    CharLit(char),

    #[regex(r#""([^"\\]|\\.)*""#, parse_string)]
    StringLit(SmolStr),

    #[regex(r"[A-Za-z_][A-Za-z0-9_]*", |lex| SmolStr::from(lex.slice()))]
    Ident(SmolStr),
}

fn parse_int(lex: &mut logos::Lexer<'_, RawTok>) -> Option<i64> {
    lex.slice().replace('_', "").parse::<i64>().ok()
}

fn parse_long(lex: &mut logos::Lexer<'_, RawTok>) -> Option<i64> {
    let s = lex.slice();
    // Strip trailing L/l suffix before parsing
    s[..s.len() - 1].replace('_', "").parse::<i64>().ok()
}

fn parse_float(lex: &mut logos::Lexer<'_, RawTok>) -> Option<f64> {
    lex.slice().replace('_', "").parse::<f64>().ok()
}

fn parse_float32(lex: &mut logos::Lexer<'_, RawTok>) -> Option<f32> {
    let s = lex.slice();
    // Strip trailing f/F suffix before parsing
    s[..s.len() - 1].replace('_', "").parse::<f32>().ok()
}

fn parse_char(lex: &mut logos::Lexer<'_, RawTok>) -> Option<char> {
    let raw = lex.slice();
    // Strip surrounding single quotes
    let inner = &raw[1..raw.len() - 1];
    let mut chars = inner.chars();
    let c = chars.next()?;
    if c != '\\' {
        return Some(c);
    }
    match chars.next()? {
        'n' => Some('\n'),
        't' => Some('\t'),
        'r' => Some('\r'),
        '\\' => Some('\\'),
        '\'' => Some('\''),
        '0' => Some('\0'),
        other => Some(other),
    }
}

fn parse_string(lex: &mut logos::Lexer<'_, RawTok>) -> Option<SmolStr> {
    let raw = lex.slice();
    let inner = &raw[1..raw.len() - 1];
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next()? {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            '\\' => out.push('\\'),
            '"' => out.push('"'),
            '0' => out.push('\0'),
            other => {
                out.push('\\');
                out.push(other);
            }
        }
    }
    Some(SmolStr::from(out))
}

/// UTF-8 BOM character (U+FEFF) that may appear at the start of source files.
const UTF8_BOM: &str = "\u{FEFF}";

/// Strip a leading UTF-8 BOM if present, returning the remaining source.
fn strip_bom(source: &str) -> &str {
    source.strip_prefix(UTF8_BOM).unwrap_or(source)
}

/// Stateful token iterator that wraps a logos lexer and tracks diagnostics.
pub struct Lexer<'src> {
    inner: logos::Lexer<'src, RawTok>,
    file_id: FileId,
    eof_emitted: bool,
    diagnostics: Diagnostics,
    /// Byte offset added to all spans to account for a stripped BOM.
    bom_offset: u32,
}

impl<'src> Lexer<'src> {
    /// Create a new lexer for the given source text and file identifier.
    ///
    /// A leading UTF-8 BOM (U+FEFF) is silently stripped before lexing.
    /// # Panics
    /// Panics if `source.len() > u32::MAX` (use [`lex`] for a graceful error).
    pub fn new(source: &'src str, file_id: FileId) -> Self {
        assert!(
            source.len() <= u32::MAX as usize,
            "source file too large for u32 byte offsets ({} bytes)",
            source.len()
        );
        let stripped = strip_bom(source);
        let bom_offset = (source.len() - stripped.len()) as u32;
        Self {
            inner: RawTok::lexer(stripped),
            file_id,
            eof_emitted: false,
            diagnostics: Diagnostics::new(),
            bom_offset,
        }
    }

    /// Advance to the next token, returning its kind and span. Returns `None` after EOF.
    pub fn next_token(&mut self) -> Option<(TokenKind, Span)> {
        let Some(raw) = self.inner.next() else {
            if self.eof_emitted {
                return None;
            }
            self.eof_emitted = true;
            let end = self.inner.source().len() as u32 + self.bom_offset;
            return Some((TokenKind::Eof, Span::new(end, end, self.file_id)));
        };
        let range = self.inner.span();
        let span = Span::new(
            range.start as u32 + self.bom_offset,
            range.end as u32 + self.bom_offset,
            self.file_id,
        );
        let kind = match raw {
            Ok(tok) => map_token(tok),
            Err(()) => {
                let slice = self.inner.slice();
                if slice.bytes().all(|b| b.is_ascii_digit() || b == b'_') && !slice.is_empty() {
                    self.diagnostics.error(
                        DiagCode::LEX_INT_OVERFLOW,
                        span,
                        SmolStr::from(format!("integer literal `{slice}` overflows i64")),
                    );
                }
                TokenKind::Error(SmolStr::from(slice))
            }
        };
        Some((kind, span))
    }

    /// Consume the lexer and return accumulated diagnostics.
    pub fn into_diagnostics(self) -> Diagnostics {
        self.diagnostics
    }
}

/// Convenience function: lex the entire source into a token vector and diagnostics.
///
/// Returns an immediate error diagnostic if the source exceeds `u32::MAX` bytes,
/// since [`Span`] uses `u32` byte offsets.
pub fn lex(source: &str, file_id: FileId) -> (Vec<(TokenKind, Span)>, Diagnostics) {
    if source.len() > u32::MAX as usize {
        let mut diags = Diagnostics::new();
        diags.error(
            DiagCode::LEX_FILE_TOO_LARGE,
            Span::new(0, 0, file_id),
            SmolStr::from(format!(
                "source file is too large ({} bytes); maximum supported size is {} bytes",
                source.len(),
                u32::MAX,
            )),
        );
        let eof = vec![(TokenKind::Eof, Span::new(0, 0, file_id))];
        return (eof, diags);
    }
    let mut lex = Lexer::new(source, file_id);
    let mut out = Vec::new();
    while let Some(tok) = lex.next_token() {
        out.push(tok);
    }
    (out, lex.into_diagnostics())
}

fn map_token(raw: RawTok) -> TokenKind {
    match raw {
        // Keywords
        RawTok::Fn => TokenKind::Fn,
        RawTok::Let => TokenKind::Let,
        RawTok::Mut => TokenKind::Mut,
        RawTok::SelfKw => TokenKind::SelfKw,
        RawTok::Return => TokenKind::Return,
        RawTok::If => TokenKind::If,
        RawTok::Else => TokenKind::Else,
        RawTok::Match => TokenKind::Match,
        RawTok::Class => TokenKind::Class,
        RawTok::Data => TokenKind::Data,
        RawTok::Enum => TokenKind::Enum,
        RawTok::Trait => TokenKind::Trait,
        RawTok::Impl => TokenKind::Impl,
        RawTok::Pub => TokenKind::Pub,
        RawTok::Internal => TokenKind::Internal,
        RawTok::Private => TokenKind::Private,
        RawTok::Open => TokenKind::Open,
        RawTok::Override => TokenKind::Override,
        RawTok::Abstract => TokenKind::Abstract,
        RawTok::Sealed => TokenKind::Sealed,
        RawTok::Package => TokenKind::Package,
        RawTok::Import => TokenKind::Import,
        RawTok::For => TokenKind::For,
        RawTok::In => TokenKind::In,
        RawTok::While => TokenKind::While,
        RawTok::Break => TokenKind::Break,
        RawTok::Continue => TokenKind::Continue,
        RawTok::Loop => TokenKind::Loop,
        RawTok::True => TokenKind::BoolLit(true),
        RawTok::False => TokenKind::BoolLit(false),
        RawTok::As => TokenKind::As,
        RawTok::Safe => TokenKind::Safe,
        RawTok::Suspend => TokenKind::Suspend,
        RawTok::Async => TokenKind::Async,
        RawTok::Await => TokenKind::Await,
        RawTok::Yield => TokenKind::Yield,
        RawTok::TypeAlias => TokenKind::TypeAlias,
        RawTok::Type => TokenKind::Type,
        RawTok::Annotation => TokenKind::Annotation,
        RawTok::Static => TokenKind::Static,
        RawTok::Void => TokenKind::Void,
        RawTok::New => TokenKind::Ident(SmolStr::from("new")),
        RawTok::This => TokenKind::This,
        RawTok::Super => TokenKind::Super,
        RawTok::Null => TokenKind::Null,
        RawTok::Throw => TokenKind::Throw,
        RawTok::Try => TokenKind::Try,
        RawTok::Catch => TokenKind::Catch,
        RawTok::Finally => TokenKind::Finally,
        RawTok::Extends => TokenKind::Extends,
        RawTok::Implements => TokenKind::Implements,
        // Punctuation
        RawTok::LParen => TokenKind::LParen,
        RawTok::RParen => TokenKind::RParen,
        RawTok::LBrace => TokenKind::LBrace,
        RawTok::RBrace => TokenKind::RBrace,
        RawTok::LBracket => TokenKind::LBracket,
        RawTok::RBracket => TokenKind::RBracket,
        RawTok::Comma => TokenKind::Comma,
        RawTok::Semi => TokenKind::Semi,
        RawTok::Colon => TokenKind::Colon,
        RawTok::DoubleColon => TokenKind::DoubleColon,
        RawTok::Dot => TokenKind::Dot,
        RawTok::DotDot => TokenKind::DotDot,
        RawTok::DotDotEq => TokenKind::DotDotEq,
        RawTok::Arrow => TokenKind::Arrow,
        RawTok::FatArrow => TokenKind::FatArrow,
        RawTok::Question => TokenKind::Question,
        RawTok::At => TokenKind::At,
        RawTok::Underscore => TokenKind::Underscore,
        // Operators
        RawTok::Eq => TokenKind::Eq,
        RawTok::EqEqEq => TokenKind::EqEqEq,
        RawTok::NotEqEq => TokenKind::NotEqEq,
        RawTok::EqEq => TokenKind::EqEq,
        RawTok::NotEq => TokenKind::NotEq,
        RawTok::Lt => TokenKind::Lt,
        RawTok::Le => TokenKind::Le,
        RawTok::Gt => TokenKind::Gt,
        RawTok::Ge => TokenKind::Ge,
        RawTok::Shl => TokenKind::Shl,
        RawTok::Shr => TokenKind::Shr,
        RawTok::AmpAmp => TokenKind::AmpAmp,
        RawTok::PipePipe => TokenKind::PipePipe,
        RawTok::Amp => TokenKind::Amp,
        RawTok::Pipe => TokenKind::Pipe,
        RawTok::Caret => TokenKind::Caret,
        RawTok::Bang => TokenKind::Bang,
        RawTok::PlusEq => TokenKind::PlusEq,
        RawTok::MinusEq => TokenKind::MinusEq,
        RawTok::StarEq => TokenKind::StarEq,
        RawTok::SlashEq => TokenKind::SlashEq,
        RawTok::PercentEq => TokenKind::PercentEq,
        RawTok::Plus => TokenKind::Plus,
        RawTok::Minus => TokenKind::Minus,
        RawTok::Star => TokenKind::Star,
        RawTok::Slash => TokenKind::Slash,
        RawTok::Percent => TokenKind::Percent,
        // Literals
        RawTok::LongLit(n) => TokenKind::LongLit(n),
        RawTok::Float32Lit(n) => TokenKind::FloatLit(n),
        RawTok::FloatLit(n) => TokenKind::DoubleLit(n),
        RawTok::IntLit(n) => TokenKind::IntLit(n),
        RawTok::CharLit(c) => TokenKind::CharLit(c),
        RawTok::StringLit(s) => TokenKind::StringLit(s),
        RawTok::Ident(s) => TokenKind::Ident(s),
    }
}
