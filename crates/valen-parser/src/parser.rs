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
    AssignExpr, AtPattern, BinaryExpr, BinaryOp, BindingPattern, Block, BreakExpr, CallArg,
    CallExpr, ClassDecl, ClassKind, ClassMember, ContinueExpr, CtorParam, DataClassDecl, EnumDecl,
    EnumField, EnumVariant, EnumVariantFields, Expr, FieldAccess, FileId, FnDecl, ForExpr,
    GenericParam, IfExpr, ImplBlock, ImplItem, ImportDecl, Item, LambdaExpr, LambdaParam, LetStmt,
    Literal, LoopExpr, MatchArm, MatchExpr, MethodCallExpr, PackageDecl, Param, Path, PathSegment,
    Pattern, RangeExpr, RangePattern, ReturnExpr, Span, Stmt, StructPattern, StructPatternField,
    TraitDecl, TraitItem, TryExpr, Type, TypePath, TypePathSegment, UnaryExpr, UnaryOp, Variance,
    Visibility, WhileExpr,
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
        let (tokens, lex_diagnostics) = lex(source, file_id);
        let mut diagnostics = Diagnostics::new();
        for diag in lex_diagnostics.iter() {
            diagnostics.push(diag.clone());
        }
        Self {
            tokens,
            pos: 0,
            file_id,
            diagnostics,
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
        if self.at(&TokenKind::Package) {
            return self.parse_package().map(Item::Package);
        }
        if self.at(&TokenKind::Import) {
            return self.parse_import().map(Item::Import);
        }

        let start = self.peek_span();
        let vis = self.parse_visibility();
        match self.peek() {
            TokenKind::Fn => self
                .parse_fn_decl(vis, start, false, false, false)
                .map(Item::Fn),
            TokenKind::Class => self
                .parse_class(vis, ClassKind::Final, start)
                .map(Item::Class),
            TokenKind::Open | TokenKind::Abstract | TokenKind::Sealed => {
                let kind = self.parse_class_kind();
                self.parse_class(vis, kind, start).map(Item::Class)
            }
            TokenKind::Data => self.parse_data_class(vis, start).map(Item::DataClass),
            TokenKind::Enum => self.parse_enum(vis, start).map(Item::Enum),
            TokenKind::Trait => self.parse_trait(vis, start).map(Item::Trait),
            TokenKind::Impl => self.parse_impl(start).map(Item::Impl),
            _ => {
                let span = self.peek_span();
                self.diagnostics.error(
                    DiagCode::PARSE_UNEXPECTED_TOKEN,
                    span,
                    SmolStr::from(
                        "expected top-level item (e.g. `fn`, `class`, `enum`, `trait`, `impl`)",
                    ),
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

    fn parse_fn_decl(
        &mut self,
        visibility: Visibility,
        start: Span,
        is_open: bool,
        is_override: bool,
        is_abstract: bool,
    ) -> Option<FnDecl> {
        self.expect(TokenKind::Fn)?;
        let name = self.expect_ident()?;
        let generics = self.parse_generic_params()?;
        self.expect(TokenKind::LParen)?;
        let params = self.parse_param_list()?;
        self.expect(TokenKind::RParen)?;

        let return_type = if self.eat(&TokenKind::Arrow).is_some() {
            Some(self.parse_type()?)
        } else {
            None
        };

        let body = self.parse_block()?;
        let span = start.merge(body.span);
        Some(FnDecl {
            visibility,
            name,
            generics,
            params,
            return_type,
            body: Some(body),
            is_open,
            is_override,
            is_abstract,
            span,
        })
    }

    fn parse_param_list(&mut self) -> Option<Vec<Param>> {
        let mut params = Vec::new();
        while !self.at(&TokenKind::RParen) && !self.at_eof() {
            if !params.is_empty() {
                self.expect(TokenKind::Comma)?;
                if self.at(&TokenKind::RParen) {
                    break;
                }
            }
            let param_start = self.peek_span();

            if self.at(&TokenKind::SelfKw)
                || self.at(&TokenKind::Mut) && self.lookahead(1) == &TokenKind::SelfKw
            {
                let mutable = self.eat(&TokenKind::Mut).is_some();
                let self_kw_span = self.peek_span();
                self.expect(TokenKind::SelfKw)?;
                let self_type = Type::Path(TypePath {
                    segments: vec![TypePathSegment {
                        name: SmolStr::from("Self"),
                        generics: Vec::new(),
                        span: self_kw_span,
                    }],
                    span: self_kw_span,
                });
                let span = param_start.merge(self.prev_span());
                params.push(Param {
                    name: SmolStr::from("self"),
                    ty: self_type,
                    mutable,
                    span,
                });
                continue;
            }

            let mutable = self.eat(&TokenKind::Mut).is_some();
            let name = self.expect_ident()?;
            self.expect(TokenKind::Colon)?;
            let ty = self.parse_type()?;
            let span = param_start.merge(type_span(&ty));
            params.push(Param {
                name,
                ty,
                mutable,
                span,
            });
        }
        Some(params)
    }

    fn parse_type(&mut self) -> Option<Type> {
        let start = self.peek_span();
        let name = self.expect_ident()?;
        let mut segments = vec![self.parse_type_path_segment(name, start)?];

        while self.eat(&TokenKind::Dot).is_some() {
            let seg_start = self.peek_span();
            let seg_name = self.expect_ident()?;
            segments.push(self.parse_type_path_segment(seg_name, seg_start)?);
        }

        let end = segments.last().map(|s| s.span).unwrap_or(start);
        let mut ty = Type::Path(TypePath {
            segments,
            span: start.merge(end),
        });

        if let Some(q_span) = self.eat(&TokenKind::Question) {
            let inner_span = type_span(&ty);
            ty = Type::Nullable {
                inner: Box::new(ty),
                span: inner_span.merge(q_span),
            };
        }

        Some(ty)
    }

    fn parse_type_path_segment(&mut self, name: SmolStr, start: Span) -> Option<TypePathSegment> {
        let mut generics = Vec::new();
        if self.eat(&TokenKind::Lt).is_some() {
            while !self.at(&TokenKind::Gt) && !self.at_eof() {
                if !generics.is_empty() {
                    self.expect(TokenKind::Comma)?;
                }
                generics.push(self.parse_type()?);
            }
            self.expect(TokenKind::Gt)?;
        }
        let end = if generics.is_empty() {
            start
        } else {
            self.tokens
                .get(self.pos.saturating_sub(1))
                .map(|(_, s)| *s)
                .unwrap_or(start)
        };
        Some(TypePathSegment {
            name,
            generics,
            span: start.merge(end),
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
        let generics = self.parse_generic_params()?;

        let ctor_params = if self.at(&TokenKind::LParen) {
            self.parse_ctor_params()?
        } else {
            Vec::new()
        };

        let supertypes = self.parse_supertypes()?;

        let (body, end) = if self.at(&TokenKind::LBrace) {
            self.expect(TokenKind::LBrace)?;
            let members = self.parse_class_body()?;
            let e = self.expect(TokenKind::RBrace)?;
            (members, e)
        } else {
            let e = self.expect(TokenKind::Semi)?;
            (Vec::new(), e)
        };

        Some(ClassDecl {
            visibility,
            kind,
            name,
            generics,
            ctor_params,
            supertypes,
            body,
            span: start.merge(end),
        })
    }

    fn parse_ctor_params(&mut self) -> Option<Vec<CtorParam>> {
        self.expect(TokenKind::LParen)?;
        let mut params = Vec::new();
        while !self.at(&TokenKind::RParen) && !self.at_eof() {
            if !params.is_empty() {
                self.expect(TokenKind::Comma)?;
                if self.at(&TokenKind::RParen) {
                    break;
                }
            }
            let param_start = self.peek_span();
            let vis = self.parse_visibility();
            let mutable = self.eat(&TokenKind::Mut).is_some();
            let name = self.expect_ident()?;
            self.expect(TokenKind::Colon)?;
            let ty = self.parse_type()?;
            let span = param_start.merge(type_span(&ty));
            params.push(CtorParam {
                visibility: vis,
                name,
                ty,
                mutable,
                span,
            });
        }
        self.expect(TokenKind::RParen)?;
        Some(params)
    }

    fn parse_supertypes(&mut self) -> Option<Vec<Type>> {
        if self.eat(&TokenKind::Colon).is_none() {
            return Some(Vec::new());
        }
        let mut types = vec![self.parse_type()?];
        while self.eat(&TokenKind::Comma).is_some() {
            types.push(self.parse_type()?);
        }
        Some(types)
    }

    fn parse_generic_params(&mut self) -> Option<Vec<GenericParam>> {
        if self.eat(&TokenKind::Lt).is_none() {
            return Some(Vec::new());
        }
        let mut params = Vec::new();
        while !self.at(&TokenKind::Gt) && !self.at_eof() {
            if !params.is_empty() {
                self.expect(TokenKind::Comma)?;
                if self.at(&TokenKind::Gt) {
                    break;
                }
            }
            let start = self.peek_span();
            let name = self.expect_ident()?;
            let mut bounds = Vec::new();
            if self.eat(&TokenKind::Colon).is_some() {
                bounds.push(self.parse_type()?);
                while self.eat(&TokenKind::Plus).is_some() {
                    bounds.push(self.parse_type()?);
                }
            }
            let end = bounds.last().map(type_span).unwrap_or(start);
            params.push(GenericParam {
                name,
                variance: Variance::Invariant,
                bounds,
                span: start.merge(end),
            });
        }
        self.expect(TokenKind::Gt)?;
        Some(params)
    }

    fn parse_class_body(&mut self) -> Option<Vec<ClassMember>> {
        let mut members = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            let member_start = self.peek_span();
            let vis = self.parse_visibility();
            match self.peek() {
                TokenKind::Fn | TokenKind::Open | TokenKind::Override | TokenKind::Abstract => {
                    let is_open = self.eat(&TokenKind::Open).is_some();
                    let is_override = self.eat(&TokenKind::Override).is_some();
                    let is_abstract = self.eat(&TokenKind::Abstract).is_some();
                    let method =
                        self.parse_fn_decl(vis, member_start, is_open, is_override, is_abstract)?;
                    members.push(ClassMember::Method(method));
                }
                _ => {
                    let span = self.peek_span();
                    self.diagnostics.error(
                        DiagCode::PARSE_UNEXPECTED_TOKEN,
                        span,
                        SmolStr::from("expected method declaration in class body"),
                    );
                    return None;
                }
            }
        }
        Some(members)
    }

    fn parse_data_class(&mut self, visibility: Visibility, start: Span) -> Option<DataClassDecl> {
        self.expect(TokenKind::Data)?;
        self.expect(TokenKind::Class)?;
        let name = self.expect_ident()?;
        let generics = self.parse_generic_params()?;
        let ctor_params = self.parse_ctor_params()?;
        let end = self.expect(TokenKind::Semi)?;
        Some(DataClassDecl {
            visibility,
            name,
            generics,
            ctor_params,
            span: start.merge(end),
        })
    }

    fn parse_enum(&mut self, visibility: Visibility, start: Span) -> Option<EnumDecl> {
        self.expect(TokenKind::Enum)?;
        let name = self.expect_ident()?;
        let generics = self.parse_generic_params()?;
        self.expect(TokenKind::LBrace)?;
        let mut variants = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            if !variants.is_empty() {
                self.expect(TokenKind::Comma)?;
                if self.at(&TokenKind::RBrace) {
                    break;
                }
            }
            variants.push(self.parse_enum_variant()?);
        }
        let end = self.expect(TokenKind::RBrace)?;
        Some(EnumDecl {
            visibility,
            name,
            generics,
            variants,
            span: start.merge(end),
        })
    }

    fn parse_enum_variant(&mut self) -> Option<EnumVariant> {
        let start = self.peek_span();
        let name = self.expect_ident()?;
        let fields = if self.at(&TokenKind::LParen) {
            self.expect(TokenKind::LParen)?;
            let mut fs = Vec::new();
            while !self.at(&TokenKind::RParen) && !self.at_eof() {
                if !fs.is_empty() {
                    self.expect(TokenKind::Comma)?;
                    if self.at(&TokenKind::RParen) {
                        break;
                    }
                }
                let field_start = self.peek_span();
                let field_name = self.expect_ident()?;
                self.expect(TokenKind::Colon)?;
                let ty = self.parse_type()?;
                let span = field_start.merge(type_span(&ty));
                fs.push(EnumField {
                    name: field_name,
                    ty,
                    span,
                });
            }
            self.expect(TokenKind::RParen)?;
            EnumVariantFields::Named(fs)
        } else {
            EnumVariantFields::Unit
        };
        let end = self.prev_span();
        Some(EnumVariant {
            name,
            fields,
            span: start.merge(end),
        })
    }

    fn parse_trait(&mut self, visibility: Visibility, start: Span) -> Option<TraitDecl> {
        self.expect(TokenKind::Trait)?;
        let name = self.expect_ident()?;
        let generics = self.parse_generic_params()?;
        self.expect(TokenKind::LBrace)?;
        let mut items = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            let item_start = self.peek_span();
            self.expect(TokenKind::Fn)?;
            let fn_name = self.expect_ident()?;
            let fn_generics = self.parse_generic_params()?;
            self.expect(TokenKind::LParen)?;
            let params = self.parse_param_list()?;
            self.expect(TokenKind::RParen)?;
            let return_type = if self.eat(&TokenKind::Arrow).is_some() {
                Some(self.parse_type()?)
            } else {
                None
            };
            let body = if self.at(&TokenKind::LBrace) {
                Some(self.parse_block()?)
            } else {
                self.expect(TokenKind::Semi)?;
                None
            };
            let end = self.prev_span();
            let is_abstract = body.is_none();
            items.push(TraitItem::Fn(FnDecl {
                visibility: Visibility::Pub,
                name: fn_name,
                generics: fn_generics,
                params,
                return_type,
                body,
                is_open: false,
                is_override: false,
                is_abstract,
                span: item_start.merge(end),
            }));
        }
        let end = self.expect(TokenKind::RBrace)?;
        Some(TraitDecl {
            visibility,
            name,
            generics,
            items,
            span: start.merge(end),
        })
    }

    fn parse_impl(&mut self, start: Span) -> Option<ImplBlock> {
        self.expect(TokenKind::Impl)?;
        let impl_generics = self.parse_generic_params()?;
        let trait_type = self.parse_type()?;
        self.expect(TokenKind::For)?;
        let target = self.parse_type()?;
        self.expect(TokenKind::LBrace)?;
        let mut items = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            let item_start = self.peek_span();
            let fn_decl = self.parse_fn_decl(Visibility::Pub, item_start, false, false, false)?;
            items.push(ImplItem::Fn(fn_decl));
        }
        let end = self.expect(TokenKind::RBrace)?;
        Some(ImplBlock {
            generics: impl_generics,
            trait_ref: Some(trait_type),
            target,
            items,
            span: start.merge(end),
        })
    }

    fn parse_package(&mut self) -> Option<PackageDecl> {
        let start = self.expect(TokenKind::Package)?;
        let mut path = vec![self.expect_ident()?];
        while self.eat(&TokenKind::Dot).is_some() {
            path.push(self.expect_ident()?);
        }
        let end = self.expect(TokenKind::Semi)?;
        Some(PackageDecl {
            path,
            span: start.merge(end),
        })
    }

    fn parse_import(&mut self) -> Option<ImportDecl> {
        let start = self.expect(TokenKind::Import)?;
        let mut path = vec![self.expect_ident()?];
        while self.eat(&TokenKind::Dot).is_some() {
            path.push(self.expect_ident()?);
        }
        let alias = if self.eat(&TokenKind::As).is_some() {
            Some(self.expect_ident()?)
        } else {
            None
        };
        let end = self.expect(TokenKind::Semi)?;
        Some(ImportDecl {
            path,
            alias,
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
            let is_block_expr = matches!(
                &expr,
                Expr::If(_)
                    | Expr::Match(_)
                    | Expr::Block(_)
                    | Expr::For(_)
                    | Expr::While(_)
                    | Expr::Loop(_)
            );
            if self.at(&TokenKind::Semi) {
                self.bump();
                stmts.push(Stmt::ExprSemi(expr));
            } else if self.at(&TokenKind::RBrace) {
                tail = Some(Box::new(expr));
                break;
            } else if is_block_expr {
                stmts.push(Stmt::ExprSemi(expr));
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
        let ty = if self.eat(&TokenKind::Colon).is_some() {
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect(TokenKind::Eq)?;
        let init = self.parse_expr()?;
        let end = self.expect(TokenKind::Semi)?;
        Some(LetStmt {
            mutable,
            name,
            ty,
            init,
            span: start.merge(end),
        })
    }

    fn parse_expr(&mut self) -> Option<Expr> {
        let lhs = self.parse_or()?;

        if self.at(&TokenKind::Eq)
            || self.at(&TokenKind::PlusEq)
            || self.at(&TokenKind::MinusEq)
            || self.at(&TokenKind::StarEq)
            || self.at(&TokenKind::SlashEq)
            || self.at(&TokenKind::PercentEq)
        {
            let op_tok = self.bump();
            let rhs = self.parse_expr()?;
            let op = match op_tok.0 {
                TokenKind::Eq => None,
                TokenKind::PlusEq => Some(BinaryOp::Add),
                TokenKind::MinusEq => Some(BinaryOp::Sub),
                TokenKind::StarEq => Some(BinaryOp::Mul),
                TokenKind::SlashEq => Some(BinaryOp::Div),
                TokenKind::PercentEq => Some(BinaryOp::Rem),
                _ => unreachable!(),
            };
            let span = expr_span(&lhs).merge(expr_span(&rhs));
            return Some(Expr::Assign(AssignExpr {
                target: Box::new(lhs),
                op,
                value: Box::new(rhs),
                span,
            }));
        }

        Some(lhs)
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
        let mut lhs = self.parse_bitor()?;
        while self.at(&TokenKind::AmpAmp) {
            self.bump();
            let rhs = self.parse_bitor()?;
            lhs = combine_binary(BinaryOp::And, lhs, rhs);
        }
        Some(lhs)
    }

    fn parse_bitor(&mut self) -> Option<Expr> {
        let mut lhs = self.parse_bitxor()?;
        while self.at(&TokenKind::Pipe) && !self.at(&TokenKind::PipePipe) {
            self.bump();
            let rhs = self.parse_bitxor()?;
            lhs = combine_binary(BinaryOp::BitOr, lhs, rhs);
        }
        Some(lhs)
    }

    fn parse_bitxor(&mut self) -> Option<Expr> {
        let mut lhs = self.parse_bitand()?;
        while self.at(&TokenKind::Caret) {
            self.bump();
            let rhs = self.parse_bitand()?;
            lhs = combine_binary(BinaryOp::BitXor, lhs, rhs);
        }
        Some(lhs)
    }

    fn parse_bitand(&mut self) -> Option<Expr> {
        let mut lhs = self.parse_eq()?;
        while self.at(&TokenKind::Amp) && !self.at(&TokenKind::AmpAmp) {
            self.bump();
            let rhs = self.parse_eq()?;
            lhs = combine_binary(BinaryOp::BitAnd, lhs, rhs);
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
        let mut lhs = self.parse_range()?;
        loop {
            let op = match self.peek() {
                TokenKind::Lt => BinaryOp::Lt,
                TokenKind::Le => BinaryOp::Le,
                TokenKind::Gt => BinaryOp::Gt,
                TokenKind::Ge => BinaryOp::Ge,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_range()?;
            lhs = combine_binary(op, lhs, rhs);
        }
        Some(lhs)
    }

    fn parse_range(&mut self) -> Option<Expr> {
        let lhs = self.parse_shift()?;
        if self.at(&TokenKind::DotDot) || self.at(&TokenKind::DotDotEq) {
            let inclusive = self.at(&TokenKind::DotDotEq);
            self.bump();
            let rhs = if !self.at(&TokenKind::Semi)
                && !self.at(&TokenKind::RBrace)
                && !self.at(&TokenKind::RParen)
                && !self.at(&TokenKind::Comma)
                && !self.at_eof()
            {
                Some(Box::new(self.parse_shift()?))
            } else {
                None
            };
            let start_span = expr_span(&lhs);
            let end_span = rhs.as_ref().map(|e| expr_span(e)).unwrap_or(start_span);
            return Some(Expr::Range(RangeExpr {
                start: Some(Box::new(lhs)),
                end: rhs,
                inclusive,
                span: start_span.merge(end_span),
            }));
        }
        Some(lhs)
    }

    fn parse_shift(&mut self) -> Option<Expr> {
        let mut lhs = self.parse_add()?;
        loop {
            let op = match self.peek() {
                TokenKind::Shl => BinaryOp::Shl,
                TokenKind::Shr => BinaryOp::Shr,
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
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Option<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.at(&TokenKind::LParen) {
                let args = self.parse_call_args()?;
                let span = expr_span(&expr).merge(self.prev_span());
                expr = Expr::Call(CallExpr {
                    callee: Box::new(expr),
                    args,
                    span,
                });
            } else if self.eat(&TokenKind::Dot).is_some() {
                let method_name = self.expect_ident()?;
                if self.at(&TokenKind::LParen) {
                    let args = self.parse_call_args()?;
                    let span = expr_span(&expr).merge(self.prev_span());
                    expr = Expr::MethodCall(Box::new(MethodCallExpr {
                        receiver: Box::new(expr),
                        method: method_name,
                        generics: Vec::new(),
                        args,
                        span,
                    }));
                } else {
                    let span = expr_span(&expr).merge(self.prev_span());
                    expr = Expr::Field(FieldAccess {
                        receiver: Box::new(expr),
                        field: method_name,
                        span,
                    });
                }
            } else if self.eat(&TokenKind::Question).is_some() {
                let span = expr_span(&expr).merge(self.prev_span());
                expr = Expr::Try(TryExpr {
                    expr: Box::new(expr),
                    span,
                });
            } else {
                break;
            }
        }
        Some(expr)
    }

    fn parse_call_args(&mut self) -> Option<Vec<CallArg>> {
        self.expect(TokenKind::LParen)?;
        let mut args = Vec::new();
        while !self.at(&TokenKind::RParen) && !self.at_eof() {
            if !args.is_empty() {
                self.expect(TokenKind::Comma)?;
                if self.at(&TokenKind::RParen) {
                    break;
                }
            }
            let arg_start = self.peek_span();
            let name = if let TokenKind::Ident(n) = self.peek().clone() {
                if self.lookahead(1) == &TokenKind::Eq {
                    self.bump();
                    self.bump();
                    Some(n)
                } else {
                    None
                }
            } else {
                None
            };
            let value = self.parse_expr()?;
            let span = arg_start.merge(expr_span(&value));
            args.push(CallArg { name, value, span });
        }
        self.expect(TokenKind::RParen)?;
        Some(args)
    }

    fn parse_primary(&mut self) -> Option<Expr> {
        let span = self.peek_span();
        match self.peek().clone() {
            TokenKind::IntLit(n) => {
                self.bump();
                Some(Expr::Literal(Literal::Int(n, span)))
            }
            TokenKind::DoubleLit(n) => {
                self.bump();
                Some(Expr::Literal(Literal::Double(n, span)))
            }
            TokenKind::StringLit(s) => {
                self.bump();
                Some(Expr::Literal(Literal::String(s, span)))
            }
            TokenKind::BoolLit(b) => {
                self.bump();
                Some(Expr::Literal(Literal::Bool(b, span)))
            }
            TokenKind::Ident(_) => self.parse_path_expr(),
            TokenKind::SelfKw => {
                self.bump();
                Some(Expr::Path(valen_ast::Path {
                    segments: vec![valen_ast::PathSegment {
                        name: SmolStr::from("self"),
                        double_colon: false,
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
            TokenKind::LBrace => {
                let block = self.parse_block()?;
                Some(Expr::Block(block))
            }
            TokenKind::If => self.parse_if_expr(),
            TokenKind::Match => self.parse_match_expr(),
            TokenKind::For => self.parse_for_expr(),
            TokenKind::While => self.parse_while_expr(),
            TokenKind::Loop => self.parse_loop_expr(),
            TokenKind::Break => self.parse_break_expr(),
            TokenKind::Continue => self.parse_continue_expr(),
            TokenKind::Return => self.parse_return_expr(),
            TokenKind::Pipe | TokenKind::PipePipe => self.parse_lambda_expr(),
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

    fn parse_path_expr(&mut self) -> Option<Expr> {
        let start = self.peek_span();
        let first_name = self.expect_ident()?;
        let mut segments = vec![PathSegment {
            name: first_name,
            double_colon: false,
            generics: Vec::new(),
            span: start,
        }];

        while self.eat(&TokenKind::DoubleColon).is_some() {
            let seg_span = self.peek_span();
            let seg_name = self.expect_ident()?;
            segments.push(PathSegment {
                name: seg_name,
                double_colon: true,
                generics: Vec::new(),
                span: seg_span,
            });
        }

        let end = segments.last().map(|s| s.span).unwrap_or(start);
        Some(Expr::Path(Path {
            segments,
            span: start.merge(end),
        }))
    }

    fn parse_if_expr(&mut self) -> Option<Expr> {
        let start = self.expect(TokenKind::If)?;
        let cond = self.parse_expr()?;
        let then_branch = self.parse_block()?;
        let else_branch = if self.eat(&TokenKind::Else).is_some() {
            if self.at(&TokenKind::If) {
                Some(Box::new(self.parse_if_expr()?))
            } else {
                let block = self.parse_block()?;
                Some(Box::new(Expr::Block(block)))
            }
        } else {
            None
        };
        let end = else_branch
            .as_ref()
            .map(|e| expr_span(e))
            .unwrap_or(then_branch.span);
        Some(Expr::If(Box::new(IfExpr {
            cond: Box::new(cond),
            then_branch,
            else_branch,
            span: start.merge(end),
        })))
    }

    fn parse_match_expr(&mut self) -> Option<Expr> {
        let start = self.expect(TokenKind::Match)?;
        let scrutinee = self.parse_expr()?;
        self.expect(TokenKind::LBrace)?;
        let mut arms = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            if !arms.is_empty() {
                self.expect(TokenKind::Comma)?;
                if self.at(&TokenKind::RBrace) {
                    break;
                }
            }
            arms.push(self.parse_match_arm()?);
        }
        let end = self.expect(TokenKind::RBrace)?;
        Some(Expr::Match(MatchExpr {
            scrutinee: Box::new(scrutinee),
            arms,
            span: start.merge(end),
        }))
    }

    fn parse_match_arm(&mut self) -> Option<MatchArm> {
        let start = self.peek_span();
        let pattern = self.parse_pattern()?;
        let guard = if self.eat(&TokenKind::If).is_some() {
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect(TokenKind::FatArrow)?;
        let body = self.parse_expr()?;
        let span = start.merge(expr_span(&body));
        Some(MatchArm {
            pattern,
            guard,
            body,
            span,
        })
    }

    fn parse_pattern(&mut self) -> Option<Pattern> {
        let mut pat = self.parse_pattern_atom()?;
        if self.at(&TokenKind::Pipe) {
            let start = pattern_span(&pat);
            let mut alternatives = vec![pat];
            while self.eat(&TokenKind::Pipe).is_some() {
                alternatives.push(self.parse_pattern_atom()?);
            }
            let end = alternatives.last().map(pattern_span).unwrap_or(start);
            pat = Pattern::Or(alternatives, start.merge(end));
        }
        Some(pat)
    }

    fn parse_pattern_atom(&mut self) -> Option<Pattern> {
        let span = self.peek_span();
        match self.peek().clone() {
            TokenKind::Underscore => {
                self.bump();
                Some(Pattern::Wildcard(span))
            }
            TokenKind::IntLit(n) => {
                self.bump();
                let lit = Literal::Int(n, span);
                if self.at(&TokenKind::DotDot) || self.at(&TokenKind::DotDotEq) {
                    return self.parse_range_pattern(Some(lit));
                }
                Some(Pattern::Literal(lit))
            }
            TokenKind::StringLit(s) => {
                self.bump();
                Some(Pattern::Literal(Literal::String(s, span)))
            }
            TokenKind::BoolLit(b) => {
                self.bump();
                Some(Pattern::Literal(Literal::Bool(b, span)))
            }
            TokenKind::Mut => {
                self.bump();
                let name = self.expect_ident()?;
                let bind_span = span.merge(self.prev_span());
                Some(Pattern::Binding(BindingPattern {
                    name,
                    mutable: true,
                    span: bind_span,
                }))
            }
            TokenKind::Ident(_) => self.parse_ident_pattern(),
            _ => {
                self.diagnostics.error(
                    DiagCode::PARSE_EXPECTED_EXPR,
                    span,
                    SmolStr::from("expected pattern"),
                );
                None
            }
        }
    }

    fn parse_ident_pattern(&mut self) -> Option<Pattern> {
        let start = self.peek_span();
        let name = self.expect_ident()?;

        if self.eat(&TokenKind::DoubleColon).is_some() {
            let mut segments = vec![PathSegment {
                name,
                double_colon: false,
                generics: Vec::new(),
                span: start,
            }];
            loop {
                let seg_span = self.peek_span();
                let seg_name = self.expect_ident()?;
                segments.push(PathSegment {
                    name: seg_name,
                    double_colon: true,
                    generics: Vec::new(),
                    span: seg_span,
                });
                if self.eat(&TokenKind::DoubleColon).is_none() {
                    break;
                }
            }
            let end = segments.last().map(|s| s.span).unwrap_or(start);
            let path = Path {
                segments,
                span: start.merge(end),
            };
            if self.at(&TokenKind::LParen) {
                return self.parse_struct_pattern(path);
            }
            return Some(Pattern::Path(path));
        }

        if self.at(&TokenKind::LParen) {
            let path = Path {
                segments: vec![PathSegment {
                    name: name.clone(),
                    double_colon: false,
                    generics: Vec::new(),
                    span: start,
                }],
                span: start,
            };
            return self.parse_struct_pattern(path);
        }

        if self.eat(&TokenKind::At).is_some() {
            let inner = self.parse_pattern_atom()?;
            let span = start.merge(pattern_span(&inner));
            return Some(Pattern::At(AtPattern {
                name,
                pattern: Box::new(inner),
                span,
            }));
        }

        Some(Pattern::Binding(BindingPattern {
            name,
            mutable: false,
            span: start,
        }))
    }

    fn parse_struct_pattern(&mut self, path: Path) -> Option<Pattern> {
        let start = path.span;
        self.expect(TokenKind::LParen)?;
        let mut fields = Vec::new();
        let mut rest = false;
        while !self.at(&TokenKind::RParen) && !self.at_eof() {
            if !fields.is_empty() {
                self.expect(TokenKind::Comma)?;
                if self.at(&TokenKind::RParen) {
                    break;
                }
            }
            if self.eat(&TokenKind::DotDot).is_some() {
                rest = true;
                break;
            }
            let field_span = self.peek_span();
            let field_name = self.expect_ident()?;
            let pattern = if self.eat(&TokenKind::Colon).is_some() {
                Some(self.parse_pattern()?)
            } else {
                None
            };
            fields.push(StructPatternField {
                name: field_name,
                pattern,
                span: field_span,
            });
        }
        let end = self.expect(TokenKind::RParen)?;
        Some(Pattern::Struct(StructPattern {
            path,
            fields,
            rest,
            span: start.merge(end),
        }))
    }

    fn parse_range_pattern(&mut self, start_lit: Option<Literal>) -> Option<Pattern> {
        let inclusive = if self.eat(&TokenKind::DotDotEq).is_some() {
            true
        } else {
            self.expect(TokenKind::DotDot)?;
            false
        };
        let end_lit = if let TokenKind::IntLit(n) = self.peek().clone() {
            let span = self.peek_span();
            self.bump();
            Some(Literal::Int(n, span))
        } else {
            None
        };
        let span_start = start_lit
            .as_ref()
            .map(literal_span)
            .unwrap_or(self.peek_span());
        let span_end = end_lit
            .as_ref()
            .map(literal_span)
            .unwrap_or(self.prev_span());
        Some(Pattern::Range(RangePattern {
            start: start_lit,
            end: end_lit,
            inclusive,
            span: span_start.merge(span_end),
        }))
    }

    fn parse_return_expr(&mut self) -> Option<Expr> {
        let start = self.expect(TokenKind::Return)?;
        let value = if !self.at(&TokenKind::Semi) && !self.at(&TokenKind::RBrace) && !self.at_eof()
        {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        let end = value.as_ref().map(|v| expr_span(v)).unwrap_or(start);
        Some(Expr::Return(ReturnExpr {
            value,
            span: start.merge(end),
        }))
    }

    fn parse_for_expr(&mut self) -> Option<Expr> {
        let start = self.expect(TokenKind::For)?;
        let var = self.expect_ident()?;
        self.expect(TokenKind::In)?;
        let iter = self.parse_expr()?;
        let body = self.parse_block()?;
        let span = start.merge(body.span);
        Some(Expr::For(Box::new(ForExpr {
            var,
            iter: Box::new(iter),
            body,
            span,
        })))
    }

    fn parse_while_expr(&mut self) -> Option<Expr> {
        let start = self.expect(TokenKind::While)?;
        let cond = self.parse_expr()?;
        let body = self.parse_block()?;
        let span = start.merge(body.span);
        Some(Expr::While(Box::new(WhileExpr {
            cond: Box::new(cond),
            body,
            span,
        })))
    }

    fn parse_loop_expr(&mut self) -> Option<Expr> {
        let start = self.expect(TokenKind::Loop)?;
        let body = self.parse_block()?;
        let span = start.merge(body.span);
        Some(Expr::Loop(LoopExpr { body, span }))
    }

    fn parse_break_expr(&mut self) -> Option<Expr> {
        let start = self.expect(TokenKind::Break)?;
        let value = if !self.at(&TokenKind::Semi) && !self.at(&TokenKind::RBrace) && !self.at_eof()
        {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        let end = value.as_ref().map(|v| expr_span(v)).unwrap_or(start);
        Some(Expr::Break(BreakExpr {
            value,
            span: start.merge(end),
        }))
    }

    fn parse_continue_expr(&mut self) -> Option<Expr> {
        let start = self.expect(TokenKind::Continue)?;
        Some(Expr::Continue(ContinueExpr { span: start }))
    }

    fn parse_lambda_expr(&mut self) -> Option<Expr> {
        let start = self.peek_span();
        let params = if self.eat(&TokenKind::PipePipe).is_some() {
            Vec::new()
        } else {
            self.expect(TokenKind::Pipe)?;
            let mut ps = Vec::new();
            while !self.at(&TokenKind::Pipe) && !self.at_eof() {
                if !ps.is_empty() {
                    self.expect(TokenKind::Comma)?;
                }
                let p_start = self.peek_span();
                let name = self.expect_ident()?;
                let ty = if self.eat(&TokenKind::Colon).is_some() {
                    Some(self.parse_type()?)
                } else {
                    None
                };
                let p_span = ty
                    .as_ref()
                    .map(|t| p_start.merge(type_span(t)))
                    .unwrap_or(p_start);
                ps.push(LambdaParam {
                    name,
                    ty,
                    span: p_span,
                });
            }
            self.expect(TokenKind::Pipe)?;
            ps
        };
        let return_type = if self.eat(&TokenKind::Arrow).is_some() {
            Some(self.parse_type()?)
        } else {
            None
        };
        let body = self.parse_expr()?;
        let span = start.merge(expr_span(&body));
        Some(Expr::Lambda(Box::new(LambdaExpr {
            params,
            return_type,
            body: Box::new(body),
            span,
        })))
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

    fn prev_span(&self) -> Span {
        self.tokens
            .get(self.pos.saturating_sub(1))
            .map(|(_, s)| *s)
            .unwrap_or(Span::new(0, 0, self.file_id))
    }

    fn lookahead(&self, offset: usize) -> &TokenKind {
        self.tokens
            .get(self.pos + offset)
            .map(|(k, _)| k)
            .unwrap_or(&TokenKind::Eof)
    }

    fn bump(&mut self) -> (TokenKind, Span) {
        let tok = self
            .tokens
            .get(self.pos)
            .cloned()
            .unwrap_or((TokenKind::Eof, Span::DUMMY));
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
            DiagCode::PARSE_EXPECTED_TOKEN,
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
            DiagCode::PARSE_EXPECTED_IDENT,
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
                    | TokenKind::Data
                    | TokenKind::Enum
                    | TokenKind::Trait
                    | TokenKind::Impl
                    | TokenKind::Package
                    | TokenKind::Import
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
        Expr::Literal(Literal::Long(_, s)) => *s,
        Expr::Literal(Literal::Float(_, s)) => *s,
        Expr::Literal(Literal::Double(_, s)) => *s,
        Expr::Literal(Literal::Char(_, s)) => *s,
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
        Expr::Break(b) => b.span,
        Expr::Continue(c) => c.span,
        Expr::For(f) => f.span,
        Expr::While(w) => w.span,
        Expr::Loop(l) => l.span,
        Expr::Lambda(l) => l.span,
        Expr::Range(r) => r.span,
        Expr::Try(t) => t.span,
        Expr::StringInterp(s) => s.span,
        Expr::Safe(s) => s.span,
    }
}

fn type_span(ty: &Type) -> Span {
    match ty {
        Type::Path(p) => p.span,
        Type::Nullable { span, .. } => *span,
        Type::Fn(f) => f.span,
        Type::Tuple(ts) => {
            if let (Some(first), Some(last)) = (ts.first(), ts.last()) {
                type_span(first).merge(type_span(last))
            } else {
                Span::DUMMY
            }
        }
    }
}

fn pattern_span(pat: &Pattern) -> Span {
    match pat {
        Pattern::Wildcard(s) => *s,
        Pattern::Literal(l) => literal_span(l),
        Pattern::Binding(b) => b.span,
        Pattern::Path(p) => p.span,
        Pattern::Struct(s) => s.span,
        Pattern::Tuple(_, s) => *s,
        Pattern::Range(r) => r.span,
        Pattern::Or(_, s) => *s,
        Pattern::At(a) => a.span,
    }
}

fn literal_span(lit: &Literal) -> Span {
    match lit {
        Literal::Int(_, s) => *s,
        Literal::Long(_, s) => *s,
        Literal::Float(_, s) => *s,
        Literal::Double(_, s) => *s,
        Literal::Char(_, s) => *s,
        Literal::String(_, s) => *s,
        Literal::Bool(_, s) => *s,
        Literal::Unit(s) => *s,
    }
}

fn describe_token(kind: &TokenKind) -> &'static str {
    match kind {
        TokenKind::Fn => "`fn`",
        TokenKind::Let => "`let`",
        TokenKind::Mut => "`mut`",
        TokenKind::Class => "`class`",
        TokenKind::If => "`if`",
        TokenKind::Match => "`match`",
        TokenKind::Return => "`return`",
        TokenKind::LParen => "`(`",
        TokenKind::RParen => "`)`",
        TokenKind::LBrace => "`{`",
        TokenKind::RBrace => "`}`",
        TokenKind::Semi => "`;`",
        TokenKind::Eq => "`=`",
        TokenKind::Arrow => "`->`",
        TokenKind::FatArrow => "`=>`",
        TokenKind::Colon => "`:`",
        TokenKind::DoubleColon => "`::`",
        TokenKind::Comma => "`,`",
        TokenKind::Dot => "`.`",
        TokenKind::DotDot => "`..`",
        TokenKind::DotDotEq => "`..=`",
        TokenKind::Lt => "`<`",
        TokenKind::Gt => "`>`",
        TokenKind::Question => "`?`",
        _ => "token",
    }
}
