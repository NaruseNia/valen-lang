//! Recursive descent parser producing `valen_ast::Item` nodes.
//!
//! Key decisions:
//! - Hand-written RD parser (not chumsky) for control over error recovery.
//! - `;` is **statement terminator**; block-tail expressions without `;` become the block value.
//! - `::` appears only in enum variant / associated item paths; `.` is used for
//!   package paths, type paths, and member access.
//! - `?` operator binds tightly to the preceding expression.
//!
//! MVP coverage (Phase 0 parser spike):
//! - top-level `fn NAME() { BLOCK }` (no params / return type / generics / visibility yet)
//! - `let` / `let mut` bindings with initializer, no type annotation
//! - expression statement (`expr;`) and tail expression (no `;`)
//! - literals: int / string / bool
//! - single-segment path (identifier reference)
//! - parenthesized expression, unary `-` / `!`, binary arithmetic / comparison / logical

use smol_str::SmolStr;
use valen_ast::token::TokenKind;
use valen_ast::{
    BinaryExpr, BinaryOp, Block, ClassDecl, ClassKind, Expr, FileId, FnDecl, Item, LetStmt,
    Literal, Path, PathSegment, Span, Stmt, UnaryExpr, UnaryOp, Visibility,
};
use valen_diagnostics::{DiagCode, Diagnostics};

use crate::lexer::lex;

pub struct Parser {
    tokens: Vec<(TokenKind, Span)>,
    pos: usize,
    file_id: FileId,
    diagnostics: Diagnostics,
}

impl Parser {
    pub fn new(source: &str, file_id: FileId) -> Self {
        Self {
            tokens: lex(source, file_id),
            pos: 0,
            file_id,
            diagnostics: Diagnostics::new(),
        }
    }

    pub fn parse_file(&mut self) -> Vec<Item> {
        let mut items = Vec::new();
        while !self.at_eof() {
            match self.parse_item() {
                Some(item) => items.push(item),
                None => {
                    self.recover_to_item_boundary();
                }
            }
        }
        items
    }

    pub fn into_diagnostics(self) -> Diagnostics {
        self.diagnostics
    }

    fn parse_item(&mut self) -> Option<Item> {
        let start = self.peek_span();
        let vis = self.parse_visibility();
        match self.peek() {
            TokenKind::Fn => self.parse_fn_decl(vis, start).map(Item::Fn),
            TokenKind::Class => self
                .parse_class(vis, ClassKind::Final, start)
                .map(Item::Class),
            TokenKind::Open | TokenKind::Abstract | TokenKind::Sealed => {
                let kind = self.parse_class_kind();
                self.parse_class(vis, kind, start).map(Item::Class)
            }
            _ => {
                let span = self.peek_span();
                self.diagnostics.error(
                    DiagCode::PARSE_EXPECTED_EXPR,
                    span,
                    SmolStr::from("expected top-level item (e.g. `fn`, `class`)"),
                );
                None
            }
        }
    }

    fn parse_visibility(&mut self) -> Visibility {
        match self.peek() {
            TokenKind::Pub => {
                self.bump();
                Visibility::Pub
            }
            TokenKind::Internal => {
                self.bump();
                Visibility::Internal
            }
            TokenKind::Private => {
                self.bump();
                Visibility::Private
            }
            _ => Visibility::Internal,
        }
    }

    fn parse_class_kind(&mut self) -> ClassKind {
        match self.peek() {
            TokenKind::Open => {
                self.bump();
                ClassKind::Open
            }
            TokenKind::Abstract => {
                self.bump();
                ClassKind::Abstract
            }
            TokenKind::Sealed => {
                self.bump();
                ClassKind::Sealed
            }
            _ => ClassKind::Final,
        }
    }

    fn parse_fn_decl(&mut self, visibility: Visibility, start: Span) -> Option<FnDecl> {
        self.expect(TokenKind::Fn)?;
        let name = self.expect_ident()?;
        self.expect(TokenKind::LParen)?;
        self.expect(TokenKind::RParen)?;
        let body = self.parse_block()?;
        let span = start.merge(body.span);
        Some(FnDecl {
            visibility,
            name,
            generics: Vec::new(),
            params: Vec::new(),
            return_type: None,
            body: Some(body),
            span,
        })
    }

    fn parse_class(
        &mut self,
        visibility: Visibility,
        kind: ClassKind,
        start: Span,
    ) -> Option<ClassDecl> {
        self.expect(TokenKind::Class)?;
        let name = self.expect_ident()?;
        self.expect(TokenKind::LBrace)?;
        let end = self.expect(TokenKind::RBrace)?;
        Some(ClassDecl {
            visibility,
            kind,
            name,
            generics: Vec::new(),
            ctor_params: Vec::new(),
            superclass: None,
            traits: Vec::new(),
            body: Vec::new(),
            span: start.merge(end),
        })
    }

    fn parse_block(&mut self) -> Option<Block> {
        let start = self.peek_span();
        self.expect(TokenKind::LBrace)?;

        let mut stmts: Vec<Stmt> = Vec::new();
        let mut tail: Option<Box<Expr>> = None;

        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            if self.at(&TokenKind::Let) {
                let let_stmt = self.parse_let()?;
                stmts.push(Stmt::Let(let_stmt));
                continue;
            }

            let expr = self.parse_expr()?;
            if self.at(&TokenKind::Semi) {
                self.bump();
                stmts.push(Stmt::ExprSemi(expr));
            } else if self.at(&TokenKind::RBrace) {
                tail = Some(Box::new(expr));
                break;
            } else {
                let span = self.peek_span();
                self.diagnostics.error(
                    DiagCode::PARSE_EXPECTED_SEMI,
                    span,
                    SmolStr::from("expected `;` after expression or `}` to end block"),
                );
                return None;
            }
        }

        let end = self.expect(TokenKind::RBrace)?;
        Some(Block {
            stmts,
            tail,
            span: start.merge(end),
        })
    }

    fn parse_let(&mut self) -> Option<LetStmt> {
        let start = self.expect(TokenKind::Let)?;
        let mutable = self.eat(&TokenKind::Mut).is_some();
        let name = self.expect_ident()?;
        self.expect(TokenKind::Eq)?;
        let init = self.parse_expr()?;
        let end = self.expect(TokenKind::Semi)?;
        Some(LetStmt {
            mutable,
            name,
            ty: None,
            init,
            span: start.merge(end),
        })
    }

    fn parse_expr(&mut self) -> Option<Expr> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Option<Expr> {
        let mut lhs = self.parse_and()?;
        while self.at(&TokenKind::PipePipe) {
            self.bump();
            let rhs = self.parse_and()?;
            lhs = combine_binary(BinaryOp::Or, lhs, rhs);
        }
        Some(lhs)
    }

    fn parse_and(&mut self) -> Option<Expr> {
        let mut lhs = self.parse_eq()?;
        while self.at(&TokenKind::AmpAmp) {
            self.bump();
            let rhs = self.parse_eq()?;
            lhs = combine_binary(BinaryOp::And, lhs, rhs);
        }
        Some(lhs)
    }

    fn parse_eq(&mut self) -> Option<Expr> {
        let mut lhs = self.parse_cmp()?;
        loop {
            let op = match self.peek() {
                TokenKind::EqEq => BinaryOp::Eq,
                TokenKind::NotEq => BinaryOp::Ne,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_cmp()?;
            lhs = combine_binary(op, lhs, rhs);
        }
        Some(lhs)
    }

    fn parse_cmp(&mut self) -> Option<Expr> {
        let mut lhs = self.parse_add()?;
        loop {
            let op = match self.peek() {
                TokenKind::Lt => BinaryOp::Lt,
                TokenKind::Le => BinaryOp::Le,
                TokenKind::Gt => BinaryOp::Gt,
                TokenKind::Ge => BinaryOp::Ge,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_add()?;
            lhs = combine_binary(op, lhs, rhs);
        }
        Some(lhs)
    }

    fn parse_add(&mut self) -> Option<Expr> {
        let mut lhs = self.parse_mul()?;
        loop {
            let op = match self.peek() {
                TokenKind::Plus => BinaryOp::Add,
                TokenKind::Minus => BinaryOp::Sub,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_mul()?;
            lhs = combine_binary(op, lhs, rhs);
        }
        Some(lhs)
    }

    fn parse_mul(&mut self) -> Option<Expr> {
        let mut lhs = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                TokenKind::Star => BinaryOp::Mul,
                TokenKind::Slash => BinaryOp::Div,
                TokenKind::Percent => BinaryOp::Rem,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_unary()?;
            lhs = combine_binary(op, lhs, rhs);
        }
        Some(lhs)
    }

    fn parse_unary(&mut self) -> Option<Expr> {
        let op = match self.peek() {
            TokenKind::Minus => Some(UnaryOp::Neg),
            TokenKind::Bang => Some(UnaryOp::Not),
            _ => None,
        };
        if let Some(op) = op {
            let start = self.peek_span();
            self.bump();
            let inner = self.parse_unary()?;
            let span = start.merge(expr_span(&inner));
            return Some(Expr::Unary(UnaryExpr {
                op,
                expr: Box::new(inner),
                span,
            }));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Option<Expr> {
        let span = self.peek_span();
        match self.peek().clone() {
            TokenKind::IntLit(n) => {
                self.bump();
                Some(Expr::Literal(Literal::Int(n, span)))
            }
            TokenKind::StringLit(s) => {
                self.bump();
                Some(Expr::Literal(Literal::String(s, span)))
            }
            TokenKind::BoolLit(b) => {
                self.bump();
                Some(Expr::Literal(Literal::Bool(b, span)))
            }
            TokenKind::Ident(name) => {
                self.bump();
                Some(Expr::Path(Path {
                    segments: vec![PathSegment {
                        name,
                        turbofish: false,
                        generics: Vec::new(),
                        span,
                    }],
                    span,
                }))
            }
            TokenKind::LParen => {
                self.bump();
                let inner = self.parse_expr()?;
                self.expect(TokenKind::RParen)?;
                Some(inner)
            }
            _ => {
                self.diagnostics.error(
                    DiagCode::PARSE_EXPECTED_EXPR,
                    span,
                    SmolStr::from("expected expression"),
                );
                None
            }
        }
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek(), TokenKind::Eof)
    }

    fn peek(&self) -> &TokenKind {
        self.tokens
            .get(self.pos)
            .map(|(k, _)| k)
            .unwrap_or(&TokenKind::Eof)
    }

    fn peek_span(&self) -> Span {
        self.tokens
            .get(self.pos)
            .map(|(_, s)| *s)
            .unwrap_or(Span::new(0, 0, self.file_id))
    }

    fn bump(&mut self) -> (TokenKind, Span) {
        let tok = self.tokens[self.pos].clone();
        self.pos += 1;
        tok
    }

    fn at(&self, kind: &TokenKind) -> bool {
        self.peek() == kind
    }

    fn eat(&mut self, kind: &TokenKind) -> Option<Span> {
        if self.at(kind) {
            let (_, span) = self.bump();
            Some(span)
        } else {
            None
        }
    }

    fn expect(&mut self, kind: TokenKind) -> Option<Span> {
        if self.at(&kind) {
            let (_, span) = self.bump();
            return Some(span);
        }
        let span = self.peek_span();
        self.diagnostics.error(
            DiagCode::PARSE_EXPECTED_EXPR,
            span,
            SmolStr::from(format!("expected {}", describe_token(&kind))),
        );
        None
    }

    fn expect_ident(&mut self) -> Option<SmolStr> {
        if let TokenKind::Ident(name) = self.peek().clone() {
            self.bump();
            return Some(name);
        }
        let span = self.peek_span();
        self.diagnostics.error(
            DiagCode::PARSE_EXPECTED_EXPR,
            span,
            SmolStr::from("expected identifier"),
        );
        None
    }

    fn recover_to_item_boundary(&mut self) {
        while !self.at_eof() {
            if matches!(
                self.peek(),
                TokenKind::Fn
                    | TokenKind::Class
                    | TokenKind::Pub
                    | TokenKind::Internal
                    | TokenKind::Private
                    | TokenKind::Open
                    | TokenKind::Abstract
                    | TokenKind::Sealed
            ) {
                return;
            }
            self.bump();
        }
    }
}

fn combine_binary(op: BinaryOp, lhs: Expr, rhs: Expr) -> Expr {
    let span = expr_span(&lhs).merge(expr_span(&rhs));
    Expr::Binary(BinaryExpr {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
        span,
    })
}

fn expr_span(expr: &Expr) -> Span {
    match expr {
        Expr::Literal(Literal::Int(_, s)) => *s,
        Expr::Literal(Literal::Float(_, s)) => *s,
        Expr::Literal(Literal::String(_, s)) => *s,
        Expr::Literal(Literal::Bool(_, s)) => *s,
        Expr::Literal(Literal::Unit(s)) => *s,
        Expr::Path(p) => p.span,
        Expr::Call(c) => c.span,
        Expr::MethodCall(m) => m.span,
        Expr::Field(f) => f.span,
        Expr::Binary(b) => b.span,
        Expr::Unary(u) => u.span,
        Expr::Assign(a) => a.span,
        Expr::If(i) => i.span,
        Expr::Match(m) => m.span,
        Expr::Block(b) => b.span,
        Expr::Return(r) => r.span,
        Expr::For(f) => f.span,
        Expr::While(w) => w.span,
        Expr::Loop(l) => l.span,
        Expr::Lambda(l) => l.span,
        Expr::Try(t) => t.span,
        Expr::StringInterp(s) => s.span,
        Expr::Safe(s) => s.span,
    }
}

fn describe_token(kind: &TokenKind) -> &'static str {
    match kind {
        TokenKind::Fn => "`fn`",
        TokenKind::Let => "`let`",
        TokenKind::Mut => "`mut`",
        TokenKind::Class => "`class`",
        TokenKind::LParen => "`(`",
        TokenKind::RParen => "`)`",
        TokenKind::LBrace => "`{`",
        TokenKind::RBrace => "`}`",
        TokenKind::Semi => "`;`",
        TokenKind::Eq => "`=`",
        TokenKind::Arrow => "`->`",
        TokenKind::Colon => "`:`",
        TokenKind::Comma => "`,`",
        _ => "token",
    }
}
