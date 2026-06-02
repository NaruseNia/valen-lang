//! Lowers typed HIR expressions and statements into JVM bytecode operations.

use indexmap::IndexMap;
use smol_str::SmolStr;
use valen_ast::{BinaryOp, UnaryOp};
use valen_hir::{
    DefId, DefKind, FnDef, Hir, PrimTy, Ty, TypedBody, TypedExpr, TypedExprKind, TypedStmt,
    TypedStringPart,
};

use crate::descriptor::tyref_to_jvm;
use crate::jvm_const::*;
use crate::jvm_ir::{
    ArithOp, BitwiseOp, BootstrapArg, BootstrapMethodRef, CmpKind, ExceptionHandler,
    JvmBootstrapMethod, JvmMethodBody, JvmOp, JvmType, Label, MethodHandleKind, SyntheticLambda,
};

struct LoopContext {
    break_label: Label,
    continue_label: Label,
}

/// Undo entry for a single variable binding: the variable name and its
/// previous binding (`None` if it was newly introduced in this scope).
type ScopeUndo = Vec<(SmolStr, Option<(u16, JvmType)>)>;

struct ExprLowering<'a> {
    hir: &'a Hir,
    typed_bodies: &'a IndexMap<DefId, TypedBody>,
    ops: Vec<JvmOp>,
    locals: IndexMap<SmolStr, (u16, JvmType)>,
    next_slot: u16,
    next_label: u32,
    class_internal: &'a str,
    return_ty: JvmType,
    loop_stack: Vec<LoopContext>,
    pkg: Option<&'a [SmolStr]>,
    /// Undo log stack for lexical scoping.
    scope_stack: Vec<ScopeUndo>,
    /// Exception handlers collected during lowering (for safe {} blocks).
    exception_handlers: Vec<ExceptionHandler>,
    /// Counter for generating unique lambda synthetic method names.
    lambda_counter: u32,
    /// Synthetic lambda methods collected during expression lowering.
    synthetic_lambdas: Vec<SyntheticLambda>,
    /// Bootstrap method entries collected for lambda `invokedynamic` call sites.
    bootstrap_methods: Vec<JvmBootstrapMethod>,
}

/// Result of lowering a method body, including any lambda artifacts.
pub struct LowerBodyResult {
    /// The lowered method body.
    pub body: JvmMethodBody,
    /// Synthetic lambda methods generated within this body.
    pub synthetic_lambdas: Vec<SyntheticLambda>,
    /// Bootstrap method entries for lambda `invokedynamic` call sites.
    pub bootstrap_methods: Vec<JvmBootstrapMethod>,
}

/// Lowers a typed method body into JVM bytecode operations.
#[allow(clippy::too_many_arguments)]
pub fn lower_body(
    body: &TypedBody,
    class_internal: &str,
    params: &[(SmolStr, JvmType)],
    return_ty: &JvmType,
    has_self: bool,
    pkg: Option<&[SmolStr]>,
    hir: &Hir,
    typed_bodies: &IndexMap<DefId, TypedBody>,
) -> LowerBodyResult {
    let mut ctx = ExprLowering {
        hir,
        typed_bodies,
        ops: Vec::new(),
        locals: IndexMap::new(),
        next_slot: 0,
        next_label: 1000,
        class_internal,
        return_ty: return_ty.clone(),
        loop_stack: Vec::new(),
        pkg,
        scope_stack: Vec::new(),
        exception_handlers: Vec::new(),
        lambda_counter: 0,
        synthetic_lambdas: Vec::new(),
        bootstrap_methods: Vec::new(),
    };

    if has_self {
        ctx.locals.insert(
            "self".into(),
            (0, JvmType::Object(class_internal.to_string())),
        );
        ctx.next_slot = 1;
    }

    for (name, ty) in params {
        let slot = ctx.next_slot;
        ctx.next_slot += ty.slot_count();
        ctx.locals.insert(name.clone(), (slot, ty.clone()));
    }

    ctx.lower_body(body);

    let last_is_return = ctx
        .ops
        .last()
        .is_some_and(|op| matches!(op, JvmOp::Return(_)));
    if !last_is_return {
        ctx.ops.push(JvmOp::Return(return_ty.clone()));
    }

    LowerBodyResult {
        body: JvmMethodBody {
            max_locals: ctx.next_slot,
            ops: ctx.ops,
            exception_handlers: ctx.exception_handlers,
        },
        synthetic_lambdas: ctx.synthetic_lambdas,
        bootstrap_methods: ctx.bootstrap_methods,
    }
}

impl<'a> ExprLowering<'a> {
    fn alloc_label(&mut self) -> Label {
        let l = self.next_label;
        self.next_label += 1;
        l
    }

    fn push_scope(&mut self) {
        self.scope_stack.push(Vec::new());
    }

    // TODO(#068): Scope slots are never reclaimed — `pop_scope` restores name bindings
    // but does not reset `next_slot` to the value at scope entry. This means deeply
    // nested scopes inflate `max_locals` even though the inner slots are no longer live.
    // Fix: save `self.next_slot` in `push_scope` and restore it here.
    fn pop_scope(&mut self) {
        if let Some(undo_log) = self.scope_stack.pop() {
            for (name, prev) in undo_log.into_iter().rev() {
                match prev {
                    Some(binding) => {
                        self.locals.insert(name, binding);
                    }
                    None => {
                        self.locals.shift_remove(&name);
                    }
                }
            }
        }
    }

    fn alloc_local(&mut self, name: SmolStr, ty: JvmType) -> u16 {
        let slot = self.next_slot;
        self.next_slot += ty.slot_count();
        // Record undo entry before inserting the new binding.
        if let Some(undo_log) = self.scope_stack.last_mut() {
            let prev = self.locals.get(&name).cloned();
            undo_log.push((name.clone(), prev));
        }
        self.locals.insert(name, (slot, ty));
        slot
    }

    /// Returns a snapshot of the current locals array for StackMapTable frame generation.
    fn locals_snapshot(&self) -> Vec<JvmType> {
        let mut slot_map: Vec<Option<JvmType>> = vec![None; self.next_slot as usize];
        for (_name, (slot, ty)) in &self.locals {
            let s = *slot as usize;
            if s < slot_map.len() {
                slot_map[s] = Some(ty.clone());
            }
        }

        let mut result = Vec::new();
        let mut slot = 0usize;
        while slot < slot_map.len() {
            match &slot_map[slot] {
                Some(ty) => {
                    result.push(ty.clone());
                    slot += ty.slot_count() as usize;
                }
                None => {
                    result.push(JvmType::Void);
                    slot += 1;
                }
            }
        }
        result
    }

    fn resolve_variant_field_types(&self, path: &valen_ast::Path) -> IndexMap<String, JvmType> {
        let mut result = IndexMap::new();
        let segments: Vec<&str> = path.segments.iter().map(|s| s.name.as_str()).collect();
        let (enum_name, variant_name) = if segments.len() == 2 {
            (segments[0], segments[1])
        } else if segments.len() == 1 {
            ("", segments[0])
        } else {
            return result;
        };

        for def in self.hir.defs.values() {
            if let DefKind::Enum(enum_def) = &def.kind {
                if !enum_name.is_empty() && def.name != enum_name {
                    continue;
                }
                for variant in &enum_def.variants {
                    if variant.name == variant_name {
                        for (fname, tyref) in &variant.fields {
                            let jvm_ty =
                                crate::descriptor::tyref_to_jvm(tyref, self.pkg, &self.hir.imports);
                            result.insert(fname.to_string(), jvm_ty);
                        }
                        return result;
                    }
                }
            }
        }
        result
    }

    /// Emits a `JvmOp::Frame` hint with the current locals snapshot and the given stack.
    fn emit_frame(&mut self, stack: Vec<JvmType>) {
        let locals = self.locals_snapshot();
        self.ops.push(JvmOp::Frame { locals, stack });
    }

    /// Emits a frame using a previously captured locals snapshot instead of the
    /// current live locals. Used after `pop_scope()` to avoid stale slot gaps.
    fn emit_frame_with_locals(&mut self, locals: Vec<JvmType>, stack: Vec<JvmType>) {
        self.ops.push(JvmOp::Frame { locals, stack });
    }

    fn pop_if_needed(&mut self, ty: &Ty) {
        if matches!(ty, Ty::Prim(PrimTy::Unit | PrimTy::Nothing) | Ty::Error) {
            return;
        }
        let jvm_ty = self.ty_to_jvm(ty);
        if jvm_ty.is_wide() {
            self.ops.push(JvmOp::Pop2);
        } else {
            self.ops.push(JvmOp::Pop);
        }
    }

    fn ty_to_jvm(&self, ty: &Ty) -> JvmType {
        match ty {
            Ty::Prim(p) => match p {
                PrimTy::Int => JvmType::Int,
                PrimTy::Long => JvmType::Long,
                PrimTy::Float => JvmType::Float,
                PrimTy::Double => JvmType::Double,
                PrimTy::Bool => JvmType::Boolean,
                PrimTy::Char => JvmType::Char,
                PrimTy::Byte => JvmType::Byte,
                PrimTy::Short => JvmType::Short,
                PrimTy::String => JvmType::Object(JVM_STRING.to_string()),
                PrimTy::Unit => JvmType::Void,
                PrimTy::Nothing => JvmType::Void,
            },
            Ty::Named(n) => {
                if n == "Any" {
                    return JvmType::Object(JVM_OBJECT.to_string());
                }
                if let Some(target) = self.resolve_type_alias(n) {
                    return self.ty_to_jvm(&target);
                }
                JvmType::Object(crate::descriptor::resolve_type_internal_name(
                    n,
                    self.pkg,
                    &self.hir.imports,
                ))
            }
            Ty::Generic(n, _) => {
                if let Some(target) = self.resolve_type_alias(n) {
                    return self.ty_to_jvm(&target);
                }
                JvmType::Object(crate::descriptor::resolve_type_internal_name(
                    n,
                    self.pkg,
                    &self.hir.imports,
                ))
            }
            Ty::Nullable(inner) => {
                let inner_jvm = self.ty_to_jvm(inner);
                match JvmType::boxed_name(&inner_jvm) {
                    Some(boxed) => JvmType::Object(boxed.to_string()),
                    None => inner_jvm,
                }
            }
            Ty::RefMut(inner) => {
                let class = ref_mut_wrapper_class(inner);
                JvmType::Object(class)
            }
            Ty::TypeParam(_) | Ty::Fn(_, _) | Ty::Error => JvmType::Object(JVM_OBJECT.to_string()),
        }
    }

    fn is_newtype_or_class_ctor(&self, name: &str) -> bool {
        use valen_hir::DefKind;
        self.hir.defs.values().any(|d| {
            d.name == name
                && matches!(
                    &d.kind,
                    DefKind::NewType(_) | DefKind::Class(_) | DefKind::DataClass(_)
                )
        })
    }

    fn is_enum_variant(&self, enum_name: &str, variant_name: &str) -> bool {
        use valen_hir::DefKind;
        self.hir.defs.values().any(|d| {
            d.name == enum_name
                && matches!(&d.kind, DefKind::Enum(e) if e.variants.iter().any(|v| v.name == variant_name))
        })
    }

    fn is_enum_unit_variant(&self, enum_name: &str, variant_name: &str) -> bool {
        use valen_hir::DefKind;
        self.hir.defs.values().any(|d| {
            d.name == enum_name
                && matches!(&d.kind, DefKind::Enum(e) if e.variants.iter().any(|v| v.name == variant_name && v.fields.is_empty()))
        })
    }

    fn resolve_type_alias(&self, name: &str) -> Option<Ty> {
        use valen_hir::DefKind;
        for def in self.hir.defs.values() {
            if def.name == name {
                if let DefKind::TypeAlias(alias) = &def.kind {
                    let target = valen_hir::tyref_to_ty(&alias.target);
                    return Some(target);
                }
            }
        }
        None
    }

    /// Extracts the Valen type name from a `Ty` for trait/interface checking.
    fn receiver_type_name(&self, ty: &Ty) -> Option<String> {
        match ty {
            Ty::Named(n) => Some(n.to_string()),
            Ty::Generic(n, _) => Some(n.to_string()),
            _ => None,
        }
    }

    /// Returns `true` if the given type name corresponds to a trait (JVM interface).
    ///
    /// Checks both user-defined traits in the HIR and enum definitions (which are
    /// emitted as sealed interfaces).
    fn is_trait_or_interface(&self, type_name: &str) -> bool {
        use valen_hir::DefKind;
        for def in self.hir.defs.values() {
            if def.name == type_name {
                match &def.kind {
                    DefKind::Trait(_) => return true,
                    DefKind::Enum(_) => return true,
                    _ => return false,
                }
            }
        }
        if let Some(foreign) = self.hir.foreign_types.get(type_name) {
            return foreign.is_interface;
        }
        false
    }

    fn lower_body(&mut self, body: &TypedBody) {
        self.push_scope();
        for stmt in &body.stmts {
            self.lower_stmt(stmt);
        }
        if let Some(tail) = &body.tail {
            self.lower_expr(tail);
        }
        self.pop_scope();
    }

    fn lower_stmt(&mut self, stmt: &TypedStmt) {
        match stmt {
            TypedStmt::Let { name, ty, init, .. } => {
                let jvm_ty = self.ty_to_jvm(ty);
                self.lower_expr(init);
                let slot = self.alloc_local(name.clone(), jvm_ty.clone());
                self.ops.push(JvmOp::StoreLocal(slot, jvm_ty));
            }
            TypedStmt::LetElse {
                pattern,
                scrutinee,
                ty,
                else_body,
                ..
            } => {
                self.lower_let_else(pattern, scrutinee, ty, else_body);
            }
            TypedStmt::Expr(expr) => {
                self.lower_expr(expr);
                self.pop_if_needed(&expr.ty);
            }
            TypedStmt::ExprSemi(expr) => {
                self.lower_expr(expr);
                self.pop_if_needed(&expr.ty);
            }
        }
    }

    fn lower_expr(&mut self, expr: &TypedExpr) {
        match &expr.kind {
            TypedExprKind::IntLit(n) => {
                if matches!(expr.ty, Ty::Prim(PrimTy::Long)) {
                    self.ops.push(JvmOp::PushLong(*n));
                } else {
                    match i32::try_from(*n) {
                        Ok(i) => self.ops.push(JvmOp::PushInt(i)),
                        Err(_) => {
                            eprintln!(
                                "[codegen] error: integer literal {} overflows i32 range, \
                                 clamping to i32::MAX",
                                n
                            );
                            self.ops.push(JvmOp::PushInt(i32::MAX));
                        }
                    }
                }
            }
            TypedExprKind::LongLit(n) => {
                self.ops.push(JvmOp::PushLong(*n));
            }
            TypedExprKind::FloatLit(n) => {
                if matches!(expr.ty, Ty::Prim(PrimTy::Double)) {
                    self.ops.push(JvmOp::PushDouble(*n));
                } else {
                    self.ops.push(JvmOp::PushFloat(*n as f32));
                }
            }
            TypedExprKind::Float32Lit(n) => {
                self.ops.push(JvmOp::PushFloat(*n));
            }
            TypedExprKind::CharLit(c) => {
                self.ops.push(JvmOp::PushInt(*c as i32));
            }
            TypedExprKind::StringLit(s) => {
                self.ops.push(JvmOp::PushString(s.to_string()));
            }
            TypedExprKind::BoolLit(b) => {
                self.ops.push(JvmOp::PushInt(if *b { 1 } else { 0 }));
            }
            TypedExprKind::UnitLit => {}
            TypedExprKind::NullLit => {
                self.ops.push(JvmOp::PushNull);
            }
            TypedExprKind::LocalVar(name) => {
                if let Some((slot, ty)) = self.locals.get(name) {
                    if name == "self" {
                        self.ops.push(JvmOp::LoadThis);
                    } else {
                        self.ops.push(JvmOp::LoadLocal(*slot, ty.clone()));
                    }
                }
            }
            TypedExprKind::FieldAccess { receiver, field } => {
                self.lower_expr(receiver);
                let owner_ty = self.ty_to_jvm(&receiver.ty);
                let field_ty = self.ty_to_jvm(&expr.ty);
                if let JvmType::Object(owner) = owner_ty {
                    self.ops.push(JvmOp::GetField {
                        owner,
                        name: field.to_string(),
                        descriptor: field_ty,
                    });
                }
            }
            TypedExprKind::Call {
                callee,
                args,
                type_args,
            } => {
                self.lower_call(callee, args, type_args, &expr.ty);
            }
            TypedExprKind::MethodCall {
                receiver,
                method,
                args,
            } => {
                let handled = self.try_lower_iterator_intrinsic(receiver, method, args, &expr.ty)
                    || self.try_lower_numeric_conversion(receiver, method);
                if !handled {
                    self.lower_expr(receiver);
                    for arg in args {
                        self.lower_expr(arg);
                    }
                    let receiver_ty = self.ty_to_jvm(&receiver.ty);
                    let type_name_str = self.receiver_type_name(&receiver.ty);
                    // For foreign (Java) methods, use actual Java parameter and return types
                    // from the class file instead of Valen-inferred types.
                    let (param_tys, ret_ty) =
                        if let Some(ref tn) = type_name_str {
                            if let Some(info) = self.hir.foreign_types.get(tn.as_str()) {
                                if let Some(m) = info.methods.iter().find(|m| {
                                    m.name == method.as_str() && m.params.len() == args.len()
                                }) {
                                    (
                                        m.params
                                            .iter()
                                            .map(|p| tyref_to_jvm(p, self.pkg, &self.hir.imports))
                                            .collect(),
                                        tyref_to_jvm(&m.return_ty, self.pkg, &self.hir.imports),
                                    )
                                } else {
                                    (
                                        args.iter().map(|a| self.ty_to_jvm(&a.ty)).collect(),
                                        self.ty_to_jvm(&expr.ty),
                                    )
                                }
                            } else {
                                (
                                    args.iter().map(|a| self.ty_to_jvm(&a.ty)).collect(),
                                    self.ty_to_jvm(&expr.ty),
                                )
                            }
                        } else {
                            (
                                args.iter().map(|a| self.ty_to_jvm(&a.ty)).collect(),
                                self.ty_to_jvm(&expr.ty),
                            )
                        };
                    if let JvmType::Object(owner) = receiver_ty {
                        let needs_pop = !matches!(ret_ty, JvmType::Void)
                            && matches!(expr.ty, Ty::Prim(PrimTy::Unit) | Ty::Error);
                        let wide_pop = ret_ty.is_wide();
                        let type_name = self.receiver_type_name(&receiver.ty);
                        if type_name
                            .as_deref()
                            .is_some_and(|n| self.is_trait_or_interface(n))
                        {
                            self.ops.push(JvmOp::InvokeInterface {
                                owner,
                                name: method.to_string(),
                                params: param_tys,
                                ret: ret_ty,
                            });
                        } else {
                            self.ops.push(JvmOp::InvokeVirtual {
                                owner,
                                name: method.to_string(),
                                params: param_tys,
                                ret: ret_ty,
                            });
                        }
                        if needs_pop {
                            if wide_pop {
                                self.ops.push(JvmOp::Pop2);
                            } else {
                                self.ops.push(JvmOp::Pop);
                            }
                        }
                    }
                }
            }
            TypedExprKind::Binary { op, lhs, rhs } => {
                self.lower_binary(*op, lhs, rhs, &expr.ty);
            }
            TypedExprKind::Unary { op, expr: inner } => {
                self.lower_expr(inner);
                match op {
                    UnaryOp::Neg => {
                        let ty = self.ty_to_jvm(&inner.ty);
                        self.ops.push(JvmOp::Neg(ty));
                    }
                    UnaryOp::Not => {
                        self.ops.push(JvmOp::PushInt(1));
                        self.ops.push(JvmOp::Bitwise(BitwiseOp::Xor, JvmType::Int));
                    }
                }
            }
            TypedExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.lower_if(cond, then_branch, else_branch.as_deref(), &expr.ty);
            }
            TypedExprKind::Match { scrutinee, arms } => {
                self.lower_match(scrutinee, arms, &expr.ty);
            }
            TypedExprKind::Block(body) => {
                self.lower_body(body);
            }
            TypedExprKind::Return(val) => {
                if let Some(v) = val {
                    self.lower_expr(v);
                    self.ops.push(JvmOp::Return(self.return_ty.clone()));
                } else {
                    self.ops.push(JvmOp::Return(JvmType::Void));
                }
            }
            TypedExprKind::Break(val) => {
                if let Some(v) = val {
                    self.lower_expr(v);
                }
                if let Some(ctx) = self.loop_stack.last() {
                    self.ops.push(JvmOp::Goto(ctx.break_label));
                }
            }
            TypedExprKind::Continue => {
                if let Some(ctx) = self.loop_stack.last() {
                    self.ops.push(JvmOp::Goto(ctx.continue_label));
                }
            }
            TypedExprKind::Assign { target, value } => {
                self.lower_assign(target, value);
            }
            TypedExprKind::While { cond, body } => {
                self.lower_while(cond, body);
            }
            TypedExprKind::Loop { body } => {
                self.lower_loop(body);
            }
            TypedExprKind::For { var, iter, body } => {
                self.lower_for(var, iter, body);
            }
            TypedExprKind::Lambda { params, body } => {
                self.lower_lambda(params, body, &expr.ty);
            }
            TypedExprKind::Range {
                start,
                end,
                inclusive,
            } => {
                self.lower_range(start.as_deref(), end.as_deref(), *inclusive, &expr.ty);
            }
            TypedExprKind::StringInterp(parts) => {
                self.lower_string_interp(parts);
            }
            TypedExprKind::Safe(body) => {
                self.lower_safe(body, &expr.ty);
            }
            TypedExprKind::Try {
                inner, is_option, ..
            } => {
                self.lower_try(inner, *is_option, &expr.ty);
            }
            TypedExprKind::IfLet {
                pattern,
                expr: scrutinee,
                then_branch,
                else_branch,
            } => {
                self.lower_if_let(
                    pattern,
                    scrutinee,
                    then_branch,
                    else_branch.as_deref(),
                    &expr.ty,
                );
            }
            TypedExprKind::WhileLet {
                pattern,
                expr: scrutinee,
                body,
            } => {
                self.lower_while_let(pattern, scrutinee, body);
            }
            TypedExprKind::ListLiteral(elements) => {
                self.lower_list_literal(elements, &expr.ty);
            }
            TypedExprKind::MapLiteral(entries) => {
                self.lower_map_literal(entries, &expr.ty);
            }
            TypedExprKind::Unsafe(body) => {
                self.push_scope();
                self.lower_body(body);
                self.pop_scope();
            }
            TypedExprKind::Cast {
                expr: inner,
                target_ty,
            } => {
                self.lower_expr(inner);
                self.lower_cast(&inner.ty, target_ty);
            }
            TypedExprKind::Deref { expr: inner } => {
                self.lower_expr(inner);
                self.lower_deref_read(&inner.ty);
            }
            TypedExprKind::RefMutCreate { expr: inner } => {
                self.lower_ref_mut_create(inner);
            }
            TypedExprKind::DerefAssign { target, value } => {
                self.lower_expr(target);
                self.lower_expr(value);
                self.lower_deref_write(&target.ty, &value.ty);
            }
            TypedExprKind::Error => {}
        }
    }

    fn lower_call(
        &mut self,
        callee: &TypedExpr,
        args: &[TypedExpr],
        type_args: &IndexMap<SmolStr, Ty>,
        result_ty: &Ty,
    ) {
        if let TypedExprKind::LocalVar(name) = &callee.kind {
            if !matches!(callee.ty, Ty::Fn(_, _))
                && self.try_inline_call(name, args, type_args, result_ty)
            {
                return;
            }
        }

        let ret_ty = self.ty_to_jvm(result_ty);
        let param_tys: Vec<JvmType> = args.iter().map(|a| self.ty_to_jvm(&a.ty)).collect();

        match &callee.kind {
            TypedExprKind::LocalVar(name) => {
                if name == "println" || name == "print" {
                    self.lower_builtin_print(name, args);
                    return;
                }
                if name == "iter" {
                    self.lower_builtin_iter(args);
                    return;
                }
                // Check if the callee is a function-typed local variable (lambda).
                if matches!(callee.ty, Ty::Fn(_, _)) {
                    self.lower_lambda_call(callee, args, result_ty);
                    return;
                }
                // Qualified static calls: "ClassName::method" → invokestatic on that class
                // Must be checked BEFORE constructor detection, since
                // WidgetBuilder::create() returns WidgetBuilder but is not a ctor.
                if let Some((class_name, method_name)) = name.split_once("::") {
                    let owner = crate::descriptor::resolve_type_internal_name(
                        class_name,
                        self.pkg,
                        &self.hir.imports,
                    );
                    // Unit enum variant: EnumType::Variant with no args
                    // → getstatic EnumType$Variant.INSTANCE
                    if args.is_empty() && self.is_enum_unit_variant(class_name, method_name) {
                        let variant_class = format!("{}${}", owner, method_name);
                        self.ops.push(JvmOp::GetStatic {
                            owner: variant_class.clone(),
                            name: "INSTANCE".to_string(),
                            descriptor: JvmType::Object(variant_class),
                        });
                        return;
                    }
                    // Enum variant with payload: EnumType::Variant(args...)
                    // → new EnumType$Variant + invokespecial <init>
                    if self.is_enum_variant(class_name, method_name) {
                        let variant_class = format!("{}${}", owner, method_name);
                        self.ops.push(JvmOp::New(variant_class.clone()));
                        self.ops.push(JvmOp::Dup);
                        for arg in args {
                            self.lower_expr(arg);
                        }
                        self.ops.push(JvmOp::InvokeSpecial {
                            owner: variant_class,
                            name: "<init>".to_string(),
                            params: param_tys,
                            ret: JvmType::Void,
                        });
                        return;
                    }
                    for arg in args {
                        self.lower_expr(arg);
                    }
                    self.ops.push(JvmOp::InvokeStatic {
                        owner,
                        name: method_name.to_string(),
                        params: param_tys,
                        ret: ret_ty,
                    });
                    return;
                }
                // Constructor call: Meters(10.0), Widget(...), ArrayList(), etc.
                if let Ty::Named(n) = result_ty {
                    if self.is_newtype_or_class_ctor(n) {
                        let owner = crate::descriptor::resolve_type_internal_name(
                            n,
                            self.pkg,
                            &self.hir.imports,
                        );
                        self.ops.push(JvmOp::New(owner.clone()));
                        self.ops.push(JvmOp::Dup);
                        for arg in args {
                            self.lower_expr(arg);
                        }
                        self.ops.push(JvmOp::InvokeSpecial {
                            owner,
                            name: "<init>".to_string(),
                            params: param_tys,
                            ret: JvmType::Void,
                        });
                        return;
                    }
                    // Foreign (Java) constructor — use actual Java constructor descriptor
                    if let Some(info) = self.hir.foreign_types.get(n.as_str()) {
                        let owner = info.internal_name.clone();
                        let ctor_params = info
                            .constructors
                            .iter()
                            .find(|c| c.params.len() == args.len())
                            .map(|c| {
                                c.params
                                    .iter()
                                    .map(|p| tyref_to_jvm(p, self.pkg, &self.hir.imports))
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or(param_tys);
                        self.ops.push(JvmOp::New(owner.clone()));
                        self.ops.push(JvmOp::Dup);
                        for arg in args {
                            self.lower_expr(arg);
                        }
                        self.ops.push(JvmOp::InvokeSpecial {
                            owner,
                            name: "<init>".to_string(),
                            params: ctor_params,
                            ret: JvmType::Void,
                        });
                        return;
                    }
                }
                for arg in args {
                    self.lower_expr(arg);
                }
                self.ops.push(JvmOp::InvokeStatic {
                    owner: self.class_internal.to_string(),
                    name: name.to_string(),
                    params: param_tys,
                    ret: ret_ty,
                });
            }
            _ => {
                // Non-local callees that are function-typed also need lambda call path.
                if matches!(callee.ty, Ty::Fn(_, _)) {
                    self.lower_lambda_call(callee, args, result_ty);
                    return;
                }

                // Check if this is a newtype or class constructor call
                if let Ty::Named(n) = result_ty {
                    if self.is_newtype_or_class_ctor(n) {
                        let owner = crate::descriptor::resolve_type_internal_name(
                            n,
                            self.pkg,
                            &self.hir.imports,
                        );
                        self.ops.push(JvmOp::New(owner.clone()));
                        self.ops.push(JvmOp::Dup);
                        for arg in args {
                            self.lower_expr(arg);
                        }
                        self.ops.push(JvmOp::InvokeSpecial {
                            owner,
                            name: "<init>".to_string(),
                            params: param_tys,
                            ret: JvmType::Void,
                        });
                        return;
                    }
                }

                let owner = match &callee.ty {
                    Ty::Named(n) => crate::descriptor::resolve_type_internal_name(
                        n,
                        self.pkg,
                        &self.hir.imports,
                    ),
                    _ => self.class_internal.to_string(),
                };
                let call_name = match &callee.kind {
                    TypedExprKind::FieldAccess { field, .. } => field.to_string(),
                    _ => "apply".to_string(),
                };
                for arg in args {
                    self.lower_expr(arg);
                }
                self.ops.push(JvmOp::InvokeStatic {
                    owner,
                    name: call_name,
                    params: param_tys,
                    ret: ret_ty,
                });
            }
        }
    }

    /// Attempts to inline a call to an `inline fn`. Returns true if inlined.
    fn try_inline_call(
        &mut self,
        name: &str,
        args: &[TypedExpr],
        type_args: &IndexMap<SmolStr, Ty>,
        _result_ty: &Ty,
    ) -> bool {
        let Some((def_id, fn_def)) = self.find_inline_fn(name) else {
            return false;
        };
        let Some(body) = self.typed_bodies.get(&def_id) else {
            return false;
        };
        let body = body.clone();
        let params: Vec<(SmolStr, bool)> = fn_def
            .params
            .iter()
            .filter(|p| !p.is_self)
            .map(|p| (p.name.clone(), p.is_self))
            .collect();
        let type_args = type_args.clone();

        self.push_scope();
        for (i, (param_name, _)) in params.iter().enumerate() {
            if let Some(arg) = args.get(i) {
                self.lower_expr(arg);
                let jvm_ty = self.ty_to_jvm(&arg.ty);
                let slot = self.next_slot;
                self.next_slot += jvm_ty.slot_count();
                self.ops.push(JvmOp::StoreLocal(slot, jvm_ty.clone()));
                self.scope_stack
                    .last_mut()
                    .unwrap()
                    .push((param_name.clone(), self.locals.get(param_name).cloned()));
                self.locals.insert(param_name.clone(), (slot, jvm_ty));
            }
        }

        // Lower the inlined body, substituting reified types via instanceof/checkcast.
        self.lower_inline_body(&body, &type_args);

        self.pop_scope();
        true
    }

    fn find_inline_fn(&self, name: &str) -> Option<(DefId, FnDef)> {
        for &def_id in self.hir.lookup_by_name(name) {
            if let Some(def) = self.hir.defs.get(&def_id) {
                if let DefKind::Fn(fn_def) = &def.kind {
                    if fn_def.is_inline {
                        return Some((def_id, fn_def.clone()));
                    }
                }
            }
        }
        None
    }

    fn lower_inline_body(&mut self, body: &TypedBody, type_args: &IndexMap<SmolStr, Ty>) {
        for stmt in &body.stmts {
            self.lower_inline_stmt(stmt, type_args);
        }
        if let Some(tail) = &body.tail {
            self.lower_inline_expr(tail, type_args);
        }
    }

    fn lower_inline_stmt(&mut self, stmt: &TypedStmt, type_args: &IndexMap<SmolStr, Ty>) {
        match stmt {
            TypedStmt::Let {
                name,
                ty,
                init,
                mutable: _,
                ..
            } => {
                let resolved_ty = self.resolve_reified_ty(ty, type_args);
                self.lower_inline_expr(init, type_args);
                let jvm_ty = self.ty_to_jvm(&resolved_ty);
                let slot = self.next_slot;
                self.next_slot += jvm_ty.slot_count();
                self.ops.push(JvmOp::StoreLocal(slot, jvm_ty.clone()));
                self.scope_stack
                    .last_mut()
                    .unwrap()
                    .push((name.clone(), self.locals.get(name).cloned()));
                self.locals.insert(name.clone(), (slot, jvm_ty));
            }
            TypedStmt::Expr(e) | TypedStmt::ExprSemi(e) => {
                self.lower_inline_expr(e, type_args);
                if matches!(stmt, TypedStmt::ExprSemi(_)) {
                    let jvm_ty = self.ty_to_jvm(&e.ty);
                    if jvm_ty != JvmType::Void {
                        self.ops.push(JvmOp::Pop);
                    }
                }
            }
            _ => {
                self.lower_stmt(stmt);
            }
        }
    }

    fn lower_inline_expr(&mut self, expr: &TypedExpr, type_args: &IndexMap<SmolStr, Ty>) {
        match &expr.kind {
            TypedExprKind::Cast {
                expr: inner,
                target_ty,
            } => {
                let resolved = self.resolve_reified_ty(target_ty, type_args);
                self.lower_inline_expr(inner, type_args);
                if let Ty::Named(n) = &resolved {
                    let internal = crate::descriptor::resolve_type_internal_name(
                        n,
                        self.pkg,
                        &self.hir.imports,
                    );
                    self.ops.push(JvmOp::Checkcast(internal));
                }
            }
            _ => {
                self.lower_expr(expr);
            }
        }
    }

    fn resolve_reified_ty(&self, ty: &Ty, type_args: &IndexMap<SmolStr, Ty>) -> Ty {
        match ty {
            Ty::TypeParam(name) => type_args.get(name).cloned().unwrap_or_else(|| ty.clone()),
            Ty::Generic(n, args) => Ty::Generic(
                n.clone(),
                args.iter()
                    .map(|t| self.resolve_reified_ty(t, type_args))
                    .collect(),
            ),
            Ty::Nullable(inner) => {
                Ty::Nullable(Box::new(self.resolve_reified_ty(inner, type_args)))
            }
            Ty::Fn(params, ret) => Ty::Fn(
                params
                    .iter()
                    .map(|t| self.resolve_reified_ty(t, type_args))
                    .collect(),
                Box::new(self.resolve_reified_ty(ret, type_args)),
            ),
            Ty::RefMut(inner) => Ty::RefMut(Box::new(self.resolve_reified_ty(inner, type_args))),
            _ => ty.clone(),
        }
    }

    /// Lowers a lambda expression into an `invokedynamic` call site and a synthetic method.
    ///
    /// The lambda body is compiled into a `private static synthetic` method, and an
    /// `invokedynamic` instruction referencing `LambdaMetafactory.metafactory` is emitted
    /// at the call site to create a functional interface proxy.
    fn lower_lambda(&mut self, params: &[(SmolStr, Ty)], body: &TypedExpr, _lambda_ty: &Ty) {
        if params.len() > 22 {
            eprintln!(
                "[codegen] error: lambda with {} parameters exceeds arity limit (max 22). \
                 Emitting UnsupportedOperationException.",
                params.len()
            );
            self.ops
                .extend(crate::jvm_ir::throw_unsupported_ops(&format!(
                    "lambda with {} parameters exceeds arity limit (max 22)",
                    params.len()
                )));
            return;
        }

        let lambda_idx = self.lambda_counter;
        self.lambda_counter += 1;
        let synth_name = format!("lambda${lambda_idx}");

        // Build the synthetic method's param types and return type.
        let param_types: Vec<JvmType> = params.iter().map(|(_, ty)| self.ty_to_jvm(ty)).collect();
        let return_type = self.ty_to_jvm(&body.ty);

        // Lower the lambda body into a separate method body.
        let param_pairs: Vec<(SmolStr, JvmType)> = params
            .iter()
            .map(|(name, ty)| (name.clone(), self.ty_to_jvm(ty)))
            .collect();
        let synth_body = {
            let tb = TypedBody {
                stmts: vec![],
                tail: Some(Box::new(body.clone())),
                ty: body.ty.clone(),
            };
            let result = lower_body(
                &tb,
                self.class_internal,
                &param_pairs,
                &return_type,
                false,
                self.pkg,
                self.hir,
                self.typed_bodies,
            );
            // Hoist any nested lambdas up.
            // Nested lambda bootstrap indices are offset by the current bootstrap table size.
            let base_bsm = self.bootstrap_methods.len() as u32;
            self.synthetic_lambdas.extend(result.synthetic_lambdas);
            self.bootstrap_methods.extend(result.bootstrap_methods);
            // Fix up bootstrap indices for nested lambdas in the body ops.
            let mut body_result = result.body;
            if base_bsm > 0 {
                for op in &mut body_result.ops {
                    if let JvmOp::InvokeDynamic {
                        bootstrap_index, ..
                    } = op
                    {
                        *bootstrap_index += base_bsm as u16;
                    }
                }
            }
            body_result
        };

        // Store the synthetic method.
        self.synthetic_lambdas.push(SyntheticLambda {
            name: synth_name.clone(),
            params: param_types.clone(),
            return_type: return_type.clone(),
            body: synth_body,
        });

        // Determine the functional interface based on arity.
        let (func_iface, sam_name, erased_sam_desc, specialized_sam_desc) =
            self.lambda_functional_interface(&param_types, &return_type);

        // Build the implementation method descriptor (uses actual primitive types).
        let impl_descriptor = crate::descriptor::jvm_method_descriptor(&param_types, &return_type);

        // Register the bootstrap method (LambdaMetafactory.metafactory).
        let bsm_index = self.bootstrap_methods.len() as u16;
        self.bootstrap_methods.push(JvmBootstrapMethod {
            method_ref: BootstrapMethodRef::LambdaMetafactory,
            arguments: vec![
                // arg0: erased SAM type  e.g. (Ljava/lang/Object;)Ljava/lang/Object;
                BootstrapArg::MethodType(erased_sam_desc),
                // arg1: MethodHandle to the implementation method
                BootstrapArg::MethodHandle {
                    kind: MethodHandleKind::InvokeStatic,
                    owner: self.class_internal.to_string(),
                    name: synth_name,
                    descriptor: impl_descriptor,
                },
                // arg2: specialized SAM type  e.g. (Ljava/lang/Integer;)Ljava/lang/Integer;
                BootstrapArg::MethodType(specialized_sam_desc),
            ],
        });

        // Emit the invokedynamic instruction.
        // Factory type for no-capture lambda: "()L<func_iface>;"
        let factory_descriptor = format!("()L{func_iface};");
        self.ops.push(JvmOp::InvokeDynamic {
            bootstrap_index: bsm_index,
            name: sam_name,
            descriptor: factory_descriptor,
        });
    }

    /// Returns `(interface_internal, sam_name, erased_descriptor, specialized_descriptor)`
    /// for the given lambda parameter/return types.
    fn lambda_functional_interface(
        &self,
        param_types: &[JvmType],
        return_type: &JvmType,
    ) -> (String, String, String, String) {
        let obj = "Ljava/lang/Object;";
        match param_types.len() {
            0 => {
                // java.util.function.Supplier<R>
                let erased = format!("(){obj}");
                let specialized_ret = self.boxed_descriptor(return_type);
                let specialized = format!("(){specialized_ret}");
                (
                    "java/util/function/Supplier".to_string(),
                    "get".to_string(),
                    erased,
                    specialized,
                )
            }
            1 => {
                // java.util.function.Function<T, R>
                let erased = format!("({obj}){obj}");
                let specialized_param = self.boxed_descriptor(&param_types[0]);
                let specialized_ret = self.boxed_descriptor(return_type);
                let specialized = format!("({specialized_param}){specialized_ret}");
                (
                    "java/util/function/Function".to_string(),
                    "apply".to_string(),
                    erased,
                    specialized,
                )
            }
            2 => {
                // java.util.function.BiFunction<T, U, R>
                let erased = format!("({obj}{obj}){obj}");
                let s0 = self.boxed_descriptor(&param_types[0]);
                let s1 = self.boxed_descriptor(&param_types[1]);
                let sr = self.boxed_descriptor(return_type);
                let specialized = format!("({s0}{s1}){sr}");
                (
                    "java/util/function/BiFunction".to_string(),
                    "apply".to_string(),
                    erased,
                    specialized,
                )
            }
            n @ 3..=22 => {
                // valen/core/FunctionN<A, B, ..., R> — compiler-generated interface
                let iface = format!("valen/core/Function{n}");
                let erased_params: String = (0..n).map(|_| obj.to_string()).collect::<String>();
                let erased = format!("({erased_params}){obj}");
                let specialized_params: String = param_types
                    .iter()
                    .map(|t| self.boxed_descriptor(t))
                    .collect::<String>();
                let sr = self.boxed_descriptor(return_type);
                let specialized = format!("({specialized_params}){sr}");
                (iface, "apply".to_string(), erased, specialized)
            }
            _ => {
                unreachable!(
                    "lambda_functional_interface called with {} params; \
                     should have been caught in lower_lambda",
                    param_types.len()
                );
            }
        }
    }

    /// Returns the boxed type descriptor for a JVM type.
    /// Primitive types are mapped to their wrapper classes; reference types stay as-is.
    fn boxed_descriptor(&self, ty: &JvmType) -> String {
        match JvmType::boxed_name(ty) {
            Some(boxed) => format!("L{boxed};"),
            None => crate::descriptor::jvm_type_descriptor(ty),
        }
    }

    /// Emits boxing instructions for a primitive value on the stack.
    fn emit_box(&mut self, ty: &JvmType) {
        if let Some(boxed) = JvmType::boxed_name(ty) {
            self.ops.push(JvmOp::InvokeStatic {
                owner: boxed.to_string(),
                name: "valueOf".to_string(),
                params: vec![ty.clone()],
                ret: JvmType::Object(boxed.to_string()),
            });
        }
    }

    /// Emits unboxing instructions: checkcast + intValue/longValue/etc.
    fn emit_unbox(&mut self, ty: &JvmType) {
        if let Some(boxed) = JvmType::boxed_name(ty) {
            self.ops.push(JvmOp::Checkcast(boxed.to_string()));
            let unbox_method = match ty {
                JvmType::Int => "intValue",
                JvmType::Long => "longValue",
                JvmType::Float => "floatValue",
                JvmType::Double => "doubleValue",
                JvmType::Boolean => "booleanValue",
                JvmType::Char => "charValue",
                JvmType::Byte => "byteValue",
                JvmType::Short => "shortValue",
                _ => return,
            };
            self.ops.push(JvmOp::InvokeVirtual {
                owner: boxed.to_string(),
                name: unbox_method.to_string(),
                params: vec![],
                ret: ty.clone(),
            });
        }
    }

    /// Lowers a call to a function-typed local variable (lambda invocation).
    ///
    /// Loads the lambda reference, boxes primitive arguments, calls the SAM method
    /// via `invokeinterface`, then unboxes the result if needed.
    fn lower_lambda_call(&mut self, callee: &TypedExpr, args: &[TypedExpr], result_ty: &Ty) {
        let ret_ty = self.ty_to_jvm(result_ty);

        // Load the functional interface reference.
        self.lower_expr(callee);

        if args.len() > 22 {
            self.ops.push(JvmOp::Pop);
            self.ops
                .extend(crate::jvm_ir::throw_unsupported_ops(&format!(
                    "lambda call with {} arguments exceeds arity limit (max 22)",
                    args.len()
                )));
            return;
        }
        let func_iface: String = match args.len() {
            0 => "java/util/function/Supplier".to_string(),
            1 => "java/util/function/Function".to_string(),
            2 => "java/util/function/BiFunction".to_string(),
            n @ 3..=22 => format!("valen/core/Function{n}"),
            _ => unreachable!(),
        };
        let sam_name = match args.len() {
            0 => "get",
            _ => "apply",
        };

        // Box each argument and emit.
        let mut erased_params = Vec::new();
        for arg in args {
            self.lower_expr(arg);
            let arg_ty = self.ty_to_jvm(&arg.ty);
            self.emit_box(&arg_ty);
            erased_params.push(JvmType::Object(JVM_OBJECT.to_string()));
        }

        // Call via invokeinterface on the functional interface.
        self.ops.push(JvmOp::InvokeInterface {
            owner: func_iface.to_string(),
            name: sam_name.to_string(),
            params: erased_params,
            ret: JvmType::Object(JVM_OBJECT.to_string()),
        });

        // Unbox the result if the expected type is primitive.
        if JvmType::boxed_name(&ret_ty).is_some() {
            self.emit_unbox(&ret_ty);
        } else if let JvmType::Object(ref name) = ret_ty {
            if name != JVM_OBJECT {
                self.ops.push(JvmOp::Checkcast(name.clone()));
            }
        }
    }

    fn lower_binary(&mut self, op: BinaryOp, lhs: &TypedExpr, rhs: &TypedExpr, _result_ty: &Ty) {
        match op {
            BinaryOp::And => {
                self.lower_short_circuit_and(lhs, rhs);
                return;
            }
            BinaryOp::Or => {
                self.lower_short_circuit_or(lhs, rhs);
                return;
            }
            _ => {}
        }

        self.lower_expr(lhs);
        self.lower_expr(rhs);
        let operand_ty = self.ty_to_jvm(&lhs.ty);

        match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                let aop = match op {
                    BinaryOp::Add => ArithOp::Add,
                    BinaryOp::Sub => ArithOp::Sub,
                    BinaryOp::Mul => ArithOp::Mul,
                    BinaryOp::Div => ArithOp::Div,
                    BinaryOp::Rem => ArithOp::Rem,
                    _ => unreachable!(),
                };
                self.ops.push(JvmOp::Arith(aop, operand_ty));
            }
            BinaryOp::Eq
            | BinaryOp::Ne
            | BinaryOp::Lt
            | BinaryOp::Le
            | BinaryOp::Gt
            | BinaryOp::Ge => {
                self.lower_comparison(op, &operand_ty);
            }
            BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::Shl
            | BinaryOp::Shr => {
                let bop = match op {
                    BinaryOp::BitAnd => BitwiseOp::And,
                    BinaryOp::BitOr => BitwiseOp::Or,
                    BinaryOp::BitXor => BitwiseOp::Xor,
                    BinaryOp::Shl => BitwiseOp::Shl,
                    BinaryOp::Shr => BitwiseOp::Shr,
                    _ => unreachable!(),
                };
                self.ops.push(JvmOp::Bitwise(bop, operand_ty));
            }
            BinaryOp::And | BinaryOp::Or => unreachable!(),
            BinaryOp::RefEq | BinaryOp::RefNe => {
                self.lower_comparison(op, &operand_ty);
            }
        }
    }

    fn lower_comparison(&mut self, op: BinaryOp, operand_ty: &JvmType) {
        let false_label = self.alloc_label();
        let end_label = self.alloc_label();

        match operand_ty {
            JvmType::Int | JvmType::Byte | JvmType::Short | JvmType::Char | JvmType::Boolean => {
                let branch = match op {
                    BinaryOp::Eq => JvmOp::IfICmpNe(false_label),
                    BinaryOp::Ne => JvmOp::IfICmpEq(false_label),
                    BinaryOp::Lt => JvmOp::IfICmpGe(false_label),
                    BinaryOp::Le => JvmOp::IfICmpGt(false_label),
                    BinaryOp::Gt => JvmOp::IfICmpLe(false_label),
                    BinaryOp::Ge => JvmOp::IfICmpLt(false_label),
                    _ => unreachable!(),
                };
                self.ops.push(branch);
            }
            JvmType::Long => {
                self.ops.push(JvmOp::Cmp(CmpKind::LCmp));
                self.push_cmp_branch(op, false_label);
            }
            JvmType::Float => {
                let cmp = match op {
                    BinaryOp::Lt | BinaryOp::Le => CmpKind::FCmpG,
                    _ => CmpKind::FCmpL,
                };
                self.ops.push(JvmOp::Cmp(cmp));
                self.push_cmp_branch(op, false_label);
            }
            JvmType::Double => {
                let cmp = match op {
                    BinaryOp::Lt | BinaryOp::Le => CmpKind::DCmpG,
                    _ => CmpKind::DCmpL,
                };
                self.ops.push(JvmOp::Cmp(cmp));
                self.push_cmp_branch(op, false_label);
            }
            JvmType::Object(_) | JvmType::Array(_) => {
                let branch = match op {
                    BinaryOp::Eq | BinaryOp::RefEq => JvmOp::IfACmpNe(false_label),
                    BinaryOp::Ne | BinaryOp::RefNe => JvmOp::IfACmpEq(false_label),
                    other => {
                        self.ops.push(JvmOp::Pop);
                        self.ops.push(JvmOp::Pop);
                        self.ops.push(JvmOp::PushInt(0));
                        eprintln!(
                            "codegen warning: ordering comparison {other:?} on Object/Array is not supported, \
                             emitting constant false"
                        );
                        return;
                    }
                };
                self.ops.push(branch);
            }
            _ => {
                self.ops.push(JvmOp::IfICmpNe(false_label));
            }
        }

        self.ops.push(JvmOp::PushInt(1));
        self.ops.push(JvmOp::Goto(end_label));
        self.ops.push(JvmOp::Label(false_label));
        self.emit_frame(vec![]);
        self.ops.push(JvmOp::PushInt(0));
        self.ops.push(JvmOp::Label(end_label));
        self.emit_frame(vec![JvmType::Int]);
    }

    fn push_cmp_branch(&mut self, op: BinaryOp, false_label: Label) {
        let branch = match op {
            BinaryOp::Eq => JvmOp::IfNe(false_label),
            BinaryOp::Ne => JvmOp::IfEq(false_label),
            BinaryOp::Lt => JvmOp::IfGe(false_label),
            BinaryOp::Le => JvmOp::IfGt(false_label),
            BinaryOp::Gt => JvmOp::IfLe(false_label),
            BinaryOp::Ge => JvmOp::IfLt(false_label),
            _ => unreachable!(),
        };
        self.ops.push(branch);
    }

    fn lower_short_circuit_and(&mut self, lhs: &TypedExpr, rhs: &TypedExpr) {
        let false_label = self.alloc_label();
        let end_label = self.alloc_label();

        self.lower_expr(lhs);
        self.ops.push(JvmOp::IfEq(false_label));
        self.lower_expr(rhs);
        self.ops.push(JvmOp::Goto(end_label));
        self.ops.push(JvmOp::Label(false_label));
        self.emit_frame(vec![]);
        self.ops.push(JvmOp::PushInt(0));
        self.ops.push(JvmOp::Label(end_label));
        self.emit_frame(vec![JvmType::Int]);
    }

    fn lower_short_circuit_or(&mut self, lhs: &TypedExpr, rhs: &TypedExpr) {
        let true_label = self.alloc_label();
        let end_label = self.alloc_label();

        self.lower_expr(lhs);
        self.ops.push(JvmOp::IfNe(true_label));
        self.lower_expr(rhs);
        self.ops.push(JvmOp::Goto(end_label));
        self.ops.push(JvmOp::Label(true_label));
        self.emit_frame(vec![]);
        self.ops.push(JvmOp::PushInt(1));
        self.ops.push(JvmOp::Label(end_label));
        self.emit_frame(vec![JvmType::Int]);
    }

    fn lower_if(
        &mut self,
        cond: &TypedExpr,
        then_branch: &TypedBody,
        else_branch: Option<&TypedExpr>,
        result_ty: &Ty,
    ) {
        let else_label = self.alloc_label();
        let end_label = self.alloc_label();

        let pre_if_locals = self.locals_snapshot();

        self.lower_expr(cond);
        self.ops.push(JvmOp::IfEq(else_label));
        self.push_scope();
        self.lower_body(then_branch);
        self.pop_scope();

        if else_branch.is_some() {
            self.ops.push(JvmOp::Goto(end_label));
        }

        self.ops.push(JvmOp::Label(else_label));
        self.emit_frame_with_locals(pre_if_locals.clone(), vec![]);

        if let Some(else_expr) = else_branch {
            self.push_scope();
            self.lower_expr(else_expr);
            self.pop_scope();
            self.ops.push(JvmOp::Label(end_label));
            let jvm_result = self.ty_to_jvm(result_ty);
            let end_stack = if matches!(jvm_result, JvmType::Void) {
                vec![]
            } else {
                vec![jvm_result]
            };
            self.emit_frame_with_locals(pre_if_locals, end_stack);
        }
    }

    fn lower_match(
        &mut self,
        scrutinee: &TypedExpr,
        arms: &[valen_hir::TypedMatchArm],
        result_ty: &Ty,
    ) {
        self.lower_expr(scrutinee);
        let scrutinee_ty = self.ty_to_jvm(&scrutinee.ty);
        let temp_slot = self.alloc_local(SmolStr::from("__match_scrutinee"), scrutinee_ty.clone());
        self.ops
            .push(JvmOp::StoreLocal(temp_slot, scrutinee_ty.clone()));

        let end_label = self.alloc_label();

        let pre_match_locals = self.locals_snapshot();

        for (i, arm) in arms.iter().enumerate() {
            let is_last = i == arms.len() - 1;
            let next_arm = if is_last {
                end_label
            } else {
                self.alloc_label()
            };

            self.push_scope();
            self.lower_pattern_check(&arm.pattern, temp_slot, &scrutinee_ty, next_arm);

            if let Some(guard) = &arm.guard {
                self.lower_expr(guard);
                self.ops.push(JvmOp::IfEq(next_arm));
            }

            self.lower_expr(&arm.body);
            self.pop_scope();
            if !is_last {
                self.ops.push(JvmOp::Goto(end_label));
                self.ops.push(JvmOp::Label(next_arm));
                self.emit_frame_with_locals(pre_match_locals.clone(), vec![]);
            }
        }

        self.ops.push(JvmOp::Label(end_label));
        let jvm_result = self.ty_to_jvm(result_ty);
        let end_stack = if matches!(jvm_result, JvmType::Void) {
            vec![]
        } else {
            vec![jvm_result]
        };
        self.emit_frame_with_locals(pre_match_locals, end_stack);
    }

    /// Lower `let Pattern = expr else { diverge };` into JVM bytecode.
    ///
    /// Evaluates the scrutinee, checks the pattern. If the pattern matches,
    /// binds variables and continues. If it fails, executes the else block
    /// (which must diverge, so control never reaches past it).
    ///
    /// Pattern variables are bound in the **enclosing** scope (no push/pop),
    /// so they remain accessible after the let-else statement.
    fn lower_let_else(
        &mut self,
        pattern: &valen_ast::Pattern,
        scrutinee: &TypedExpr,
        _ty: &Ty,
        else_body: &TypedBody,
    ) {
        // Evaluate the scrutinee and store in a temp slot
        self.lower_expr(scrutinee);
        let scrutinee_ty = self.ty_to_jvm(&scrutinee.ty);
        let temp_slot =
            self.alloc_local(SmolStr::from("__let_else_scrutinee"), scrutinee_ty.clone());
        self.ops
            .push(JvmOp::StoreLocal(temp_slot, scrutinee_ty.clone()));

        // Allocate labels
        let else_label = self.alloc_label();
        let continue_label = self.alloc_label();

        // Snapshot before pattern bindings — else branch sees pre-pattern state
        let pre_pattern_locals = self.locals_snapshot();

        // Check the pattern — if it fails, jump to else_label.
        // Do NOT wrap in push_scope/pop_scope: pattern bindings must
        // persist in the enclosing scope after the let-else.
        self.lower_pattern_check(pattern, temp_slot, &scrutinee_ty, else_label);

        // Pattern matched — jump over the else block
        self.ops.push(JvmOp::Goto(continue_label));

        // Else block — executes when pattern doesn't match (diverges)
        self.ops.push(JvmOp::Label(else_label));
        self.emit_frame_with_locals(pre_pattern_locals, vec![]);
        self.lower_body(else_body);

        // Continue label — after the pattern match succeeded
        self.ops.push(JvmOp::Label(continue_label));
        self.emit_frame(vec![]);
    }

    fn lower_pattern_check(
        &mut self,
        pattern: &valen_ast::Pattern,
        temp_slot: u16,
        scrutinee_ty: &JvmType,
        fail_label: Label,
    ) {
        use valen_ast::Pattern;
        match pattern {
            Pattern::Wildcard(_) => {}
            Pattern::Literal(lit) => {
                self.ops
                    .push(JvmOp::LoadLocal(temp_slot, scrutinee_ty.clone()));
                self.lower_literal(lit);
                match scrutinee_ty {
                    JvmType::Int
                    | JvmType::Byte
                    | JvmType::Short
                    | JvmType::Char
                    | JvmType::Boolean => {
                        self.ops.push(JvmOp::IfICmpNe(fail_label));
                    }
                    JvmType::Object(_) => {
                        self.ops.push(JvmOp::InvokeVirtual {
                            owner: JVM_OBJECT.to_string(),
                            name: EQUALS.to_string(),
                            params: vec![JvmType::Object(JVM_OBJECT.to_string())],
                            ret: JvmType::Boolean,
                        });
                        self.ops.push(JvmOp::IfEq(fail_label));
                    }
                    _ => {
                        self.ops.push(JvmOp::IfICmpNe(fail_label));
                    }
                }
            }
            Pattern::Binding(binding) => {
                let ty = scrutinee_ty.clone();
                self.ops.push(JvmOp::LoadLocal(temp_slot, ty.clone()));
                let slot = self.alloc_local(binding.name.clone(), ty.clone());
                self.ops.push(JvmOp::StoreLocal(slot, ty));
            }
            Pattern::Path(path) => {
                let variant_name = path
                    .segments
                    .iter()
                    .map(|s| s.name.as_str())
                    .collect::<Vec<_>>()
                    .join("$");
                let variant_internal =
                    crate::descriptor::class_internal_name(&variant_name, self.pkg);
                self.ops
                    .push(JvmOp::LoadLocal(temp_slot, scrutinee_ty.clone()));
                self.ops.push(JvmOp::Instanceof(variant_internal));
                self.ops.push(JvmOp::IfEq(fail_label));
            }
            Pattern::Struct(sp) => {
                let variant_name = sp
                    .path
                    .segments
                    .iter()
                    .map(|s| s.name.as_str())
                    .collect::<Vec<_>>()
                    .join("$");
                let variant_internal =
                    crate::descriptor::class_internal_name(&variant_name, self.pkg);
                self.ops
                    .push(JvmOp::LoadLocal(temp_slot, scrutinee_ty.clone()));
                self.ops.push(JvmOp::Instanceof(variant_internal.clone()));
                self.ops.push(JvmOp::IfEq(fail_label));

                self.ops
                    .push(JvmOp::LoadLocal(temp_slot, scrutinee_ty.clone()));
                self.ops.push(JvmOp::Checkcast(variant_internal.clone()));
                let cast_slot = self.next_slot;
                let cast_ty = JvmType::Object(variant_internal.clone());
                self.next_slot += cast_ty.slot_count();
                self.ops.push(JvmOp::StoreLocal(cast_slot, cast_ty.clone()));

                let variant_field_types = self.resolve_variant_field_types(&sp.path);

                // Two-phase pattern lowering: check phase uses temp slots only,
                // publish phase promotes bindings to lexical locals after all
                // checks pass. This prevents uninitialized locals from appearing
                // in StackMapTable frames when a later field's pattern fails.
                let mut deferred_bindings: Vec<(SmolStr, JvmType, u16)> = Vec::new();

                // Phase 1: Check — all field tests use temp slots.
                for (idx, field) in sp.fields.iter().enumerate() {
                    self.ops.push(JvmOp::LoadLocal(cast_slot, cast_ty.clone()));
                    let (actual_field_name, field_ty) = if field.pattern.is_some() {
                        variant_field_types
                            .get(field.name.as_str())
                            .map(|ty| (field.name.to_string(), ty.clone()))
                            .unwrap_or_else(|| {
                                (
                                    field.name.to_string(),
                                    JvmType::Object(JVM_OBJECT.to_string()),
                                )
                            })
                    } else {
                        variant_field_types
                            .get_index(idx)
                            .map(|(name, ty)| (name.clone(), ty.clone()))
                            .unwrap_or_else(|| {
                                (
                                    field.name.to_string(),
                                    JvmType::Object(JVM_OBJECT.to_string()),
                                )
                            })
                    };
                    // Enum variant fields are record components (private) —
                    // use accessor method instead of direct field access.
                    self.ops.push(JvmOp::InvokeVirtual {
                        owner: variant_internal.clone(),
                        name: actual_field_name,
                        params: vec![],
                        ret: field_ty.clone(),
                    });
                    if let Some(pat) = &field.pattern {
                        let inner_slot = self.next_slot;
                        self.next_slot += field_ty.slot_count();
                        self.ops
                            .push(JvmOp::StoreLocal(inner_slot, field_ty.clone()));
                        self.lower_pattern_check(pat, inner_slot, &field_ty, fail_label);
                    } else {
                        // Store to a temp slot; defer alloc_local to the publish phase.
                        let temp = self.next_slot;
                        self.next_slot += field_ty.slot_count();
                        self.ops.push(JvmOp::StoreLocal(temp, field_ty.clone()));
                        deferred_bindings.push((field.name.clone(), field_ty, temp));
                    }
                }

                // Phase 2: Publish — all checks passed, promote temp slots to
                // lexical locals so locals_snapshot() reflects only initialized
                // bindings.
                for (name, ty, temp) in deferred_bindings {
                    let local = self.alloc_local(name, ty.clone());
                    self.ops.push(JvmOp::LoadLocal(temp, ty.clone()));
                    self.ops.push(JvmOp::StoreLocal(local, ty));
                }
            }
            Pattern::Or(pats, _) => {
                let success_label = self.alloc_label();
                for (i, pat) in pats.iter().enumerate() {
                    let is_last = i == pats.len() - 1;
                    if is_last {
                        self.lower_pattern_check(pat, temp_slot, scrutinee_ty, fail_label);
                    } else {
                        let next_try = self.alloc_label();
                        self.lower_pattern_check(pat, temp_slot, scrutinee_ty, next_try);
                        self.ops.push(JvmOp::Goto(success_label));
                        self.ops.push(JvmOp::Label(next_try));
                        self.emit_frame(vec![]);
                    }
                }
                self.ops.push(JvmOp::Label(success_label));
                self.emit_frame(vec![]);
            }
            Pattern::Range(range) => {
                if let Some(start) = &range.start {
                    self.ops
                        .push(JvmOp::LoadLocal(temp_slot, scrutinee_ty.clone()));
                    self.lower_literal(start);
                    self.ops.push(JvmOp::IfICmpLt(fail_label));
                }
                if let Some(end) = &range.end {
                    self.ops
                        .push(JvmOp::LoadLocal(temp_slot, scrutinee_ty.clone()));
                    self.lower_literal(end);
                    if range.inclusive {
                        self.ops.push(JvmOp::IfICmpGt(fail_label));
                    } else {
                        self.ops.push(JvmOp::IfICmpGe(fail_label));
                    }
                }
            }
            Pattern::At(at) => {
                self.lower_pattern_check(&at.pattern, temp_slot, scrutinee_ty, fail_label);
                let ty = scrutinee_ty.clone();
                self.ops.push(JvmOp::LoadLocal(temp_slot, ty.clone()));
                let slot = self.alloc_local(at.name.clone(), ty.clone());
                self.ops.push(JvmOp::StoreLocal(slot, ty));
            }
            Pattern::Tuple(_, _) => {
                // Valen doesn't have tuple types in MVP
            }
            Pattern::VariantShorthand(vs) => {
                // Resolve enum for the variant, preferring the scrutinee type
                let (enum_name, variant_name) = {
                    let vn = vs.variant_name.as_str();
                    let mut found_enum = String::new();

                    // 1. Try to resolve from scrutinee type (preferred, avoids
                    //    wrong `instanceof` when two enums share a variant name)
                    if let JvmType::Object(ref internal) = scrutinee_ty {
                        for def in self.hir.defs.values() {
                            let def_internal =
                                crate::descriptor::class_internal_name(&def.name, self.pkg);
                            if def_internal == *internal {
                                if let DefKind::Enum(e) = &def.kind {
                                    if e.variants.iter().any(|v| v.name == vn) {
                                        found_enum = def.name.to_string();
                                    }
                                }
                                break;
                            }
                        }
                    }

                    // 2. Fallback: search all enums (for non-enum scrutinee types)
                    if found_enum.is_empty() {
                        for def in self.hir.defs.values() {
                            if let DefKind::Enum(edef) = &def.kind {
                                if edef.variants.iter().any(|v| v.name == vn) {
                                    found_enum = def.name.to_string();
                                    break;
                                }
                            }
                        }
                    }
                    (found_enum, vn.to_string())
                };

                let qualified = format!("{enum_name}${variant_name}");
                let variant_internal = crate::descriptor::class_internal_name(&qualified, self.pkg);

                // instanceof check
                self.ops
                    .push(JvmOp::LoadLocal(temp_slot, scrutinee_ty.clone()));
                self.ops.push(JvmOp::Instanceof(variant_internal.clone()));
                self.ops.push(JvmOp::IfEq(fail_label));

                if !vs.fields.is_empty() {
                    // Cast and extract fields
                    self.ops
                        .push(JvmOp::LoadLocal(temp_slot, scrutinee_ty.clone()));
                    self.ops.push(JvmOp::Checkcast(variant_internal.clone()));
                    let cast_slot = self.next_slot;
                    let cast_ty = JvmType::Object(variant_internal.clone());
                    self.next_slot += cast_ty.slot_count();
                    self.ops.push(JvmOp::StoreLocal(cast_slot, cast_ty.clone()));

                    // Build a synthetic path for variant field type resolution
                    let synth_path = valen_ast::Path {
                        segments: vec![
                            valen_ast::PathSegment {
                                name: SmolStr::from(enum_name.as_str()),
                                double_colon: false,
                                generics: vec![],
                                span: vs.span,
                            },
                            valen_ast::PathSegment {
                                name: SmolStr::from(variant_name.as_str()),
                                double_colon: true,
                                generics: vec![],
                                span: vs.span,
                            },
                        ],
                        span: vs.span,
                    };
                    let variant_field_types = self.resolve_variant_field_types(&synth_path);

                    let mut deferred_bindings: Vec<(SmolStr, JvmType, u16)> = Vec::new();

                    for (idx, field) in vs.fields.iter().enumerate() {
                        self.ops.push(JvmOp::LoadLocal(cast_slot, cast_ty.clone()));
                        let (actual_field_name, field_ty) = if field.pattern.is_some() {
                            variant_field_types
                                .get(field.name.as_str())
                                .map(|ty| (field.name.to_string(), ty.clone()))
                                .unwrap_or_else(|| {
                                    (
                                        field.name.to_string(),
                                        JvmType::Object(JVM_OBJECT.to_string()),
                                    )
                                })
                        } else {
                            variant_field_types
                                .get_index(idx)
                                .map(|(name, ty)| (name.clone(), ty.clone()))
                                .unwrap_or_else(|| {
                                    (
                                        field.name.to_string(),
                                        JvmType::Object(JVM_OBJECT.to_string()),
                                    )
                                })
                        };
                        self.ops.push(JvmOp::InvokeVirtual {
                            owner: variant_internal.clone(),
                            name: actual_field_name,
                            params: vec![],
                            ret: field_ty.clone(),
                        });
                        if let Some(pat) = &field.pattern {
                            let inner_slot = self.next_slot;
                            self.next_slot += field_ty.slot_count();
                            self.ops
                                .push(JvmOp::StoreLocal(inner_slot, field_ty.clone()));
                            self.lower_pattern_check(pat, inner_slot, &field_ty, fail_label);
                        } else {
                            let temp = self.next_slot;
                            self.next_slot += field_ty.slot_count();
                            self.ops.push(JvmOp::StoreLocal(temp, field_ty.clone()));
                            deferred_bindings.push((field.name.clone(), field_ty, temp));
                        }
                    }

                    // Publish phase
                    for (name, ty, temp) in deferred_bindings {
                        let local = self.alloc_local(name, ty.clone());
                        self.ops.push(JvmOp::LoadLocal(temp, ty.clone()));
                        self.ops.push(JvmOp::StoreLocal(local, ty));
                    }
                }
            }
        }
    }

    fn lower_literal(&mut self, lit: &valen_ast::Literal) {
        match lit {
            valen_ast::Literal::Int(n, _) => match i32::try_from(*n) {
                Ok(i) => self.ops.push(JvmOp::PushInt(i)),
                Err(_) => {
                    eprintln!(
                        "[codegen] error: integer literal {} overflows i32 range, \
                             clamping to i32::MAX",
                        n
                    );
                    self.ops.push(JvmOp::PushInt(i32::MAX));
                }
            },
            valen_ast::Literal::Long(n, _) => self.ops.push(JvmOp::PushLong(*n)),
            valen_ast::Literal::Float(n, _) => self.ops.push(JvmOp::PushFloat(*n)),
            valen_ast::Literal::Double(n, _) => self.ops.push(JvmOp::PushDouble(*n)),
            valen_ast::Literal::Char(c, _) => self.ops.push(JvmOp::PushInt(*c as i32)),
            valen_ast::Literal::String(s, _) => self.ops.push(JvmOp::PushString(s.to_string())),
            valen_ast::Literal::Bool(b, _) => self.ops.push(JvmOp::PushInt(if *b { 1 } else { 0 })),
            valen_ast::Literal::Unit(_) => {}
            valen_ast::Literal::Null(_) => self.ops.push(JvmOp::PushNull),
        }
    }

    fn lower_assign(&mut self, target: &TypedExpr, value: &TypedExpr) {
        match &target.kind {
            TypedExprKind::LocalVar(name) => {
                self.lower_expr(value);
                if let Some((slot, ty)) = self.locals.get(name).cloned() {
                    self.ops.push(JvmOp::StoreLocal(slot, ty));
                }
            }
            TypedExprKind::FieldAccess { receiver, field } => {
                self.lower_expr(receiver);
                self.lower_expr(value);
                let owner_ty = self.ty_to_jvm(&receiver.ty);
                let field_ty = self.ty_to_jvm(&value.ty);
                if let JvmType::Object(owner) = owner_ty {
                    self.ops.push(JvmOp::PutField {
                        owner,
                        name: field.to_string(),
                        descriptor: field_ty,
                    });
                }
            }
            _ => {}
        }
    }

    fn lower_while(&mut self, cond: &TypedExpr, body: &TypedBody) {
        let continue_label = self.alloc_label();
        let break_label = self.alloc_label();

        self.ops.push(JvmOp::Label(continue_label));
        self.emit_frame(vec![]);
        self.lower_expr(cond);
        self.ops.push(JvmOp::IfEq(break_label));

        self.loop_stack.push(LoopContext {
            break_label,
            continue_label,
        });
        self.push_scope();
        self.lower_body(body);
        self.pop_scope();
        self.loop_stack.pop();

        self.ops.push(JvmOp::Goto(continue_label));
        self.ops.push(JvmOp::Label(break_label));
        self.emit_frame(vec![]);
    }

    fn lower_loop(&mut self, body: &TypedBody) {
        let continue_label = self.alloc_label();
        let break_label = self.alloc_label();

        self.ops.push(JvmOp::Label(continue_label));
        self.emit_frame(vec![]);

        self.loop_stack.push(LoopContext {
            break_label,
            continue_label,
        });
        self.push_scope();
        self.lower_body(body);
        self.pop_scope();
        self.loop_stack.pop();

        self.ops.push(JvmOp::Goto(continue_label));
        self.ops.push(JvmOp::Label(break_label));
        self.emit_frame(vec![]);
    }

    fn lower_for(&mut self, var: &SmolStr, iter: &TypedExpr, body: &TypedBody) {
        if let TypedExprKind::Range {
            start,
            end,
            inclusive,
        } = &iter.kind
        {
            self.lower_for_range(
                var,
                start.as_deref(),
                end.as_deref(),
                *inclusive,
                &iter.ty,
                body,
            );
        } else if self.is_java_iterable(&iter.ty) {
            self.lower_for_java_iterator(var, iter, body);
        } else {
            self.lower_for_iterator(var, iter, body);
        }
    }

    fn lower_for_range(
        &mut self,
        var: &SmolStr,
        start: Option<&TypedExpr>,
        end: Option<&TypedExpr>,
        inclusive: bool,
        range_ty: &Ty,
        body: &TypedBody,
    ) {
        let elem_ty = match range_ty {
            Ty::Generic(_, args) if !args.is_empty() => self.ty_to_jvm(&args[0]),
            _ => JvmType::Int,
        };

        self.push_scope();

        // Store start value into loop variable
        if let Some(s) = start {
            self.lower_expr(s);
        } else {
            match &elem_ty {
                JvmType::Long => self.ops.push(JvmOp::PushLong(0)),
                JvmType::Float => self.ops.push(JvmOp::PushFloat(0.0)),
                JvmType::Double => self.ops.push(JvmOp::PushDouble(0.0)),
                _ => self.ops.push(JvmOp::PushInt(0)),
            }
        }
        let var_slot = self.alloc_local(var.clone(), elem_ty.clone());
        self.ops.push(JvmOp::StoreLocal(var_slot, elem_ty.clone()));

        // Store end value into limit variable
        if let Some(e) = end {
            self.lower_expr(e);
        } else {
            match &elem_ty {
                JvmType::Long => self.ops.push(JvmOp::PushLong(i64::MAX)),
                JvmType::Float => self.ops.push(JvmOp::PushFloat(f32::MAX)),
                JvmType::Double => self.ops.push(JvmOp::PushDouble(f64::MAX)),
                _ => self.ops.push(JvmOp::PushInt(i32::MAX)),
            }
        }
        let limit_slot = self.next_slot;
        self.next_slot += elem_ty.slot_count();
        self.ops
            .push(JvmOp::StoreLocal(limit_slot, elem_ty.clone()));

        let loop_label = self.alloc_label();
        let continue_label = self.alloc_label();
        let condition_label = self.alloc_label();
        let break_label = self.alloc_label();

        self.ops.push(JvmOp::Goto(condition_label));

        // Loop body
        self.ops.push(JvmOp::Label(loop_label));
        self.emit_frame(vec![]);

        self.loop_stack.push(LoopContext {
            break_label,
            continue_label,
        });
        self.lower_body(body);
        self.loop_stack.pop();

        // Increment (continue target)
        self.ops.push(JvmOp::Label(continue_label));
        self.emit_frame(vec![]);

        match &elem_ty {
            JvmType::Int | JvmType::Byte | JvmType::Short | JvmType::Char => {
                self.ops.push(JvmOp::IInc(var_slot, 1));
            }
            JvmType::Long => {
                self.ops.push(JvmOp::LoadLocal(var_slot, JvmType::Long));
                self.ops.push(JvmOp::PushLong(1));
                self.ops.push(JvmOp::Arith(ArithOp::Add, JvmType::Long));
                self.ops.push(JvmOp::StoreLocal(var_slot, JvmType::Long));
            }
            ty @ (JvmType::Float | JvmType::Double) => {
                self.ops.push(JvmOp::LoadLocal(var_slot, ty.clone()));
                if matches!(ty, JvmType::Float) {
                    self.ops.push(JvmOp::PushFloat(1.0));
                } else {
                    self.ops.push(JvmOp::PushDouble(1.0));
                }
                self.ops.push(JvmOp::Arith(ArithOp::Add, ty.clone()));
                self.ops.push(JvmOp::StoreLocal(var_slot, ty.clone()));
            }
            _ => {}
        }

        // Condition check
        self.ops.push(JvmOp::Label(condition_label));
        self.emit_frame(vec![]);

        self.ops.push(JvmOp::LoadLocal(var_slot, elem_ty.clone()));
        self.ops.push(JvmOp::LoadLocal(limit_slot, elem_ty.clone()));

        match &elem_ty {
            JvmType::Int | JvmType::Byte | JvmType::Short | JvmType::Char => {
                if inclusive {
                    self.ops.push(JvmOp::IfICmpLe(loop_label));
                } else {
                    self.ops.push(JvmOp::IfICmpLt(loop_label));
                }
            }
            JvmType::Long => {
                self.ops.push(JvmOp::Cmp(CmpKind::LCmp));
                if inclusive {
                    self.ops.push(JvmOp::IfLe(loop_label));
                } else {
                    self.ops.push(JvmOp::IfLt(loop_label));
                }
            }
            JvmType::Float => {
                self.ops.push(JvmOp::Cmp(CmpKind::FCmpG));
                if inclusive {
                    self.ops.push(JvmOp::IfLe(loop_label));
                } else {
                    self.ops.push(JvmOp::IfLt(loop_label));
                }
            }
            JvmType::Double => {
                self.ops.push(JvmOp::Cmp(CmpKind::DCmpG));
                if inclusive {
                    self.ops.push(JvmOp::IfLe(loop_label));
                } else {
                    self.ops.push(JvmOp::IfLt(loop_label));
                }
            }
            _ => {}
        }

        // Loop exit
        self.ops.push(JvmOp::Label(break_label));
        self.emit_frame(vec![]);

        self.pop_scope();
    }

    /// Check if a type is a Java collection (Iterable) that uses java.util.Iterator.
    fn is_java_iterable(&self, ty: &Ty) -> bool {
        let name = match ty {
            Ty::Named(n) => n.as_str(),
            Ty::Generic(n, _) => n.as_str(),
            _ => return false,
        };
        self.hir.foreign_types.get(name).is_some_and(|info| {
            info.methods
                .iter()
                .any(|m| m.name == "iterator" && m.params.is_empty())
        })
    }

    /// Emits a for-loop over a Java Iterable: calls .iterator(), then
    /// hasNext()/next() in a loop.
    fn lower_for_java_iterator(&mut self, var: &SmolStr, iter: &TypedExpr, body: &TypedBody) {
        let elem_ty = JvmType::Object(JVM_OBJECT.to_string());
        let java_iter = "java/util/Iterator";

        self.push_scope();

        // Call .iterator() on the collection
        self.lower_expr(iter);
        let coll_ty = self.ty_to_jvm(&iter.ty);
        if let JvmType::Object(ref owner) = coll_ty {
            self.ops.push(JvmOp::InvokeVirtual {
                owner: owner.clone(),
                name: "iterator".to_string(),
                params: vec![],
                ret: JvmType::Object(java_iter.to_string()),
            });
        }
        let iter_slot = self.alloc_local(
            SmolStr::from("__java_iter"),
            JvmType::Object(java_iter.to_string()),
        );
        self.ops.push(JvmOp::StoreLocal(
            iter_slot,
            JvmType::Object(java_iter.to_string()),
        ));

        let loop_label = self.alloc_label();
        let break_label = self.alloc_label();

        let pre_loop_locals = self.locals_snapshot();

        self.ops.push(JvmOp::Label(loop_label));
        self.emit_frame_with_locals(pre_loop_locals.clone(), vec![]);

        // hasNext()
        self.ops.push(JvmOp::LoadLocal(
            iter_slot,
            JvmType::Object(java_iter.to_string()),
        ));
        self.ops.push(JvmOp::InvokeInterface {
            owner: java_iter.to_string(),
            name: "hasNext".to_string(),
            params: vec![],
            ret: JvmType::Boolean,
        });
        self.ops.push(JvmOp::IfEq(break_label));

        // next()
        self.ops.push(JvmOp::LoadLocal(
            iter_slot,
            JvmType::Object(java_iter.to_string()),
        ));
        self.ops.push(JvmOp::InvokeInterface {
            owner: java_iter.to_string(),
            name: "next".to_string(),
            params: vec![],
            ret: JvmType::Object(JVM_OBJECT.to_string()),
        });

        let var_slot = self.alloc_local(var.clone(), elem_ty.clone());
        self.ops.push(JvmOp::StoreLocal(var_slot, elem_ty));

        // Body
        self.loop_stack.push(LoopContext {
            break_label,
            continue_label: loop_label,
        });
        self.lower_body(body);
        self.loop_stack.pop();

        self.ops.push(JvmOp::Goto(loop_label));

        self.ops.push(JvmOp::Label(break_label));
        self.emit_frame_with_locals(pre_loop_locals, vec![]);

        self.pop_scope();
    }

    /// Emits a general for-loop over an Iterator: calls next() in a loop,
    /// checks for Option$Some vs Option$None via instanceof.
    fn lower_for_iterator(&mut self, var: &SmolStr, iter: &TypedExpr, body: &TypedBody) {
        let elem_ty = match &iter.ty {
            Ty::Generic(_, args) if !args.is_empty() => self.ty_to_jvm(&args[0]),
            _ => JvmType::Object(JVM_OBJECT.to_string()),
        };

        let obj = JvmType::Object(JVM_OBJECT.to_string());
        let option_iface = "valen/core/Option";
        let some_class = "valen/core/Option$Some";

        self.push_scope();

        // Evaluate iterator and store
        self.lower_expr(iter);
        let iter_slot = self.next_slot;
        self.next_slot += 1;
        let iter_jvm = self.ty_to_jvm(&iter.ty);
        self.ops
            .push(JvmOp::StoreLocal(iter_slot, iter_jvm.clone()));

        let loop_label = self.alloc_label();
        let break_label = self.alloc_label();

        // Loop head — also the continue target so a single frame covers both
        self.ops.push(JvmOp::Label(loop_label));
        self.emit_frame(vec![]);

        // Call next() → Option<T>
        self.ops.push(JvmOp::LoadLocal(iter_slot, iter_jvm.clone()));
        self.ops.push(JvmOp::InvokeInterface {
            owner: "valen/core/Iterator".to_string(),
            name: "next".to_string(),
            params: vec![],
            ret: JvmType::Object(option_iface.to_string()),
        });

        // Store the Option result
        let opt_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            opt_slot,
            JvmType::Object(option_iface.to_string()),
        ));

        // Check if Some (instanceof Option$Some)
        self.ops.push(JvmOp::LoadLocal(
            opt_slot,
            JvmType::Object(option_iface.to_string()),
        ));
        self.ops.push(JvmOp::Instanceof(some_class.to_string()));
        self.ops.push(JvmOp::IfEq(break_label));

        // Extract value: cast to Some, get `value` field
        self.ops.push(JvmOp::LoadLocal(
            opt_slot,
            JvmType::Object(option_iface.to_string()),
        ));
        self.ops.push(JvmOp::Checkcast(some_class.to_string()));
        self.ops.push(JvmOp::GetField {
            owner: some_class.to_string(),
            name: "value".to_string(),
            descriptor: obj,
        });

        // Unbox if the element type is a primitive
        if JvmType::boxed_name(&elem_ty).is_some() {
            self.emit_unbox(&elem_ty);
        } else if let JvmType::Object(ref name) = elem_ty {
            if name != JVM_OBJECT {
                self.ops.push(JvmOp::Checkcast(name.clone()));
            }
        }

        let var_slot = self.alloc_local(var.clone(), elem_ty.clone());
        self.ops.push(JvmOp::StoreLocal(var_slot, elem_ty));

        // Body
        self.loop_stack.push(LoopContext {
            break_label,
            continue_label: loop_label,
        });
        self.lower_body(body);
        self.loop_stack.pop();

        self.ops.push(JvmOp::Goto(loop_label));

        // Exit
        self.ops.push(JvmOp::Label(break_label));
        self.emit_frame(vec![]);

        self.pop_scope();
    }

    /// Handles numeric conversion methods (`toInt`, `toLong`, `toFloat`, `toDouble`,
    /// `toByte`, `toShort`, `toChar`) on primitive receivers by emitting `JvmOp::Convert`.
    ///
    /// Returns `true` if the method was handled as an intrinsic conversion.
    fn try_lower_numeric_conversion(&mut self, receiver: &TypedExpr, method: &str) -> bool {
        let target_ty = match method {
            "toInt" => JvmType::Int,
            "toLong" => JvmType::Long,
            "toFloat" => JvmType::Float,
            "toDouble" => JvmType::Double,
            "toByte" => JvmType::Byte,
            "toShort" => JvmType::Short,
            "toChar" => JvmType::Char,
            _ => return false,
        };

        let from_ty = self.ty_to_jvm(&receiver.ty);

        // Only handle conversions between primitive numeric types.
        let is_numeric_prim = matches!(
            from_ty,
            JvmType::Int
                | JvmType::Long
                | JvmType::Float
                | JvmType::Double
                | JvmType::Byte
                | JvmType::Short
                | JvmType::Char
        );
        if !is_numeric_prim {
            return false;
        }

        self.lower_expr(receiver);

        // Skip no-op conversions (e.g. Int.toInt()).
        if from_ty != target_ty {
            self.ops.push(JvmOp::Convert {
                from: from_ty,
                to: target_ty,
            });
        }

        true
    }

    /// Returns `true` if the method call is on an Iterator and was handled as an intrinsic.
    fn try_lower_iterator_intrinsic(
        &mut self,
        receiver: &TypedExpr,
        method: &str,
        args: &[TypedExpr],
        result_ty: &Ty,
    ) -> bool {
        let is_iterator = match &receiver.ty {
            Ty::Generic(n, _) | Ty::Named(n) => n.as_str() == "Iterator",
            _ => false,
        };
        if !is_iterator {
            return false;
        }

        let elem_ty = match &receiver.ty {
            Ty::Generic(_, targs) if !targs.is_empty() => self.ty_to_jvm(&targs[0]),
            _ => JvmType::Object(JVM_OBJECT.to_string()),
        };

        match method {
            "collect" => self.emit_iter_collect(receiver, &elem_ty),
            "map" if !args.is_empty() => {
                self.emit_iter_map(receiver, &args[0], &elem_ty, result_ty)
            }
            "filter" if !args.is_empty() => self.emit_iter_filter(receiver, &args[0], &elem_ty),
            "fold" if args.len() >= 2 => {
                self.emit_iter_fold(receiver, &args[0], &args[1], result_ty)
            }
            "forEach" if !args.is_empty() => self.emit_iter_for_each(receiver, &args[0], &elem_ty),
            "count" => self.emit_iter_count(receiver),
            "any" if !args.is_empty() => self.emit_iter_any(receiver, &args[0], &elem_ty),
            "all" if !args.is_empty() => self.emit_iter_all(receiver, &args[0], &elem_ty),
            "find" if !args.is_empty() => self.emit_iter_find(receiver, &args[0], &elem_ty),
            _ => return false,
        }
        true
    }

    /// Helper: store iterator in a local slot and return (iter_slot, iter_jvm_type).
    fn store_iterator(&mut self, receiver: &TypedExpr) -> (u16, JvmType) {
        self.lower_expr(receiver);
        let iter_jvm = self.ty_to_jvm(&receiver.ty);
        let iter_slot = self.next_slot;
        self.next_slot += 1;
        self.ops
            .push(JvmOp::StoreLocal(iter_slot, iter_jvm.clone()));
        (iter_slot, iter_jvm)
    }

    /// Helper: emit `iter.next()` call, leaving Option on the stack.
    fn emit_iter_next(&mut self, iter_slot: u16, iter_jvm: &JvmType) {
        self.ops.push(JvmOp::LoadLocal(iter_slot, iter_jvm.clone()));
        self.ops.push(JvmOp::InvokeInterface {
            owner: "valen/core/Iterator".to_string(),
            name: "next".to_string(),
            params: vec![],
            ret: JvmType::Object("valen/core/Option".to_string()),
        });
    }

    /// Helper: extract value from Option$Some on top of stack, cast/unbox to elem_ty.
    fn emit_extract_some_value(&mut self, opt_slot: u16, elem_ty: &JvmType) {
        let some_class = "valen/core/Option$Some";
        self.ops.push(JvmOp::LoadLocal(
            opt_slot,
            JvmType::Object("valen/core/Option".to_string()),
        ));
        self.ops.push(JvmOp::Checkcast(some_class.to_string()));
        self.ops.push(JvmOp::GetField {
            owner: some_class.to_string(),
            name: "value".to_string(),
            descriptor: JvmType::Object(JVM_OBJECT.to_string()),
        });
        if JvmType::boxed_name(elem_ty).is_some() {
            self.emit_unbox(elem_ty);
        } else if let JvmType::Object(ref name) = elem_ty {
            if name != JVM_OBJECT {
                self.ops.push(JvmOp::Checkcast(name.clone()));
            }
        }
    }

    /// Helper: invoke a 1-arg closure (Function.apply). Closure ref and arg must be on stack.
    fn emit_invoke_fn1(&mut self, result_jvm: &JvmType) {
        self.ops.push(JvmOp::InvokeInterface {
            owner: "java/util/function/Function".to_string(),
            name: "apply".to_string(),
            params: vec![JvmType::Object(JVM_OBJECT.to_string())],
            ret: JvmType::Object(JVM_OBJECT.to_string()),
        });
        if JvmType::boxed_name(result_jvm).is_some() {
            self.emit_unbox(result_jvm);
        } else if let JvmType::Object(ref name) = result_jvm {
            if name != JVM_OBJECT {
                self.ops.push(JvmOp::Checkcast(name.clone()));
            }
        }
    }

    /// Helper: invoke a 2-arg closure (BiFunction.apply). Closure ref and two args must be on stack.
    fn emit_invoke_fn2(&mut self, result_jvm: &JvmType) {
        self.ops.push(JvmOp::InvokeInterface {
            owner: "java/util/function/BiFunction".to_string(),
            name: "apply".to_string(),
            params: vec![
                JvmType::Object(JVM_OBJECT.to_string()),
                JvmType::Object(JVM_OBJECT.to_string()),
            ],
            ret: JvmType::Object(JVM_OBJECT.to_string()),
        });
        if JvmType::boxed_name(result_jvm).is_some() {
            self.emit_unbox(result_jvm);
        } else if let JvmType::Object(ref name) = result_jvm {
            if name != JVM_OBJECT {
                self.ops.push(JvmOp::Checkcast(name.clone()));
            }
        }
    }

    /// Helper: create new ArrayList and store in a local slot.
    fn emit_new_arraylist(&mut self) -> u16 {
        self.ops.push(JvmOp::New("java/util/ArrayList".to_string()));
        self.ops.push(JvmOp::Dup);
        self.ops.push(JvmOp::InvokeSpecial {
            owner: "java/util/ArrayList".to_string(),
            name: "<init>".to_string(),
            params: vec![],
            ret: JvmType::Void,
        });
        let slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            slot,
            JvmType::Object("java/util/ArrayList".to_string()),
        ));
        slot
    }

    /// Helper: call list.add(item). List ref in list_slot, item on stack.
    fn emit_list_add(&mut self, list_slot: u16, elem_ty: &JvmType) {
        // box primitive before adding to list
        self.emit_box(elem_ty);
        let item_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            item_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));
        self.ops.push(JvmOp::LoadLocal(
            list_slot,
            JvmType::Object("java/util/ArrayList".to_string()),
        ));
        self.ops.push(JvmOp::LoadLocal(
            item_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));
        self.ops.push(JvmOp::InvokeInterface {
            owner: "java/util/List".to_string(),
            name: "add".to_string(),
            params: vec![JvmType::Object(JVM_OBJECT.to_string())],
            ret: JvmType::Boolean,
        });
        self.ops.push(JvmOp::Pop); // discard boolean
    }

    /// `iter.collect() -> List<T>`: consume all elements into an ArrayList.
    fn emit_iter_collect(&mut self, receiver: &TypedExpr, elem_ty: &JvmType) {
        self.push_scope();
        let (iter_slot, iter_jvm) = self.store_iterator(receiver);
        let list_slot = self.emit_new_arraylist();

        let loop_label = self.alloc_label();
        let end_label = self.alloc_label();

        self.ops.push(JvmOp::Label(loop_label));
        self.emit_frame(vec![]);

        self.emit_iter_next(iter_slot, &iter_jvm);
        let opt_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            opt_slot,
            JvmType::Object("valen/core/Option".to_string()),
        ));
        self.ops.push(JvmOp::LoadLocal(
            opt_slot,
            JvmType::Object("valen/core/Option".to_string()),
        ));
        self.ops
            .push(JvmOp::Instanceof("valen/core/Option$Some".to_string()));
        self.ops.push(JvmOp::IfEq(end_label));

        self.emit_extract_some_value(opt_slot, elem_ty);
        self.emit_list_add(list_slot, elem_ty);

        self.ops.push(JvmOp::Goto(loop_label));

        self.ops.push(JvmOp::Label(end_label));
        self.emit_frame(vec![]);
        self.ops.push(JvmOp::LoadLocal(
            list_slot,
            JvmType::Object("java/util/ArrayList".to_string()),
        ));
        self.pop_scope();
    }

    // TODO(#014): Iterator intrinsics (map, filter, etc.) currently use eager evaluation,
    // collecting results into ArrayList immediately. This is a deliberate MVP simplification.
    // Future improvement: implement lazy iterator wrapper classes to avoid O(n) extra memory
    // per transformation and to enable infinite iterators. This will require generating
    // anonymous inner classes or leveraging java.util.stream on JVM 8+.

    /// `iter.map(f) -> List<U>`: apply f to each element and collect results.
    fn emit_iter_map(
        &mut self,
        receiver: &TypedExpr,
        f_expr: &TypedExpr,
        elem_ty: &JvmType,
        result_ty: &Ty,
    ) {
        let mapped_ty = match result_ty {
            Ty::Generic(_, args) if !args.is_empty() => self.ty_to_jvm(&args[0]),
            _ => JvmType::Object(JVM_OBJECT.to_string()),
        };

        self.push_scope();
        let (iter_slot, iter_jvm) = self.store_iterator(receiver);

        self.lower_expr(f_expr);
        let f_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            f_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));

        let list_slot = self.emit_new_arraylist();

        let loop_label = self.alloc_label();
        let end_label = self.alloc_label();

        self.ops.push(JvmOp::Label(loop_label));
        self.emit_frame(vec![]);

        self.emit_iter_next(iter_slot, &iter_jvm);
        let opt_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            opt_slot,
            JvmType::Object("valen/core/Option".to_string()),
        ));
        self.ops.push(JvmOp::LoadLocal(
            opt_slot,
            JvmType::Object("valen/core/Option".to_string()),
        ));
        self.ops
            .push(JvmOp::Instanceof("valen/core/Option$Some".to_string()));
        self.ops.push(JvmOp::IfEq(end_label));

        self.emit_extract_some_value(opt_slot, elem_ty);
        // box elem for Function.apply
        self.emit_box(elem_ty);
        let elem_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            elem_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));
        self.ops.push(JvmOp::LoadLocal(
            f_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));
        self.ops.push(JvmOp::LoadLocal(
            elem_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));
        self.emit_invoke_fn1(&mapped_ty);
        self.emit_list_add(list_slot, &mapped_ty);

        self.ops.push(JvmOp::Goto(loop_label));

        self.ops.push(JvmOp::Label(end_label));
        self.emit_frame(vec![]);
        self.ops.push(JvmOp::LoadLocal(
            list_slot,
            JvmType::Object("java/util/ArrayList".to_string()),
        ));
        self.pop_scope();
    }

    /// `iter.filter(predicate) -> List<T>`: keep elements matching predicate.
    fn emit_iter_filter(&mut self, receiver: &TypedExpr, pred_expr: &TypedExpr, elem_ty: &JvmType) {
        self.push_scope();
        let (iter_slot, iter_jvm) = self.store_iterator(receiver);

        self.lower_expr(pred_expr);
        let pred_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            pred_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));

        let list_slot = self.emit_new_arraylist();

        let loop_label = self.alloc_label();
        let end_label = self.alloc_label();
        let skip_label = self.alloc_label();

        self.ops.push(JvmOp::Label(loop_label));
        self.emit_frame(vec![]);

        self.emit_iter_next(iter_slot, &iter_jvm);
        let opt_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            opt_slot,
            JvmType::Object("valen/core/Option".to_string()),
        ));
        self.ops.push(JvmOp::LoadLocal(
            opt_slot,
            JvmType::Object("valen/core/Option".to_string()),
        ));
        self.ops
            .push(JvmOp::Instanceof("valen/core/Option$Some".to_string()));
        self.ops.push(JvmOp::IfEq(end_label));

        self.emit_extract_some_value(opt_slot, elem_ty);
        self.emit_box(elem_ty);
        let elem_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            elem_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));

        // Call predicate
        self.ops.push(JvmOp::LoadLocal(
            pred_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));
        self.ops.push(JvmOp::LoadLocal(
            elem_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));
        self.emit_invoke_fn1(&JvmType::Boolean);
        self.ops.push(JvmOp::IfEq(skip_label));

        // Predicate true → add to list
        self.ops.push(JvmOp::LoadLocal(
            elem_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));
        if JvmType::boxed_name(elem_ty).is_some() {
            self.emit_unbox(elem_ty);
        } else if let JvmType::Object(ref name) = elem_ty {
            if name != JVM_OBJECT {
                self.ops.push(JvmOp::Checkcast(name.clone()));
            }
        }
        self.emit_list_add(list_slot, elem_ty);

        self.ops.push(JvmOp::Label(skip_label));
        self.emit_frame(vec![]);
        self.ops.push(JvmOp::Goto(loop_label));

        self.ops.push(JvmOp::Label(end_label));
        self.emit_frame(vec![]);
        self.ops.push(JvmOp::LoadLocal(
            list_slot,
            JvmType::Object("java/util/ArrayList".to_string()),
        ));
        self.pop_scope();
    }

    /// `iter.fold(init, f) -> A`: reduce all elements with an accumulator.
    fn emit_iter_fold(
        &mut self,
        receiver: &TypedExpr,
        init_expr: &TypedExpr,
        f_expr: &TypedExpr,
        result_ty: &Ty,
    ) {
        let acc_jvm = self.ty_to_jvm(result_ty);

        self.push_scope();
        let (iter_slot, iter_jvm) = self.store_iterator(receiver);

        self.lower_expr(init_expr);
        self.emit_box(&acc_jvm);
        let acc_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            acc_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));

        self.lower_expr(f_expr);
        let f_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            f_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));

        let elem_ty = match &receiver.ty {
            Ty::Generic(_, targs) if !targs.is_empty() => self.ty_to_jvm(&targs[0]),
            _ => JvmType::Object(JVM_OBJECT.to_string()),
        };

        let loop_label = self.alloc_label();
        let end_label = self.alloc_label();

        self.ops.push(JvmOp::Label(loop_label));
        self.emit_frame(vec![]);

        self.emit_iter_next(iter_slot, &iter_jvm);
        let opt_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            opt_slot,
            JvmType::Object("valen/core/Option".to_string()),
        ));
        self.ops.push(JvmOp::LoadLocal(
            opt_slot,
            JvmType::Object("valen/core/Option".to_string()),
        ));
        self.ops
            .push(JvmOp::Instanceof("valen/core/Option$Some".to_string()));
        self.ops.push(JvmOp::IfEq(end_label));

        self.emit_extract_some_value(opt_slot, &elem_ty);
        self.emit_box(&elem_ty);
        let elem_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            elem_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));

        // acc = f(acc, item)
        self.ops.push(JvmOp::LoadLocal(
            f_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));
        self.ops.push(JvmOp::LoadLocal(
            acc_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));
        self.ops.push(JvmOp::LoadLocal(
            elem_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));
        self.emit_invoke_fn2(&JvmType::Object(JVM_OBJECT.to_string()));
        // Result is already Object from BiFunction.apply — store directly
        self.ops.push(JvmOp::StoreLocal(
            acc_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));

        self.ops.push(JvmOp::Goto(loop_label));

        self.ops.push(JvmOp::Label(end_label));
        self.emit_frame(vec![]);
        self.ops.push(JvmOp::LoadLocal(
            acc_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));
        if JvmType::boxed_name(&acc_jvm).is_some() {
            self.emit_unbox(&acc_jvm);
        } else if let JvmType::Object(ref name) = acc_jvm {
            if name != JVM_OBJECT {
                self.ops.push(JvmOp::Checkcast(name.clone()));
            }
        }
        self.pop_scope();
    }

    /// `iter.forEach(f)`: call f for each element, returns Unit.
    fn emit_iter_for_each(&mut self, receiver: &TypedExpr, f_expr: &TypedExpr, elem_ty: &JvmType) {
        self.push_scope();
        let (iter_slot, iter_jvm) = self.store_iterator(receiver);

        self.lower_expr(f_expr);
        let f_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            f_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));

        let loop_label = self.alloc_label();
        let end_label = self.alloc_label();

        self.ops.push(JvmOp::Label(loop_label));
        self.emit_frame(vec![]);

        self.emit_iter_next(iter_slot, &iter_jvm);
        let opt_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            opt_slot,
            JvmType::Object("valen/core/Option".to_string()),
        ));
        self.ops.push(JvmOp::LoadLocal(
            opt_slot,
            JvmType::Object("valen/core/Option".to_string()),
        ));
        self.ops
            .push(JvmOp::Instanceof("valen/core/Option$Some".to_string()));
        self.ops.push(JvmOp::IfEq(end_label));

        self.emit_extract_some_value(opt_slot, elem_ty);
        self.emit_box(elem_ty);
        let elem_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            elem_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));
        self.ops.push(JvmOp::LoadLocal(
            f_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));
        self.ops.push(JvmOp::LoadLocal(
            elem_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));
        self.emit_invoke_fn1(&JvmType::Void);
        // Function.apply returns Object, pop unused result
        self.ops.push(JvmOp::Pop);

        self.ops.push(JvmOp::Goto(loop_label));

        self.ops.push(JvmOp::Label(end_label));
        self.emit_frame(vec![]);
        self.pop_scope();
    }

    /// `iter.count() -> Int`: count elements.
    fn emit_iter_count(&mut self, receiver: &TypedExpr) {
        self.push_scope();
        let (iter_slot, iter_jvm) = self.store_iterator(receiver);

        self.ops.push(JvmOp::PushInt(0));
        let count_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(count_slot, JvmType::Int));

        let loop_label = self.alloc_label();
        let end_label = self.alloc_label();

        self.ops.push(JvmOp::Label(loop_label));
        self.emit_frame(vec![]);

        self.emit_iter_next(iter_slot, &iter_jvm);
        let opt_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            opt_slot,
            JvmType::Object("valen/core/Option".to_string()),
        ));
        self.ops.push(JvmOp::LoadLocal(
            opt_slot,
            JvmType::Object("valen/core/Option".to_string()),
        ));
        self.ops
            .push(JvmOp::Instanceof("valen/core/Option$Some".to_string()));
        self.ops.push(JvmOp::IfEq(end_label));

        // count++
        self.ops.push(JvmOp::LoadLocal(count_slot, JvmType::Int));
        self.ops.push(JvmOp::PushInt(1));
        self.ops.push(JvmOp::Arith(ArithOp::Add, JvmType::Int));
        self.ops.push(JvmOp::StoreLocal(count_slot, JvmType::Int));

        self.ops.push(JvmOp::Goto(loop_label));

        self.ops.push(JvmOp::Label(end_label));
        self.emit_frame(vec![]);
        self.ops.push(JvmOp::LoadLocal(count_slot, JvmType::Int));
        self.pop_scope();
    }

    /// `iter.any(pred) -> Bool`: true if any element matches.
    fn emit_iter_any(&mut self, receiver: &TypedExpr, pred_expr: &TypedExpr, elem_ty: &JvmType) {
        self.push_scope();
        let (iter_slot, iter_jvm) = self.store_iterator(receiver);

        self.lower_expr(pred_expr);
        let pred_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            pred_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));

        // result = false
        self.ops.push(JvmOp::PushInt(0));
        let result_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(result_slot, JvmType::Int));

        let loop_label = self.alloc_label();
        let end_label = self.alloc_label();

        self.ops.push(JvmOp::Label(loop_label));
        self.emit_frame(vec![]);

        self.emit_iter_next(iter_slot, &iter_jvm);
        let opt_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            opt_slot,
            JvmType::Object("valen/core/Option".to_string()),
        ));
        self.ops.push(JvmOp::LoadLocal(
            opt_slot,
            JvmType::Object("valen/core/Option".to_string()),
        ));
        self.ops
            .push(JvmOp::Instanceof("valen/core/Option$Some".to_string()));
        self.ops.push(JvmOp::IfEq(end_label));

        self.emit_extract_some_value(opt_slot, elem_ty);
        self.emit_box(elem_ty);
        let elem_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            elem_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));

        self.ops.push(JvmOp::LoadLocal(
            pred_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));
        self.ops.push(JvmOp::LoadLocal(
            elem_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));
        self.emit_invoke_fn1(&JvmType::Boolean);
        self.ops.push(JvmOp::IfEq(loop_label));
        // predicate matched — store true and exit
        self.ops.push(JvmOp::PushInt(1));
        self.ops.push(JvmOp::StoreLocal(result_slot, JvmType::Int));

        self.ops.push(JvmOp::Label(end_label));
        self.emit_frame(vec![]);
        self.ops.push(JvmOp::LoadLocal(result_slot, JvmType::Int));
        self.pop_scope();
    }

    /// `iter.all(pred) -> Bool`: true if all elements match.
    fn emit_iter_all(&mut self, receiver: &TypedExpr, pred_expr: &TypedExpr, elem_ty: &JvmType) {
        self.push_scope();
        let (iter_slot, iter_jvm) = self.store_iterator(receiver);

        self.lower_expr(pred_expr);
        let pred_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            pred_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));

        // result = true (assume all match until proven otherwise)
        self.ops.push(JvmOp::PushInt(1));
        let result_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(result_slot, JvmType::Int));

        let loop_label = self.alloc_label();
        let end_label = self.alloc_label();

        self.ops.push(JvmOp::Label(loop_label));
        self.emit_frame(vec![]);

        self.emit_iter_next(iter_slot, &iter_jvm);
        let opt_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            opt_slot,
            JvmType::Object("valen/core/Option".to_string()),
        ));
        self.ops.push(JvmOp::LoadLocal(
            opt_slot,
            JvmType::Object("valen/core/Option".to_string()),
        ));
        self.ops
            .push(JvmOp::Instanceof("valen/core/Option$Some".to_string()));
        self.ops.push(JvmOp::IfEq(end_label));

        self.emit_extract_some_value(opt_slot, elem_ty);
        self.emit_box(elem_ty);
        let elem_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            elem_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));

        self.ops.push(JvmOp::LoadLocal(
            pred_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));
        self.ops.push(JvmOp::LoadLocal(
            elem_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));
        self.emit_invoke_fn1(&JvmType::Boolean);
        self.ops.push(JvmOp::IfNe(loop_label));
        // predicate failed — store false and exit
        self.ops.push(JvmOp::PushInt(0));
        self.ops.push(JvmOp::StoreLocal(result_slot, JvmType::Int));

        self.ops.push(JvmOp::Label(end_label));
        self.emit_frame(vec![]);
        self.ops.push(JvmOp::LoadLocal(result_slot, JvmType::Int));
        self.pop_scope();
    }

    /// `iter.find(pred) -> Option<T>`: return first matching element.
    fn emit_iter_find(&mut self, receiver: &TypedExpr, pred_expr: &TypedExpr, elem_ty: &JvmType) {
        self.push_scope();
        let (iter_slot, iter_jvm) = self.store_iterator(receiver);

        self.lower_expr(pred_expr);
        let pred_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            pred_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));

        // result = None initially
        self.ops
            .push(JvmOp::New("valen/core/Option$None".to_string()));
        self.ops.push(JvmOp::Dup);
        self.ops.push(JvmOp::InvokeSpecial {
            owner: "valen/core/Option$None".to_string(),
            name: "<init>".to_string(),
            params: vec![],
            ret: JvmType::Void,
        });
        let result_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            result_slot,
            JvmType::Object("valen/core/Option".to_string()),
        ));

        let loop_label = self.alloc_label();
        let end_label = self.alloc_label();

        self.ops.push(JvmOp::Label(loop_label));
        self.emit_frame(vec![]);

        self.emit_iter_next(iter_slot, &iter_jvm);
        let opt_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            opt_slot,
            JvmType::Object("valen/core/Option".to_string()),
        ));
        self.ops.push(JvmOp::LoadLocal(
            opt_slot,
            JvmType::Object("valen/core/Option".to_string()),
        ));
        self.ops
            .push(JvmOp::Instanceof("valen/core/Option$Some".to_string()));
        self.ops.push(JvmOp::IfEq(end_label));

        self.emit_extract_some_value(opt_slot, elem_ty);
        self.emit_box(elem_ty);
        let elem_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            elem_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));

        self.ops.push(JvmOp::LoadLocal(
            pred_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));
        self.ops.push(JvmOp::LoadLocal(
            elem_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));
        self.emit_invoke_fn1(&JvmType::Boolean);
        self.ops.push(JvmOp::IfEq(loop_label));

        // predicate matched — store Some(elem) and exit
        self.ops
            .push(JvmOp::New("valen/core/Option$Some".to_string()));
        self.ops.push(JvmOp::Dup);
        self.ops.push(JvmOp::LoadLocal(
            elem_slot,
            JvmType::Object(JVM_OBJECT.to_string()),
        ));
        self.ops.push(JvmOp::InvokeSpecial {
            owner: "valen/core/Option$Some".to_string(),
            name: "<init>".to_string(),
            params: vec![JvmType::Object(JVM_OBJECT.to_string())],
            ret: JvmType::Void,
        });
        self.ops.push(JvmOp::StoreLocal(
            result_slot,
            JvmType::Object("valen/core/Option".to_string()),
        ));

        self.ops.push(JvmOp::Label(end_label));
        self.emit_frame(vec![]);
        self.ops.push(JvmOp::LoadLocal(
            result_slot,
            JvmType::Object("valen/core/Option".to_string()),
        ));
        self.pop_scope();
    }

    /// Constructs a `valen/core/Range` data class instance from a range expression.
    fn lower_range(
        &mut self,
        start: Option<&TypedExpr>,
        end: Option<&TypedExpr>,
        inclusive: bool,
        range_ty: &Ty,
    ) {
        let elem_ty = match range_ty {
            Ty::Generic(_, args) if !args.is_empty() => self.ty_to_jvm(&args[0]),
            _ => JvmType::Int,
        };
        let obj = JvmType::Object(JVM_OBJECT.to_string());
        let range_class = "valen/core/Range";

        self.ops.push(JvmOp::New(range_class.to_string()));
        self.ops.push(JvmOp::Dup);

        // start (boxed)
        if let Some(s) = start {
            self.lower_expr(s);
        } else {
            self.ops.push(JvmOp::PushInt(0));
        }
        self.emit_box(&elem_ty);

        // end (boxed)
        if let Some(e) = end {
            self.lower_expr(e);
        } else {
            self.ops.push(JvmOp::PushInt(i32::MAX));
        }
        self.emit_box(&elem_ty);

        // inclusive flag
        self.ops.push(JvmOp::PushInt(if inclusive { 1 } else { 0 }));

        self.ops.push(JvmOp::InvokeSpecial {
            owner: range_class.to_string(),
            name: INIT.to_string(),
            params: vec![obj.clone(), obj, JvmType::Boolean],
            ret: JvmType::Void,
        });
    }

    fn lower_builtin_print(&mut self, name: &str, args: &[TypedExpr]) {
        self.ops.push(JvmOp::GetStatic {
            owner: "java/lang/System".to_string(),
            name: "out".to_string(),
            descriptor: JvmType::Object("java/io/PrintStream".to_string()),
        });

        // Determine the PrintStream overload descriptor based on the argument type.
        // JVM PrintStream has overloads for: int, long, float, double, char, boolean,
        // String, and Object. For no-arg calls we emit an empty string.
        let param_ty = if let Some(arg) = args.first() {
            self.lower_expr(arg);
            self.print_stream_param_type(&arg.ty)
        } else {
            self.ops.push(JvmOp::PushString(String::new()));
            JvmType::Object(JVM_STRING.to_string())
        };

        self.ops.push(JvmOp::InvokeVirtual {
            owner: "java/io/PrintStream".to_string(),
            name: name.to_string(),
            params: vec![param_ty],
            ret: JvmType::Void,
        });
    }

    /// Returns the JVM type to use for the `PrintStream.println`/`print` overload
    /// corresponding to the given Valen type.
    ///
    /// JVM `PrintStream` provides overloads for `int`, `long`, `float`, `double`,
    /// `char`, `boolean`, `String`, and `Object`. Byte/Short are promoted to `int`.
    fn print_stream_param_type(&self, ty: &Ty) -> JvmType {
        match ty {
            Ty::Prim(PrimTy::Int | PrimTy::Byte | PrimTy::Short) => JvmType::Int,
            Ty::Prim(PrimTy::Long) => JvmType::Long,
            Ty::Prim(PrimTy::Float) => JvmType::Float,
            Ty::Prim(PrimTy::Double) => JvmType::Double,
            Ty::Prim(PrimTy::Char) => JvmType::Char,
            Ty::Prim(PrimTy::Bool) => JvmType::Boolean,
            Ty::Prim(PrimTy::String) => JvmType::Object(JVM_STRING.to_string()),
            // For all other types (named types, generics, etc.) use the Object overload.
            _ => JvmType::Object(JVM_OBJECT.to_string()),
        }
    }

    /// `[expr, ...]` → `new ArrayList(); .add(expr); ...`
    fn lower_list_literal(&mut self, elements: &[TypedExpr], result_ty: &Ty) {
        let elem_ty = match result_ty {
            Ty::Generic(_, args) if !args.is_empty() => self.ty_to_jvm(&args[0]),
            _ => JvmType::Object(JVM_OBJECT.to_string()),
        };

        let list_slot = self.emit_new_arraylist();
        for elem in elements {
            self.lower_expr(elem);
            self.emit_list_add(list_slot, &elem_ty);
        }
        self.ops.push(JvmOp::LoadLocal(
            list_slot,
            JvmType::Object("java/util/ArrayList".to_string()),
        ));
    }

    /// `#{key: value, ...}` → `new HashMap(); .put(key, value); ...`
    fn lower_map_literal(&mut self, entries: &[(TypedExpr, TypedExpr)], _result_ty: &Ty) {
        self.ops.push(JvmOp::New("java/util/HashMap".to_string()));
        self.ops.push(JvmOp::Dup);
        self.ops.push(JvmOp::InvokeSpecial {
            owner: "java/util/HashMap".to_string(),
            name: "<init>".to_string(),
            params: vec![],
            ret: JvmType::Void,
        });
        let map_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            map_slot,
            JvmType::Object("java/util/HashMap".to_string()),
        ));

        for (key, value) in entries {
            self.ops.push(JvmOp::LoadLocal(
                map_slot,
                JvmType::Object("java/util/HashMap".to_string()),
            ));
            self.lower_expr(key);
            let key_ty = self.ty_to_jvm(&key.ty);
            self.emit_box(&key_ty);
            self.lower_expr(value);
            let val_ty = self.ty_to_jvm(&value.ty);
            self.emit_box(&val_ty);
            self.ops.push(JvmOp::InvokeInterface {
                owner: "java/util/Map".to_string(),
                name: "put".to_string(),
                params: vec![
                    JvmType::Object(JVM_OBJECT.to_string()),
                    JvmType::Object(JVM_OBJECT.to_string()),
                ],
                ret: JvmType::Object(JVM_OBJECT.to_string()),
            });
            self.ops.push(JvmOp::Pop);
        }

        self.ops.push(JvmOp::LoadLocal(
            map_slot,
            JvmType::Object("java/util/HashMap".to_string()),
        ));
    }

    /// `iter(list)` → `new ListIterator(list)`
    fn lower_builtin_iter(&mut self, args: &[TypedExpr]) {
        if let Some(arg) = args.first() {
            self.ops
                .push(JvmOp::New("valen/core/ListIterator".to_string()));
            self.ops.push(JvmOp::Dup);
            self.lower_expr(arg);
            self.ops.push(JvmOp::InvokeSpecial {
                owner: "valen/core/ListIterator".to_string(),
                name: "<init>".to_string(),
                params: vec![JvmType::Object("java/util/List".to_string())],
                ret: JvmType::Void,
            });
        }
    }

    fn lower_if_let(
        &mut self,
        pattern: &valen_ast::Pattern,
        scrutinee: &TypedExpr,
        then_branch: &valen_hir::TypedBody,
        else_branch: Option<&TypedExpr>,
        result_ty: &Ty,
    ) {
        self.lower_expr(scrutinee);
        let scrutinee_ty = self.ty_to_jvm(&scrutinee.ty);
        let temp_slot = self.alloc_local(SmolStr::from("__if_let_scrutinee"), scrutinee_ty.clone());
        self.ops
            .push(JvmOp::StoreLocal(temp_slot, scrutinee_ty.clone()));

        let else_label = self.alloc_label();
        let end_label = self.alloc_label();

        let pre_scope_locals = self.locals_snapshot();

        self.push_scope();
        self.lower_pattern_check(pattern, temp_slot, &scrutinee_ty, else_label);
        self.lower_body(then_branch);
        self.pop_scope();
        self.ops.push(JvmOp::Goto(end_label));

        self.ops.push(JvmOp::Label(else_label));
        self.emit_frame_with_locals(pre_scope_locals.clone(), vec![]);
        if let Some(else_expr) = else_branch {
            self.lower_expr(else_expr);
        }

        self.ops.push(JvmOp::Label(end_label));
        let jvm_result = self.ty_to_jvm(result_ty);
        let end_stack = if matches!(jvm_result, JvmType::Void) {
            vec![]
        } else {
            vec![jvm_result]
        };
        self.emit_frame_with_locals(pre_scope_locals, end_stack);
    }

    fn lower_while_let(
        &mut self,
        pattern: &valen_ast::Pattern,
        scrutinee: &TypedExpr,
        body: &valen_hir::TypedBody,
    ) {
        let loop_start = self.alloc_label();
        let loop_end = self.alloc_label();

        let pre_loop_locals = self.locals_snapshot();

        self.ops.push(JvmOp::Label(loop_start));
        self.emit_frame_with_locals(pre_loop_locals.clone(), vec![]);

        self.lower_expr(scrutinee);
        let scrutinee_ty = self.ty_to_jvm(&scrutinee.ty);
        let temp_slot =
            self.alloc_local(SmolStr::from("__while_let_scrutinee"), scrutinee_ty.clone());
        self.ops
            .push(JvmOp::StoreLocal(temp_slot, scrutinee_ty.clone()));

        self.push_scope();
        self.lower_pattern_check(pattern, temp_slot, &scrutinee_ty, loop_end);
        self.loop_stack.push(LoopContext {
            break_label: loop_end,
            continue_label: loop_start,
        });
        self.lower_body(body);
        self.loop_stack.pop();
        self.pop_scope();
        self.ops.push(JvmOp::Goto(loop_start));

        self.ops.push(JvmOp::Label(loop_end));
        self.emit_frame_with_locals(pre_loop_locals, vec![]);
    }

    fn lower_try(&mut self, inner: &TypedExpr, is_option: bool, result_ty: &Ty) {
        let ok_label = self.alloc_label();
        let obj = JvmType::Object(JVM_OBJECT.to_string());

        self.lower_expr(inner);

        if is_option {
            self.ops.push(JvmOp::Dup);
            self.ops
                .push(JvmOp::Instanceof("valen/core/Option$Some".to_string()));
            self.ops.push(JvmOp::IfNe(ok_label));
            self.ops.push(JvmOp::Return(obj.clone()));

            self.ops.push(JvmOp::Label(ok_label));
            self.emit_frame(vec![JvmType::Object("valen/core/Option".to_string())]);
            self.ops
                .push(JvmOp::Checkcast("valen/core/Option$Some".to_string()));
            self.ops.push(JvmOp::GetField {
                owner: "valen/core/Option$Some".to_string(),
                name: "value".to_string(),
                descriptor: obj,
            });
        } else {
            self.ops.push(JvmOp::Dup);
            self.ops
                .push(JvmOp::Instanceof("valen/core/Result$Ok".to_string()));
            self.ops.push(JvmOp::IfNe(ok_label));
            self.ops.push(JvmOp::Return(obj.clone()));

            self.ops.push(JvmOp::Label(ok_label));
            self.emit_frame(vec![JvmType::Object("valen/core/Result".to_string())]);
            self.ops
                .push(JvmOp::Checkcast("valen/core/Result$Ok".to_string()));
            self.ops.push(JvmOp::GetField {
                owner: "valen/core/Result$Ok".to_string(),
                name: "value".to_string(),
                descriptor: obj,
            });
        }

        let jvm_ty = self.ty_to_jvm(result_ty);
        self.emit_unbox(&jvm_ty);
    }

    fn lower_cast(&mut self, from: &Ty, to: &Ty) {
        let from_jvm = self.ty_to_jvm(from);
        let to_jvm = self.ty_to_jvm(to);
        match (&from_jvm, &to_jvm) {
            (JvmType::Object(_), JvmType::Object(target)) => {
                self.ops.push(JvmOp::Checkcast(target.clone()));
            }
            _ if from_jvm != to_jvm => {
                self.ops.push(JvmOp::Convert {
                    from: from_jvm,
                    to: to_jvm,
                });
            }
            _ => {}
        }
    }

    fn lower_deref_read(&mut self, ref_ty: &Ty) {
        let Ty::RefMut(inner) = ref_ty else { return };
        let wrapper = ref_mut_wrapper_class(inner);
        let field_ty = self.inner_jvm_for_ref(inner);
        self.ops.push(JvmOp::GetField {
            owner: wrapper,
            name: "value".to_string(),
            descriptor: field_ty.clone(),
        });
        // For object refs, checkcast to the concrete type
        if matches!(field_ty, JvmType::Object(_)) {
            let target = self.ty_to_jvm(inner);
            if let JvmType::Object(ref target_name) = target {
                if target_name != JVM_OBJECT {
                    self.ops.push(JvmOp::Checkcast(target_name.clone()));
                }
            }
        }
    }

    fn lower_deref_write(&mut self, ref_ty: &Ty, _value_ty: &Ty) {
        let Ty::RefMut(inner) = ref_ty else { return };
        let wrapper = ref_mut_wrapper_class(inner);
        let field_ty = self.inner_jvm_for_ref(inner);
        self.ops.push(JvmOp::PutField {
            owner: wrapper,
            name: "value".to_string(),
            descriptor: field_ty,
        });
    }

    fn lower_ref_mut_create(&mut self, inner: &TypedExpr) {
        let wrapper = ref_mut_wrapper_class(&inner.ty);
        let field_ty = self.inner_jvm_for_ref(&inner.ty);
        self.ops.push(JvmOp::New(wrapper.clone()));
        self.ops.push(JvmOp::Dup);
        self.lower_expr(inner);
        self.ops.push(JvmOp::InvokeSpecial {
            owner: wrapper,
            name: "<init>".to_string(),
            params: vec![field_ty],
            ret: JvmType::Void,
        });
    }

    fn inner_jvm_for_ref(&self, inner: &Ty) -> JvmType {
        match inner {
            Ty::Prim(PrimTy::Int | PrimTy::Byte | PrimTy::Short | PrimTy::Char) => JvmType::Int,
            Ty::Prim(PrimTy::Long) => JvmType::Long,
            Ty::Prim(PrimTy::Float) => JvmType::Float,
            Ty::Prim(PrimTy::Double) => JvmType::Double,
            Ty::Prim(PrimTy::Bool) => JvmType::Boolean,
            _ => JvmType::Object(JVM_OBJECT.to_string()),
        }
    }

    /// Lowers a `safe {}` block into a JVM try-catch that produces
    /// `Result<T, JavaException>`. Success wraps in `Result$Ok`,
    /// exception wraps in `Result$Err(JavaException(message, className))`.
    fn lower_safe(&mut self, body: &TypedBody, result_ty: &Ty) {
        let try_start = self.alloc_label();
        let try_end = self.alloc_label();
        let handler_label = self.alloc_label();
        let end_label = self.alloc_label();

        let result_jvm = self.ty_to_jvm(result_ty);
        let inner_jvm = match result_ty {
            Ty::Generic(_, args) if !args.is_empty() => self.ty_to_jvm(&args[0]),
            _ => JvmType::Object(JVM_OBJECT.to_string()),
        };
        let obj = JvmType::Object(JVM_OBJECT.to_string());
        let str_ty = JvmType::Object(JVM_STRING.to_string());

        let result_slot = self.next_slot;
        self.next_slot += 1;

        // --- try region ---
        self.ops.push(JvmOp::Label(try_start));

        self.push_scope();
        self.lower_body(body);
        self.pop_scope();

        // Box primitive body value for erasure-safe storage in Ok(Object)
        if matches!(inner_jvm, JvmType::Void) {
            self.ops.push(JvmOp::PushNull);
        } else {
            self.emit_box(&inner_jvm);
        }
        let val_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(val_slot, obj.clone()));

        self.ops
            .push(JvmOp::New("valen/core/Result$Ok".to_string()));
        self.ops.push(JvmOp::Dup);
        self.ops.push(JvmOp::LoadLocal(val_slot, obj.clone()));
        self.ops.push(JvmOp::InvokeSpecial {
            owner: "valen/core/Result$Ok".to_string(),
            name: INIT.to_string(),
            params: vec![obj.clone()],
            ret: JvmType::Void,
        });
        self.ops
            .push(JvmOp::StoreLocal(result_slot, result_jvm.clone()));
        self.ops.push(JvmOp::Goto(end_label));

        // --- exception handler ---
        self.ops.push(JvmOp::Label(try_end));
        self.ops.push(JvmOp::Label(handler_label));
        self.emit_frame(vec![JvmType::Object("java/lang/Exception".to_string())]);

        let exc_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            exc_slot,
            JvmType::Object("java/lang/Exception".to_string()),
        ));

        // exception.getMessage()
        self.ops.push(JvmOp::LoadLocal(
            exc_slot,
            JvmType::Object("java/lang/Exception".to_string()),
        ));
        self.ops.push(JvmOp::InvokeVirtual {
            owner: "java/lang/Exception".to_string(),
            name: "getMessage".to_string(),
            params: vec![],
            ret: str_ty.clone(),
        });
        let msg_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(msg_slot, str_ty.clone()));

        // exception.getClass().getName()
        self.ops.push(JvmOp::LoadLocal(
            exc_slot,
            JvmType::Object("java/lang/Exception".to_string()),
        ));
        self.ops.push(JvmOp::InvokeVirtual {
            owner: JVM_OBJECT.to_string(),
            name: "getClass".to_string(),
            params: vec![],
            ret: JvmType::Object("java/lang/Class".to_string()),
        });
        self.ops.push(JvmOp::InvokeVirtual {
            owner: "java/lang/Class".to_string(),
            name: "getName".to_string(),
            params: vec![],
            ret: str_ty.clone(),
        });
        let cls_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(cls_slot, str_ty.clone()));

        // new JavaException(message, class_name)
        self.ops
            .push(JvmOp::New("valen/core/JavaException".to_string()));
        self.ops.push(JvmOp::Dup);
        self.ops.push(JvmOp::LoadLocal(msg_slot, str_ty.clone()));
        self.ops.push(JvmOp::LoadLocal(cls_slot, str_ty));
        self.ops.push(JvmOp::InvokeSpecial {
            owner: "valen/core/JavaException".to_string(),
            name: INIT.to_string(),
            params: vec![
                JvmType::Object(JVM_STRING.to_string()),
                JvmType::Object(JVM_STRING.to_string()),
            ],
            ret: JvmType::Void,
        });
        let je_slot = self.next_slot;
        self.next_slot += 1;
        self.ops.push(JvmOp::StoreLocal(
            je_slot,
            JvmType::Object("valen/core/JavaException".to_string()),
        ));

        // new Result$Err(javaException)
        self.ops
            .push(JvmOp::New("valen/core/Result$Err".to_string()));
        self.ops.push(JvmOp::Dup);
        self.ops.push(JvmOp::LoadLocal(
            je_slot,
            JvmType::Object("valen/core/JavaException".to_string()),
        ));
        self.ops.push(JvmOp::InvokeSpecial {
            owner: "valen/core/Result$Err".to_string(),
            name: INIT.to_string(),
            params: vec![obj],
            ret: JvmType::Void,
        });
        self.ops
            .push(JvmOp::StoreLocal(result_slot, result_jvm.clone()));

        // --- end ---
        self.ops.push(JvmOp::Label(end_label));
        self.emit_frame(vec![result_jvm.clone()]);
        self.ops.push(JvmOp::LoadLocal(result_slot, result_jvm));

        self.exception_handlers.push(ExceptionHandler {
            start: try_start,
            end: try_end,
            handler: handler_label,
            catch_type: Some("java/lang/Exception".to_string()),
        });
    }

    // TODO(#069): String interpolation uses StringBuilder instead of JDK 9+
    // invokedynamic with StringConcatFactory bootstrap. The invokedynamic approach
    // produces less bytecode and allows the JVM to optimize small concatenations.
    // To implement: emit `invokedynamic` with `java/lang/invoke/StringConcatFactory`
    // as the bootstrap method, with a recipe string matching the interpolation pattern.
    fn lower_string_interp(&mut self, parts: &[TypedStringPart]) {
        self.ops.push(JvmOp::New(JVM_STRING_BUILDER.to_string()));
        self.ops.push(JvmOp::Dup);
        self.ops.push(JvmOp::InvokeSpecial {
            owner: JVM_STRING_BUILDER.to_string(),
            name: INIT.to_string(),
            params: vec![],
            ret: JvmType::Void,
        });

        for part in parts {
            match part {
                TypedStringPart::Text(s) => {
                    self.ops.push(JvmOp::PushString(s.to_string()));
                    self.ops.push(JvmOp::InvokeVirtual {
                        owner: JVM_STRING_BUILDER.to_string(),
                        name: APPEND.to_string(),
                        params: vec![JvmType::Object(JVM_STRING.to_string())],
                        ret: JvmType::Object(JVM_STRING_BUILDER.to_string()),
                    });
                }
                TypedStringPart::Expr(expr) => {
                    self.lower_expr(expr);
                    let append_ty = self.sb_append_type(&expr.ty);
                    self.ops.push(JvmOp::InvokeVirtual {
                        owner: JVM_STRING_BUILDER.to_string(),
                        name: APPEND.to_string(),
                        params: vec![append_ty],
                        ret: JvmType::Object(JVM_STRING_BUILDER.to_string()),
                    });
                }
            }
        }

        self.ops.push(JvmOp::InvokeVirtual {
            owner: JVM_STRING_BUILDER.to_string(),
            name: TO_STRING.to_string(),
            params: vec![],
            ret: JvmType::Object(JVM_STRING.to_string()),
        });
    }

    fn sb_append_type(&self, ty: &Ty) -> JvmType {
        match ty {
            Ty::Prim(PrimTy::Int | PrimTy::Byte | PrimTy::Short) => JvmType::Int,
            Ty::Prim(PrimTy::Long) => JvmType::Long,
            Ty::Prim(PrimTy::Float) => JvmType::Float,
            Ty::Prim(PrimTy::Double) => JvmType::Double,
            Ty::Prim(PrimTy::Char) => JvmType::Char,
            Ty::Prim(PrimTy::Bool) => JvmType::Boolean,
            _ => JvmType::Object(JVM_OBJECT.to_string()),
        }
    }
}

fn ref_mut_wrapper_class(inner: &Ty) -> String {
    match inner {
        Ty::Prim(PrimTy::Int | PrimTy::Byte | PrimTy::Short | PrimTy::Char) => {
            "valen/core/IntRef".to_string()
        }
        Ty::Prim(PrimTy::Long) => "valen/core/LongRef".to_string(),
        Ty::Prim(PrimTy::Float) => "valen/core/FloatRef".to_string(),
        Ty::Prim(PrimTy::Double) => "valen/core/DoubleRef".to_string(),
        Ty::Prim(PrimTy::Bool) => "valen/core/BoolRef".to_string(),
        _ => "valen/core/Ref".to_string(),
    }
}
