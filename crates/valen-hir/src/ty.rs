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
    in_unsafe: bool,
    type_params: IndexSet<SmolStr>,
    type_param_bounds: IndexMap<SmolStr, Vec<SmolStr>>,
    /// The concrete type that `Self` resolves to in the current class/impl context.
    current_self_ty: Option<Ty>,
}

impl<'hir> TypeChecker<'hir> {
    fn new(hir: &'hir Hir) -> Self {
        Self {
            hir,
            env: TypeEnv::new(),
            diags: Diagnostics::new(),
            current_self_ty: None,
            bodies: IndexMap::new(),
            return_ty: None,
            in_loop: false,
            in_unsafe: false,
            type_params: IndexSet::new(),
            type_param_bounds: IndexMap::new(),
        }
    }

    // -- top-level dispatch -------------------------------------------------

    fn check_items(&mut self, items: &[valen_ast::Item]) {
        self.register_top_level_types(items);
        for name in self.hir.foreign_types.keys() {
            self.env
                .define(name.clone(), Ty::Named(name.clone()), false);
        }
        self.register_prelude_functions();

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

    fn expand_aliases_in_ty(&self, ty: &Ty) -> Ty {
        match ty {
            Ty::Generic(name, args) => {
                let expanded_args: Vec<Ty> =
                    args.iter().map(|a| self.expand_aliases_in_ty(a)).collect();
                if let Some(expanded) = self.expand_type_alias(name, &expanded_args) {
                    expanded
                } else {
                    Ty::Generic(name.clone(), expanded_args)
                }
            }
            Ty::Named(name) => {
                if let Some(expanded) = self.expand_type_alias(name, &[]) {
                    expanded
                } else {
                    ty.clone()
                }
            }
            Ty::Nullable(inner) => Ty::Nullable(Box::new(self.expand_aliases_in_ty(inner))),
            Ty::Fn(params, ret) => Ty::Fn(
                params
                    .iter()
                    .map(|p| self.expand_aliases_in_ty(p))
                    .collect(),
                Box::new(self.expand_aliases_in_ty(ret)),
            ),
            _ => ty.clone(),
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
            TyRef::RefMut(inner) => {
                Ty::RefMut(Box::new(self.substitute_tyref(inner, params, args)))
            }
            TyRef::SelfTy | TyRef::Error => Ty::Error,
        }
    }

    /// Look up a def by name using the O(1) name index (#027).
    fn lookup_def_id(&self, name: &str) -> Option<DefId> {
        self.hir.lookup_by_name(name).first().copied()
    }

    /// Look up a method on a class, impl, or trait by name using the name index (#027).
    fn lookup_method_def_id(&self, class_name: &str, method_name: &str) -> Option<DefId> {
        for &def_id in self.hir.lookup_by_name(class_name) {
            let Some(def) = self.hir.defs.get(&def_id) else {
                continue;
            };
            match &def.kind {
                DefKind::Class(c) => {
                    for &mid in &c.methods {
                        if let Some(mdef) = self.hir.defs.get(&mid) {
                            if mdef.name == method_name {
                                return Some(mid);
                            }
                        }
                    }
                }
                DefKind::Trait(t) => {
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
        // Also check all impl blocks for matching methods (impls have empty names)
        for def in self.hir.defs.values() {
            if let DefKind::Impl(imp) = &def.kind {
                for &mid in &imp.methods {
                    if let Some(mdef) = self.hir.defs.get(&mid) {
                        if mdef.name == method_name {
                            return Some(mid);
                        }
                    }
                }
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
                valen_ast::Item::NewType(nt) => {
                    self.env
                        .define(nt.name.clone(), Ty::Named(nt.name.clone()), false);
                }
                _ => {}
            }
        }
    }

    fn register_prelude_functions(&mut self) {
        let string_to_unit = Ty::Fn(vec![Ty::Prim(PrimTy::String)], Box::new(Ty::unit()));
        self.env
            .define(SmolStr::from("println"), string_to_unit.clone(), false);
        self.env
            .define(SmolStr::from("print"), string_to_unit, false);

        let list_t = Ty::Generic(
            SmolStr::from("List"),
            vec![Ty::TypeParam(SmolStr::from("T"))],
        );
        let iter_t = Ty::Generic(
            SmolStr::from("Iterator"),
            vec![Ty::TypeParam(SmolStr::from("T"))],
        );
        self.env.define(
            SmolStr::from("iter"),
            Ty::Fn(vec![list_t], Box::new(iter_t)),
            false,
        );
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
                    if seg.name == "Self" {
                        if let Some(ref sty) = self.current_self_ty {
                            return sty.clone();
                        }
                    }
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
            valen_ast::Type::RefMut { inner, .. } => {
                Ty::RefMut(Box::new(self.resolve_ast_type(inner)))
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

        let prev_unsafe = self.in_unsafe;
        if f.is_unsafe {
            self.in_unsafe = true;
        }

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
        self.in_unsafe = prev_unsafe;
        self.type_params = prev_type_params;
        self.type_param_bounds = prev_bounds;

        if let Some(id) = def_id {
            self.bodies.insert(id, typed_body);
        }
    }

    fn check_class(&mut self, c: &valen_ast::ClassDecl) {
        let prev_type_params = std::mem::take(&mut self.type_params);
        let prev_bounds = std::mem::take(&mut self.type_param_bounds);
        let prev_self_ty = self.current_self_ty.take();
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
        self.current_self_ty = Some(self_ty.clone());
        for member in &c.body {
            if let valen_ast::ClassMember::Method(m) = member {
                let def_id = self.lookup_method_def_id(&c.name, &m.name);
                self.check_fn_decl(m, Some(&self_ty), def_id);
            }
        }
        self.type_params = prev_type_params;
        self.type_param_bounds = prev_bounds;
        self.current_self_ty = prev_self_ty;
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
        let prev_self_ty = self.current_self_ty.replace(self_ty.clone());
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
        self.current_self_ty = prev_self_ty;
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
            let last_diverges = stmts.last().is_some_and(|s| match s {
                TypedStmt::Expr(e) | TypedStmt::ExprSemi(e) => e.ty == Ty::nothing(),
                _ => false,
            });
            let ty = if last_diverges {
                Ty::nothing()
            } else {
                Ty::unit()
            };
            (None, ty)
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
            valen_ast::Stmt::LetElse(le) => self.check_let_else(le),
            valen_ast::Stmt::Expr(e) => TypedStmt::Expr(self.infer_expr(e)),
            valen_ast::Stmt::ExprSemi(e) => TypedStmt::ExprSemi(self.infer_expr(e)),
        }
    }

    /// Type-check a `let Pattern = expr else { diverge };` statement.
    ///
    /// The else block must diverge (its type must be `Nothing`).
    /// Pattern variables are bound in the enclosing scope (not inside the else block).
    fn check_let_else(&mut self, le: &valen_ast::LetElseStmt) -> TypedStmt {
        // Type-check the scrutinee expression
        let expected = le.ty.as_ref().map(|t| self.resolve_ast_type(t));
        let scrutinee = if let Some(exp) = &expected {
            self.check_expr(&le.expr, Some(exp))
        } else {
            self.infer_expr(&le.expr)
        };
        let scrutinee_ty = expected.unwrap_or_else(|| scrutinee.ty.clone());

        // Type-check the else block — it must diverge (type = Nothing)
        let else_body = self.check_block(&le.else_block, Some(&Ty::nothing()));
        if else_body.ty != Ty::nothing() {
            self.diags.error(
                DiagCode::LET_ELSE_NOT_DIVERGING,
                le.else_block.span,
                SmolStr::from(
                    "else block in `let ... else` must diverge (return, break, continue, or panic)",
                ),
            );
        }

        // Bind the pattern variables in the current (enclosing) scope
        self.bind_pattern(&le.pattern, &scrutinee_ty);

        TypedStmt::LetElse {
            pattern: le.pattern.clone(),
            scrutinee,
            ty: scrutinee_ty,
            else_body,
            span: le.span,
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
            valen_ast::Expr::Path(path) => self.synth_path(path, expected),
            valen_ast::Expr::Binary(bin) => self.synth_binary(bin),
            valen_ast::Expr::Unary(un) => self.synth_unary(un),
            valen_ast::Expr::Call(call) => self.synth_call(call, expected),
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
            valen_ast::Expr::IfLet(il) => self.synth_if_let(il, expected),
            valen_ast::Expr::WhileLet(wl) => self.synth_while_let(wl),
            valen_ast::Expr::VariantShorthand(vs) => self.synth_variant_shorthand(vs, expected),
            valen_ast::Expr::Pipeline(p) => self.synth_pipeline(p),
            valen_ast::Expr::ListLiteral(l) => self.synth_list_literal(l, expected),
            valen_ast::Expr::MapLiteral(m) => self.synth_map_literal(m, expected),
            valen_ast::Expr::Unsafe(u) => self.synth_unsafe(u),
            valen_ast::Expr::Cast(c) => self.synth_cast(c),
            valen_ast::Expr::Deref(d) => self.synth_deref(d),
            valen_ast::Expr::RefMutCreate(r) => self.synth_ref_mut_create(r, expected),
        }
    }

    // -- literals -----------------------------------------------------------

    fn synth_literal(&mut self, lit: &valen_ast::Literal) -> TypedExpr {
        match lit {
            valen_ast::Literal::Int(v, span) => {
                if *v < i32::MIN as i64 || *v > i32::MAX as i64 {
                    self.diags.error(
                        DiagCode::INT_LITERAL_OVERFLOW,
                        *span,
                        SmolStr::from(format!(
                            "integer literal `{}` overflows Int (must fit i32 range {}..={}); use `{}L` for Long",
                            v,
                            i32::MIN,
                            i32::MAX,
                            v,
                        )),
                    );
                }
                TypedExpr {
                    kind: TypedExprKind::IntLit(*v),
                    ty: Ty::Prim(PrimTy::Int),
                    span: *span,
                }
            }
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

    fn synth_path(&mut self, path: &valen_ast::Path, expected: Option<&Ty>) -> TypedExpr {
        if path.segments.len() == 1 {
            let name = &path.segments[0].name;
            if let Some(ty) = self.env.lookup(name).cloned() {
                return TypedExpr {
                    kind: TypedExprKind::LocalVar(name.clone()),
                    ty,
                    span: path.span,
                };
            }
            if let Some(expr) = self.resolve_bare_variant(name, expected, path.span) {
                return expr;
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
                            let ty = match expected {
                                Some(Ty::Generic(n, args)) if n == first => {
                                    Ty::Generic(n.clone(), args.clone())
                                }
                                _ => Ty::Named(first.clone()),
                            };
                            return TypedExpr {
                                kind: TypedExprKind::Call {
                                    callee: Box::new(TypedExpr {
                                        kind: TypedExprKind::LocalVar(SmolStr::from(format!(
                                            "{first}::{second}"
                                        ))),
                                        ty: ty.clone(),
                                        span: path.span,
                                    }),
                                    args: vec![],
                                },
                                ty,
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

    fn synth_call(&mut self, call: &valen_ast::CallExpr, expected: Option<&Ty>) -> TypedExpr {
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
                    let has_type_params = param_tys.iter().any(|t| t.has_type_params());
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

                // Check unsafe fn call outside unsafe context
                if !self.in_unsafe {
                    if let valen_ast::Expr::Path(path) = &*call.callee {
                        if path.segments.len() == 1 {
                            let fn_name = &path.segments[0].name;
                            for def in self.hir.defs.values() {
                                if def.name == *fn_name {
                                    if let DefKind::Fn(f) = &def.kind {
                                        if f.is_unsafe {
                                            self.diags.error(
                                                DiagCode::UNSAFE_CONTEXT_REQUIRED,
                                                call.span,
                                                SmolStr::from(
                                                    "call to `unsafe fn` requires an `unsafe` block",
                                                ),
                                            );
                                        }
                                    }
                                    break;
                                }
                            }
                        }
                    }
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
                            expected,
                        ) {
                            return result;
                        }

                        // Unit variant called with parens: Option::None()
                        if args.is_empty() {
                            if let Some(edef) = self.hir.defs.values().find(|d| {
                                d.name == *class_name && matches!(d.kind, DefKind::Enum(_))
                            }) {
                                if let DefKind::Enum(e) = &edef.kind {
                                    if e.variants
                                        .iter()
                                        .any(|v| v.name == *member_name && v.fields.is_empty())
                                    {
                                        let ty = match expected {
                                            Some(Ty::Generic(n, ga)) if n == class_name => {
                                                Ty::Generic(n.clone(), ga.clone())
                                            }
                                            _ => Ty::Named(class_name.clone()),
                                        };
                                        return TypedExpr {
                                            kind: TypedExprKind::Call {
                                                callee: Box::new(callee),
                                                args,
                                            },
                                            ty,
                                            span: call.span,
                                        };
                                    }
                                }
                            }
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

                // Bare variant call (e.g. `Some(42)` → `Option::Some(42)`)
                if let TypedExprKind::LocalVar(ref qname) = callee.kind {
                    if let Some((en, vn)) = qname.split_once("::") {
                        let en = SmolStr::from(en);
                        let vn = SmolStr::from(vn);
                        if let Some(result) = self.resolve_enum_variant_call(
                            &en, &vn, &callee, &args, call.span, expected,
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
                DefKind::NewType(nt) => {
                    if args.len() != 1 {
                        self.diags.error(
                            DiagCode::ARG_COUNT_MISMATCH,
                            span,
                            SmolStr::from(format!(
                                "`{name}` newtype constructor expects 1 argument, found {}",
                                args.len()
                            )),
                        );
                    } else {
                        let expected = tyref_to_ty_generic(&nt.inner_ty);
                        if !args[0].ty.is_error()
                            && !expected.is_error()
                            && args[0].ty != expected
                            && !is_subtype(&args[0].ty, &expected)
                        {
                            self.diags.error(
                                DiagCode::TYPE_MISMATCH,
                                args[0].span,
                                SmolStr::from(format!(
                                    "expected `{expected}`, found `{}`",
                                    args[0].ty
                                )),
                            );
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
        expected: Option<&Ty>,
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
            for (arg, exp_ty) in args.iter().zip(field_tys.iter()) {
                let resolved = if field_tys.iter().any(|t| matches!(t, Ty::TypeParam(_))) {
                    let bindings = infer_type_bindings(&field_tys, args);
                    substitute_ty(exp_ty, &bindings)
                } else {
                    exp_ty.clone()
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
        let ret_ty = if let Some(Ty::Generic(n, expected_args)) = expected {
            if n == enum_name {
                Ty::Generic(n.clone(), expected_args.clone())
            } else {
                self.infer_variant_return_type(enum_name, &edef.variants, &bindings)
            }
        } else {
            self.infer_variant_return_type(enum_name, &edef.variants, &bindings)
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

    fn infer_variant_return_type(
        &self,
        enum_name: &SmolStr,
        variants: &[crate::EnumVariantDef],
        bindings: &IndexMap<SmolStr, Ty>,
    ) -> Ty {
        let all_field_tys: Vec<Ty> = variants
            .iter()
            .flat_map(|v| v.fields.iter().map(|(_, tyref)| tyref_to_ty_generic(tyref)))
            .collect();
        let type_params = collect_type_params_ordered(&all_field_tys);
        if type_params.is_empty() {
            Ty::Named(enum_name.clone())
        } else {
            let resolved_params: Vec<Ty> = type_params
                .iter()
                .map(|p| substitute_ty(p, bindings))
                .collect();
            Ty::Generic(enum_name.clone(), resolved_params)
        }
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
        if fn_def.is_unsafe && !self.in_unsafe {
            self.diags.error(
                DiagCode::UNSAFE_CONTEXT_REQUIRED,
                span,
                SmolStr::from("call to `unsafe fn` requires an `unsafe` block"),
            );
        }
        if fn_def.params.iter().any(|p| p.is_self) {
            return None;
        }

        let resolve_self = |tyref: &TyRef| -> Ty {
            if *tyref == TyRef::SelfTy {
                Ty::Named(class_name.clone())
            } else {
                tyref_to_ty_generic(tyref)
            }
        };
        let param_tys: Vec<Ty> = fn_def.params.iter().map(|p| resolve_self(&p.ty)).collect();
        let ret_ty = fn_def
            .return_ty
            .as_ref()
            .map(&resolve_self)
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
                            if fdef.is_unsafe && !self.in_unsafe {
                                self.diags.error(
                                    DiagCode::UNSAFE_CONTEXT_REQUIRED,
                                    mc.span,
                                    SmolStr::from("call to `unsafe fn` requires an `unsafe` block"),
                                );
                            }
                            let resolve_self_ty = |tyref: &TyRef| -> Ty {
                                if *tyref == TyRef::SelfTy {
                                    Ty::Named(tn.clone())
                                } else {
                                    tyref_to_ty_generic(tyref)
                                }
                            };
                            let raw_ret_ty = fdef
                                .return_ty
                                .as_ref()
                                .map(&resolve_self_ty)
                                .unwrap_or_else(Ty::unit);

                            let mut bindings = if !generic_args.is_empty() {
                                self.build_class_type_bindings(tn, &generic_args)
                            } else {
                                IndexMap::new()
                            };

                            let method_type_params: Vec<SmolStr> = fdef
                                .generic_bounds
                                .iter()
                                .map(|(name, _)| name.clone())
                                .collect();

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
                                if !method_type_params.is_empty() {
                                    for (arg, param) in args.iter().zip(non_self_params.iter()) {
                                        let expected = tyref_to_ty_generic(&param.ty);
                                        let substituted = if !bindings.is_empty() {
                                            substitute_ty(&expected, &bindings)
                                        } else {
                                            expected
                                        };
                                        collect_bindings(&substituted, &arg.ty, &mut bindings);
                                    }
                                }

                                for (arg, param) in args.iter().zip(non_self_params.iter()) {
                                    let mut expected = tyref_to_ty_generic(&param.ty);
                                    if !bindings.is_empty() {
                                        expected = substitute_ty(&expected, &bindings);
                                    }
                                    expected = self.expand_aliases_in_ty(&expected);
                                    if !arg.ty.is_error()
                                        && !expected.is_error()
                                        && !expected.has_type_params()
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

                            // Check generic bounds at call site
                            for (tp_name, tp_bounds) in &fdef.generic_bounds {
                                if let Some(actual_ty) = bindings.get(tp_name) {
                                    for bound_trait in tp_bounds {
                                        if !self.type_satisfies_bound(actual_ty, bound_trait) {
                                            self.diags.error(
                                                DiagCode::BOUND_NOT_SATISFIED,
                                                mc.span,
                                                SmolStr::from(format!(
                                                    "type `{actual_ty}` does not satisfy bound `{bound_trait}` required by type parameter `{tp_name}`"
                                                )),
                                            );
                                        }
                                    }
                                }
                            }

                            let ret_ty = if !bindings.is_empty() {
                                substitute_ty(&raw_ret_ty, &bindings)
                            } else {
                                raw_ret_ty
                            };
                            let ret_ty = self.expand_aliases_in_ty(&ret_ty);

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
                    if let Some(foreign_result) =
                        self.resolve_foreign_method_with_generics(tn, &mc.method, &generic_args)
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
                    DefKind::Trait(tdef) => {
                        param_names.extend(tdef.generics.iter().cloned());
                    }
                    DefKind::Enum(edef) => {
                        for v in &edef.variants {
                            for (_, ty) in &v.fields {
                                if let TyRef::Unresolved(n) = ty {
                                    if !param_names.contains(n) {
                                        param_names.push(n.clone());
                                    }
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
            Ty::Generic(n, _) => n.clone(),
            Ty::Prim(p) => SmolStr::from(format!("{p}")),
            _ => return false,
        };

        // Check explicit trait impls
        if self
            .hir
            .trait_impls
            .iter()
            .any(|entry| entry.trait_name == *trait_name && entry.target_name == type_name)
        {
            return true;
        }

        // Check data class implicit traits (Eq, Hash, Display, Clone)
        for def in self.hir.defs.values() {
            if def.name == type_name {
                if let DefKind::DataClass(_) = &def.kind {
                    if trait_name == "Eq"
                        || trait_name == "Hash"
                        || trait_name == "Display"
                        || trait_name == "Clone"
                    {
                        return true;
                    }
                }
            }
        }

        // Check type parameter bounds (if ty is itself a type param with bounds)
        if let Ty::TypeParam(tp_name) = ty {
            if let Some(bounds) = self.type_param_bounds.get(tp_name) {
                return bounds.iter().any(|b| b == trait_name);
            }
        }

        false
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
        self.resolve_foreign_method_with_generics(type_name, method_name, &[])
    }

    fn resolve_foreign_method_with_generics(
        &self,
        type_name: &str,
        method_name: &str,
        generic_args: &[Ty],
    ) -> Option<Ty> {
        let info = self.hir.foreign_types.get(type_name)?;
        let matching: Vec<_> = info
            .methods
            .iter()
            .filter(|m| m.name == method_name)
            .collect();
        let m = matching.first()?;

        if let Some(generic_ret) = &m.generic_return_ty {
            if generic_args.len() == info.type_params.len() && !info.type_params.is_empty() {
                let mut bindings = IndexMap::new();
                for (param, arg) in info.type_params.iter().zip(generic_args.iter()) {
                    bindings.insert(param.clone(), arg.clone());
                }
                let ret = tyref_to_ty_generic(generic_ret);
                let substituted = substitute_ty(&ret, &bindings);
                if !substituted.has_type_params() {
                    return Some(nullable_if_reference(&substituted));
                }
            }
        }

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

    // -- if let / while let -------------------------------------------------

    fn synth_if_let(&mut self, il: &valen_ast::IfLetExpr, expected: Option<&Ty>) -> TypedExpr {
        let expr = self.infer_expr(&il.expr);

        self.env.push_scope();
        self.bind_pattern(&il.pattern, &expr.ty);
        let then_expected = if il.else_branch.is_none() {
            Some(&Ty::unit())
        } else {
            expected
        };
        let then_body = self.check_block(&il.then_branch, then_expected.or(expected));
        self.env.pop_scope();

        let (else_expr, ty) = if let Some(else_e) = &il.else_branch {
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
                    il.span,
                    SmolStr::from(format!(
                        "if let/else branches have incompatible types: `{}` vs `{}`",
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
            kind: TypedExprKind::IfLet {
                pattern: il.pattern.clone(),
                expr: Box::new(expr),
                then_branch: Box::new(then_body),
                else_branch: else_expr,
            },
            ty,
            span: il.span,
        }
    }

    fn synth_while_let(&mut self, wl: &valen_ast::WhileLetExpr) -> TypedExpr {
        let expr = self.infer_expr(&wl.expr);

        self.env.push_scope();
        self.bind_pattern(&wl.pattern, &expr.ty);
        let prev_in_loop = self.in_loop;
        self.in_loop = true;
        let body = self.check_block(&wl.body, Some(&Ty::unit()));
        self.in_loop = prev_in_loop;
        self.env.pop_scope();

        TypedExpr {
            kind: TypedExprKind::WhileLet {
                pattern: wl.pattern.clone(),
                expr: Box::new(expr),
                body: Box::new(body),
            },
            ty: Ty::unit(),
            span: wl.span,
        }
    }

    // -- variant shorthand expression ----------------------------------------

    /// Synthesize `.Variant` or `.Variant(args)` by resolving the enum from the
    /// expected type context (or by searching all enums).
    /// `lhs |> f(a, b)` desugars to `f(lhs, a, b)`.
    fn synth_pipeline(&mut self, p: &valen_ast::PipelineExpr) -> TypedExpr {
        match &p.rhs {
            valen_ast::Expr::Call(call) => {
                let mut new_args = vec![valen_ast::CallArg {
                    name: None,
                    value: p.lhs.clone(),
                    span: p.span,
                }];
                new_args.extend(call.args.iter().cloned());
                let desugared = valen_ast::CallExpr {
                    callee: call.callee.clone(),
                    args: new_args,
                    span: p.span,
                };
                self.synth_call(&desugared, None)
            }
            valen_ast::Expr::Path(_) => {
                let desugared = valen_ast::CallExpr {
                    callee: Box::new(p.rhs.clone()),
                    args: vec![valen_ast::CallArg {
                        name: None,
                        value: p.lhs.clone(),
                        span: p.span,
                    }],
                    span: p.span,
                };
                self.synth_call(&desugared, None)
            }
            _ => {
                self.diags.error(
                    DiagCode::TYPE_MISMATCH,
                    p.span,
                    SmolStr::from("pipeline RHS must be a function call or function name"),
                );
                TypedExpr {
                    kind: TypedExprKind::Error,
                    ty: Ty::Error,
                    span: p.span,
                }
            }
        }
    }

    fn synth_list_literal(
        &mut self,
        lit: &valen_ast::ListLiteralExpr,
        expected: Option<&Ty>,
    ) -> TypedExpr {
        let expected_elem = match expected {
            Some(Ty::Generic(name, args)) if !args.is_empty() => {
                let n = name.as_str();
                if n == "List"
                    || n == "java.util.List"
                    || n == "ArrayList"
                    || n == "java.util.ArrayList"
                {
                    Some(args[0].clone())
                } else {
                    None
                }
            }
            _ => None,
        };

        let has_list_annotation = expected_elem.is_some()
            || matches!(expected, Some(Ty::Named(n)) if n.contains("List") || n.contains("java.util"));

        if lit.elements.is_empty() && !has_list_annotation {
            self.diags.error(
                DiagCode::TYPE_MISMATCH,
                lit.span,
                SmolStr::from("empty list literal requires a type annotation"),
            );
        }

        let mut typed_elements = Vec::new();
        let mut elem_ty = expected_elem;

        for e in &lit.elements {
            let te = self.infer_expr(e);
            if let Some(ref expected) = elem_ty {
                if !te.ty.is_error() && te.ty != *expected && !is_subtype(&te.ty, expected) {
                    self.diags.error(
                        DiagCode::TYPE_MISMATCH,
                        te.span,
                        SmolStr::from(format!(
                            "expected list element `{expected}`, found `{}`",
                            te.ty
                        )),
                    );
                }
            } else {
                elem_ty = Some(te.ty.clone());
            }
            typed_elements.push(te);
        }

        let elem_ty = elem_ty.unwrap_or(Ty::Error);
        let list_ty = Ty::Generic(SmolStr::from("List"), vec![elem_ty]);
        let list_ty = self.expand_aliases_in_ty(&list_ty);

        TypedExpr {
            kind: TypedExprKind::ListLiteral(typed_elements),
            ty: list_ty,
            span: lit.span,
        }
    }

    fn synth_map_literal(
        &mut self,
        lit: &valen_ast::MapLiteralExpr,
        expected: Option<&Ty>,
    ) -> TypedExpr {
        let expected_kv = match expected {
            Some(Ty::Generic(name, args))
                if (name == "Map" || name == "java.util.Map") && args.len() >= 2 =>
            {
                Some((args[0].clone(), args[1].clone()))
            }
            _ => None,
        };

        let has_map_annotation = expected_kv.is_some()
            || matches!(expected, Some(Ty::Named(n)) if n.contains("Map") || n.contains("java.util"));

        if lit.entries.is_empty() && !has_map_annotation {
            self.diags.error(
                DiagCode::TYPE_MISMATCH,
                lit.span,
                SmolStr::from("empty map literal requires a type annotation"),
            );
        }

        let mut typed_entries = Vec::new();
        let mut key_ty = expected_kv.as_ref().map(|(k, _)| k.clone());
        let mut val_ty = expected_kv.as_ref().map(|(_, v)| v.clone());

        for (k, v) in &lit.entries {
            let tk = self.infer_expr(k);
            let tv = self.infer_expr(v);

            if let Some(ref expected_k) = key_ty {
                if !tk.ty.is_error() && tk.ty != *expected_k && !is_subtype(&tk.ty, expected_k) {
                    self.diags.error(
                        DiagCode::TYPE_MISMATCH,
                        tk.span,
                        SmolStr::from(format!(
                            "expected map key `{expected_k}`, found `{}`",
                            tk.ty
                        )),
                    );
                }
            } else {
                key_ty = Some(tk.ty.clone());
            }

            if let Some(ref expected_v) = val_ty {
                if !tv.ty.is_error() && tv.ty != *expected_v && !is_subtype(&tv.ty, expected_v) {
                    self.diags.error(
                        DiagCode::TYPE_MISMATCH,
                        tv.span,
                        SmolStr::from(format!(
                            "expected map value `{expected_v}`, found `{}`",
                            tv.ty
                        )),
                    );
                }
            } else {
                val_ty = Some(tv.ty.clone());
            }

            typed_entries.push((tk, tv));
        }

        let key_ty = key_ty.unwrap_or(Ty::Error);
        let val_ty = val_ty.unwrap_or(Ty::Error);
        let map_ty = Ty::Generic(SmolStr::from("Map"), vec![key_ty, val_ty]);
        let map_ty = self.expand_aliases_in_ty(&map_ty);

        TypedExpr {
            kind: TypedExprKind::MapLiteral(typed_entries),
            ty: map_ty,
            span: lit.span,
        }
    }

    fn synth_variant_shorthand(
        &mut self,
        vs: &valen_ast::VariantShorthandExpr,
        expected: Option<&Ty>,
    ) -> TypedExpr {
        let variant_name = &vs.variant_name;

        // Try to resolve from expected type first, then fall back to searching all enums.
        let resolved = self.resolve_variant_shorthand_enum(variant_name, expected);

        let Some((enum_name, enum_def_clone)) = resolved else {
            self.diags.error(
                DiagCode::NAME_NOT_FOUND,
                vs.span,
                SmolStr::from(format!(
                    "cannot resolve variant shorthand `.{variant_name}`: no matching enum found"
                )),
            );
            return TypedExpr {
                kind: TypedExprKind::Error,
                ty: Ty::Error,
                span: vs.span,
            };
        };

        let Some(variant) = enum_def_clone
            .variants
            .iter()
            .find(|v| v.name == *variant_name)
        else {
            self.diags.error(
                DiagCode::NAME_NOT_FOUND,
                vs.span,
                SmolStr::from(format!(
                    "enum `{enum_name}` has no variant `{variant_name}`"
                )),
            );
            return TypedExpr {
                kind: TypedExprKind::Error,
                ty: Ty::Error,
                span: vs.span,
            };
        };

        let qualified = SmolStr::from(format!("{enum_name}::{variant_name}"));

        // Unit variant (no fields)
        if variant.fields.is_empty() {
            if !vs.args.is_empty() {
                self.diags.error(
                    DiagCode::ARG_COUNT_MISMATCH,
                    vs.span,
                    SmolStr::from(format!(
                        "`.{variant_name}` takes no arguments, found {}",
                        vs.args.len()
                    )),
                );
            }
            let ty = match expected {
                Some(Ty::Generic(n, args)) if *n == enum_name => {
                    Ty::Generic(n.clone(), args.clone())
                }
                _ => Ty::Named(enum_name.clone()),
            };
            return TypedExpr {
                kind: TypedExprKind::Call {
                    callee: Box::new(TypedExpr {
                        kind: TypedExprKind::LocalVar(qualified),
                        ty: ty.clone(),
                        span: vs.span,
                    }),
                    args: vec![],
                },
                ty,
                span: vs.span,
            };
        }

        // Record variant with fields — synthesize as Enum::Variant(args)
        let args: Vec<TypedExpr> = vs.args.iter().map(|a| self.infer_expr(&a.value)).collect();

        let callee = TypedExpr {
            kind: TypedExprKind::LocalVar(qualified),
            ty: Ty::Named(enum_name.clone()),
            span: vs.span,
        };

        if let Some(result) = self.resolve_enum_variant_call(
            &enum_name,
            variant_name,
            &callee,
            &args,
            vs.span,
            expected,
        ) {
            return result;
        }

        // Fallback if resolve_enum_variant_call returns None (unit variant with args)
        TypedExpr {
            kind: TypedExprKind::Call {
                callee: Box::new(callee),
                args,
            },
            ty: Ty::Named(enum_name.clone()),
            span: vs.span,
        }
    }

    /// Resolve which enum a variant shorthand belongs to, preferring expected type.
    /// When no expected type is available and multiple enums contain a variant
    /// with the same name, returns `None` so the caller can emit an error.
    fn resolve_variant_shorthand_enum(
        &self,
        variant_name: &SmolStr,
        expected: Option<&Ty>,
    ) -> Option<(SmolStr, crate::EnumDef)> {
        // 1. Try expected type
        let expected_name = match expected {
            Some(Ty::Named(n)) => Some(n.clone()),
            Some(Ty::Generic(n, _)) => Some(n.clone()),
            _ => None,
        };

        if let Some(ref ename) = expected_name {
            for def in self.hir.defs.values() {
                if def.name == *ename {
                    if let DefKind::Enum(edef) = &def.kind {
                        if edef.variants.iter().any(|v| v.name == *variant_name) {
                            return Some((ename.clone(), edef.clone()));
                        }
                    }
                }
            }
        }

        // 2. Search all enums — collect candidates and check for ambiguity
        let candidates: Vec<_> = self
            .hir
            .defs
            .values()
            .filter_map(|d| match &d.kind {
                DefKind::Enum(e) if e.variants.iter().any(|v| v.name == *variant_name) => {
                    Some((d.name.clone(), e.clone()))
                }
                _ => None,
            })
            .collect();

        match candidates.len() {
            1 => Some(candidates.into_iter().next().unwrap()),
            _ => None, // 0 or ambiguous (>1)
        }
    }

    // -- match expression ---------------------------------------------------

    fn synth_match(&mut self, me: &valen_ast::MatchExpr, expected: Option<&Ty>) -> TypedExpr {
        let scrutinee = self.infer_expr(&me.scrutinee);
        let mut arms = Vec::new();
        let mut result_ty: Option<Ty> = expected.cloned();

        for arm in &me.arms {
            self.env.push_scope();
            self.bind_pattern(&arm.pattern, &scrutinee.ty);

            let guard = arm
                .guard
                .as_ref()
                .map(|g| self.check_expr(g, Some(&Ty::Prim(PrimTy::Bool))));
            let body = if let Some(ref rty) = result_ty {
                self.check_expr(&arm.body, Some(rty))
            } else {
                self.infer_expr(&arm.body)
            };

            self.env.pop_scope();

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

    /// Walk a pattern and register all binding variables in the current scope.
    fn bind_pattern(&mut self, pattern: &valen_ast::Pattern, scrutinee_ty: &Ty) {
        use valen_ast::Pattern;
        match pattern {
            Pattern::Wildcard(_) | Pattern::Literal(_) | Pattern::Range(_) | Pattern::Path(_) => {}
            Pattern::Binding(binding) => {
                self.env
                    .define(binding.name.clone(), scrutinee_ty.clone(), binding.mutable);
            }
            Pattern::Struct(sp) => {
                self.bind_struct_pattern(sp, scrutinee_ty);
            }
            Pattern::Tuple(pats, _) => {
                for pat in pats {
                    self.bind_pattern(pat, scrutinee_ty);
                }
            }
            Pattern::Or(pats, span) => {
                // Bind from the first alternative (provides the actual scope bindings).
                if let Some(first) = pats.first() {
                    self.bind_pattern(first, scrutinee_ty);
                }
                // Verify all alternatives bind the same set of variable names.
                if pats.len() >= 2 {
                    let first_names = collect_pattern_names(&pats[0]);
                    for alt in &pats[1..] {
                        let alt_names = collect_pattern_names(alt);
                        if alt_names != first_names {
                            let missing_from_alt: Vec<_> =
                                first_names.difference(&alt_names).collect();
                            let extra_in_alt: Vec<_> = alt_names.difference(&first_names).collect();
                            let mut parts = Vec::new();
                            if !missing_from_alt.is_empty() {
                                parts.push(format!(
                                    "missing: {}",
                                    missing_from_alt
                                        .iter()
                                        .map(|n| format!("`{n}`"))
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                ));
                            }
                            if !extra_in_alt.is_empty() {
                                parts.push(format!(
                                    "extra: {}",
                                    extra_in_alt
                                        .iter()
                                        .map(|n| format!("`{n}`"))
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                ));
                            }
                            self.diags.error(
                                DiagCode::OR_PATTERN_BINDING_MISMATCH,
                                *span,
                                SmolStr::from(format!(
                                    "or-pattern alternatives must bind the same variables ({})",
                                    parts.join("; ")
                                )),
                            );
                            break;
                        }
                    }
                }
            }
            Pattern::At(at) => {
                self.env
                    .define(at.name.clone(), scrutinee_ty.clone(), false);
                self.bind_pattern(&at.pattern, scrutinee_ty);
            }
            Pattern::VariantShorthand(vs) => {
                self.bind_variant_shorthand_pattern(vs, scrutinee_ty);
            }
        }
    }

    /// Bind variables from a `.Variant(fields)` shorthand pattern by resolving
    /// the enum from the scrutinee type context or by searching all enums.
    fn bind_variant_shorthand_pattern(
        &mut self,
        vs: &valen_ast::VariantShorthandPattern,
        scrutinee_ty: &Ty,
    ) {
        let variant_name = vs.variant_name.as_str();

        // Try to determine the enum from the scrutinee type.
        // When the scrutinee is a known enum, do NOT fall back to global search
        // to avoid hiding type errors (e.g. `.Some(x)` on a `Color` scrutinee).
        let enum_name = match scrutinee_ty {
            Ty::Named(n) | Ty::Generic(n, _) => {
                let is_enum_with_variant = self.hir.defs.values().any(|d| {
                    d.name == *n
                        && matches!(&d.kind, DefKind::Enum(e) if e.variants.iter().any(|v| v.name == variant_name))
                });
                if is_enum_with_variant {
                    Some(n.clone())
                } else {
                    // Check if scrutinee is an enum at all (without the variant)
                    let is_enum = self
                        .hir
                        .defs
                        .values()
                        .any(|d| d.name == *n && matches!(&d.kind, DefKind::Enum(_)));
                    if is_enum {
                        // Scrutinee is an enum but doesn't have this variant — error
                        self.diags.error(
                            DiagCode::NAME_NOT_FOUND,
                            vs.span,
                            SmolStr::from(format!("enum `{n}` has no variant `{variant_name}`")),
                        );
                        return;
                    }
                    // Scrutinee is not an enum, try global search
                    self.find_enum_for_variant(variant_name)
                }
            }
            _ => self.find_enum_for_variant(variant_name),
        };

        let Some(enum_name) = enum_name else {
            return;
        };

        // Build a synthetic StructPattern to reuse bind_variant_fields
        let sp = valen_ast::StructPattern {
            path: valen_ast::Path {
                segments: vec![
                    valen_ast::PathSegment {
                        name: enum_name.clone(),
                        double_colon: false,
                        generics: vec![],
                        span: vs.span,
                    },
                    valen_ast::PathSegment {
                        name: SmolStr::from(variant_name),
                        double_colon: true,
                        generics: vec![],
                        span: vs.span,
                    },
                ],
                span: vs.span,
            },
            fields: vs.fields.clone(),
            rest: vs.rest,
            span: vs.span,
        };
        self.bind_variant_fields(&sp, &enum_name, variant_name, scrutinee_ty);
    }

    /// Resolve field types from an enum variant pattern and bind variables.
    fn bind_struct_pattern(&mut self, sp: &valen_ast::StructPattern, scrutinee_ty: &Ty) {
        let segments: Vec<&str> = sp.path.segments.iter().map(|s| s.name.as_str()).collect();
        let (enum_name, variant_name) = match segments.len() {
            2 => (segments[0], segments[1]),
            1 => {
                let vn = segments[0];
                let en = self.find_enum_for_variant(vn);
                if let Some(ref name) = en {
                    // Re-borrow to avoid lifetime issue: copy into local
                    let name = name.clone();
                    return self.bind_variant_fields(sp, &name, vn, scrutinee_ty);
                }
                return;
            }
            _ => return,
        };
        self.bind_variant_fields(sp, enum_name, variant_name, scrutinee_ty);
    }

    /// Resolve a bare variant name (e.g. `Some`, `None`, `Ok`, `Err`) to its
    /// qualified enum path. Produces the same typed expression as `Enum::Variant`.
    fn resolve_bare_variant(
        &self,
        name: &SmolStr,
        expected: Option<&Ty>,
        span: Span,
    ) -> Option<TypedExpr> {
        for def in self.hir.defs.values() {
            if let DefKind::Enum(enum_def) = &def.kind {
                let Some(variant) = enum_def.variants.iter().find(|v| v.name == *name) else {
                    continue;
                };
                let enum_name = &def.name;
                let qualified = SmolStr::from(format!("{enum_name}::{name}"));
                if variant.fields.is_empty() {
                    let ty = match expected {
                        Some(Ty::Generic(n, args)) if n == enum_name => {
                            Ty::Generic(n.clone(), args.clone())
                        }
                        _ => Ty::Named(enum_name.clone()),
                    };
                    return Some(TypedExpr {
                        kind: TypedExprKind::Call {
                            callee: Box::new(TypedExpr {
                                kind: TypedExprKind::LocalVar(qualified),
                                ty: ty.clone(),
                                span,
                            }),
                            args: vec![],
                        },
                        ty,
                        span,
                    });
                }
                return Some(TypedExpr {
                    kind: TypedExprKind::LocalVar(qualified),
                    ty: Ty::Named(enum_name.clone()),
                    span,
                });
            }
        }
        None
    }

    fn find_enum_for_variant(&self, variant_name: &str) -> Option<SmolStr> {
        for def in self.hir.defs.values() {
            if let DefKind::Enum(enum_def) = &def.kind {
                if enum_def.variants.iter().any(|v| v.name == variant_name) {
                    return Some(def.name.clone());
                }
            }
        }
        None
    }

    fn bind_variant_fields(
        &mut self,
        sp: &valen_ast::StructPattern,
        enum_name: &str,
        variant_name: &str,
        scrutinee_ty: &Ty,
    ) {
        let type_args = match scrutinee_ty {
            Ty::Generic(_, args) => args.clone(),
            _ => vec![],
        };

        let field_types = self.resolve_variant_field_types(enum_name, variant_name, &type_args);

        if !sp.rest && sp.fields.len() > field_types.len() {
            self.diags.error(
                DiagCode::ARG_COUNT_MISMATCH,
                sp.span,
                SmolStr::from(format!(
                    "`{enum_name}::{variant_name}` pattern expects {} field(s), found {}",
                    field_types.len(),
                    sp.fields.len(),
                )),
            );
            return;
        }

        for (idx, field) in sp.fields.iter().enumerate() {
            let field_ty = if field.pattern.is_some() {
                field_types
                    .get(field.name.as_str())
                    .cloned()
                    .unwrap_or(Ty::Error)
            } else {
                field_types
                    .get_index(idx)
                    .map(|(_, ty)| ty.clone())
                    .unwrap_or(Ty::Error)
            };

            if let Some(ref sub_pat) = field.pattern {
                self.bind_pattern(sub_pat, &field_ty);
            } else {
                self.env.define(field.name.clone(), field_ty, false);
            }
        }
    }

    fn resolve_variant_field_types(
        &self,
        enum_name: &str,
        variant_name: &str,
        type_args: &[Ty],
    ) -> IndexMap<SmolStr, Ty> {
        let mut result = IndexMap::new();
        for def in self.hir.defs.values() {
            if def.name != enum_name {
                continue;
            }
            if let DefKind::Enum(enum_def) = &def.kind {
                // Collect type params from ALL variants to match enum-level ordering
                let all_variant_field_tys: Vec<Ty> = enum_def
                    .variants
                    .iter()
                    .flat_map(|v| v.fields.iter().map(|(_, tyref)| tyref_to_ty_generic(tyref)))
                    .collect();
                let enum_type_params = collect_type_params_ordered(&all_variant_field_tys);
                let bindings: IndexMap<SmolStr, Ty> = enum_type_params
                    .iter()
                    .filter_map(|tp| {
                        if let Ty::TypeParam(name) = tp {
                            Some(name.clone())
                        } else {
                            None
                        }
                    })
                    .zip(type_args.iter().cloned())
                    .collect();

                for variant in &enum_def.variants {
                    if variant.name != variant_name {
                        continue;
                    }
                    for (fname, tyref) in &variant.fields {
                        let ty = tyref_to_ty_generic(tyref);
                        let resolved = substitute_ty(&ty, &bindings);
                        result.insert(fname.clone(), resolved);
                    }
                    return result;
                }
            }
        }
        result
    }

    // -- assign -------------------------------------------------------------

    fn synth_assign(&mut self, asgn: &valen_ast::AssignExpr) -> TypedExpr {
        // `*ref_expr = value` — deref assign
        if let valen_ast::Expr::Deref(d) = &*asgn.target {
            let ref_expr = self.synth_expr(&d.expr, None);
            let inner_ty = match &ref_expr.ty {
                Ty::RefMut(inner) => (**inner).clone(),
                other => {
                    if !other.is_error() {
                        self.diags.error(
                            DiagCode::DEREF_NOT_REF_MUT,
                            d.span,
                            SmolStr::from(format!(
                                "cannot dereference type `{other}`, expected `ref mut T`"
                            )),
                        );
                    }
                    Ty::Error
                }
            };
            let value = self.check_expr(&asgn.value, Some(&inner_ty));
            return TypedExpr {
                kind: TypedExprKind::DerefAssign {
                    target: Box::new(ref_expr),
                    value: Box::new(value),
                },
                ty: Ty::unit(),
                span: asgn.span,
            };
        }

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
                self.check_try_return_type("Option", t.span);
                (args[0].clone(), true)
            }
            Ty::Generic(name, args) if name == "Result" && !args.is_empty() => {
                self.check_try_return_type("Result", t.span);
                (args[0].clone(), false)
            }
            Ty::Error => (Ty::Error, false),
            _ => {
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

    fn check_try_return_type(&mut self, expected_wrapper: &str, span: Span) {
        if let Some(ret_ty) = &self.return_ty {
            let ok = match ret_ty {
                Ty::Generic(name, _) if name == expected_wrapper => true,
                Ty::Error => true,
                _ => false,
            };
            if !ok {
                self.diags.error(
                    DiagCode::TRY_RETURN_MISMATCH,
                    span,
                    SmolStr::from(format!(
                        "`?` on `{expected_wrapper}` requires the function to return \
                         `{expected_wrapper}<..>`, found `{ret_ty}`"
                    )),
                );
            }
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

    fn synth_unsafe(&mut self, u: &valen_ast::UnsafeExpr) -> TypedExpr {
        let prev = self.in_unsafe;
        self.in_unsafe = true;
        let inner = self.synth_expr(&u.body, None);
        self.in_unsafe = prev;
        let ty = inner.ty.clone();
        let body = TypedBody {
            stmts: vec![],
            tail: Some(Box::new(inner)),
            ty: ty.clone(),
        };
        TypedExpr {
            kind: TypedExprKind::Unsafe(body),
            ty,
            span: u.span,
        }
    }

    fn synth_cast(&mut self, c: &valen_ast::CastExpr) -> TypedExpr {
        let inner = self.synth_expr(&c.expr, None);
        let target = self.resolve_ast_type(&c.target_ty);
        if !self.is_valid_cast(&inner.ty, &target) {
            self.diags.error(
                DiagCode::INVALID_CAST,
                c.span,
                SmolStr::from(format!("cannot cast `{}` to `{}`", inner.ty, target)),
            );
        }
        let is_safe = self.is_safe_cast(&inner.ty, &target);
        if !is_safe && !self.in_unsafe {
            self.diags.error(
                DiagCode::UNSAFE_REQUIRED,
                c.span,
                SmolStr::from("downcast requires `unsafe` context"),
            );
        }
        TypedExpr {
            kind: TypedExprKind::Cast {
                expr: Box::new(inner),
                target_ty: target.clone(),
            },
            ty: target,
            span: c.span,
        }
    }

    fn is_safe_cast(&self, from: &Ty, to: &Ty) -> bool {
        if from == to || from.is_error() || to.is_error() {
            return true;
        }
        #[allow(clippy::match_like_matches_macro)]
        match (from, to) {
            (
                Ty::Prim(PrimTy::Byte),
                Ty::Prim(
                    PrimTy::Short | PrimTy::Int | PrimTy::Long | PrimTy::Float | PrimTy::Double,
                ),
            ) => true,
            (
                Ty::Prim(PrimTy::Short),
                Ty::Prim(PrimTy::Int | PrimTy::Long | PrimTy::Float | PrimTy::Double),
            ) => true,
            (Ty::Prim(PrimTy::Int), Ty::Prim(PrimTy::Long | PrimTy::Float | PrimTy::Double)) => {
                true
            }
            (Ty::Prim(PrimTy::Long), Ty::Prim(PrimTy::Float | PrimTy::Double)) => true,
            (Ty::Prim(PrimTy::Float), Ty::Prim(PrimTy::Double)) => true,
            (
                Ty::Prim(PrimTy::Char),
                Ty::Prim(PrimTy::Int | PrimTy::Long | PrimTy::Float | PrimTy::Double),
            ) => true,
            _ => false,
        }
    }

    /// Returns true if the cast from `from` to `to` is structurally valid
    /// (safe or unsafe). Returns false for impossible casts like String→Int.
    fn is_valid_cast(&self, from: &Ty, to: &Ty) -> bool {
        if from == to || from.is_error() || to.is_error() {
            return true;
        }
        // Numeric casts (widening or narrowing) are all valid
        if from.is_numeric() && to.is_numeric() {
            return true;
        }
        // Char to numeric
        if matches!(from, Ty::Prim(PrimTy::Char)) && to.is_numeric() {
            return true;
        }
        // Reference types: downcast between named/generic types
        let is_ref_from = matches!(from, Ty::Named(_) | Ty::Generic(_, _) | Ty::Nullable(_));
        let is_ref_to = matches!(to, Ty::Named(_) | Ty::Generic(_, _) | Ty::Nullable(_));
        if is_ref_from && is_ref_to {
            return true;
        }
        false
    }

    fn synth_deref(&mut self, d: &valen_ast::DerefExpr) -> TypedExpr {
        let inner = self.synth_expr(&d.expr, None);
        let ty = match &inner.ty {
            Ty::RefMut(inner_ty) => (**inner_ty).clone(),
            other => {
                if !other.is_error() {
                    self.diags.error(
                        DiagCode::DEREF_NOT_REF_MUT,
                        d.span,
                        SmolStr::from(format!(
                            "cannot dereference type `{other}`, expected `ref mut T`"
                        )),
                    );
                }
                Ty::Error
            }
        };
        TypedExpr {
            kind: TypedExprKind::Deref {
                expr: Box::new(inner),
            },
            ty,
            span: d.span,
        }
    }

    fn synth_ref_mut_create(
        &mut self,
        r: &valen_ast::RefMutExpr,
        _expected: Option<&Ty>,
    ) -> TypedExpr {
        let inner = self.synth_expr(&r.expr, None);
        let ty = Ty::RefMut(Box::new(inner.ty.clone()));
        TypedExpr {
            kind: TypedExprKind::RefMutCreate {
                expr: Box::new(inner),
            },
            ty,
            span: r.span,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Recursively collect all variable names bound by a pattern.
fn collect_pattern_names(pattern: &valen_ast::Pattern) -> IndexSet<SmolStr> {
    let mut names = IndexSet::new();
    collect_pattern_names_inner(pattern, &mut names);
    names
}

fn collect_pattern_names_inner(pattern: &valen_ast::Pattern, names: &mut IndexSet<SmolStr>) {
    use valen_ast::Pattern;
    match pattern {
        Pattern::Wildcard(_) | Pattern::Literal(_) | Pattern::Range(_) | Pattern::Path(_) => {}
        Pattern::Binding(b) => {
            names.insert(b.name.clone());
        }
        Pattern::Struct(sp) => {
            for field in &sp.fields {
                if let Some(sub) = &field.pattern {
                    collect_pattern_names_inner(sub, names);
                } else {
                    // Shorthand binding: field name = variable name.
                    names.insert(field.name.clone());
                }
            }
        }
        Pattern::Tuple(pats, _) => {
            for p in pats {
                collect_pattern_names_inner(p, names);
            }
        }
        Pattern::Or(pats, _) => {
            // For nested or-patterns, collect from the first alternative
            // (all should be identical if well-formed).
            if let Some(first) = pats.first() {
                collect_pattern_names_inner(first, names);
            }
        }
        Pattern::At(at) => {
            names.insert(at.name.clone());
            collect_pattern_names_inner(&at.pattern, names);
        }
        Pattern::VariantShorthand(vs) => {
            for field in &vs.fields {
                if let Some(sub) = &field.pattern {
                    collect_pattern_names_inner(sub, names);
                } else {
                    names.insert(field.name.clone());
                }
            }
        }
    }
}

fn is_subtype(sub: &Ty, sup: &Ty) -> bool {
    if sub == sup {
        return true;
    }
    // Any concrete type is compatible with a TypeParam (at call site)
    if matches!(sup, Ty::TypeParam(_)) {
        return true;
    }
    // All types are subtypes of Any (java.lang.Object)
    if matches!(sup, Ty::Named(n) if n == "Any") {
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
        Ty::Generic(pn, param_args) => {
            if let Ty::Generic(an, arg_args) = arg_ty {
                if pn == an {
                    for (p, a) in param_args.iter().zip(arg_args.iter()) {
                        collect_bindings(p, a, bindings);
                    }
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

fn nullable_if_reference(ty: &Ty) -> Ty {
    match ty {
        Ty::Prim(
            PrimTy::Unit
            | PrimTy::Bool
            | PrimTy::Byte
            | PrimTy::Short
            | PrimTy::Int
            | PrimTy::Long
            | PrimTy::Float
            | PrimTy::Double
            | PrimTy::Char
            | PrimTy::Nothing,
        ) => ty.clone(),
        Ty::Nullable(_) => ty.clone(),
        other => Ty::Nullable(Box::new(other.clone())),
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

    #[test]
    fn unit_variant_infers_generic_from_expected() {
        let r = check_source(
            r#"
            enum Option<T> {
                Some(value: T),
                None
            }
            class Rect(w: Float, h: Float) {}
            fn maybe() -> Option<Rect> {
                Option::None
            }
            "#,
        );
        assert_no_errors(&r);
    }

    #[test]
    fn unit_variant_infers_in_if_else() {
        let r = check_source(
            r#"
            enum Option<T> {
                Some(value: T),
                None
            }
            fn test(x: Int) -> Option<Int> {
                if x > 0 {
                    Option::Some(x)
                } else {
                    Option::None
                }
            }
            "#,
        );
        assert_no_errors(&r);
    }

    #[test]
    fn unit_variant_with_parens_infers_generic() {
        let r = check_source(
            r#"
            enum Option<T> {
                Some(value: T),
                None
            }
            class Rect(w: Float, h: Float) {}
            fn maybe(r: Rect) -> Option<Rect> {
                if r.w > 0.0f {
                    Option::Some(r)
                } else {
                    Option::None()
                }
            }
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

    #[test]
    fn enum_destructure_bind_variable() {
        let r = check_source(
            r#"
            enum Color {
                Red,
                Blue(value: Int),
            }
            fn describe(c: Color) -> Int {
                match c {
                    Color::Red => 0,
                    Color::Blue(value) => value,
                }
            }
            "#,
        );
        assert_no_errors(&r);
    }

    #[test]
    fn enum_destructure_multiple_fields() {
        let r = check_source(
            r#"
            enum Shape {
                Point,
                Rect(w: Int, h: Int),
            }
            fn area(s: Shape) -> Int {
                match s {
                    Shape::Point => 0,
                    Shape::Rect(w, h) => w,
                }
            }
            "#,
        );
        assert_no_errors(&r);
    }

    #[test]
    fn generic_enum_destructure_bind() {
        let r = check_source(
            r#"
            fn unwrap_or(opt: Option<Int>, default: Int) -> Int {
                match opt {
                    Option::Some(value) => value,
                    Option::None => default,
                }
            }
            "#,
        );
        assert_no_errors(&r);
    }

    #[test]
    fn foreign_type_constructor_in_expression() {
        let src = r#"
            import java.util.ArrayList;
            fn test() -> ArrayList { ArrayList() }
        "#;
        let parsed = parse(src, FileId(0));
        assert!(!parsed.diagnostics.has_errors());
        let mut resolved = resolve::resolve(&parsed.items);
        assert!(!resolved.diagnostics.has_errors());
        resolved.hir.foreign_types.insert(
            SmolStr::from("ArrayList"),
            crate::ForeignClassInfo {
                internal_name: "java/util/ArrayList".to_string(),
                methods: vec![],
                constructors: vec![crate::ForeignCtorInfo { params: vec![] }],
                fields: vec![],
                super_class: None,
                interfaces: vec![],
                permitted_subclasses: vec![],
                has_valen_closed: false,
                type_params: vec![],
            },
        );
        let r = type_check(&resolved.hir, &parsed.items);
        assert_no_errors(&r);
    }

    #[test]
    fn try_operator_option() {
        let r = check_source(
            r#"
            fn get_value(x: Int) -> Option<Int> {
                if x > 0 {
                    Option::Some(x)
                } else {
                    Option::None
                }
            }
            fn use_option(x: Int) -> Option<Int> {
                let v = get_value(x)?;
                Option::Some(v)
            }
            "#,
        );
        assert_no_errors(&r);
    }

    #[test]
    fn try_operator_result_return_type_validated() {
        let r = check_source(
            r#"
            fn get_result() -> Result<Int, String> { Result::Ok(42) }
            fn test() -> Int {
                get_result()?
            }
            "#,
        );
        assert_has_error(&r, DiagCode::TRY_RETURN_MISMATCH);
    }

    #[test]
    fn try_operator_on_non_option_result_errors() {
        let r = check_source(
            r#"
            fn test(x: Int) -> Int {
                x?
            }
            "#,
        );
        assert_has_error(&r, DiagCode::TRY_INVALID_TYPE);
    }

    #[test]
    fn try_operator_option_return_type_validated() {
        let r = check_source(
            r#"
            fn get_value() -> Option<Int> { Option::Some(42) }
            fn test() -> Int {
                get_value()?
            }
            "#,
        );
        assert_has_error(&r, DiagCode::TRY_RETURN_MISMATCH);
    }

    #[test]
    fn return_with_value() {
        let r = check_source("fn test() -> Int { return 42; }");
        assert_no_errors(&r);
    }

    #[test]
    fn return_no_value_in_unit_fn() {
        let r = check_source("fn test() -> Unit { return; }");
        assert_no_errors(&r);
    }

    #[test]
    fn return_early_in_if() {
        let r = check_source(
            r#"
            fn test(x: Int) -> Int {
                if x < 0 {
                    return 0;
                }
                x
            }
            "#,
        );
        assert_no_errors(&r);
    }

    #[test]
    fn return_value_type_mismatch() {
        let r = check_source(r#"fn test() -> Int { return "hello"; }"#);
        assert_has_error(&r, DiagCode::TYPE_MISMATCH);
    }

    #[test]
    fn bare_some_none_in_option() {
        let r = check_source(
            r#"
            fn test(x: Int) -> Option<Int> {
                if x > 0 { Some(x) } else { None }
            }
            "#,
        );
        assert_no_errors(&r);
    }

    #[test]
    fn bare_ok_err_in_result() {
        let r = check_source(
            r#"
            fn test(x: Int) -> Result<Int, String> {
                if x > 0 { Ok(x) } else { Err("negative") }
            }
            "#,
        );
        assert_no_errors(&r);
    }

    #[test]
    fn bare_none_in_match_pattern_position() {
        let r = check_source(
            r#"
            fn test(opt: Option<Int>) -> Int {
                match opt {
                    Option::Some(v) => v,
                    Option::None => 0,
                }
            }
            "#,
        );
        assert_no_errors(&r);
    }

    #[test]
    fn if_let_option() {
        let r = check_source(
            r#"
            fn test(opt: Option<Int>) -> Int {
                if let Option::Some(v) = opt {
                    v
                } else {
                    0
                }
            }
            "#,
        );
        assert_no_errors(&r);
    }

    #[test]
    fn if_let_no_else_returns_unit() {
        let r = check_source(
            r#"
            fn test(opt: Option<Int>) -> Unit {
                if let Option::Some(v) = opt {
                    let x = v;
                }
            }
            "#,
        );
        assert_no_errors(&r);
    }

    #[test]
    fn while_let_option() {
        let r = check_source(
            r#"
            fn test(opt: Option<Int>) -> Unit {
                while let Option::Some(v) = opt {
                    let x = v;
                    break;
                }
            }
            "#,
        );
        assert_no_errors(&r);
    }

    // -- let-else -------------------------------------------------------------

    #[test]
    fn let_else_basic() {
        let r = check_source(
            r#"
            enum Color { Red, Blue(value: Int) }
            fn test(c: Color) -> Int {
                let Color::Blue(value) = c else { return 0; };
                value
            }
            "#,
        );
        assert_no_errors(&r);
    }

    #[test]
    fn let_else_diverges_required() {
        let r = check_source(
            r#"
            enum Color { Red, Blue(value: Int) }
            fn test(c: Color) -> Int {
                let Color::Blue(value) = c else { 42 };
                value
            }
            "#,
        );
        assert_has_error(&r, DiagCode::LET_ELSE_NOT_DIVERGING);
    }

    #[test]
    fn let_else_option() {
        let r = check_source(
            r#"
            fn test(opt: Option<Int>) -> Int {
                let Option::Some(v) = opt else { return 0; };
                v
            }
            "#,
        );
        assert_no_errors(&r);
    }
}
