//! Bidirectional type checker producing typed HIR bodies.

use indexmap::IndexMap;
use smol_str::SmolStr;
use valen_ast::{self, BinaryOp, Span, UnaryOp};
use valen_diagnostics::{DiagCode, Diagnostics};

use crate::{
    tyref_to_ty, DefId, DefKind, Hir, PrimTy, Ty, TyRef, TypedBody, TypedExpr, TypedExprKind,
    TypedMatchArm, TypedStmt, TypedStringPart,
};

/// Output of the type checking pass.
pub struct TypeCheckResult {
    /// Typed bodies keyed by [`DefId`] to avoid name collisions across classes.
    pub bodies: IndexMap<DefId, TypedBody>,
    pub diagnostics: Diagnostics,
}

/// Type-check all items against the resolved HIR, producing typed bodies for each function.
pub fn type_check(hir: &Hir, items: &[valen_ast::Item]) -> TypeCheckResult {
    let mut tc = TypeChecker::new(hir);
    tc.check_items(items);
    TypeCheckResult {
        bodies: tc.bodies,
        diagnostics: tc.diags,
    }
}

// ---------------------------------------------------------------------------
// Type environment (scoped)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct VarBinding {
    ty: Ty,
    mutable: bool,
}

#[derive(Debug, Clone)]
struct TypeEnv {
    scopes: Vec<IndexMap<SmolStr, VarBinding>>,
}

impl TypeEnv {
    fn new() -> Self {
        Self {
            scopes: vec![IndexMap::new()],
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(IndexMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn define(&mut self, name: SmolStr, ty: Ty, mutable: bool) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, VarBinding { ty, mutable });
        }
    }

    fn lookup(&self, name: &str) -> Option<&Ty> {
        for scope in self.scopes.iter().rev() {
            if let Some(binding) = scope.get(name) {
                return Some(&binding.ty);
            }
        }
        None
    }

    fn is_mutable(&self, name: &str) -> Option<bool> {
        for scope in self.scopes.iter().rev() {
            if let Some(binding) = scope.get(name) {
                return Some(binding.mutable);
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// TypeChecker
// ---------------------------------------------------------------------------

struct TypeChecker<'hir> {
    hir: &'hir Hir,
    env: TypeEnv,
    diags: Diagnostics,
    bodies: IndexMap<DefId, TypedBody>,
    return_ty: Option<Ty>,
    in_loop: bool,
}

impl<'hir> TypeChecker<'hir> {
    fn new(hir: &'hir Hir) -> Self {
        Self {
            hir,
            env: TypeEnv::new(),
            diags: Diagnostics::new(),
            bodies: IndexMap::new(),
            return_ty: None,
            in_loop: false,
        }
    }

    // -- top-level dispatch -------------------------------------------------

    fn check_items(&mut self, items: &[valen_ast::Item]) {
        self.register_top_level_types(items);

        for item in items {
            match item {
                valen_ast::Item::Fn(f) => {
                    let def_id = self.lookup_def_id(&f.name);
                    self.check_fn_decl(f, None, def_id);
                }
                valen_ast::Item::Class(c) => self.check_class(c),
                valen_ast::Item::Impl(imp) => self.check_impl(imp),
                valen_ast::Item::Trait(t) => self.check_trait(t),
                _ => {}
            }
        }
    }

    fn expand_type_alias(&self, name: &str, args: &[Ty]) -> Option<Ty> {
        let def = self.hir.defs.values().find(|d| d.name == name)?;
        let alias = match &def.kind {
            DefKind::TypeAlias(a) => a,
            _ => return None,
        };
        Some(self.substitute_tyref(&alias.target, &alias.generics, args))
    }

    fn substitute_tyref(&self, tyref: &TyRef, params: &[SmolStr], args: &[Ty]) -> Ty {
        match tyref {
            TyRef::Unresolved(name) => {
                if let Some(idx) = params.iter().position(|p| p == name) {
                    args.get(idx).cloned().unwrap_or(Ty::Error)
                } else {
                    Ty::Named(name.clone())
                }
            }
            TyRef::Named(name) => Ty::Named(name.clone()),
            TyRef::Prim(p) => Ty::Prim(*p),
            TyRef::Generic(name, ga) => {
                let resolved: Vec<Ty> = ga
                    .iter()
                    .map(|a| self.substitute_tyref(a, params, args))
                    .collect();
                Ty::Generic(name.clone(), resolved)
            }
            TyRef::Nullable(inner) => {
                Ty::Nullable(Box::new(self.substitute_tyref(inner, params, args)))
            }
            TyRef::Fn(p, r) => {
                let ps = p
                    .iter()
                    .map(|t| self.substitute_tyref(t, params, args))
                    .collect();
                let ret = Box::new(self.substitute_tyref(r, params, args));
                Ty::Fn(ps, ret)
            }
            TyRef::SelfTy | TyRef::Error => Ty::Error,
        }
    }

    fn lookup_def_id(&self, name: &str) -> Option<DefId> {
        self.hir
            .defs
            .values()
            .find(|d| d.name == name)
            .map(|d| d.id)
    }

    fn lookup_method_def_id(&self, class_name: &str, method_name: &str) -> Option<DefId> {
        for def in self.hir.defs.values() {
            match &def.kind {
                DefKind::Class(c) if def.name == class_name => {
                    for &mid in &c.methods {
                        if let Some(mdef) = self.hir.defs.get(&mid) {
                            if mdef.name == method_name {
                                return Some(mid);
                            }
                        }
                    }
                }
                DefKind::Impl(imp) => {
                    for &mid in &imp.methods {
                        if let Some(mdef) = self.hir.defs.get(&mid) {
                            if mdef.name == method_name {
                                return Some(mid);
                            }
                        }
                    }
                }
                DefKind::Trait(t) if def.name == class_name => {
                    for &mid in &t.methods {
                        if let Some(mdef) = self.hir.defs.get(&mid) {
                            if mdef.name == method_name {
                                return Some(mid);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn register_top_level_types(&mut self, items: &[valen_ast::Item]) {
        for item in items {
            match item {
                valen_ast::Item::Fn(f) => {
                    let ty = self.fn_decl_ty(f);
                    self.env.define(f.name.clone(), ty, false);
                }
                valen_ast::Item::Class(c) => {
                    self.env
                        .define(c.name.clone(), Ty::Named(c.name.clone()), false);
                }
                valen_ast::Item::DataClass(dc) => {
                    self.env
                        .define(dc.name.clone(), Ty::Named(dc.name.clone()), false);
                }
                valen_ast::Item::Enum(e) => {
                    self.env
                        .define(e.name.clone(), Ty::Named(e.name.clone()), false);
                }
                valen_ast::Item::Trait(t) => {
                    self.env
                        .define(t.name.clone(), Ty::Named(t.name.clone()), false);
                }
                _ => {}
            }
        }
    }

    fn fn_decl_ty(&self, f: &valen_ast::FnDecl) -> Ty {
        let params: Vec<Ty> = f
            .params
            .iter()
            .filter(|p| p.name != "self")
            .map(|p| self.resolve_ast_type(&p.ty))
            .collect();
        let ret = f
            .return_type
            .as_ref()
            .map(|t| self.resolve_ast_type(t))
            .unwrap_or_else(Ty::unit);
        Ty::Fn(params, Box::new(ret))
    }

    fn resolve_ast_type(&self, ty: &valen_ast::Type) -> Ty {
        match ty {
            valen_ast::Type::Path(tp) => {
                if tp.segments.len() == 1 {
                    let seg = &tp.segments[0];
                    if let Some(prim) = crate::resolve_prim(&seg.name) {
                        return Ty::Prim(prim);
                    }
                    let args: Vec<Ty> = seg
                        .generics
                        .iter()
                        .map(|g| self.resolve_ast_type(g))
                        .collect();
                    if let Some(expanded) = self.expand_type_alias(&seg.name, &args) {
                        return expanded;
                    }
                    if args.is_empty() {
                        return Ty::Named(seg.name.clone());
                    }
                    return Ty::Generic(seg.name.clone(), args);
                }
                let full: String = tp
                    .segments
                    .iter()
                    .map(|s| s.name.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                Ty::Named(SmolStr::from(full))
            }
            valen_ast::Type::Nullable { inner, .. } => {
                Ty::Nullable(Box::new(self.resolve_ast_type(inner)))
            }
            valen_ast::Type::Fn(ft) => {
                let params = ft.params.iter().map(|p| self.resolve_ast_type(p)).collect();
                let ret = Box::new(self.resolve_ast_type(&ft.return_type));
                Ty::Fn(params, ret)
            }
            valen_ast::Type::Tuple(_) => Ty::Error,
        }
    }

    // -- function / class / impl --------------------------------------------

    fn check_fn_decl(
        &mut self,
        f: &valen_ast::FnDecl,
        self_ty: Option<&Ty>,
        def_id: Option<DefId>,
    ) {
        let Some(body) = &f.body else { return };

        let ret_ty = f
            .return_type
            .as_ref()
            .map(|t| self.resolve_ast_type(t))
            .unwrap_or_else(Ty::unit);

        let prev_return = self.return_ty.take();
        self.return_ty = Some(ret_ty.clone());

        self.env.push_scope();

        if let Some(sty) = self_ty {
            self.env.define(SmolStr::from("self"), sty.clone(), false);
        }

        for p in &f.params {
            if p.name == "self" {
                continue;
            }
            let pty = self.resolve_ast_type(&p.ty);
            self.env.define(p.name.clone(), pty, p.mutable);
        }

        let typed_body = self.check_block(body, Some(&ret_ty));

        if !ret_ty.is_error()
            && !typed_body.ty.is_error()
            && typed_body.ty != ret_ty
            && typed_body.ty != Ty::nothing()
            && !is_subtype(&typed_body.ty, &ret_ty)
        {
            self.diags.error(
                DiagCode::TYPE_MISMATCH,
                body.span,
                SmolStr::from(format!(
                    "expected return type `{ret_ty}`, found `{}`",
                    typed_body.ty
                )),
            );
        }

        self.env.pop_scope();
        self.return_ty = prev_return;

        if let Some(id) = def_id {
            self.bodies.insert(id, typed_body);
        }
    }

    fn check_class(&mut self, c: &valen_ast::ClassDecl) {
        let self_ty = Ty::Named(c.name.clone());
        for member in &c.body {
            if let valen_ast::ClassMember::Method(m) = member {
                let def_id = self.lookup_method_def_id(&c.name, &m.name);
                self.check_fn_decl(m, Some(&self_ty), def_id);
            }
        }
    }

    fn check_impl(&mut self, imp: &valen_ast::ImplBlock) {
        let self_ty = self.resolve_ast_type(&imp.target);
        for item in &imp.items {
            if let valen_ast::ImplItem::Fn(m) = item {
                let def_id = self.lookup_method_def_id("", &m.name);
                self.check_fn_decl(m, Some(&self_ty), def_id);
            }
        }
    }

    fn check_trait(&mut self, t: &valen_ast::TraitDecl) {
        let self_ty = Ty::Named(t.name.clone());
        for item in &t.items {
            if let valen_ast::TraitItem::Fn(f) = item {
                if f.body.is_some() {
                    let def_id = self.lookup_method_def_id(&t.name, &f.name);
                    self.check_fn_decl(f, Some(&self_ty), def_id);
                }
            }
        }
    }

    // -- block --------------------------------------------------------------

    fn check_block(&mut self, block: &valen_ast::Block, expected: Option<&Ty>) -> TypedBody {
        self.env.push_scope();
        let mut stmts = Vec::new();

        for stmt in &block.stmts {
            stmts.push(self.check_stmt(stmt));
        }

        let (tail, ty) = if let Some(tail_expr) = &block.tail {
            let te = if let Some(exp) = expected {
                self.check_expr(tail_expr, Some(exp))
            } else {
                self.infer_expr(tail_expr)
            };
            let t = te.ty.clone();
            (Some(Box::new(te)), t)
        } else {
            (None, Ty::unit())
        };

        self.env.pop_scope();
        TypedBody { stmts, tail, ty }
    }

    // -- statements ---------------------------------------------------------

    fn check_stmt(&mut self, stmt: &valen_ast::Stmt) -> TypedStmt {
        match stmt {
            valen_ast::Stmt::Let(ls) => {
                let expected = ls.ty.as_ref().map(|t| self.resolve_ast_type(t));
                let init = if let Some(exp) = &expected {
                    self.check_expr(&ls.init, Some(exp))
                } else {
                    self.infer_expr(&ls.init)
                };
                let ty = expected.unwrap_or_else(|| init.ty.clone());
                self.env.define(ls.name.clone(), ty.clone(), ls.mutable);
                TypedStmt::Let {
                    name: ls.name.clone(),
                    ty,
                    init,
                    mutable: ls.mutable,
                    span: ls.span,
                }
            }
            valen_ast::Stmt::Expr(e) => TypedStmt::Expr(self.infer_expr(e)),
            valen_ast::Stmt::ExprSemi(e) => TypedStmt::ExprSemi(self.infer_expr(e)),
        }
    }

    // -- bidirectional entry points -----------------------------------------

    fn infer_expr(&mut self, expr: &valen_ast::Expr) -> TypedExpr {
        self.check_expr(expr, None)
    }

    fn check_expr(&mut self, expr: &valen_ast::Expr, expected: Option<&Ty>) -> TypedExpr {
        let te = self.synth_expr(expr, expected);
        if let Some(exp) = expected {
            if !te.ty.is_error()
                && !exp.is_error()
                && te.ty != *exp
                && te.ty != Ty::nothing()
                && !is_subtype(&te.ty, exp)
            {
                self.diags.error(
                    DiagCode::TYPE_MISMATCH,
                    te.span,
                    SmolStr::from(format!("expected `{exp}`, found `{}`", te.ty)),
                );
            }
        }
        te
    }

    // -- expression synthesis -----------------------------------------------

    fn synth_expr(&mut self, expr: &valen_ast::Expr, expected: Option<&Ty>) -> TypedExpr {
        match expr {
            valen_ast::Expr::Literal(lit) => self.synth_literal(lit),
            valen_ast::Expr::Path(path) => self.synth_path(path),
            valen_ast::Expr::Binary(bin) => self.synth_binary(bin),
            valen_ast::Expr::Unary(un) => self.synth_unary(un),
            valen_ast::Expr::Call(call) => self.synth_call(call),
            valen_ast::Expr::MethodCall(mc) => self.synth_method_call(mc),
            valen_ast::Expr::Field(fa) => self.synth_field_access(fa),
            valen_ast::Expr::If(ife) => self.synth_if(ife, expected),
            valen_ast::Expr::Match(me) => self.synth_match(me, expected),
            valen_ast::Expr::Block(blk) => {
                let body = self.check_block(blk, expected);
                let ty = body.ty.clone();
                TypedExpr {
                    kind: TypedExprKind::Block(body),
                    ty,
                    span: blk.span,
                }
            }
            valen_ast::Expr::Assign(asgn) => self.synth_assign(asgn),
            valen_ast::Expr::Return(ret) => self.synth_return(ret),
            valen_ast::Expr::Break(brk) => self.synth_break(brk),
            valen_ast::Expr::Continue(cont) => self.synth_continue(cont),
            valen_ast::Expr::For(f) => self.synth_for(f),
            valen_ast::Expr::While(w) => self.synth_while(w),
            valen_ast::Expr::Loop(l) => self.synth_loop(l),
            valen_ast::Expr::Lambda(lam) => self.synth_lambda(lam, expected),
            valen_ast::Expr::Range(r) => self.synth_range(r),
            valen_ast::Expr::StringInterp(si) => self.synth_string_interp(si),
            valen_ast::Expr::Try(t) => self.synth_try(t),
            valen_ast::Expr::Safe(s) => self.synth_safe(s),
        }
    }

    // -- literals -----------------------------------------------------------

    fn synth_literal(&self, lit: &valen_ast::Literal) -> TypedExpr {
        match lit {
            valen_ast::Literal::Int(v, span) => TypedExpr {
                kind: TypedExprKind::IntLit(*v),
                ty: Ty::Prim(PrimTy::Int),
                span: *span,
            },
            valen_ast::Literal::Long(v, span) => TypedExpr {
                kind: TypedExprKind::LongLit(*v),
                ty: Ty::Prim(PrimTy::Long),
                span: *span,
            },
            valen_ast::Literal::Float(v, span) => TypedExpr {
                kind: TypedExprKind::Float32Lit(*v),
                ty: Ty::Prim(PrimTy::Float),
                span: *span,
            },
            valen_ast::Literal::Double(v, span) => TypedExpr {
                kind: TypedExprKind::FloatLit(*v),
                ty: Ty::Prim(PrimTy::Double),
                span: *span,
            },
            valen_ast::Literal::Char(v, span) => TypedExpr {
                kind: TypedExprKind::CharLit(*v),
                ty: Ty::Prim(PrimTy::Char),
                span: *span,
            },
            valen_ast::Literal::String(v, span) => TypedExpr {
                kind: TypedExprKind::StringLit(v.clone()),
                ty: Ty::Prim(PrimTy::String),
                span: *span,
            },
            valen_ast::Literal::Bool(v, span) => TypedExpr {
                kind: TypedExprKind::BoolLit(*v),
                ty: Ty::Prim(PrimTy::Bool),
                span: *span,
            },
            valen_ast::Literal::Unit(span) => TypedExpr {
                kind: TypedExprKind::UnitLit,
                ty: Ty::unit(),
                span: *span,
            },
        }
    }

    // -- path (variable reference) ------------------------------------------

    fn synth_path(&mut self, path: &valen_ast::Path) -> TypedExpr {
        if path.segments.len() == 1 {
            let name = &path.segments[0].name;
            if let Some(ty) = self.env.lookup(name).cloned() {
                return TypedExpr {
                    kind: TypedExprKind::LocalVar(name.clone()),
                    ty,
                    span: path.span,
                };
            }
            self.diags.error(
                DiagCode::UNDECLARED_VAR,
                path.span,
                SmolStr::from(format!("undeclared variable `{name}`")),
            );
            return TypedExpr {
                kind: TypedExprKind::Error,
                ty: Ty::Error,
                span: path.span,
            };
        }
        // multi-segment path (e.g. Enum::Variant) — resolve first segment
        let first = &path.segments[0].name;
        let ty = self.env.lookup(first).cloned().unwrap_or(Ty::Error);
        TypedExpr {
            kind: TypedExprKind::LocalVar(first.clone()),
            ty,
            span: path.span,
        }
    }

    // -- binary ops ---------------------------------------------------------

    fn synth_binary(&mut self, bin: &valen_ast::BinaryExpr) -> TypedExpr {
        let lhs = self.infer_expr(&bin.lhs);
        let rhs = self.infer_expr(&bin.rhs);

        if lhs.ty.is_error() || rhs.ty.is_error() {
            return TypedExpr {
                kind: TypedExprKind::Binary {
                    op: bin.op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
                ty: Ty::Error,
                span: bin.span,
            };
        }

        // No implicit numeric conversion
        if lhs.ty != rhs.ty && lhs.ty.is_numeric() && rhs.ty.is_numeric() {
            self.diags.error(
                DiagCode::NO_IMPLICIT_CONVERSION,
                bin.span,
                SmolStr::from(format!(
                    "no implicit conversion between `{}` and `{}`; use explicit conversion",
                    lhs.ty, rhs.ty
                )),
            );
            return TypedExpr {
                kind: TypedExprKind::Binary {
                    op: bin.op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
                ty: Ty::Error,
                span: bin.span,
            };
        }

        let result_ty = self.binary_result_ty(bin.op, &lhs.ty, &rhs.ty, bin.span);

        TypedExpr {
            kind: TypedExprKind::Binary {
                op: bin.op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
            ty: result_ty,
            span: bin.span,
        }
    }

    fn binary_result_ty(&mut self, op: BinaryOp, lhs: &Ty, rhs: &Ty, span: Span) -> Ty {
        match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                if lhs.is_numeric() && lhs == rhs {
                    return lhs.clone();
                }
                if op == BinaryOp::Add
                    && *lhs == Ty::Prim(PrimTy::String)
                    && *rhs == Ty::Prim(PrimTy::String)
                {
                    return Ty::Prim(PrimTy::String);
                }
                self.diags.error(
                    DiagCode::INVALID_OPERATOR,
                    span,
                    SmolStr::from(format!(
                        "operator `{op:?}` not supported for `{lhs}` and `{rhs}`"
                    )),
                );
                Ty::Error
            }
            BinaryOp::Eq | BinaryOp::Ne => {
                if lhs == rhs {
                    Ty::Prim(PrimTy::Bool)
                } else {
                    self.diags.error(
                        DiagCode::TYPE_MISMATCH,
                        span,
                        SmolStr::from(format!("cannot compare `{lhs}` and `{rhs}` for equality")),
                    );
                    Ty::Prim(PrimTy::Bool)
                }
            }
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                if lhs.is_numeric() && lhs == rhs {
                    return Ty::Prim(PrimTy::Bool);
                }
                self.diags.error(
                    DiagCode::INVALID_OPERATOR,
                    span,
                    SmolStr::from(format!("comparison not supported for `{lhs}` and `{rhs}`")),
                );
                Ty::Prim(PrimTy::Bool)
            }
            BinaryOp::And | BinaryOp::Or => {
                if lhs.is_bool() && rhs.is_bool() {
                    return Ty::Prim(PrimTy::Bool);
                }
                self.diags.error(
                    DiagCode::TYPE_MISMATCH,
                    span,
                    SmolStr::from(format!(
                        "logical operators require Bool, found `{lhs}` and `{rhs}`"
                    )),
                );
                Ty::Prim(PrimTy::Bool)
            }
            BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::Shl
            | BinaryOp::Shr => {
                if lhs.is_integer() && lhs == rhs {
                    return lhs.clone();
                }
                self.diags.error(
                    DiagCode::INVALID_OPERATOR,
                    span,
                    SmolStr::from(format!(
                        "bitwise operators require matching integer types, found `{lhs}` and `{rhs}`"
                    )),
                );
                Ty::Error
            }
            BinaryOp::RefEq | BinaryOp::RefNe => Ty::Prim(PrimTy::Bool),
        }
    }

    // -- unary ops ----------------------------------------------------------

    fn synth_unary(&mut self, un: &valen_ast::UnaryExpr) -> TypedExpr {
        let operand = self.infer_expr(&un.expr);
        if operand.ty.is_error() {
            return TypedExpr {
                kind: TypedExprKind::Unary {
                    op: un.op,
                    expr: Box::new(operand),
                },
                ty: Ty::Error,
                span: un.span,
            };
        }

        let ty = match un.op {
            UnaryOp::Neg => {
                if operand.ty.is_numeric() {
                    operand.ty.clone()
                } else {
                    self.diags.error(
                        DiagCode::INVALID_OPERATOR,
                        un.span,
                        SmolStr::from(format!("cannot negate `{}`", operand.ty)),
                    );
                    Ty::Error
                }
            }
            UnaryOp::Not => {
                if operand.ty.is_bool() {
                    Ty::Prim(PrimTy::Bool)
                } else {
                    self.diags.error(
                        DiagCode::INVALID_OPERATOR,
                        un.span,
                        SmolStr::from(format!("cannot apply `!` to `{}`", operand.ty)),
                    );
                    Ty::Error
                }
            }
        };

        TypedExpr {
            kind: TypedExprKind::Unary {
                op: un.op,
                expr: Box::new(operand),
            },
            ty,
            span: un.span,
        }
    }

    // -- function call ------------------------------------------------------

    fn synth_call(&mut self, call: &valen_ast::CallExpr) -> TypedExpr {
        let callee = self.infer_expr(&call.callee);
        let args: Vec<TypedExpr> = call
            .args
            .iter()
            .map(|a| self.infer_expr(&a.value))
            .collect();

        match &callee.ty {
            Ty::Fn(param_tys, ret_ty) => {
                if args.len() != param_tys.len() {
                    self.diags.error(
                        DiagCode::ARG_COUNT_MISMATCH,
                        call.span,
                        SmolStr::from(format!(
                            "expected {} argument(s), found {}",
                            param_tys.len(),
                            args.len()
                        )),
                    );
                } else {
                    for (arg, expected) in args.iter().zip(param_tys.iter()) {
                        if !arg.ty.is_error() && !expected.is_error() && arg.ty != *expected {
                            self.diags.error(
                                DiagCode::TYPE_MISMATCH,
                                arg.span,
                                SmolStr::from(format!("expected `{expected}`, found `{}`", arg.ty)),
                            );
                        }
                    }
                }
                let ret = (**ret_ty).clone();
                TypedExpr {
                    kind: TypedExprKind::Call {
                        callee: Box::new(callee),
                        args,
                    },
                    ty: ret,
                    span: call.span,
                }
            }
            Ty::Named(name) => {
                // Constructor call — look up class/data class
                let ctor_ty = self.resolve_ctor_type(name, &args, call.span);
                TypedExpr {
                    kind: TypedExprKind::Call {
                        callee: Box::new(callee),
                        args,
                    },
                    ty: ctor_ty,
                    span: call.span,
                }
            }
            Ty::Error => TypedExpr {
                kind: TypedExprKind::Call {
                    callee: Box::new(callee),
                    args,
                },
                ty: Ty::Error,
                span: call.span,
            },
            other => {
                self.diags.error(
                    DiagCode::NOT_CALLABLE,
                    call.span,
                    SmolStr::from(format!("`{other}` is not callable")),
                );
                TypedExpr {
                    kind: TypedExprKind::Call {
                        callee: Box::new(callee),
                        args,
                    },
                    ty: Ty::Error,
                    span: call.span,
                }
            }
        }
    }

    fn resolve_ctor_type(&mut self, name: &SmolStr, args: &[TypedExpr], span: Span) -> Ty {
        for def in self.hir.defs.values() {
            if def.name != *name {
                continue;
            }
            match &def.kind {
                DefKind::Class(c) => {
                    if args.len() != c.ctor_params.len() {
                        self.diags.error(
                            DiagCode::ARG_COUNT_MISMATCH,
                            span,
                            SmolStr::from(format!(
                                "`{name}` constructor expects {} argument(s), found {}",
                                c.ctor_params.len(),
                                args.len()
                            )),
                        );
                    } else {
                        for (arg, param) in args.iter().zip(c.ctor_params.iter()) {
                            let expected = tyref_to_ty(&param.ty);
                            if !arg.ty.is_error() && !expected.is_error() && arg.ty != expected {
                                self.diags.error(
                                    DiagCode::TYPE_MISMATCH,
                                    arg.span,
                                    SmolStr::from(format!(
                                        "expected `{expected}`, found `{}`",
                                        arg.ty
                                    )),
                                );
                            }
                        }
                    }
                    return Ty::Named(name.clone());
                }
                DefKind::DataClass(dc) => {
                    if args.len() != dc.ctor_params.len() {
                        self.diags.error(
                            DiagCode::ARG_COUNT_MISMATCH,
                            span,
                            SmolStr::from(format!(
                                "`{name}` constructor expects {} argument(s), found {}",
                                dc.ctor_params.len(),
                                args.len()
                            )),
                        );
                    } else {
                        for (arg, param) in args.iter().zip(dc.ctor_params.iter()) {
                            let expected = tyref_to_ty(&param.ty);
                            if !arg.ty.is_error() && !expected.is_error() && arg.ty != expected {
                                self.diags.error(
                                    DiagCode::TYPE_MISMATCH,
                                    arg.span,
                                    SmolStr::from(format!(
                                        "expected `{expected}`, found `{}`",
                                        arg.ty
                                    )),
                                );
                            }
                        }
                    }
                    return Ty::Named(name.clone());
                }
                _ => {}
            }
        }
        if self.resolve_foreign_ctor(name).is_some() {
            return Ty::Named(name.clone());
        }
        Ty::Named(name.clone())
    }

    // -- method call --------------------------------------------------------

    fn synth_method_call(&mut self, mc: &valen_ast::MethodCallExpr) -> TypedExpr {
        let receiver = self.infer_expr(&mc.receiver);
        let args: Vec<TypedExpr> = mc.args.iter().map(|a| self.infer_expr(&a.value)).collect();

        if receiver.ty.is_error() {
            return TypedExpr {
                kind: TypedExprKind::MethodCall {
                    receiver: Box::new(receiver),
                    method: mc.method.clone(),
                    args,
                },
                ty: Ty::Error,
                span: mc.span,
            };
        }

        let type_name = match &receiver.ty {
            Ty::Named(n) => Some(n.clone()),
            Ty::Prim(p) => Some(SmolStr::from(format!("{p:?}"))),
            _ => None,
        };

        if let Some(tn) = &type_name {
            let resolution = self.hir.resolve_method(tn, &mc.method);
            match resolution {
                crate::MethodResolution::Found(def_id) => {
                    if let Some(def) = self.hir.defs.get(&def_id) {
                        if let DefKind::Fn(fdef) = &def.kind {
                            let ret_ty = fdef
                                .return_ty
                                .as_ref()
                                .map(tyref_to_ty)
                                .unwrap_or_else(Ty::unit);

                            let non_self_params: Vec<_> =
                                fdef.params.iter().filter(|p| !p.is_self).collect();

                            if args.len() != non_self_params.len() {
                                self.diags.error(
                                    DiagCode::ARG_COUNT_MISMATCH,
                                    mc.span,
                                    SmolStr::from(format!(
                                        "method `{}` expects {} argument(s), found {}",
                                        mc.method,
                                        non_self_params.len(),
                                        args.len()
                                    )),
                                );
                            } else {
                                for (arg, param) in args.iter().zip(non_self_params.iter()) {
                                    let expected = tyref_to_ty(&param.ty);
                                    if !arg.ty.is_error()
                                        && !expected.is_error()
                                        && arg.ty != expected
                                    {
                                        self.diags.error(
                                            DiagCode::TYPE_MISMATCH,
                                            arg.span,
                                            SmolStr::from(format!(
                                                "expected `{expected}`, found `{}`",
                                                arg.ty
                                            )),
                                        );
                                    }
                                }
                            }

                            return TypedExpr {
                                kind: TypedExprKind::MethodCall {
                                    receiver: Box::new(receiver),
                                    method: mc.method.clone(),
                                    args,
                                },
                                ty: ret_ty,
                                span: mc.span,
                            };
                        }
                    }
                }
                crate::MethodResolution::Ambiguous(_) => {
                    self.diags.error(
                        DiagCode::AMBIGUOUS_METHOD,
                        mc.span,
                        SmolStr::from(format!(
                            "ambiguous method `{}` on type `{}`",
                            mc.method, receiver.ty
                        )),
                    );
                }
                crate::MethodResolution::NotFound => {
                    if let Some(foreign_result) = self.resolve_foreign_method(tn, &mc.method, &args)
                    {
                        return TypedExpr {
                            kind: TypedExprKind::MethodCall {
                                receiver: Box::new(receiver),
                                method: mc.method.clone(),
                                args,
                            },
                            ty: foreign_result,
                            span: mc.span,
                        };
                    }
                    self.diags.error(
                        DiagCode::NO_SUCH_METHOD,
                        mc.span,
                        SmolStr::from(format!(
                            "no method `{}` found on type `{}`",
                            mc.method, receiver.ty
                        )),
                    );
                }
            }
        } else if let Some(type_name) = self.ty_name(&receiver.ty) {
            if let Some(foreign_result) = self.resolve_foreign_method(&type_name, &mc.method, &args)
            {
                return TypedExpr {
                    kind: TypedExprKind::MethodCall {
                        receiver: Box::new(receiver),
                        method: mc.method.clone(),
                        args,
                    },
                    ty: foreign_result,
                    span: mc.span,
                };
            }
            self.diags.error(
                DiagCode::NO_SUCH_METHOD,
                mc.span,
                SmolStr::from(format!(
                    "cannot call method `{}` on type `{}`",
                    mc.method, receiver.ty
                )),
            );
        } else {
            self.diags.error(
                DiagCode::NO_SUCH_METHOD,
                mc.span,
                SmolStr::from(format!(
                    "cannot call method `{}` on type `{}`",
                    mc.method, receiver.ty
                )),
            );
        }

        TypedExpr {
            kind: TypedExprKind::MethodCall {
                receiver: Box::new(receiver),
                method: mc.method.clone(),
                args,
            },
            ty: Ty::Error,
            span: mc.span,
        }
    }

    fn ty_name(&self, ty: &Ty) -> Option<SmolStr> {
        match ty {
            Ty::Named(n) => Some(n.clone()),
            Ty::Generic(n, _) => Some(n.clone()),
            _ => None,
        }
    }

    fn resolve_foreign_method(
        &self,
        type_name: &str,
        method_name: &str,
        _args: &[TypedExpr],
    ) -> Option<Ty> {
        let info = self.hir.foreign_types.get(type_name)?;
        let matching: Vec<_> = info
            .methods
            .iter()
            .filter(|m| m.name == method_name)
            .collect();
        let m = matching.first()?;
        Some(tyref_to_ty(&m.return_ty))
    }

    fn resolve_foreign_ctor(&self, type_name: &str) -> Option<Ty> {
        let _info = self.hir.foreign_types.get(type_name)?;
        Some(Ty::Named(SmolStr::from(type_name)))
    }

    // -- field access -------------------------------------------------------

    fn synth_field_access(&mut self, fa: &valen_ast::FieldAccess) -> TypedExpr {
        let receiver = self.infer_expr(&fa.receiver);
        if receiver.ty.is_error() {
            return TypedExpr {
                kind: TypedExprKind::FieldAccess {
                    receiver: Box::new(receiver),
                    field: fa.field.clone(),
                },
                ty: Ty::Error,
                span: fa.span,
            };
        }

        let field_ty = self.resolve_field_type(&receiver.ty, &fa.field, fa.span);
        TypedExpr {
            kind: TypedExprKind::FieldAccess {
                receiver: Box::new(receiver),
                field: fa.field.clone(),
            },
            ty: field_ty,
            span: fa.span,
        }
    }

    fn resolve_field_type(&mut self, receiver_ty: &Ty, field: &str, span: Span) -> Ty {
        let type_name = match receiver_ty {
            Ty::Named(n) => n,
            _ => {
                self.diags.error(
                    DiagCode::NO_SUCH_FIELD,
                    span,
                    SmolStr::from(format!("no field `{field}` on type `{receiver_ty}`")),
                );
                return Ty::Error;
            }
        };

        // Determine the accessor context: if `self` is in scope and has a Named type,
        // that is the "current class" for visibility purposes.
        let accessor_type = self.env.lookup("self").and_then(|t| match t {
            Ty::Named(n) => Some(n.clone()),
            _ => None,
        });

        for def in self.hir.defs.values() {
            if def.name != *type_name {
                continue;
            }
            match &def.kind {
                DefKind::Class(c) => {
                    for p in &c.ctor_params {
                        if p.name == field {
                            // Check visibility: Private fields are only accessible
                            // from within the defining class
                            if matches!(p.vis, crate::Vis::Private) {
                                let same_class =
                                    accessor_type.as_ref().is_some_and(|a| a == type_name);
                                if !same_class {
                                    self.diags.error(
                                        DiagCode::PRIVATE_FIELD,
                                        span,
                                        SmolStr::from(format!(
                                            "field `{field}` of `{type_name}` is private"
                                        )),
                                    );
                                    return Ty::Error;
                                }
                            }
                            return tyref_to_ty(&p.ty);
                        }
                    }
                }
                DefKind::DataClass(dc) => {
                    for p in &dc.ctor_params {
                        if p.name == field {
                            if matches!(p.vis, crate::Vis::Private) {
                                let same_class =
                                    accessor_type.as_ref().is_some_and(|a| a == type_name);
                                if !same_class {
                                    self.diags.error(
                                        DiagCode::PRIVATE_FIELD,
                                        span,
                                        SmolStr::from(format!(
                                            "field `{field}` of `{type_name}` is private"
                                        )),
                                    );
                                    return Ty::Error;
                                }
                            }
                            return tyref_to_ty(&p.ty);
                        }
                    }
                }
                _ => {}
            }
        }

        self.diags.error(
            DiagCode::NO_SUCH_FIELD,
            span,
            SmolStr::from(format!("no field `{field}` on type `{type_name}`")),
        );
        Ty::Error
    }

    // -- if expression ------------------------------------------------------

    fn synth_if(&mut self, ife: &valen_ast::IfExpr, expected: Option<&Ty>) -> TypedExpr {
        let cond = self.check_expr(&ife.cond, Some(&Ty::Prim(PrimTy::Bool)));
        let then_body = self.check_block(&ife.then_branch, expected);

        let (else_expr, ty) = if let Some(else_e) = &ife.else_branch {
            let ee = self.check_expr(else_e, expected.or(Some(&then_body.ty)));
            let ty = if then_body.ty == ee.ty {
                then_body.ty.clone()
            } else if then_body.ty.is_error() || ee.ty.is_error() {
                Ty::Error
            } else if then_body.ty == Ty::nothing() {
                ee.ty.clone()
            } else if ee.ty == Ty::nothing() {
                then_body.ty.clone()
            } else {
                self.diags.error(
                    DiagCode::BRANCH_TYPE_MISMATCH,
                    ife.span,
                    SmolStr::from(format!(
                        "if/else branches have incompatible types: `{}` vs `{}`",
                        then_body.ty, ee.ty
                    )),
                );
                Ty::Error
            };
            (Some(Box::new(ee)), ty)
        } else {
            (None, Ty::unit())
        };

        TypedExpr {
            kind: TypedExprKind::If {
                cond: Box::new(cond),
                then_branch: then_body,
                else_branch: else_expr,
            },
            ty,
            span: ife.span,
        }
    }

    // -- match expression ---------------------------------------------------

    fn synth_match(&mut self, me: &valen_ast::MatchExpr, expected: Option<&Ty>) -> TypedExpr {
        let scrutinee = self.infer_expr(&me.scrutinee);
        let mut arms = Vec::new();
        let mut result_ty: Option<Ty> = expected.cloned();

        for arm in &me.arms {
            let guard = arm
                .guard
                .as_ref()
                .map(|g| self.check_expr(g, Some(&Ty::Prim(PrimTy::Bool))));
            let body = if let Some(ref rty) = result_ty {
                self.check_expr(&arm.body, Some(rty))
            } else {
                self.infer_expr(&arm.body)
            };

            if result_ty.is_none() && !body.ty.is_error() {
                result_ty = Some(body.ty.clone());
            }

            arms.push(TypedMatchArm {
                pattern: arm.pattern.clone(),
                guard,
                body,
            });
        }

        let ty = result_ty.unwrap_or(Ty::unit());
        TypedExpr {
            kind: TypedExprKind::Match {
                scrutinee: Box::new(scrutinee),
                arms,
            },
            ty,
            span: me.span,
        }
    }

    // -- assign -------------------------------------------------------------

    fn synth_assign(&mut self, asgn: &valen_ast::AssignExpr) -> TypedExpr {
        let target = self.infer_expr(&asgn.target);

        if let TypedExprKind::LocalVar(ref name) = target.kind {
            if let Some(false) = self.env.is_mutable(name) {
                self.diags.error(
                    DiagCode::IMMUTABLE_ASSIGN,
                    asgn.span,
                    SmolStr::from(format!("cannot assign to immutable variable `{name}`")),
                );
            }
        }

        let rhs = self.check_expr(&asgn.value, Some(&target.ty));

        let value = if let Some(op) = asgn.op {
            let result_ty = self.binary_result_ty(op, &target.ty, &rhs.ty, asgn.span);
            TypedExpr {
                kind: TypedExprKind::Binary {
                    op,
                    lhs: Box::new(target.clone()),
                    rhs: Box::new(rhs),
                },
                ty: result_ty,
                span: asgn.span,
            }
        } else {
            rhs
        };

        TypedExpr {
            kind: TypedExprKind::Assign {
                target: Box::new(target),
                value: Box::new(value),
            },
            ty: Ty::unit(),
            span: asgn.span,
        }
    }

    // -- return / break / continue ------------------------------------------

    fn synth_return(&mut self, ret: &valen_ast::ReturnExpr) -> TypedExpr {
        let value = if let Some(val) = &ret.value {
            let expected = self.return_ty.clone();
            Some(Box::new(self.check_expr(val, expected.as_ref())))
        } else {
            None
        };
        TypedExpr {
            kind: TypedExprKind::Return(value),
            ty: Ty::nothing(),
            span: ret.span,
        }
    }

    fn synth_break(&mut self, brk: &valen_ast::BreakExpr) -> TypedExpr {
        if !self.in_loop {
            self.diags.error(
                DiagCode::BREAK_OUTSIDE_LOOP,
                brk.span,
                SmolStr::from("`break` outside of loop"),
            );
        }
        let value = brk.value.as_ref().map(|v| Box::new(self.infer_expr(v)));
        TypedExpr {
            kind: TypedExprKind::Break(value),
            ty: Ty::nothing(),
            span: brk.span,
        }
    }

    fn synth_continue(&mut self, cont: &valen_ast::ContinueExpr) -> TypedExpr {
        if !self.in_loop {
            self.diags.error(
                DiagCode::CONTINUE_OUTSIDE_LOOP,
                cont.span,
                SmolStr::from("`continue` outside of loop"),
            );
        }
        TypedExpr {
            kind: TypedExprKind::Continue,
            ty: Ty::nothing(),
            span: cont.span,
        }
    }

    // -- loops --------------------------------------------------------------

    fn synth_for(&mut self, f: &valen_ast::ForExpr) -> TypedExpr {
        let iter = self.infer_expr(&f.iter);
        self.env.push_scope();
        let var_ty = match &iter.ty {
            Ty::Generic(name, args) if name == "Range" && !args.is_empty() => {
                if matches!(args[0], Ty::Prim(PrimTy::Float) | Ty::Prim(PrimTy::Double)) {
                    self.diags.warning(
                        DiagCode::TYPE_MISMATCH,
                        f.span,
                        SmolStr::from(
                            "floating-point range loop may produce unexpected results due to precision",
                        ),
                    );
                }
                args[0].clone()
            }
            _ => Ty::Prim(PrimTy::Int),
        };
        self.env.define(f.var.clone(), var_ty, false);
        let prev_loop = self.in_loop;
        self.in_loop = true;
        let body = self.check_block(&f.body, Some(&Ty::unit()));
        self.in_loop = prev_loop;
        self.env.pop_scope();
        TypedExpr {
            kind: TypedExprKind::For {
                var: f.var.clone(),
                iter: Box::new(iter),
                body,
            },
            ty: Ty::unit(),
            span: f.span,
        }
    }

    fn synth_while(&mut self, w: &valen_ast::WhileExpr) -> TypedExpr {
        let cond = self.check_expr(&w.cond, Some(&Ty::Prim(PrimTy::Bool)));
        let prev_loop = self.in_loop;
        self.in_loop = true;
        let body = self.check_block(&w.body, Some(&Ty::unit()));
        self.in_loop = prev_loop;
        TypedExpr {
            kind: TypedExprKind::While {
                cond: Box::new(cond),
                body,
            },
            ty: Ty::unit(),
            span: w.span,
        }
    }

    fn synth_loop(&mut self, l: &valen_ast::LoopExpr) -> TypedExpr {
        let prev_loop = self.in_loop;
        self.in_loop = true;
        let body = self.check_block(&l.body, None);
        self.in_loop = prev_loop;
        TypedExpr {
            kind: TypedExprKind::Loop { body },
            ty: Ty::unit(),
            span: l.span,
        }
    }

    // -- lambda -------------------------------------------------------------

    fn synth_lambda(&mut self, lam: &valen_ast::LambdaExpr, expected: Option<&Ty>) -> TypedExpr {
        let expected_params: Option<&[Ty]> = match expected {
            Some(Ty::Fn(params, _)) => Some(params),
            _ => None,
        };

        self.env.push_scope();
        let mut params = Vec::new();
        for (i, p) in lam.params.iter().enumerate() {
            let ty = if let Some(t) = &p.ty {
                self.resolve_ast_type(t)
            } else if let Some(ep) = expected_params.and_then(|ps| ps.get(i)) {
                ep.clone()
            } else {
                Ty::Error
            };
            self.env.define(p.name.clone(), ty.clone(), false);
            params.push((p.name.clone(), ty));
        }

        let expected_ret = match expected {
            Some(Ty::Fn(_, ret)) => Some(ret.as_ref()),
            _ => None,
        };

        let body = self.check_expr(&lam.body, expected_ret);
        let ret_ty = body.ty.clone();
        self.env.pop_scope();

        let fn_ty = Ty::Fn(
            params.iter().map(|(_, t)| t.clone()).collect(),
            Box::new(ret_ty),
        );

        TypedExpr {
            kind: TypedExprKind::Lambda {
                params,
                body: Box::new(body),
            },
            ty: fn_ty,
            span: lam.span,
        }
    }

    // -- range --------------------------------------------------------------

    fn synth_range(&mut self, r: &valen_ast::RangeExpr) -> TypedExpr {
        let start = r.start.as_ref().map(|s| Box::new(self.infer_expr(s)));
        let end = r.end.as_ref().map(|e| Box::new(self.infer_expr(e)));

        let elem_ty = start
            .as_ref()
            .map(|s| s.ty.clone())
            .or_else(|| end.as_ref().map(|e| e.ty.clone()))
            .unwrap_or(Ty::Prim(PrimTy::Int));

        TypedExpr {
            kind: TypedExprKind::Range {
                start,
                end,
                inclusive: r.inclusive,
            },
            ty: Ty::Generic(SmolStr::from("Range"), vec![elem_ty]),
            span: r.span,
        }
    }

    // -- string interpolation -----------------------------------------------

    fn synth_string_interp(&mut self, si: &valen_ast::StringInterpExpr) -> TypedExpr {
        let parts = si
            .parts
            .iter()
            .map(|part| match part {
                valen_ast::StringInterpPart::Text(t) => TypedStringPart::Text(t.clone()),
                valen_ast::StringInterpPart::Expr(e) => TypedStringPart::Expr(self.infer_expr(e)),
            })
            .collect();
        TypedExpr {
            kind: TypedExprKind::StringInterp(parts),
            ty: Ty::Prim(PrimTy::String),
            span: si.span,
        }
    }

    // -- try (?) / safe — skeleton only -------------------------------------

    fn synth_try(&mut self, t: &valen_ast::TryExpr) -> TypedExpr {
        let inner = self.infer_expr(&t.expr);
        let ty = match &inner.ty {
            Ty::Generic(name, args)
                if (name == "Result" || name == "Option") && !args.is_empty() =>
            {
                args[0].clone()
            }
            _ => inner.ty.clone(),
        };
        TypedExpr {
            kind: TypedExprKind::Error,
            ty,
            span: t.span,
        }
    }

    fn synth_safe(&mut self, s: &valen_ast::SafeExpr) -> TypedExpr {
        let body = self.check_block(&s.block, None);
        let inner_ty = body.ty.clone();
        let result_ty = Ty::Generic(
            SmolStr::from("Result"),
            vec![inner_ty, Ty::Named(SmolStr::from("JavaException"))],
        );
        TypedExpr {
            kind: TypedExprKind::Safe(body),
            ty: result_ty,
            span: s.span,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn is_subtype(sub: &Ty, sup: &Ty) -> bool {
    if sub == sup {
        return true;
    }
    // T is subtype of T? (Nullable<T>)
    if let Ty::Nullable(inner) = sup {
        if sub == inner.as_ref() || is_subtype(sub, inner) {
            return true;
        }
    }
    // Nothing is subtype of everything
    if *sub == Ty::nothing() {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve;
    use valen_ast::FileId;
    use valen_parser::parse;

    fn check_source(src: &str) -> TypeCheckResult {
        let parsed = parse(src, FileId(0));
        assert!(
            !parsed.diagnostics.has_errors(),
            "parse errors: {:?}",
            parsed.diagnostics
        );
        let resolved = resolve::resolve(&parsed.items);
        assert!(
            !resolved.diagnostics.has_errors(),
            "resolve errors: {:?}",
            resolved.diagnostics
        );
        type_check(&resolved.hir, &parsed.items)
    }

    fn assert_no_errors(r: &TypeCheckResult) {
        assert!(
            !r.diagnostics.has_errors(),
            "type errors: {:?}",
            r.diagnostics
        );
    }

    fn get_body_by_name<'a>(r: &'a TypeCheckResult, name: &str) -> &'a TypedBody {
        r.bodies.values().next().unwrap_or_else(|| {
            panic!(
                "no body found for `{name}` — bodies: {:?}",
                r.bodies.keys().collect::<Vec<_>>()
            )
        })
    }

    fn assert_has_error(r: &TypeCheckResult, code: DiagCode) {
        assert!(
            r.diagnostics.iter().any(|d| d.code == code),
            "expected error {:?}, got: {:?}",
            code,
            r.diagnostics
        );
    }

    // -- literal typing -----------------------------------------------------

    #[test]
    fn int_literal() {
        let r = check_source("fn main() -> Int { 42 }");
        assert_no_errors(&r);
        let body = get_body_by_name(&r, "main");
        assert_eq!(body.ty, Ty::Prim(PrimTy::Int));
    }

    #[test]
    fn float_literal() {
        let r = check_source("fn main() -> Double { 3.14 }");
        assert_no_errors(&r);
    }

    #[test]
    fn string_literal() {
        let r = check_source("fn main() -> String { \"hello\" }");
        assert_no_errors(&r);
    }

    #[test]
    fn bool_literal() {
        let r = check_source("fn main() -> Bool { true }");
        assert_no_errors(&r);
    }

    // -- type mismatch ------------------------------------------------------

    #[test]
    fn return_type_mismatch() {
        let r = check_source("fn main() -> Int { true }");
        assert_has_error(&r, DiagCode::TYPE_MISMATCH);
    }

    #[test]
    fn no_implicit_numeric_conversion() {
        let r = check_source("fn compute(x: Int, y: Long) -> Int { x + y }");
        assert_has_error(&r, DiagCode::NO_IMPLICIT_CONVERSION);
    }

    // -- variable resolution ------------------------------------------------

    #[test]
    fn local_variable() {
        let r = check_source("fn main() -> Int { let x = 42; x }");
        assert_no_errors(&r);
    }

    #[test]
    fn undeclared_variable() {
        let r = check_source("fn main() -> Int { y }");
        assert_has_error(&r, DiagCode::UNDECLARED_VAR);
    }

    // -- binary ops ---------------------------------------------------------

    #[test]
    fn arithmetic_same_type() {
        let r = check_source("fn main() -> Int { 1 + 2 }");
        assert_no_errors(&r);
    }

    #[test]
    fn comparison_returns_bool() {
        let r = check_source("fn main() -> Bool { 1 < 2 }");
        assert_no_errors(&r);
    }

    #[test]
    fn logical_and() {
        let r = check_source("fn main() -> Bool { true && false }");
        assert_no_errors(&r);
    }

    // -- if expression ------------------------------------------------------

    #[test]
    fn if_else_same_type() {
        let r = check_source("fn main() -> Int { if true { 1 } else { 2 } }");
        assert_no_errors(&r);
    }

    #[test]
    fn if_else_branch_mismatch() {
        let r = check_source("fn main() -> Int { if true { 1 } else { true } }");
        assert_has_error(&r, DiagCode::BRANCH_TYPE_MISMATCH);
    }

    // -- let with type annotation -------------------------------------------

    #[test]
    fn let_annotation_match() {
        let r = check_source("fn main() -> Int { let x: Int = 42; x }");
        assert_no_errors(&r);
    }

    #[test]
    fn let_annotation_mismatch() {
        let r = check_source("fn main() -> Int { let x: String = 42; x }");
        assert_has_error(&r, DiagCode::TYPE_MISMATCH);
    }

    // -- function call typing -----------------------------------------------

    #[test]
    fn call_typed_fn() {
        let r =
            check_source("fn add(a: Int, b: Int) -> Int { a + b }\nfn main() -> Int { add(1, 2) }");
        assert_no_errors(&r);
    }

    #[test]
    fn call_arg_count_mismatch() {
        let r =
            check_source("fn add(a: Int, b: Int) -> Int { a + b }\nfn main() -> Int { add(1) }");
        assert_has_error(&r, DiagCode::ARG_COUNT_MISMATCH);
    }

    #[test]
    fn call_arg_type_mismatch() {
        let r = check_source(
            "fn add(a: Int, b: Int) -> Int { a + b }\nfn main() -> Int { add(1, true) }",
        );
        assert_has_error(&r, DiagCode::TYPE_MISMATCH);
    }

    // -- method call --------------------------------------------------------

    #[test]
    fn method_call_ok() {
        let r = check_source(
            "class Dog(pub name: String) { fn greet(self) -> String { self.name } }\nfn main() -> String { let d = Dog(\"Rex\"); d.greet() }",
        );
        assert_no_errors(&r);
    }

    // -- field access -------------------------------------------------------

    #[test]
    fn field_access_ok() {
        let r = check_source(
            "class Dog(pub name: String) {}\nfn main() -> String { let d = Dog(\"Rex\"); d.name }",
        );
        assert_no_errors(&r);
    }

    #[test]
    fn field_access_no_field() {
        let r = check_source(
            "class Dog(pub name: String) {}\nfn main() -> String { let d = Dog(\"Rex\"); d.age }",
        );
        assert_has_error(&r, DiagCode::NO_SUCH_FIELD);
    }

    // -- unary ops ----------------------------------------------------------

    #[test]
    fn negate_int() {
        let r = check_source("fn main() -> Int { -42 }");
        assert_no_errors(&r);
    }

    #[test]
    fn not_bool() {
        let r = check_source("fn main() -> Bool { !true }");
        assert_no_errors(&r);
    }

    #[test]
    fn negate_bool_error() {
        let r = check_source("fn main() -> Int { -true }");
        assert_has_error(&r, DiagCode::INVALID_OPERATOR);
    }

    // -- nullable -----------------------------------------------------------

    #[test]
    fn nullable_return() {
        let r = check_source("fn find(id: Int) -> String? { \"found\" }");
        assert_no_errors(&r);
    }

    // -- break/continue outside loop ----------------------------------------

    #[test]
    fn break_outside_loop() {
        let r = check_source("fn main() { break; }");
        assert_has_error(&r, DiagCode::BREAK_OUTSIDE_LOOP);
    }

    #[test]
    fn continue_outside_loop() {
        let r = check_source("fn main() { continue; }");
        assert_has_error(&r, DiagCode::CONTINUE_OUTSIDE_LOOP);
    }
}
