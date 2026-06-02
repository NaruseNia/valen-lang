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
    Annotation, AnnotationArg, AnnotationClassDecl, AnnotationParam, AssignExpr, AssocTypeDecl,
    AssocTypeDef, AtPattern, BinaryExpr, BinaryOp, BindingPattern, Block, BreakExpr, CallArg,
    CallExpr, CastExpr, ClassDecl, ClassKind, ClassMember, ContinueExpr, CtorParam, DataClassDecl,
    DerefExpr, EnumDecl, EnumField, EnumVariant, EnumVariantFields, Expr, FieldAccess, FileId,
    FnDecl, ForExpr, GenericParam, IfExpr, IfLetExpr, ImplBlock, ImplItem, ImportDecl, Item,
    LambdaExpr, LambdaParam, LetElseStmt, LetStmt, Literal, LoopExpr, MatchArm, MatchExpr,
    MethodCallExpr, PackageDecl, Param, Path, PathSegment, Pattern, RangeExpr, RangePattern,
    RefMutExpr, ReturnExpr, Span, Stmt, StringInterpExpr, StructPattern, StructPatternField,
    TraitDecl, TraitItem, TryExpr, Type, TypeAliasDecl, TypePath, TypePathSegment, UnaryExpr,
    UnaryOp, UnsafeExpr, Variance, VariantShorthandExpr, VariantShorthandPattern, Visibility,
    WhileExpr, WhileLetExpr,
};
use valen_diagnostics::{DiagCode, Diagnostics};

use crate::lexer::lex;

/// Recursive-descent parser that converts a token stream into AST items.
pub struct Parser {
    tokens: Vec<(TokenKind, Span)>,
    pos: usize,
    file_id: FileId,
    diagnostics: Diagnostics,
    /// Tracks split `>>` tokens: when `>>` (Shr) is consumed inside a generic
    /// context, one `>` is used and `pending_gt` is incremented so the next
    /// `expect_gt` / `at_gt` sees the remaining `>`.
    pending_gt: u32,
}

impl Parser {
    /// Create a parser by lexing `source`, merging any lexer diagnostics.
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
            pending_gt: 0,
        }
    }

    /// Parse the entire token stream as a file, returning all top-level items.
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

    /// Consume the parser and return accumulated diagnostics.
    pub fn into_diagnostics(self) -> Diagnostics {
        self.diagnostics
    }

    fn parse_item(&mut self) -> Option<Item> {
        self.pending_gt = 0;
        if self.at(&TokenKind::Package) {
            return self.parse_package().map(Item::Package);
        }
        if self.at(&TokenKind::Import) {
            return self.parse_import().map(Item::Import);
        }

        let start = self.peek_span();
        let annotations = self.parse_annotations();
        let vis = self.parse_visibility();
        match self.peek() {
            TokenKind::Annotation => self
                .parse_annotation_class(annotations, vis, start)
                .map(Item::AnnotationClass),
            TokenKind::Inline => {
                self.bump();
                let mut f = self.parse_fn_decl(annotations, vis, start, false, false, false)?;
                f.is_inline = true;
                Some(Item::Fn(f))
            }
            TokenKind::Unsafe => {
                self.bump();
                if self.at(&TokenKind::Inline) {
                    self.bump();
                    let mut f = self.parse_fn_decl(annotations, vis, start, false, false, false)?;
                    f.is_unsafe = true;
                    f.is_inline = true;
                    Some(Item::Fn(f))
                } else {
                    let mut f = self.parse_fn_decl(annotations, vis, start, false, false, false)?;
                    f.is_unsafe = true;
                    Some(Item::Fn(f))
                }
            }
            TokenKind::Fn => self
                .parse_fn_decl(annotations, vis, start, false, false, false)
                .map(Item::Fn),
            TokenKind::Class => self
                .parse_class(annotations, vis, ClassKind::Final, start)
                .map(Item::Class),
            TokenKind::Open | TokenKind::Abstract => {
                let kind = self.parse_class_kind();
                self.parse_class(annotations, vis, kind, start)
                    .map(Item::Class)
            }
            TokenKind::Sealed => {
                self.bump();
                if self.at(&TokenKind::Trait) {
                    self.parse_trait(annotations, vis, true, start)
                        .map(Item::Trait)
                } else {
                    self.parse_class(annotations, vis, ClassKind::Sealed, start)
                        .map(Item::Class)
                }
            }
            TokenKind::Data => self
                .parse_data_class(annotations, vis, start)
                .map(Item::DataClass),
            TokenKind::Enum => self.parse_enum(annotations, vis, start).map(Item::Enum),
            TokenKind::Trait => self
                .parse_trait(annotations, vis, false, start)
                .map(Item::Trait),
            TokenKind::Impl => self.parse_impl(start).map(Item::Impl),
            TokenKind::TypeAlias => self.parse_type_alias(vis, start).map(Item::TypeAlias),
            TokenKind::NewType => self.parse_newtype(vis, start).map(Item::NewType),
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

    fn parse_annotations(&mut self) -> Vec<Annotation> {
        let mut annotations = Vec::new();
        while self.at(&TokenKind::At) {
            if let Some(ann) = self.parse_annotation() {
                annotations.push(ann);
            }
        }
        annotations
    }

    fn parse_annotation(&mut self) -> Option<Annotation> {
        let start = self.expect(TokenKind::At)?;
        let name = self.expect_ident()?;
        let args = if self.at(&TokenKind::LParen) {
            self.bump();
            let mut args = Vec::new();
            while !self.at(&TokenKind::RParen) && !self.at_eof() {
                if !args.is_empty() {
                    self.expect(TokenKind::Comma)?;
                    if self.at(&TokenKind::RParen) {
                        break;
                    }
                }
                let arg_start = self.peek_span();
                if self.peek_is_ident() && self.peek_ahead_is(&TokenKind::Eq) {
                    let arg_name = self.expect_ident()?;
                    self.expect(TokenKind::Eq)?;
                    let value = self.parse_literal()?;
                    let end = literal_span(&value);
                    args.push(AnnotationArg {
                        name: Some(arg_name),
                        value,
                        span: arg_start.merge(end),
                    });
                } else {
                    let value = self.parse_literal()?;
                    let end = literal_span(&value);
                    args.push(AnnotationArg {
                        name: None,
                        value,
                        span: arg_start.merge(end),
                    });
                }
            }
            self.expect(TokenKind::RParen)?;
            args
        } else {
            Vec::new()
        };
        let end = self.prev_span();
        Some(Annotation {
            name,
            args,
            span: start.merge(end),
        })
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

    fn parse_annotation_class(
        &mut self,
        annotations: Vec<Annotation>,
        visibility: Visibility,
        start: Span,
    ) -> Option<AnnotationClassDecl> {
        self.expect(TokenKind::Annotation)?;
        self.expect(TokenKind::Class)?;
        let name = self.expect_ident()?;
        let params = if self.at(&TokenKind::LParen) {
            self.parse_annotation_class_params()?
        } else {
            Vec::new()
        };
        let end = self.prev_span();
        Some(AnnotationClassDecl {
            visibility,
            name,
            annotations,
            params,
            span: start.merge(end),
        })
    }

    fn parse_annotation_class_params(&mut self) -> Option<Vec<AnnotationParam>> {
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
            let name = self.expect_ident()?;
            self.expect(TokenKind::Colon)?;
            let ty = self.parse_type()?;
            let span = param_start.merge(ty.span());
            params.push(AnnotationParam {
                visibility: vis,
                name,
                ty,
                span,
            });
        }
        self.expect(TokenKind::RParen)?;
        Some(params)
    }

    fn parse_fn_decl(
        &mut self,
        annotations: Vec<Annotation>,
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

        let (body, end_span) = if is_abstract && !self.at(&TokenKind::LBrace) {
            let semi = self.expect(TokenKind::Semi)?;
            (None, semi)
        } else {
            let b = self.parse_block()?;
            let s = b.span;
            (Some(b), s)
        };
        let span = start.merge(end_span);
        Some(FnDecl {
            annotations,
            visibility,
            name,
            generics,
            params,
            return_type,
            body,
            is_open,
            is_override,
            is_abstract,
            is_unsafe: false,
            is_inline: false,
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
                || (self.at(&TokenKind::Mut) && self.lookahead(1) == &TokenKind::SelfKw)
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
                    default: None,
                    span,
                });
                continue;
            }

            let mutable = self.eat(&TokenKind::Mut).is_some();
            let name = self.expect_ident()?;
            self.expect(TokenKind::Colon)?;
            let ty = self.parse_type()?;
            let default = if self.eat(&TokenKind::Eq).is_some() {
                Some(self.parse_expr()?)
            } else {
                None
            };
            let span = param_start.merge(self.prev_span());
            params.push(Param {
                name,
                ty,
                mutable,
                default,
                span,
            });
        }
        Some(params)
    }

    fn parse_type(&mut self) -> Option<Type> {
        let start = self.peek_span();

        // `ref mut T` — mutable reference type
        if self.at(&TokenKind::Ref) && self.lookahead(1) == &TokenKind::Mut {
            self.bump(); // ref
            self.bump(); // mut
            let inner = self.parse_type()?;
            let span = start.merge(inner.span());
            return Some(Type::RefMut {
                inner: Box::new(inner),
                span,
            });
        }

        // Function type: `fn(A, B) -> C`
        if self.at(&TokenKind::Fn) {
            self.bump();
            self.expect(TokenKind::LParen)?;
            let mut params = Vec::new();
            while !self.at(&TokenKind::RParen) && !self.at_eof() {
                if !params.is_empty() {
                    self.expect(TokenKind::Comma)?;
                }
                params.push(self.parse_type()?);
            }
            self.expect(TokenKind::RParen)?;
            self.expect(TokenKind::Arrow)?;
            let return_type = self.parse_type()?;
            let end = return_type.span();
            return Some(Type::Fn(valen_ast::FnType {
                params,
                return_type: Box::new(return_type),
                span: start.merge(end),
            }));
        }

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
            let inner_span = ty.span();
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
            while !self.at_gt() && !self.at_eof() {
                if !generics.is_empty() {
                    self.expect(TokenKind::Comma)?;
                }
                generics.push(self.parse_type()?);
            }
            self.expect_gt()?;
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
        annotations: Vec<Annotation>,
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
        let derives = self.parse_derives();

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
            annotations,
            visibility,
            kind,
            name,
            generics,
            ctor_params,
            supertypes,
            derives,
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
            let param_annotations = self.parse_annotations();
            let vis = self.parse_visibility();
            let mutable = self.eat(&TokenKind::Mut).is_some();
            let name = self.expect_ident()?;
            self.expect(TokenKind::Colon)?;
            let ty = self.parse_type()?;
            let default = if self.eat(&TokenKind::Eq).is_some() {
                Some(self.parse_expr()?)
            } else {
                None
            };
            let span = param_start.merge(self.prev_span());
            params.push(CtorParam {
                annotations: param_annotations,
                visibility: vis,
                name,
                ty,
                mutable,
                default,
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

    /// Parse an optional `derives(Trait1, Trait2, ...)` clause.
    fn parse_derives(&mut self) -> Vec<SmolStr> {
        if !matches!(self.peek(), TokenKind::Ident(ref s) if s == "derives") {
            return Vec::new();
        }
        self.bump();
        if self.eat(&TokenKind::LParen).is_none() {
            return Vec::new();
        }
        let mut traits = Vec::new();
        while !self.at(&TokenKind::RParen) && !self.at_eof() {
            if !traits.is_empty() {
                if self.expect(TokenKind::Comma).is_none() {
                    break;
                }
                if self.at(&TokenKind::RParen) {
                    break;
                }
            }
            if let Some(name) = self.expect_ident() {
                traits.push(name);
            } else {
                break;
            }
        }
        let _ = self.expect(TokenKind::RParen);
        traits
    }

    fn parse_generic_params(&mut self) -> Option<Vec<GenericParam>> {
        if self.eat(&TokenKind::Lt).is_none() {
            return Some(Vec::new());
        }
        let mut params = Vec::new();
        while !self.at_gt() && !self.at_eof() {
            if !params.is_empty() {
                self.expect(TokenKind::Comma)?;
                if self.at_gt() {
                    break;
                }
            }
            let start = self.peek_span();
            let is_reified = self.eat(&TokenKind::Reified).is_some();
            // Check for variance annotation: `in T` (contravariant) or `out T` (covariant)
            let variance = if self.at(&TokenKind::In) {
                self.bump();
                Variance::Contravariant
            } else if self.peek_is_ident_matching("out") {
                self.bump();
                Variance::Covariant
            } else {
                Variance::Invariant
            };
            let name = self.expect_ident()?;
            let mut bounds = Vec::new();
            if self.eat(&TokenKind::Colon).is_some() {
                bounds.push(self.parse_type()?);
                while self.eat(&TokenKind::Plus).is_some() {
                    bounds.push(self.parse_type()?);
                }
            }
            let end = bounds.last().map(|t| t.span()).unwrap_or(start);
            params.push(GenericParam {
                name,
                variance,
                is_reified,
                bounds,
                span: start.merge(end),
            });
        }
        self.expect_gt()?;
        Some(params)
    }

    fn parse_class_body(&mut self) -> Option<Vec<ClassMember>> {
        let mut members = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            let member_start = self.peek_span();
            let member_annotations = self.parse_annotations();
            let vis = self.parse_visibility();
            match self.peek() {
                TokenKind::Fn
                | TokenKind::Open
                | TokenKind::Override
                | TokenKind::Abstract
                | TokenKind::Unsafe
                | TokenKind::Inline => {
                    let mut is_unsafe = false;
                    let mut is_open = false;
                    let mut is_override = false;
                    let mut is_abstract = false;
                    let mut is_inline = false;
                    loop {
                        match self.peek() {
                            TokenKind::Unsafe => {
                                is_unsafe = true;
                                self.bump();
                            }
                            TokenKind::Open => {
                                is_open = true;
                                self.bump();
                            }
                            TokenKind::Override => {
                                is_override = true;
                                self.bump();
                            }
                            TokenKind::Abstract => {
                                is_abstract = true;
                                self.bump();
                            }
                            TokenKind::Inline => {
                                is_inline = true;
                                self.bump();
                            }
                            _ => break,
                        }
                    }
                    let mut method = self.parse_fn_decl(
                        member_annotations,
                        vis,
                        member_start,
                        is_open,
                        is_override,
                        is_abstract,
                    )?;
                    method.is_unsafe = is_unsafe;
                    method.is_inline = is_inline;
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

    fn parse_data_class(
        &mut self,
        annotations: Vec<Annotation>,
        visibility: Visibility,
        start: Span,
    ) -> Option<DataClassDecl> {
        self.expect(TokenKind::Data)?;
        self.expect(TokenKind::Class)?;
        let name = self.expect_ident()?;
        let generics = self.parse_generic_params()?;
        let ctor_params = self.parse_ctor_params()?;
        let supertypes = self.parse_supertypes()?;
        let derives = self.parse_derives();
        let end = self.expect(TokenKind::Semi)?;
        Some(DataClassDecl {
            annotations,
            visibility,
            name,
            generics,
            ctor_params,
            supertypes,
            derives,
            span: start.merge(end),
        })
    }

    fn parse_enum(
        &mut self,
        annotations: Vec<Annotation>,
        visibility: Visibility,
        start: Span,
    ) -> Option<EnumDecl> {
        self.expect(TokenKind::Enum)?;
        let name = self.expect_ident()?;
        let generics = self.parse_generic_params()?;
        let derives = self.parse_derives();
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
            annotations,
            visibility,
            name,
            generics,
            derives,
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
                let span = field_start.merge(ty.span());
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

    fn parse_trait(
        &mut self,
        annotations: Vec<Annotation>,
        visibility: Visibility,
        is_sealed: bool,
        start: Span,
    ) -> Option<TraitDecl> {
        self.expect(TokenKind::Trait)?;
        let name = self.expect_ident()?;
        let generics = self.parse_generic_params()?;
        self.expect(TokenKind::LBrace)?;
        let mut items = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            let item_start = self.peek_span();
            if self.at(&TokenKind::Type) {
                self.bump();
                let type_name = self.expect_ident()?;
                let default = if self.eat(&TokenKind::Eq).is_some() {
                    Some(self.parse_type()?)
                } else {
                    None
                };
                let end = self.expect(TokenKind::Semi)?;
                items.push(TraitItem::AssociatedType(AssocTypeDecl {
                    name: type_name,
                    default,
                    span: item_start.merge(end),
                }));
                continue;
            }
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
                annotations: vec![],
                visibility: Visibility::Pub,
                name: fn_name,
                generics: fn_generics,
                params,
                return_type,
                body,
                is_open: false,
                is_override: false,
                is_abstract,
                is_unsafe: false,
                is_inline: false,
                span: item_start.merge(end),
            }));
        }
        let end = self.expect(TokenKind::RBrace)?;
        Some(TraitDecl {
            annotations,
            visibility,
            is_sealed,
            name,
            generics,
            items,
            span: start.merge(end),
        })
    }

    fn parse_type_alias(&mut self, visibility: Visibility, start: Span) -> Option<TypeAliasDecl> {
        self.expect(TokenKind::TypeAlias)?;
        let name = self.expect_ident()?;
        let generics = self.parse_generic_params()?;
        self.expect(TokenKind::Eq)?;
        let ty = self.parse_type()?;
        let end = self.expect(TokenKind::Semi)?;
        Some(TypeAliasDecl {
            visibility,
            name,
            generics,
            ty,
            span: start.merge(end),
        })
    }

    fn parse_newtype(
        &mut self,
        visibility: Visibility,
        start: Span,
    ) -> Option<valen_ast::NewTypeDecl> {
        self.expect(TokenKind::NewType)?;
        let name = self.expect_ident()?;
        self.expect(TokenKind::Eq)?;
        let inner_ty = self.parse_type()?;
        let end = self.expect(TokenKind::Semi)?;
        Some(valen_ast::NewTypeDecl {
            visibility,
            name,
            inner_ty,
            span: start.merge(end),
        })
    }

    fn parse_impl(&mut self, start: Span) -> Option<ImplBlock> {
        self.expect(TokenKind::Impl)?;
        let impl_generics = self.parse_generic_params()?;
        let first_type = self.parse_type()?;

        // Distinguish `impl Trait for Type { ... }` from `impl Type { ... }`
        let (trait_ref, target) = if self.eat(&TokenKind::For).is_some() {
            let target = self.parse_type()?;
            (Some(first_type), target)
        } else {
            (None, first_type)
        };

        self.expect(TokenKind::LBrace)?;
        let mut items = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            let item_start = self.peek_span();
            let item_annotations = self.parse_annotations();
            let vis = self.parse_visibility();
            if self.at(&TokenKind::Type) {
                self.bump();
                let type_name = self.expect_ident()?;
                self.expect(TokenKind::Eq)?;
                let ty = self.parse_type()?;
                let end = self.expect(TokenKind::Semi)?;
                items.push(ImplItem::AssociatedType(AssocTypeDef {
                    name: type_name,
                    ty,
                    span: item_start.merge(end),
                }));
                continue;
            }
            let is_inline = self.eat(&TokenKind::Inline).is_some();
            let mut fn_decl =
                self.parse_fn_decl(item_annotations, vis, item_start, false, false, false)?;
            fn_decl.is_inline = is_inline;
            items.push(ImplItem::Fn(fn_decl));
        }
        let end = self.expect(TokenKind::RBrace)?;
        Some(ImplBlock {
            generics: impl_generics,
            trait_ref,
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
                let stmt = self.parse_let_or_let_else()?;
                stmts.push(stmt);
                continue;
            }

            let expr = self.parse_expr()?;
            let is_block_expr = matches!(
                &expr,
                Expr::If(_)
                    | Expr::IfLet(_)
                    | Expr::WhileLet(_)
                    | Expr::Match(_)
                    | Expr::Block(_)
                    | Expr::For(_)
                    | Expr::While(_)
                    | Expr::Loop(_)
                    | Expr::Safe(_)
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

    /// Parse a `let` statement, producing either `Stmt::Let` or `Stmt::LetElse`.
    ///
    /// The let-else form is: `let Pattern = expr else { diverge };`
    /// We detect it by checking whether the `else` keyword follows the
    /// initializer expression.
    fn parse_let_or_let_else(&mut self) -> Option<Stmt> {
        let start = self.peek_span();
        let saved_pos = self.pos;
        let saved_diag_len = self.diagnostics.len();

        // Try to parse as let-else first: `let pattern = expr else { block };`
        // If `else` is present after the expression, commit to let-else.
        // Otherwise, restore position and diagnostics, then fall back to regular let.
        if let Some(stmt) = self.try_parse_let_else(start) {
            return Some(stmt);
        }
        self.pos = saved_pos;
        self.diagnostics.truncate(saved_diag_len);

        let ls = self.parse_let()?;
        Some(Stmt::Let(ls))
    }

    /// Try to parse a let-else statement: `let pattern = expr else { block };`
    fn try_parse_let_else(&mut self, _start_hint: Span) -> Option<Stmt> {
        let start = self.expect(TokenKind::Let)?;
        let pattern = self.parse_pattern()?;

        // Extract the binding name from the pattern for the LetElseStmt.
        // For struct patterns like `Some(x)`, the name is the first segment
        // of the path (used as a descriptive label, not the bound variable).
        let name = extract_pattern_name(&pattern);

        let ty: Option<Type> = if self.eat(&TokenKind::Colon).is_some() {
            Some(self.parse_type()?)
        } else {
            None
        };

        self.expect(TokenKind::Eq)?;
        let expr = self.parse_expr()?;

        // Must see `else` keyword — otherwise this is not a let-else
        if !self.at(&TokenKind::Else) {
            return None;
        }
        self.bump(); // consume `else`

        let else_block = self.parse_block()?;
        let end = self.expect(TokenKind::Semi)?;

        Some(Stmt::LetElse(LetElseStmt {
            name,
            ty,
            pattern,
            expr,
            else_block,
            span: start.merge(end),
        }))
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
        let lhs = self.parse_pipeline()?;

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

    fn parse_pipeline(&mut self) -> Option<Expr> {
        let mut lhs = self.parse_or()?;
        while self.at(&TokenKind::PipeGt) {
            self.bump();
            let rhs = self.parse_or()?;
            let span = expr_span(&lhs).merge(expr_span(&rhs));
            lhs = Expr::Pipeline(Box::new(valen_ast::PipelineExpr { lhs, rhs, span }));
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
                TokenKind::EqEqEq => BinaryOp::RefEq,
                TokenKind::NotEqEq => BinaryOp::RefNe,
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
        // `*expr` — dereference
        if self.at(&TokenKind::Star) {
            let start = self.peek_span();
            self.bump();
            let inner = self.parse_unary()?;
            let span = start.merge(expr_span(&inner));
            return Some(Expr::Deref(DerefExpr {
                expr: Box::new(inner),
                span,
            }));
        }
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
                expr = Expr::Call(Box::new(CallExpr {
                    callee: Box::new(expr),
                    generics: Vec::new(),
                    args,
                    span,
                }));
            } else if self.at(&TokenKind::Lt) && self.looks_like_generic_args() {
                // Explicit generic type args: `ArrayList<String>(...)` or `parse<Int>(...)`
                let type_args = self.parse_generic_type_args()?;
                let args = self.parse_call_args()?;
                let span = expr_span(&expr).merge(self.prev_span());
                expr = Expr::Call(Box::new(CallExpr {
                    callee: Box::new(expr),
                    generics: type_args,
                    args,
                    span,
                }));
            } else if self.eat(&TokenKind::Dot).is_some() {
                let method_name = self.expect_ident()?;
                if self.at(&TokenKind::Lt) && self.looks_like_generic_args() {
                    let type_args = self.parse_generic_type_args()?;
                    let args = self.parse_call_args()?;
                    let span = expr_span(&expr).merge(self.prev_span());
                    expr = Expr::MethodCall(Box::new(MethodCallExpr {
                        receiver: Box::new(expr),
                        method: method_name,
                        generics: type_args,
                        args,
                        span,
                    }));
                } else if self.at(&TokenKind::LParen) {
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
            } else if self.eat(&TokenKind::As).is_some() {
                let target_ty = self.parse_type()?;
                let span = expr_span(&expr).merge(target_ty.span());
                expr = Expr::Cast(CastExpr {
                    expr: Box::new(expr),
                    target_ty,
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
            TokenKind::LongLit(n) => {
                self.bump();
                Some(Expr::Literal(Literal::Long(n, span)))
            }
            TokenKind::FloatLit(n) => {
                self.bump();
                Some(Expr::Literal(Literal::Float(n, span)))
            }
            TokenKind::DoubleLit(n) => {
                self.bump();
                Some(Expr::Literal(Literal::Double(n, span)))
            }
            TokenKind::CharLit(c) => {
                self.bump();
                Some(Expr::Literal(Literal::Char(c, span)))
            }
            TokenKind::StringLit(s) => {
                self.bump();
                Some(Expr::Literal(Literal::String(s, span)))
            }
            TokenKind::FStringLit(raw) => {
                self.bump();
                Some(Expr::StringInterp(Box::new(
                    self.parse_fstring_parts(&raw, span),
                )))
            }
            TokenKind::BoolLit(b) => {
                self.bump();
                Some(Expr::Literal(Literal::Bool(b, span)))
            }
            TokenKind::Ident(_) | TokenKind::Data => self.parse_path_expr(),
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
                if self.at(&TokenKind::RParen) {
                    let end = self.expect(TokenKind::RParen)?;
                    return Some(Expr::Literal(Literal::Unit(span.merge(end))));
                }
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
            TokenKind::Safe => self.parse_safe_expr(),
            TokenKind::Unsafe => self.parse_unsafe_expr(),
            TokenKind::Ref if self.lookahead(1) == &TokenKind::Mut => self.parse_ref_mut_expr(),
            TokenKind::LBracket => self.parse_list_literal(),
            TokenKind::Hash if self.peek_ahead_is(&TokenKind::LBrace) => self.parse_map_literal(),
            TokenKind::Dot if self.is_variant_shorthand_start() => {
                self.parse_variant_shorthand_expr()
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

    /// `[expr, expr, ...]` — empty `[]` allowed with type annotation.
    fn parse_list_literal(&mut self) -> Option<Expr> {
        let start = self.expect(TokenKind::LBracket)?;
        let mut elements = Vec::new();
        while !self.at(&TokenKind::RBracket) && !self.at_eof() {
            if !elements.is_empty() {
                self.expect(TokenKind::Comma)?;
                if self.at(&TokenKind::RBracket) {
                    break;
                }
            }
            elements.push(self.parse_expr()?);
        }
        let end = self.expect(TokenKind::RBracket)?;
        Some(Expr::ListLiteral(Box::new(valen_ast::ListLiteralExpr {
            elements,
            span: start.merge(end),
        })))
    }

    /// `#{key: value, ...}` — empty `#{}` allowed with type annotation.
    fn parse_map_literal(&mut self) -> Option<Expr> {
        let start = self.expect(TokenKind::Hash)?;
        self.expect(TokenKind::LBrace)?;
        let mut entries = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            if !entries.is_empty() {
                self.expect(TokenKind::Comma)?;
                if self.at(&TokenKind::RBrace) {
                    break;
                }
            }
            let key = self.parse_expr()?;
            self.expect(TokenKind::Colon)?;
            let value = self.parse_expr()?;
            entries.push((key, value));
        }
        let end = self.expect(TokenKind::RBrace)?;
        Some(Expr::MapLiteral(Box::new(valen_ast::MapLiteralExpr {
            entries,
            span: start.merge(end),
        })))
    }

    fn is_variant_shorthand_start(&self) -> bool {
        let look = self.pos + 1;
        look < self.tokens.len()
            && matches!(&self.tokens[look].0, TokenKind::Ident(name) if name.starts_with(|c: char| c.is_ascii_uppercase()))
    }

    fn parse_variant_shorthand_expr(&mut self) -> Option<Expr> {
        let start = self.peek_span();
        self.expect(TokenKind::Dot)?;
        let variant_name = self.expect_ident()?;
        let args = if self.at(&TokenKind::LParen) {
            self.parse_call_args()?
        } else {
            Vec::new()
        };
        let end = self.prev_span();
        Some(Expr::VariantShorthand(VariantShorthandExpr {
            variant_name,
            args,
            span: start.merge(end),
        }))
    }

    fn parse_variant_shorthand_pattern(&mut self) -> Option<Pattern> {
        let start = self.peek_span();
        self.expect(TokenKind::Dot)?;
        let variant_name = self.expect_ident()?;
        if self.at(&TokenKind::LParen) {
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
            Some(Pattern::VariantShorthand(VariantShorthandPattern {
                variant_name,
                fields,
                rest,
                span: start.merge(end),
            }))
        } else {
            let end = self.prev_span();
            Some(Pattern::VariantShorthand(VariantShorthandPattern {
                variant_name,
                fields: vec![],
                rest: false,
                span: start.merge(end),
            }))
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
            // Turbofish: `Foo::<Int, String>` -- after `::`, `<` starts generic args.
            if self.at(&TokenKind::Lt) || self.at(&TokenKind::Shr) {
                let generics = self.parse_turbofish_args()?;
                // Attach generics to the last segment.
                if let Some(last) = segments.last_mut() {
                    let gen_end = self.prev_span();
                    last.generics = generics;
                    last.span = last.span.merge(gen_end);
                }
                continue;
            }
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

    /// Parse turbofish generic arguments: `<Type, Type, ...>`.
    ///
    /// Called after `::` has been consumed when the next token is `<`.
    fn parse_turbofish_args(&mut self) -> Option<Vec<Type>> {
        self.expect(TokenKind::Lt)?;
        let mut args = Vec::new();
        while !self.at_gt() && !self.at_eof() {
            if !args.is_empty() {
                self.expect(TokenKind::Comma)?;
                if self.at_gt() {
                    break;
                }
            }
            args.push(self.parse_type()?);
        }
        self.expect_gt()?;
        Some(args)
    }

    fn parse_if_expr(&mut self) -> Option<Expr> {
        let start = self.expect(TokenKind::If)?;

        if self.at(&TokenKind::Let) {
            return self.parse_if_let_expr(start);
        }

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

    fn parse_if_let_expr(&mut self, start: Span) -> Option<Expr> {
        self.expect(TokenKind::Let)?;
        let pattern = self.parse_pattern()?;
        self.expect(TokenKind::Eq)?;
        let expr = self.parse_expr()?;
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
        Some(Expr::IfLet(Box::new(IfLetExpr {
            pattern,
            expr: Box::new(expr),
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
        Some(Expr::Match(Box::new(MatchExpr {
            scrutinee: Box::new(scrutinee),
            arms,
            span: start.merge(end),
        })))
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
            TokenKind::LongLit(n) => {
                self.bump();
                let lit = Literal::Long(n, span);
                if self.at(&TokenKind::DotDot) || self.at(&TokenKind::DotDotEq) {
                    return self.parse_range_pattern(Some(lit));
                }
                Some(Pattern::Literal(lit))
            }
            TokenKind::FloatLit(n) => {
                self.bump();
                Some(Pattern::Literal(Literal::Float(n, span)))
            }
            TokenKind::DoubleLit(n) => {
                self.bump();
                Some(Pattern::Literal(Literal::Double(n, span)))
            }
            TokenKind::CharLit(c) => {
                self.bump();
                Some(Pattern::Literal(Literal::Char(c, span)))
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
            TokenKind::Ident(_) | TokenKind::Data => self.parse_ident_pattern(),
            TokenKind::Dot if self.is_variant_shorthand_start() => {
                self.parse_variant_shorthand_pattern()
            }
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

        if self.at(&TokenKind::Let) {
            self.expect(TokenKind::Let)?;
            let pattern = self.parse_pattern()?;
            self.expect(TokenKind::Eq)?;
            let expr = self.parse_expr()?;
            let body = self.parse_block()?;
            let span = start.merge(body.span);
            return Some(Expr::WhileLet(Box::new(WhileLetExpr {
                pattern,
                expr: Box::new(expr),
                body,
                span,
            })));
        }

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

    fn parse_safe_expr(&mut self) -> Option<Expr> {
        let start = self.expect(TokenKind::Safe)?;
        if self.at(&TokenKind::LBrace) {
            let block = self.parse_block()?;
            let span = start.merge(block.span);
            Some(Expr::Safe(valen_ast::SafeExpr { block, span }))
        } else if self.eat(&TokenKind::Question).is_some() {
            // `safe? expr` => `safe { expr }?`
            let inner = self.parse_expr()?;
            let inner_span = expr_span(&inner);
            let block = Block {
                stmts: vec![],
                tail: Some(Box::new(inner)),
                span: inner_span,
            };
            let safe_span = start.merge(inner_span);
            let safe_expr = Expr::Safe(valen_ast::SafeExpr {
                block,
                span: safe_span,
            });
            let try_span = start.merge(inner_span);
            Some(Expr::Try(TryExpr {
                expr: Box::new(safe_expr),
                span: try_span,
            }))
        } else {
            // `safe expr` shorthand
            let inner = self.parse_expr()?;
            let inner_span = expr_span(&inner);
            let block = Block {
                stmts: vec![],
                tail: Some(Box::new(inner)),
                span: inner_span,
            };
            let span = start.merge(inner_span);
            Some(Expr::Safe(valen_ast::SafeExpr { block, span }))
        }
    }

    fn parse_unsafe_expr(&mut self) -> Option<Expr> {
        let start = self.expect(TokenKind::Unsafe)?;
        let body = if self.at(&TokenKind::LBrace) {
            let block = self.parse_block()?;
            Expr::Block(block)
        } else {
            self.parse_expr()?
        };
        let span = start.merge(expr_span(&body));
        Some(Expr::Unsafe(UnsafeExpr {
            body: Box::new(body),
            span,
        }))
    }

    fn parse_ref_mut_expr(&mut self) -> Option<Expr> {
        let start = self.expect(TokenKind::Ref)?;
        self.expect(TokenKind::Mut)?;
        let inner = self.parse_expr()?;
        let span = start.merge(expr_span(&inner));
        Some(Expr::RefMutCreate(RefMutExpr {
            expr: Box::new(inner),
            span,
        }))
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
                    .map(|t| p_start.merge(t.span()))
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
        // `data` is a context keyword: only a keyword before `class`, otherwise an identifier
        if matches!(self.peek(), TokenKind::Data) {
            self.bump();
            return Some(SmolStr::from("data"));
        }
        let span = self.peek_span();
        self.diagnostics.error(
            DiagCode::PARSE_EXPECTED_IDENT,
            span,
            SmolStr::from("expected identifier"),
        );
        None
    }

    /// True when the next effective token is `>`, accounting for a
    /// previously split `>>` (Shr) that left a pending `>`.
    fn at_gt(&self) -> bool {
        self.pending_gt > 0 || self.at(&TokenKind::Gt) || self.at(&TokenKind::Shr)
    }

    /// Consume a single `>` in generic context.  Handles three cases:
    /// 1. A pending `>` from a previously split `>>`.
    /// 2. A plain `>` token.
    /// 3. A `>>` (Shr) token — consumes it and saves a pending `>`.
    fn expect_gt(&mut self) -> Option<Span> {
        if self.pending_gt > 0 {
            self.pending_gt -= 1;
            let span = self
                .tokens
                .get(self.pos.saturating_sub(1))
                .map(|(_, s)| *s)
                .unwrap_or(Span::DUMMY);
            return Some(span);
        }
        if self.at(&TokenKind::Gt) {
            let (_, span) = self.bump();
            return Some(span);
        }
        if self.at(&TokenKind::Shr) {
            let (_, span) = self.bump();
            self.pending_gt = 1;
            return Some(span);
        }
        let span = self.peek_span();
        self.diagnostics.error(
            DiagCode::PARSE_EXPECTED_TOKEN,
            span,
            SmolStr::from("expected >"),
        );
        None
    }

    fn peek_is_ident(&self) -> bool {
        matches!(self.peek(), TokenKind::Ident(_))
    }

    /// Returns `true` when the current token is an `Ident` whose text equals `s`.
    fn peek_is_ident_matching(&self, s: &str) -> bool {
        matches!(self.peek(), TokenKind::Ident(name) if name.as_str() == s)
    }

    fn peek_ahead_is(&self, kind: &TokenKind) -> bool {
        self.lookahead(1) == kind
    }

    /// Lookahead to check if `<` starts a generic type argument list rather than
    /// a comparison. Scans forward matching angle brackets; returns true if the
    /// balanced `>` is followed by `(` (call).
    fn looks_like_generic_args(&self) -> bool {
        let mut depth = 0i32;
        let mut i = 0usize;
        loop {
            let tok = self.lookahead(i);
            match tok {
                TokenKind::Lt => depth += 1,
                TokenKind::Gt => {
                    depth -= 1;
                    if depth == 0 {
                        return self.lookahead(i + 1) == &TokenKind::LParen;
                    }
                }
                TokenKind::Shr => {
                    depth -= 2;
                    if depth <= 0 {
                        return self.lookahead(i + 1) == &TokenKind::LParen;
                    }
                }
                TokenKind::Ident(_)
                | TokenKind::Comma
                | TokenKind::Question
                | TokenKind::Dot
                | TokenKind::Fn
                | TokenKind::LParen
                | TokenKind::RParen
                | TokenKind::Arrow
                | TokenKind::Ref
                | TokenKind::Mut => {}
                TokenKind::Eof => return false,
                _ => return false,
            }
            i += 1;
        }
    }

    /// Parse `<Type, Type, ...>` as explicit generic type arguments.
    /// Uses `expect_gt()` to handle `>>` token splitting for nested generics.
    fn parse_generic_type_args(&mut self) -> Option<Vec<Type>> {
        self.expect(TokenKind::Lt)?;
        let mut types = Vec::new();
        if self.at_gt() {
            self.diagnostics.error(
                DiagCode::PARSE_EXPECTED_TOKEN,
                self.peek_span(),
                SmolStr::from("expected type argument"),
            );
            self.expect_gt()?;
            return Some(types);
        }
        while !self.at_gt() && !self.at_eof() {
            if !types.is_empty() {
                self.expect(TokenKind::Comma)?;
                if self.at_gt() {
                    break;
                }
            }
            types.push(self.parse_type()?);
        }
        self.expect_gt()?;
        Some(types)
    }

    fn parse_fstring_parts(&mut self, raw: &str, span: Span) -> StringInterpExpr {
        use valen_ast::StringInterpPart;
        let mut parts = Vec::new();
        let mut text = String::new();
        // `raw` is the content between the quotes of `f"..."`.
        // `span.start` points to `f`, so the content starts at offset +2 (after `f"`).
        let content_base = span.start + 2;
        let mut byte_offset: u32 = 0;
        let mut chars = raw.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '{' {
                let brace_start = content_base + byte_offset;
                byte_offset += c.len_utf8() as u32;
                if !text.is_empty() {
                    parts.push(StringInterpPart::Text(SmolStr::from(text.as_str())));
                    text.clear();
                }
                let mut expr_str = String::new();
                let mut depth = 1u32;
                for c2 in chars.by_ref() {
                    byte_offset += c2.len_utf8() as u32;
                    match c2 {
                        '{' => {
                            depth += 1;
                            expr_str.push(c2);
                        }
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                            expr_str.push(c2);
                        }
                        _ => expr_str.push(c2),
                    }
                }
                // Sub-span covering `{expr_str}` within the original source.
                let interp_span = Span::new(brace_start, content_base + byte_offset, span.file_id);
                let file_id = span.file_id;
                let expr_source = format!("fn __fstring__() -> String {{ {expr_str} }}");
                let reparsed = crate::parse(&expr_source, file_id);
                if reparsed.diagnostics.has_errors() {
                    self.diagnostics.error(
                        DiagCode::PARSE_EXPECTED_EXPR,
                        interp_span,
                        SmolStr::from(format!(
                            "invalid expression in f-string interpolation: `{expr_str}`"
                        )),
                    );
                    continue;
                }
                if let Some(valen_ast::Item::Fn(f)) = reparsed.items.first() {
                    if let Some(block) = &f.body {
                        if let Some(tail) = &block.tail {
                            parts.push(StringInterpPart::Expr(*tail.clone()));
                            continue;
                        }
                    }
                }
                // Fallback: reparsed successfully but no tail expression found.
                // Emit a diagnostic instead of silently dropping the interpolation.
                self.diagnostics.error(
                    DiagCode::PARSE_EXPECTED_EXPR,
                    interp_span,
                    SmolStr::from(format!(
                        "f-string interpolation `{expr_str}` did not produce an expression"
                    )),
                );
            } else if c == '\\' {
                byte_offset += c.len_utf8() as u32;
                if let Some(&next) = chars.peek() {
                    chars.next();
                    byte_offset += next.len_utf8() as u32;
                    match next {
                        'n' => text.push('\n'),
                        't' => text.push('\t'),
                        'r' => text.push('\r'),
                        '\\' => text.push('\\'),
                        '"' => text.push('"'),
                        '{' => text.push('{'),
                        '}' => text.push('}'),
                        other => {
                            text.push('\\');
                            text.push(other);
                        }
                    }
                }
            } else {
                byte_offset += c.len_utf8() as u32;
                text.push(c);
            }
        }
        if !text.is_empty() {
            parts.push(StringInterpPart::Text(SmolStr::from(text.as_str())));
        }
        StringInterpExpr { parts, span }
    }

    fn parse_literal(&mut self) -> Option<Literal> {
        let span = self.peek_span();
        match self.peek().clone() {
            TokenKind::IntLit(v) => {
                self.bump();
                Some(Literal::Int(v, span))
            }
            TokenKind::LongLit(v) => {
                self.bump();
                Some(Literal::Long(v, span))
            }
            TokenKind::FloatLit(v) => {
                self.bump();
                Some(Literal::Float(v, span))
            }
            TokenKind::DoubleLit(v) => {
                self.bump();
                Some(Literal::Double(v, span))
            }
            TokenKind::StringLit(v) => {
                self.bump();
                Some(Literal::String(v, span))
            }
            TokenKind::CharLit(v) => {
                self.bump();
                Some(Literal::Char(v, span))
            }
            TokenKind::BoolLit(v) => {
                self.bump();
                Some(Literal::Bool(v, span))
            }
            _ => {
                self.diagnostics.error(
                    DiagCode::PARSE_EXPECTED_EXPR,
                    span,
                    SmolStr::from("expected literal value in annotation argument"),
                );
                None
            }
        }
    }

    fn recover_to_item_boundary(&mut self) {
        self.pending_gt = 0;
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

/// Extract the source span from any expression variant.
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
        Expr::IfLet(i) => i.span,
        Expr::WhileLet(w) => w.span,
        Expr::VariantShorthand(v) => v.span,
        Expr::Pipeline(p) => p.span,
        Expr::ListLiteral(l) => l.span,
        Expr::MapLiteral(m) => m.span,
        Expr::Unsafe(u) => u.span,
        Expr::Cast(c) => c.span,
        Expr::Deref(d) => d.span,
        Expr::RefMutCreate(r) => r.span,
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
        Pattern::VariantShorthand(v) => v.span,
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

/// Extract a descriptive name from a pattern for the `LetElseStmt::name` field.
///
/// For struct patterns like `Some(x)` this returns `"x"` (the first bound variable).
/// For binding patterns like `x` it returns `"x"`.
/// For wildcards and other patterns, returns `"_"`.
fn extract_pattern_name(pattern: &Pattern) -> SmolStr {
    match pattern {
        Pattern::Binding(b) => b.name.clone(),
        Pattern::Struct(sp) => {
            // Extract the first bound field name as the "name" of the let-else binding
            if let Some(field) = sp.fields.first() {
                if let Some(ref pat) = field.pattern {
                    return extract_pattern_name(pat);
                }
                return field.name.clone();
            }
            SmolStr::from("_")
        }
        Pattern::At(at) => at.name.clone(),
        _ => SmolStr::from("_"),
    }
}

fn describe_token(kind: &TokenKind) -> &'static str {
    match kind {
        // Literals
        TokenKind::IntLit(_) => "integer literal",
        TokenKind::LongLit(_) => "long literal",
        TokenKind::FloatLit(_) => "float literal",
        TokenKind::DoubleLit(_) => "double literal",
        TokenKind::StringLit(_) => "string literal",
        TokenKind::FStringLit(_) => "interpolated string literal",
        TokenKind::CharLit(_) => "char literal",
        TokenKind::BoolLit(_) => "bool literal",

        // Identifiers
        TokenKind::Ident(_) => "identifier",

        // Keywords
        TokenKind::Fn => "`fn`",
        TokenKind::Let => "`let`",
        TokenKind::Mut => "`mut`",
        TokenKind::SelfKw => "`self`",
        TokenKind::Return => "`return`",
        TokenKind::If => "`if`",
        TokenKind::Else => "`else`",
        TokenKind::Match => "`match`",
        TokenKind::Class => "`class`",
        TokenKind::Data => "`data`",
        TokenKind::Enum => "`enum`",
        TokenKind::Trait => "`trait`",
        TokenKind::Impl => "`impl`",
        TokenKind::Pub => "`pub`",
        TokenKind::Internal => "`internal`",
        TokenKind::Private => "`private`",
        TokenKind::Open => "`open`",
        TokenKind::Override => "`override`",
        TokenKind::Abstract => "`abstract`",
        TokenKind::Sealed => "`sealed`",
        TokenKind::Package => "`package`",
        TokenKind::Import => "`import`",
        TokenKind::For => "`for`",
        TokenKind::In => "`in`",
        TokenKind::While => "`while`",
        TokenKind::Loop => "`loop`",
        TokenKind::Break => "`break`",
        TokenKind::Continue => "`continue`",
        TokenKind::True => "`true`",
        TokenKind::False => "`false`",
        TokenKind::As => "`as`",
        TokenKind::Safe => "`safe`",
        TokenKind::Unsafe => "`unsafe`",
        TokenKind::Ref => "`ref`",
        TokenKind::Inline => "`inline`",
        TokenKind::Reified => "`reified`",
        TokenKind::Annotation => "`annotation`",
        // Reserved keywords
        TokenKind::Suspend => "`suspend`",
        TokenKind::Async => "`async`",
        TokenKind::Await => "`await`",
        TokenKind::Yield => "`yield`",
        TokenKind::TypeAlias => "`typealias`",
        TokenKind::NewType => "`newtype`",
        TokenKind::Type => "`type`",
        // JVM reserved words
        TokenKind::Static => "`static`",
        TokenKind::Void => "`void`",
        TokenKind::This => "`this`",
        TokenKind::Super => "`super`",
        TokenKind::Null => "`null`",
        TokenKind::Throw => "`throw`",
        TokenKind::Try => "`try`",
        TokenKind::Catch => "`catch`",
        TokenKind::Finally => "`finally`",
        TokenKind::Extends => "`extends`",
        TokenKind::Implements => "`implements`",

        // Punctuation
        TokenKind::LParen => "`(`",
        TokenKind::RParen => "`)`",
        TokenKind::LBrace => "`{`",
        TokenKind::RBrace => "`}`",
        TokenKind::LBracket => "`[`",
        TokenKind::RBracket => "`]`",
        TokenKind::Comma => "`,`",
        TokenKind::Semi => "`;`",
        TokenKind::Colon => "`:`",
        TokenKind::DoubleColon => "`::`",
        TokenKind::Dot => "`.`",
        TokenKind::DotDot => "`..`",
        TokenKind::DotDotEq => "`..=`",
        TokenKind::Arrow => "`->`",
        TokenKind::FatArrow => "`=>`",
        TokenKind::Question => "`?`",
        TokenKind::Bang => "`!`",
        TokenKind::At => "`@`",
        TokenKind::Hash => "`#`",
        TokenKind::Underscore => "`_`",

        // Operators
        TokenKind::Eq => "`=`",
        TokenKind::EqEq => "`==`",
        TokenKind::EqEqEq => "`===`",
        TokenKind::NotEq => "`!=`",
        TokenKind::NotEqEq => "`!==`",
        TokenKind::Lt => "`<`",
        TokenKind::Le => "`<=`",
        TokenKind::Gt => "`>`",
        TokenKind::Ge => "`>=`",
        TokenKind::Plus => "`+`",
        TokenKind::Minus => "`-`",
        TokenKind::Star => "`*`",
        TokenKind::Slash => "`/`",
        TokenKind::Percent => "`%`",
        TokenKind::Amp => "`&`",
        TokenKind::AmpAmp => "`&&`",
        TokenKind::Pipe => "`|`",
        TokenKind::PipeGt => "`|>`",
        TokenKind::PipePipe => "`||`",
        TokenKind::Caret => "`^`",
        TokenKind::Shl => "`<<`",
        TokenKind::Shr => "`>>`",
        TokenKind::PlusEq => "`+=`",
        TokenKind::MinusEq => "`-=`",
        TokenKind::StarEq => "`*=`",
        TokenKind::SlashEq => "`/=`",
        TokenKind::PercentEq => "`%=`",

        // Trivia
        TokenKind::Whitespace => "whitespace",
        TokenKind::LineComment => "line comment",
        TokenKind::BlockComment => "block comment",
        TokenKind::DocComment(_) => "doc comment",

        // End of file
        TokenKind::Eof => "end of file",

        // Error
        TokenKind::Error(_) => "error token",
    }
}
