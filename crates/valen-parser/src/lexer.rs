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

#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\r\n\f]+")]
#[logos(skip r"//[^\n]*")]
enum RawTok {
    #[token("fn")]
    Fn,
    #[token("let")]
    Let,
    #[token("mut")]
    Mut,
    #[token("return")]
    Return,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("true")]
    True,
    #[token("false")]
    False,

    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token(",")]
    Comma,
    #[token(";")]
    Semi,
    #[token("::")]
    DoubleColon,
    #[token(":")]
    Colon,
    #[token("->")]
    Arrow,

    #[token("==")]
    EqEq,
    #[token("!=")]
    NotEq,
    #[token("<=")]
    Le,
    #[token(">=")]
    Ge,
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
    #[token("!")]
    Bang,
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

    #[regex(r"[0-9][0-9_]*", parse_int)]
    IntLit(i64),

    #[regex(r#""([^"\\]|\\.)*""#, parse_string)]
    StringLit(SmolStr),

    #[regex(r"[A-Za-z_][A-Za-z0-9_]*", |lex| SmolStr::from(lex.slice()))]
    Ident(SmolStr),
}

fn parse_int(lex: &mut logos::Lexer<'_, RawTok>) -> Option<i64> {
    lex.slice().replace('_', "").parse::<i64>().ok()
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

pub struct Lexer<'src> {
    inner: logos::Lexer<'src, RawTok>,
    file_id: FileId,
    eof_emitted: bool,
}

impl<'src> Lexer<'src> {
    pub fn new(source: &'src str, file_id: FileId) -> Self {
        Self {
            inner: RawTok::lexer(source),
            file_id,
            eof_emitted: false,
        }
    }

    pub fn next_token(&mut self) -> Option<(TokenKind, Span)> {
        let Some(raw) = self.inner.next() else {
            if self.eof_emitted {
                return None;
            }
            self.eof_emitted = true;
            let end = self.inner.source().len() as u32;
            return Some((TokenKind::Eof, Span::new(end, end, self.file_id)));
        };
        let range = self.inner.span();
        let span = Span::new(range.start as u32, range.end as u32, self.file_id);
        let kind = match raw {
            Ok(tok) => map_token(tok),
            Err(()) => TokenKind::Error(SmolStr::from(self.inner.slice())),
        };
        Some((kind, span))
    }
}

pub fn lex(source: &str, file_id: FileId) -> Vec<(TokenKind, Span)> {
    let mut lex = Lexer::new(source, file_id);
    let mut out = Vec::new();
    while let Some(tok) = lex.next_token() {
        out.push(tok);
    }
    out
}

fn map_token(raw: RawTok) -> TokenKind {
    match raw {
        RawTok::Fn => TokenKind::Fn,
        RawTok::Let => TokenKind::Let,
        RawTok::Mut => TokenKind::Mut,
        RawTok::Return => TokenKind::Return,
        RawTok::If => TokenKind::If,
        RawTok::Else => TokenKind::Else,
        RawTok::True => TokenKind::BoolLit(true),
        RawTok::False => TokenKind::BoolLit(false),
        RawTok::LParen => TokenKind::LParen,
        RawTok::RParen => TokenKind::RParen,
        RawTok::LBrace => TokenKind::LBrace,
        RawTok::RBrace => TokenKind::RBrace,
        RawTok::Comma => TokenKind::Comma,
        RawTok::Semi => TokenKind::Semi,
        RawTok::Colon => TokenKind::Colon,
        RawTok::DoubleColon => TokenKind::DoubleColon,
        RawTok::Arrow => TokenKind::Arrow,
        RawTok::Eq => TokenKind::Eq,
        RawTok::EqEq => TokenKind::EqEq,
        RawTok::NotEq => TokenKind::NotEq,
        RawTok::Lt => TokenKind::Lt,
        RawTok::Le => TokenKind::Le,
        RawTok::Gt => TokenKind::Gt,
        RawTok::Ge => TokenKind::Ge,
        RawTok::AmpAmp => TokenKind::AmpAmp,
        RawTok::PipePipe => TokenKind::PipePipe,
        RawTok::Bang => TokenKind::Bang,
        RawTok::Plus => TokenKind::Plus,
        RawTok::Minus => TokenKind::Minus,
        RawTok::Star => TokenKind::Star,
        RawTok::Slash => TokenKind::Slash,
        RawTok::Percent => TokenKind::Percent,
        RawTok::IntLit(n) => TokenKind::IntLit(n),
        RawTok::StringLit(s) => TokenKind::StringLit(s),
        RawTok::Ident(s) => TokenKind::Ident(s),
    }
}
