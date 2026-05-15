//! Bidirectional type checker producing typed HIR bodies.

use indexmap::{IndexMap, IndexSet};
use smol_str::SmolStr;
use valen_ast::{self, BinaryOp, Span, UnaryOp};
use valen_diagnostics::{DiagCode, Diagnostics};

use crate::{
    tyref_to_ty, tyref_to_ty_generic, DefId, DefKind, Hir, PrimTy, Ty, TyRef, TypedBody, TypedExpr,
    TypedExprKind, TypedMatchArm, TypedStmt, TypedStringPart,
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
    type_params: IndexSet<SmolStr>,
    type_param_bounds: IndexMap<SmolStr, Vec<SmolStr>>,
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
            type_params: IndexSet::new(),
            type_param_bounds: IndexMap::new(),
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

    /// Extract the type name from an AST impl target.
    fn impl_target_name(&self, target: &valen_ast::Type) -> Option<SmolStr> {
        match target {
            valen_ast::Type::Path(tp) if !tp.segments.is_empty() => {
                Some(tp.segments[0].name.clone())
            }
            _ => None,
        }
    }

    /// Look up a method def ID specifically for an impl block, matching by target
    /// type and optionally by trait reference to avoid cross-impl collisions.
    fn lookup_impl_method_def_id(
        &self,
        target_name: Option<&str>,
        trait_ref: Option<&valen_ast::Type>,
        method_name: &str,
    ) -> Option<DefId> {
        let trait_name: Option<SmolStr> = trait_ref.and_then(|t| match t {
            valen_ast::Type::Path(tp) if !tp.segments.is_empty() => {
                Some(tp.segments[0].name.clone())
            }
            _ => None,
        });

        for def in self.hir.defs.values() {
            if let DefKind::Impl(imp) = &def.kind {
                // Match target type name
                let imp_target = match &imp.target {
                    TyRef::Named(n) => Some(n.as_str()),
                    // TODO: support Prim target matching (e.g. `impl Trait for Int`)
                    TyRef::Prim(_) => None,
                    _ => None,
                };

                let target_matches = match (target_name, imp_target) {
                    (Some(tn), Some(it)) => tn == it,
                    _ => target_name.is_none(),
                };

                // Also check trait match if available
                let trait_matches = match (&trait_name, &imp.trait_ref) {
                    (Some(tn), TyRef::Named(n)) => tn == n,
                    (Some(tn), TyRef::Generic(n, _)) => tn.as_str() == n.as_str(),
                    (None, _) => true,
                    _ => false,
                };

                if target_matches && trait_matches {
                    for &mid in &imp.methods {
                        if let Some(mdef) = self.hir.defs.get(&mid) {
                            if mdef.name == method_name {
                                return Some(mid);
                            }
                        }
                    }
                }
            }
        }
        // Fallback: try the general lookup with the actual class name
        if let Some(tn) = target_name {
            return self.lookup_method_def_id(tn, method_name);
        }
        None
    }

    fn register_top_level_types(&mut self, items: &[valen_ast::Item]) {
        for item in items {
            match item {
                valen_ast::Item::Fn(f) => {
                    let prev = std::mem::take(&mut self.type_params);
                    for g in &f.generics {
                        self.type_params.insert(g.name.clone());
                    }
                    let ty = self.fn_decl_ty(f);
                    self.type_params = prev;
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
                    if self.type_params.contains(&seg.name) {
                        return Ty::TypeParam(seg.name.clone());
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
            valen_ast::Type::Tuple(..) => Ty::Error,
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

        let prev_type_params = self.type_params.clone();
        let prev_bounds = self.type_param_bounds.clone();
        for g in &f.generics {
            self.type_params.insert(g.name.clone());
            let bounds: Vec<SmolStr> = g
                .bounds
                .iter()
                .filter_map(|b| {
                    if let valen_ast::Type::Path(tp) = b {
                        if tp.segments.len() == 1 {
                            return Some(tp.segments[0].name.clone());
                        }
                    }
                    None
                })
                .collect();
            if !bounds.is_empty() {
                self.type_param_bounds.insert(g.name.clone(), bounds);
            }
        }

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
        self.type_params = prev_type_params;
        self.type_param_bounds = prev_bounds;

        if let Some(id) = def_id {
            self.bodies.insert(id, typed_body);
        }
    }

    fn check_class(&mut self, c: &valen_ast::ClassDecl) {
        let prev_type_params = std::mem::take(&mut self.type_params);
        let prev_bounds = std::mem::take(&mut self.type_param_bounds);
        for g in &c.generics {
            self.type_params.insert(g.name.clone());
            let bounds: Vec<SmolStr> = g
                .bounds
                .iter()
                .filter_map(|b| {
                    if let valen_ast::Type::Path(tp) = b {
                        if tp.segments.len() == 1 {
                            return Some(tp.segments[0].name.clone());
                        }
                    }
                    None
                })
                .collect();
            if !bounds.is_empty() {
                self.type_param_bounds.insert(g.name.clone(), bounds);
            }
        }
        let self_ty = Ty::Named(c.name.clone());
        for member in &c.body {
            if let valen_ast::ClassMember::Method(m) = member {
                let def_id = self.lookup_method_def_id(&c.name, &m.name);
                self.check_fn_decl(m, Some(&self_ty), def_id);
            }
        }
        self.type_params = prev_type_params;
        self.type_param_bounds = prev_bounds;
    }

    fn check_impl(&mut self, imp: &valen_ast::ImplBlock) {
        let prev_type_params = std::mem::take(&mut self.type_params);
        let prev_bounds = std::mem::take(&mut self.type_param_bounds);
        for g in &imp.generics {
            self.type_params.insert(g.name.clone());
            let bounds: Vec<SmolStr> = g
                .bounds
                .iter()
                .filter_map(|b| {
                    if let valen_ast::Type::Path(tp) = b {
                        if tp.segments.len() == 1 {
                            return Some(tp.segments[0].name.clone());
                        }
                    }
                    None
                })
                .collect();
            if !bounds.is_empty() {
                self.type_param_bounds.insert(g.name.clone(), bounds);
            }
        }
        let self_ty = self.resolve_ast_type(&imp.target);
        let target_type_name = self.impl_target_name(&imp.target);
        for item in &imp.items {
            if let valen_ast::ImplItem::Fn(m) = item {
                let def_id = self.lookup_impl_method_def_id(
                    target_type_name.as_deref(),
                    imp.trait_ref.as_ref(),
                    &m.name,
                );
                self.check_fn_decl(m, Some(&self_ty), def_id);
            }
        }
        self.type_params = prev_type_params;
        self.type_param_bounds = prev_bounds;
    }

    fn check_trait(&mut self, t: &valen_ast::TraitDecl) {
        let prev_type_params = std::mem::take(&mut self.type_params);
        for g in &t.generics {
            self.type_params.insert(g.name.clone());
        }
        let self_ty = Ty::Named(t.name.clone());
        for item in &t.items {
            if let valen_ast::TraitItem::Fn(f) = item {
                if f.body.is_some() {
                    let def_id = self.lookup_method_def_id(&t.name, &f.name);
                    self.check_fn_decl(f, Some(&self_ty), def_id);
                }
            }
        }
        self.type_params = prev_type_params;
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
                    has_annotation: ls.ty.is_some(),
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
        let first = &path.segments[0].name;
        if path.segments.len() == 2 {
            let second = &path.segments[1].name;

            // Enum::Variant — resolve as enum variant path
            if let Some(enum_def) = self
                .hir
                .defs
                .values()
                .find(|d| d.name == *first && matches!(d.kind, DefKind::Enum(_)))
            {
                if let DefKind::Enum(edef) = &enum_def.kind {
                    if let Some(variant) = edef.variants.iter().find(|v| v.name == *second) {
                        if variant.fields.is_empty() {
                            return TypedExpr {
                                kind: TypedExprKind::Call {
                                    callee: Box::new(TypedExpr {
                                        kind: TypedExprKind::LocalVar(SmolStr::from(format!(
                                            "{first}::{second}"
                                        ))),
                                        ty: Ty::Named(first.clone()),
                                        span: path.span,
                                    }),
                                    args: vec![],
                                },
                                ty: Ty::Named(first.clone()),
                                span: path.span,
                            };
                        }
                        // Record variant — return Named so synth_call handles arg checking
                        return TypedExpr {
                            kind: TypedExprKind::LocalVar(SmolStr::from(format!(
                                "{first}::{second}"
                            ))),
                            ty: Ty::Named(first.clone()),
                            span: path.span,
                        };
                    } else {
                        self.diags.error(
                            DiagCode::NAME_NOT_FOUND,
                            path.span,
                            SmolStr::from(format!("enum `{first}` has no variant `{second}`")),
                        );
                        return TypedExpr {
                            kind: TypedExprKind::Error,
                            ty: Ty::Error,
                            span: path.span,
                        };
                    }
                }
            }

            // Class::method — return Named so synth_call handles arg checking
            return TypedExpr {
                kind: TypedExprKind::LocalVar(SmolStr::from(format!("{first}::{second}"))),
                ty: Ty::Named(first.clone()),
                span: path.span,
            };
        }
        // Fallback for non-enum multi-segment paths
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

        if let Some((trait_method, output_ty)) =
            self.resolve_binary_op_trait(bin.op, &lhs.ty, &rhs.ty)
        {
            return TypedExpr {
                kind: TypedExprKind::MethodCall {
                    receiver: Box::new(lhs),
                    method: trait_method,
                    args: vec![rhs],
                },
                ty: output_ty,
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

    fn op_to_trait_name(op: BinaryOp) -> Option<&'static str> {
        match op {
            BinaryOp::Add => Some("Add"),
            BinaryOp::Sub => Some("Sub"),
            BinaryOp::Mul => Some("Mul"),
            BinaryOp::Div => Some("Div"),
            BinaryOp::Rem => Some("Rem"),
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => Some("Ord"),
            BinaryOp::Eq | BinaryOp::Ne => Some("Eq"),
            _ => None,
        }
    }

    fn op_to_method_name(op: BinaryOp) -> Option<&'static str> {
        match op {
            BinaryOp::Add => Some("add"),
            BinaryOp::Sub => Some("sub"),
            BinaryOp::Mul => Some("mul"),
            BinaryOp::Div => Some("div"),
            BinaryOp::Rem => Some("rem"),
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => Some("cmp"),
            BinaryOp::Eq | BinaryOp::Ne => Some("eq"),
            _ => None,
        }
    }

    fn resolve_binary_op_trait(&self, op: BinaryOp, lhs: &Ty, _rhs: &Ty) -> Option<(SmolStr, Ty)> {
        if lhs.is_numeric() || lhs.is_bool() || matches!(lhs, Ty::Prim(PrimTy::String)) {
            return None;
        }

        let trait_name = Self::op_to_trait_name(op)?;
        let method_name = Self::op_to_method_name(op)?;

        let lhs_name = match lhs {
            Ty::Named(n) => n.as_str(),
            _ => return None,
        };

        for entry in &self.hir.trait_impls {
            if entry.trait_name == trait_name && entry.target_name == lhs_name {
                let impl_def = self.hir.defs.values().find(|d| {
                    if let DefKind::Impl(imp) = &d.kind {
                        let tname = match &imp.trait_ref {
                            TyRef::Named(n) => n.as_str(),
                            _ => "",
                        };
                        let target = match &imp.target {
                            TyRef::Named(n) => n.as_str(),
                            _ => "",
                        };
                        tname == trait_name && target == lhs_name
                    } else {
                        false
                    }
                });

                let output_ty = if let Some(def) = impl_def {
                    if let DefKind::Impl(imp) = &def.kind {
                        imp.associated_types
                            .iter()
                            .find(|(n, _)| n == "Output")
                            .map(|(_, tyref)| tyref_to_ty(tyref))
                            .unwrap_or_else(|| lhs.clone())
                    } else {
                        lhs.clone()
                    }
                } else {
                    lhs.clone()
                };

                let final_ty = match op {
                    BinaryOp::Lt
                    | BinaryOp::Le
                    | BinaryOp::Gt
                    | BinaryOp::Ge
                    | BinaryOp::Eq
                    | BinaryOp::Ne => Ty::Prim(PrimTy::Bool),
                    _ => output_ty,
                };

                return Some((SmolStr::from(method_name), final_ty));
            }
        }

        None
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
                let min_args = self.min_required_args_for_callee(&call.callee, param_tys.len());
                if args.len() < min_args || args.len() > param_tys.len() {
                    self.diags.error(
                        DiagCode::ARG_COUNT_MISMATCH,
                        call.span,
                        SmolStr::from(if min_args == param_tys.len() {
                            format!(
                                "expected {} argument(s), found {}",
                                param_tys.len(),
                                args.len()
                            )
                        } else {
                            format!(
                                "expected {}-{} argument(s), found {}",
                                min_args,
                                param_tys.len(),
                                args.len()
                            )
                        }),
                    );
                } else {
                    let has_type_params = param_tys.iter().any(|t| matches!(t, Ty::TypeParam(_)));
                    if has_type_params {
                        let bindings = infer_type_bindings(param_tys, &args);
                        for (arg, expected) in args.iter().zip(param_tys.iter()) {
                            let resolved = substitute_ty(expected, &bindings);
                            if !arg.ty.is_error()
                                && !resolved.is_error()
                                && arg.ty != resolved
                                && !is_subtype(&arg.ty, &resolved)
                            {
                                self.diags.error(
                                    DiagCode::TYPE_MISMATCH,
                                    arg.span,
                                    SmolStr::from(format!(
                                        "expected `{resolved}`, found `{}`",
                                        arg.ty
                                    )),
                                );
                            }
                        }
                    } else {
                        for (arg, expected) in args.iter().zip(param_tys.iter()) {
                            if !arg.ty.is_error()
                                && !expected.is_error()
                                && arg.ty != *expected
                                && !is_subtype(&arg.ty, expected)
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
                }
                let bindings = infer_type_bindings(param_tys, &args);
                if !bindings.is_empty() {
                    self.check_call_site_bounds(&call.callee, &bindings, call.span);
                }
                let ret = substitute_ty(ret_ty, &bindings);
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
                // Check for 2-segment path calls (Enum::Variant or Class::method)
                if let valen_ast::Expr::Path(path) = &*call.callee {
                    if path.segments.len() == 2 {
                        let class_name = &path.segments[0].name;
                        let member_name = &path.segments[1].name;

                        // Enum::Variant(args) — check variant fields
                        if let Some(result) = self.resolve_enum_variant_call(
                            class_name,
                            member_name,
                            &callee,
                            &args,
                            call.span,
                        ) {
                            return result;
                        }

                        // Class::method(args) — check associated function params
                        if let Some(result) = self.resolve_associated_fn_call(
                            class_name,
                            member_name,
                            &callee,
                            &args,
                            call.span,
                        ) {
                            return result;
                        }
                    }
                }

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
                    let min_args = c.ctor_params.iter().filter(|p| !p.has_default).count();
                    if args.len() < min_args || args.len() > c.ctor_params.len() {
                        self.diags.error(
                            DiagCode::ARG_COUNT_MISMATCH,
                            span,
                            SmolStr::from(format!(
                                "`{name}` constructor expects {}-{} argument(s), found {}",
                                min_args,
                                c.ctor_params.len(),
                                args.len()
                            )),
                        );
                    } else {
                        let param_tys: Vec<Ty> = c
                            .ctor_params
                            .iter()
                            .map(|p| tyref_to_ty_generic(&p.ty))
                            .collect();
                        let has_tp = param_tys.iter().any(|t| matches!(t, Ty::TypeParam(_)));
                        if has_tp {
                            let bindings = infer_type_bindings(&param_tys, args);
                            for (arg, expected) in args.iter().zip(param_tys.iter()) {
                                let resolved = substitute_ty(expected, &bindings);
                                if !arg.ty.is_error()
                                    && !resolved.is_error()
                                    && arg.ty != resolved
                                    && !is_subtype(&arg.ty, &resolved)
                                {
                                    self.diags.error(
                                        DiagCode::TYPE_MISMATCH,
                                        arg.span,
                                        SmolStr::from(format!(
                                            "expected `{resolved}`, found `{}`",
                                            arg.ty
                                        )),
                                    );
                                }
                            }
                            if !bindings.is_empty() {
                                return Ty::Generic(
                                    name.clone(),
                                    bindings.values().cloned().collect(),
                                );
                            }
                        } else {
                            for (arg, expected) in args.iter().zip(param_tys.iter()) {
                                if !arg.ty.is_error() && !expected.is_error() && arg.ty != *expected
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
                    }
                    return Ty::Named(name.clone());
                }
                DefKind::DataClass(dc) => {
                    let min_args = dc.ctor_params.iter().filter(|p| !p.has_default).count();
                    if args.len() < min_args || args.len() > dc.ctor_params.len() {
                        self.diags.error(
                            DiagCode::ARG_COUNT_MISMATCH,
                            span,
                            SmolStr::from(format!(
                                "`{name}` constructor expects {}-{} argument(s), found {}",
                                min_args,
                                dc.ctor_params.len(),
                                args.len()
                            )),
                        );
                    } else {
                        for (arg, param) in args.iter().zip(dc.ctor_params.iter()) {
                            let expected = tyref_to_ty_generic(&param.ty);
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

    fn resolve_enum_variant_call(
        &mut self,
        enum_name: &SmolStr,
        variant_name: &SmolStr,
        callee: &TypedExpr,
        args: &[TypedExpr],
        span: Span,
    ) -> Option<TypedExpr> {
        let enum_def = self
            .hir
            .defs
            .values()
            .find(|d| d.name == *enum_name && matches!(d.kind, DefKind::Enum(_)))?;
        let edef = match &enum_def.kind {
            DefKind::Enum(e) => e,
            _ => return None,
        };
        let variant = edef.variants.iter().find(|v| v.name == *variant_name)?;
        if variant.fields.is_empty() {
            return None;
        }

        let field_tys: Vec<Ty> = variant
            .fields
            .iter()
            .map(|(_, tyref)| tyref_to_ty_generic(tyref))
            .collect();

        if args.len() != field_tys.len() {
            self.diags.error(
                DiagCode::ARG_COUNT_MISMATCH,
                span,
                SmolStr::from(format!(
                    "`{enum_name}::{variant_name}` expects {} argument(s), found {}",
                    field_tys.len(),
                    args.len()
                )),
            );
        } else {
            for (arg, expected) in args.iter().zip(field_tys.iter()) {
                let resolved = if field_tys.iter().any(|t| matches!(t, Ty::TypeParam(_))) {
                    let bindings = infer_type_bindings(&field_tys, args);
                    substitute_ty(expected, &bindings)
                } else {
                    expected.clone()
                };
                if !arg.ty.is_error()
                    && !resolved.is_error()
                    && arg.ty != resolved
                    && !is_subtype(&arg.ty, &resolved)
                {
                    self.diags.error(
                        DiagCode::TYPE_MISMATCH,
                        arg.span,
                        SmolStr::from(format!("expected `{resolved}`, found `{}`", arg.ty)),
                    );
                }
            }
        }

        let bindings = infer_type_bindings(&field_tys, args);
        let type_params: Vec<Ty> = collect_type_params_ordered(&field_tys);
        let ret_ty = if type_params.is_empty() {
            Ty::Named(enum_name.clone())
        } else {
            let resolved_params: Vec<Ty> = type_params
                .iter()
                .map(|p| substitute_ty(p, &bindings))
                .collect();
            Ty::Generic(enum_name.clone(), resolved_params)
        };

        Some(TypedExpr {
            kind: TypedExprKind::Call {
                callee: Box::new(callee.clone()),
                args: args.to_vec(),
            },
            ty: ret_ty,
            span,
        })
    }

    fn resolve_associated_fn_call(
        &mut self,
        class_name: &SmolStr,
        method_name: &SmolStr,
        callee: &TypedExpr,
        args: &[TypedExpr],
        span: Span,
    ) -> Option<TypedExpr> {
        let mid = self.lookup_method_def_id(class_name, method_name)?;
        let method_def = self.hir.defs.get(&mid)?;
        let fn_def = match &method_def.kind {
            DefKind::Fn(f) => f,
            _ => return None,
        };
        if fn_def.params.iter().any(|p| p.is_self) {
            return None;
        }

        let param_tys: Vec<Ty> = fn_def
            .params
            .iter()
            .map(|p| tyref_to_ty_generic(&p.ty))
            .collect();
        let ret_ty = fn_def
            .return_ty
            .as_ref()
            .map(tyref_to_ty_generic)
            .unwrap_or_else(Ty::unit);

        let min_args = fn_def.params.iter().filter(|p| !p.has_default).count();
        if args.len() < min_args || args.len() > param_tys.len() {
            self.diags.error(
                DiagCode::ARG_COUNT_MISMATCH,
                span,
                SmolStr::from(if min_args == param_tys.len() {
                    format!(
                        "expected {} argument(s), found {}",
                        param_tys.len(),
                        args.len()
                    )
                } else {
                    format!(
                        "expected {}-{} argument(s), found {}",
                        min_args,
                        param_tys.len(),
                        args.len()
                    )
                }),
            );
        } else {
            for (arg, expected) in args.iter().zip(param_tys.iter()) {
                if !arg.ty.is_error()
                    && !expected.is_error()
                    && arg.ty != *expected
                    && !is_subtype(&arg.ty, expected)
                {
                    self.diags.error(
                        DiagCode::TYPE_MISMATCH,
                        arg.span,
                        SmolStr::from(format!("expected `{expected}`, found `{}`", arg.ty)),
                    );
                }
            }
        }

        let bindings = infer_type_bindings(&param_tys, args);
        let resolved_ret = substitute_ty(&ret_ty, &bindings);

        Some(TypedExpr {
            kind: TypedExprKind::Call {
                callee: Box::new(callee.clone()),
                args: args.to_vec(),
            },
            ty: resolved_ret,
            span,
        })
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

        let (type_name, generic_args) = match &receiver.ty {
            Ty::Named(n) => (Some(n.clone()), vec![]),
            Ty::Generic(n, args) => (Some(n.clone()), args.clone()),
            Ty::Prim(p) => (Some(SmolStr::from(format!("{p}"))), vec![]),
            Ty::TypeParam(_) => (None, vec![]),
            _ => (None, vec![]),
        };

        if let Some(tn) = &type_name {
            let resolution = self.hir.resolve_method(tn, &mc.method);
            match resolution {
                crate::MethodResolution::Found(def_id) => {
                    if let Some(def) = self.hir.defs.get(&def_id) {
                        if let DefKind::Fn(fdef) = &def.kind {
                            let raw_ret_ty = fdef
                                .return_ty
                                .as_ref()
                                .map(tyref_to_ty_generic)
                                .unwrap_or_else(Ty::unit);
                            let ret_ty = if !generic_args.is_empty() {
                                let bindings = self.build_class_type_bindings(tn, &generic_args);
                                substitute_ty(&raw_ret_ty, &bindings)
                            } else {
                                raw_ret_ty
                            };

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
                                    let expected = tyref_to_ty_generic(&param.ty);
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
        } else if let Ty::TypeParam(tp_name) = &receiver.ty {
            // Object methods on type parameters
            let obj_ret = match mc.method.as_str() {
                "toString" if args.is_empty() => Some(Ty::Prim(PrimTy::String)),
                "hashCode" if args.is_empty() => Some(Ty::Prim(PrimTy::Int)),
                "equals" if args.len() == 1 => Some(Ty::Prim(PrimTy::Bool)),
                _ => None,
            };
            // Try bounds-based method resolution
            let bounds_ret = if obj_ret.is_none() {
                self.resolve_type_param_method(tp_name, &mc.method, &args)
            } else {
                None
            };
            let ret_ty = obj_ret.or(bounds_ret).unwrap_or_else(|| {
                self.diags.error(
                    DiagCode::NO_SUCH_METHOD,
                    mc.span,
                    SmolStr::from(format!(
                        "cannot call method `{}` on type parameter `{}`",
                        mc.method, tp_name
                    )),
                );
                Ty::Error
            });
            return TypedExpr {
                kind: TypedExprKind::MethodCall {
                    receiver: Box::new(receiver),
                    method: mc.method.clone(),
                    args,
                },
                ty: ret_ty,
                span: mc.span,
            };
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

    fn resolve_type_param_method(
        &self,
        tp_name: &SmolStr,
        method: &SmolStr,
        _args: &[TypedExpr],
    ) -> Option<Ty> {
        let bounds = self.type_param_bounds.get(tp_name)?;
        for bound_trait in bounds {
            for def in self.hir.defs.values() {
                if def.name == *bound_trait {
                    if let DefKind::Trait(tdef) = &def.kind {
                        for &mid in &tdef.methods {
                            if let Some(mdef) = self.hir.defs.get(&mid) {
                                if mdef.name == *method {
                                    if let DefKind::Fn(fdef) = &mdef.kind {
                                        return Some(
                                            fdef.return_ty
                                                .as_ref()
                                                .map(tyref_to_ty_generic)
                                                .unwrap_or_else(Ty::unit),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn build_class_type_bindings(
        &self,
        class_name: &SmolStr,
        type_args: &[Ty],
    ) -> IndexMap<SmolStr, Ty> {
        let mut param_names = Vec::new();
        for def in self.hir.defs.values() {
            if def.name == *class_name {
                match &def.kind {
                    DefKind::Class(c) => {
                        for p in &c.ctor_params {
                            if let TyRef::Unresolved(n) = &p.ty {
                                if !param_names.contains(n) {
                                    param_names.push(n.clone());
                                }
                            }
                        }
                    }
                    DefKind::DataClass(dc) => {
                        for p in &dc.ctor_params {
                            if let TyRef::Unresolved(n) = &p.ty {
                                if !param_names.contains(n) {
                                    param_names.push(n.clone());
                                }
                            }
                        }
                    }
                    _ => {}
                }
                break;
            }
        }
        param_names
            .into_iter()
            .zip(type_args.iter().cloned())
            .collect()
    }

    fn check_call_site_bounds(
        &mut self,
        callee: &valen_ast::Expr,
        bindings: &IndexMap<SmolStr, Ty>,
        span: Span,
    ) {
        let fn_name = if let valen_ast::Expr::Path(path) = callee {
            if path.segments.len() == 1 {
                Some(path.segments[0].name.clone())
            } else {
                None
            }
        } else {
            None
        };
        let Some(fn_name) = fn_name else { return };

        for def in self.hir.defs.values() {
            if def.name != fn_name {
                continue;
            }
            if let DefKind::Fn(fn_def) = &def.kind {
                for (tp_name, bounds) in &fn_def.generic_bounds {
                    if let Some(actual_ty) = bindings.get(tp_name) {
                        for bound_trait in bounds {
                            if !self.type_satisfies_bound(actual_ty, bound_trait) {
                                self.diags.error(
                                    DiagCode::BOUND_NOT_SATISFIED,
                                    span,
                                    SmolStr::from(format!(
                                        "type `{actual_ty}` does not satisfy bound `{bound_trait}` required by type parameter `{tp_name}`"
                                    )),
                                );
                            }
                        }
                    }
                }
            }
            break;
        }
    }

    fn type_satisfies_bound(&self, ty: &Ty, trait_name: &SmolStr) -> bool {
        let type_name = match ty {
            Ty::Named(n) => n.clone(),
            Ty::Prim(p) => SmolStr::from(format!("{p}")),
            _ => return false,
        };
        self.hir
            .trait_impls
            .iter()
            .any(|entry| entry.trait_name == *trait_name && entry.target_name == type_name)
    }

    fn min_required_args_for_callee(&self, callee: &valen_ast::Expr, total: usize) -> usize {
        if let valen_ast::Expr::Path(path) = callee {
            if path.segments.len() == 1 {
                let name = &path.segments[0].name;
                for def in self.hir.defs.values() {
                    if def.name == *name {
                        if let DefKind::Fn(fn_def) = &def.kind {
                            return fn_def
                                .params
                                .iter()
                                .filter(|p| !p.is_self && !p.has_default)
                                .count();
                        }
                    }
                }
            }
        }
        total
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
                            return tyref_to_ty_generic(&p.ty);
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
                            return tyref_to_ty_generic(&p.ty);
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
            // Issue #019: Extract element type from generic collections (e.g. List<String>).
            Ty::Generic(name, args) if !args.is_empty() => {
                // For generic types like List<T>, Array<T>, Set<T>, the element type
                // is the first type argument. If the type implements Iterator, the
                // element type would come from Iterator::next -> Option<T>.
                // As a reasonable approximation, use the first type argument.
                args[0].clone()
            }
            Ty::Error => Ty::Error,
            other => {
                // Issue #019: Unknown iterable type — report diagnostic instead of
                // silently defaulting to Int.
                self.diags.error(
                    DiagCode::FOR_LOOP_UNKNOWN_ELEM,
                    f.span,
                    SmolStr::from(format!(
                        "cannot iterate over `{other}`; expected `Range<T>` or a generic collection"
                    )),
                );
                Ty::Error
            }
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
        let (ty, is_option) = match &inner.ty {
            Ty::Generic(name, args) if name == "Option" && !args.is_empty() => {
                (args[0].clone(), true)
            }
            Ty::Generic(name, args) if name == "Result" && !args.is_empty() => {
                (args[0].clone(), false)
            }
            Ty::Error => (Ty::Error, false),
            _ => {
                // Issue #018: `?` operator is only valid on Option<T> or Result<T, E>.
                self.diags.error(
                    DiagCode::TRY_INVALID_TYPE,
                    t.span,
                    SmolStr::from(format!(
                        "`?` operator requires `Option<T>` or `Result<T, E>`, found `{}`",
                        inner.ty
                    )),
                );
                (Ty::Error, false)
            }
        };
        TypedExpr {
            kind: TypedExprKind::Try {
                inner: Box::new(inner),
                is_option,
            },
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
    // Any concrete type is compatible with a TypeParam (at call site)
    if matches!(sup, Ty::TypeParam(_)) {
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

fn infer_type_bindings(param_tys: &[Ty], args: &[TypedExpr]) -> IndexMap<SmolStr, Ty> {
    let mut bindings = IndexMap::new();
    for (param_ty, arg) in param_tys.iter().zip(args.iter()) {
        collect_bindings(param_ty, &arg.ty, &mut bindings);
    }
    bindings
}

fn collect_bindings(param_ty: &Ty, arg_ty: &Ty, bindings: &mut IndexMap<SmolStr, Ty>) {
    match param_ty {
        Ty::TypeParam(name) if !arg_ty.is_error() => {
            bindings
                .entry(name.clone())
                .or_insert_with(|| arg_ty.clone());
        }
        Ty::Generic(_, param_args) => {
            if let Ty::Generic(_, arg_args) = arg_ty {
                for (p, a) in param_args.iter().zip(arg_args.iter()) {
                    collect_bindings(p, a, bindings);
                }
            }
        }
        Ty::Nullable(inner) => {
            if let Ty::Nullable(arg_inner) = arg_ty {
                collect_bindings(inner, arg_inner, bindings);
            }
        }
        Ty::Fn(p_params, p_ret) => {
            if let Ty::Fn(a_params, a_ret) = arg_ty {
                for (p, a) in p_params.iter().zip(a_params.iter()) {
                    collect_bindings(p, a, bindings);
                }
                collect_bindings(p_ret, a_ret, bindings);
            }
        }
        _ => {}
    }
}

fn substitute_ty(ty: &Ty, bindings: &IndexMap<SmolStr, Ty>) -> Ty {
    match ty {
        Ty::TypeParam(name) => bindings.get(name).cloned().unwrap_or_else(|| ty.clone()),
        Ty::Generic(name, args) => Ty::Generic(
            name.clone(),
            args.iter().map(|a| substitute_ty(a, bindings)).collect(),
        ),
        Ty::Nullable(inner) => Ty::Nullable(Box::new(substitute_ty(inner, bindings))),
        Ty::Fn(params, ret) => Ty::Fn(
            params.iter().map(|p| substitute_ty(p, bindings)).collect(),
            Box::new(substitute_ty(ret, bindings)),
        ),
        _ => ty.clone(),
    }
}

fn collect_type_params_ordered(tys: &[Ty]) -> Vec<Ty> {
    let mut seen = IndexMap::new();
    for ty in tys {
        collect_type_params_inner(ty, &mut seen);
    }
    seen.into_keys().map(Ty::TypeParam).collect()
}

fn collect_type_params_inner(ty: &Ty, seen: &mut IndexMap<SmolStr, ()>) {
    match ty {
        Ty::TypeParam(name) => {
            seen.entry(name.clone()).or_insert(());
        }
        Ty::Generic(_, args) => {
            for a in args {
                collect_type_params_inner(a, seen);
            }
        }
        Ty::Nullable(inner) => collect_type_params_inner(inner, seen),
        Ty::Fn(params, ret) => {
            for p in params {
                collect_type_params_inner(p, seen);
            }
            collect_type_params_inner(ret, seen);
        }
        _ => {}
    }
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
    fn call_with_default_args_omitted() {
        let r = check_source(
            "fn greet(msg: String, count: Int = 1) -> String { msg }\nfn main() -> String { greet(\"hi\") }",
        );
        assert_no_errors(&r);
    }

    #[test]
    fn call_with_default_args_provided() {
        let r = check_source(
            "fn greet(msg: String, count: Int = 1) -> String { msg }\nfn main() -> String { greet(\"hi\", 3) }",
        );
        assert_no_errors(&r);
    }

    #[test]
    fn call_with_default_args_too_few() {
        let r = check_source(
            "fn greet(msg: String, count: Int = 1) -> String { msg }\nfn main() -> String { greet() }",
        );
        assert_has_error(&r, DiagCode::ARG_COUNT_MISMATCH);
    }

    #[test]
    fn call_arg_type_mismatch() {
        let r = check_source(
            "fn add(a: Int, b: Int) -> Int { a + b }\nfn main() -> Int { add(1, true) }",
        );
        assert_has_error(&r, DiagCode::TYPE_MISMATCH);
    }

    // -- generics type checking -----------------------------------------------

    #[test]
    fn generic_identity() {
        let r = check_source("fn identity<T>(x: T) -> T { x }\nfn main() -> Int { identity(42) }");
        assert_no_errors(&r);
    }

    #[test]
    fn generic_two_same_params() {
        let r =
            check_source("fn first<T>(x: T, y: T) -> T { x }\nfn main() -> Int { first(1, 2) }");
        assert_no_errors(&r);
    }

    #[test]
    fn generic_type_mismatch() {
        let r = check_source(
            "fn first<T>(x: T, y: T) -> T { x }\nfn main() -> Int { first(1, \"hi\") }",
        );
        assert_has_error(&r, DiagCode::TYPE_MISMATCH);
    }

    #[test]
    fn generic_class_ctor_and_method() {
        let r = check_source(
            "class Box<T>(pub value: T) {\n    fn get(self) -> T { self.value }\n}\nfn main() -> Int {\n    let b = Box(42);\n    b.get()\n}",
        );
        assert_no_errors(&r);
    }

    #[test]
    fn generic_bounds_satisfied() {
        let r = check_source(
            "trait Show { fn show(self) -> String; }\nclass Dog {}\nimpl Show for Dog { fn show(self) -> String { \"Dog\" } }\nfn display<T: Show>(x: T) -> String { x.show() }\nfn main() -> String { display(Dog()) }",
        );
        assert_no_errors(&r);
    }

    #[test]
    fn generic_bounds_not_satisfied() {
        let r = check_source(
            "trait Show { fn show(self) -> String; }\nclass Cat {}\nfn display<T: Show>(x: T) -> String { x.show() }\nfn main() -> String { display(Cat()) }",
        );
        assert_has_error(&r, DiagCode::BOUND_NOT_SATISFIED);
    }

    #[test]
    fn type_param_object_method() {
        let r = check_source("fn describe<T>(x: T) -> String { x.toString() }");
        assert_no_errors(&r);
    }

    #[test]
    fn type_param_bound_method_in_class() {
        let r = check_source(
            r#"
            trait Shape { fn area(self) -> Float; }
            class Wrapper<T: Shape>() {
                fn getArea(shape: T) -> Float {
                    shape.area()
                }
            }
            "#,
        );
        assert_no_errors(&r);
    }

    #[test]
    fn type_param_bound_method_in_impl() {
        let r = check_source(
            r#"
            trait Shape { fn area(self) -> Float; }
            trait HasArea { fn computeArea(self) -> Float; }
            class Box() {}
            impl<T: Shape> HasArea for Box {
                fn computeArea(self) -> Float { 0.0f }
            }
            "#,
        );
        assert_no_errors(&r);
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

    // -- enum variant call with generics (#064) ---

    #[test]
    fn enum_variant_call_field_types_checked() {
        let r = check_source(
            r#"
            enum Shape {
                Circle(r: Double),
                Point
            }
            fn make() -> Shape { Shape::Circle(3.14) }
            "#,
        );
        assert_no_errors(&r);
    }

    #[test]
    fn enum_variant_call_arg_count_mismatch() {
        let r = check_source(
            r#"
            enum Shape {
                Circle(r: Double),
                Point
            }
            fn make() -> Shape { Shape::Circle() }
            "#,
        );
        assert_has_error(&r, DiagCode::ARG_COUNT_MISMATCH);
    }

    #[test]
    fn enum_variant_call_arg_type_mismatch() {
        let r = check_source(
            r#"
            enum Shape {
                Circle(r: Double),
                Point
            }
            fn make() -> Shape { Shape::Circle("not a number") }
            "#,
        );
        assert_has_error(&r, DiagCode::TYPE_MISMATCH);
    }

    #[test]
    fn enum_unit_variant_path() {
        let r = check_source(
            r#"
            enum Shape {
                Circle(r: Double),
                Point
            }
            fn make() -> Shape { Shape::Point }
            "#,
        );
        assert_no_errors(&r);
    }

    // -- associated function call (#86) ---

    #[test]
    fn associated_fn_arg_count_checked() {
        let r = check_source(
            r#"
            class Shape() {
                fn detect(shape: Int) -> Shape {
                    Shape()
                }
            }
            fn test() -> Shape { Shape::detect() }
            "#,
        );
        assert_has_error(&r, DiagCode::ARG_COUNT_MISMATCH);
    }

    #[test]
    fn associated_fn_correct_call() {
        let r = check_source(
            r#"
            class Shape() {
                fn detect(shape: Int) -> Shape {
                    Shape()
                }
            }
            fn test() -> Shape { Shape::detect(42) }
            "#,
        );
        assert_no_errors(&r);
    }
}
