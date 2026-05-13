//! Lowers typed HIR expressions and statements into JVM bytecode operations.

use indexmap::IndexMap;
use smol_str::SmolStr;
use valen_ast::{BinaryOp, UnaryOp};
use valen_hir::{PrimTy, Ty, TypedBody, TypedExpr, TypedExprKind, TypedStmt, TypedStringPart};

use crate::jvm_const::*;
use crate::jvm_ir::{ArithOp, BitwiseOp, CmpKind, JvmMethodBody, JvmOp, JvmType, Label};

struct LoopContext {
    break_label: Label,
    continue_label: Label,
}

/// Undo entry for a single variable binding: the variable name and its
/// previous binding (`None` if it was newly introduced in this scope).
type ScopeUndo = Vec<(SmolStr, Option<(u16, JvmType)>)>;

struct ExprLowering<'a> {
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
}

/// Lowers a typed method body into JVM bytecode operations.
pub fn lower_body(
    body: &TypedBody,
    class_internal: &str,
    params: &[(SmolStr, JvmType)],
    return_ty: &JvmType,
    has_self: bool,
    pkg: Option<&[SmolStr]>,
) -> JvmMethodBody {
    let mut ctx = ExprLowering {
        ops: Vec::new(),
        locals: IndexMap::new(),
        next_slot: 0,
        next_label: 1000,
        class_internal,
        return_ty: return_ty.clone(),
        loop_stack: Vec::new(),
        pkg,
        scope_stack: Vec::new(),
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

    JvmMethodBody {
        max_locals: ctx.next_slot,
        ops: ctx.ops,
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

    fn pop_if_needed(&mut self, ty: &Ty) {
        if matches!(ty, Ty::Prim(PrimTy::Unit) | Ty::Error) {
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
            Ty::Named(n) => JvmType::Object(crate::descriptor::class_internal_name(n, self.pkg)),
            Ty::Generic(n, _) => {
                JvmType::Object(crate::descriptor::class_internal_name(n, self.pkg))
            }
            Ty::Nullable(inner) => {
                let inner_jvm = self.ty_to_jvm(inner);
                match JvmType::boxed_name(&inner_jvm) {
                    Some(boxed) => JvmType::Object(boxed.to_string()),
                    None => inner_jvm,
                }
            }
            Ty::Fn(_, _) => JvmType::Object(JVM_OBJECT.to_string()),
            Ty::Error => JvmType::Object(JVM_OBJECT.to_string()),
        }
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
                    let i = i32::try_from(*n)
                        .unwrap_or_else(|_| panic!("integer literal {} out of i32 range", n));
                    self.ops.push(JvmOp::PushInt(i));
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
            TypedExprKind::Call { callee, args } => {
                self.lower_call(callee, args, &expr.ty);
            }
            TypedExprKind::MethodCall {
                receiver,
                method,
                args,
            } => {
                self.lower_expr(receiver);
                for arg in args {
                    self.lower_expr(arg);
                }
                let receiver_ty = self.ty_to_jvm(&receiver.ty);
                let ret_ty = self.ty_to_jvm(&expr.ty);
                let param_tys: Vec<JvmType> = args.iter().map(|a| self.ty_to_jvm(&a.ty)).collect();
                if let JvmType::Object(owner) = receiver_ty {
                    self.ops.push(JvmOp::InvokeVirtual {
                        owner,
                        name: method.to_string(),
                        params: param_tys,
                        ret: ret_ty,
                    });
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
                self.lower_if(cond, then_branch, else_branch.as_deref());
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
            TypedExprKind::Break(_val) => {
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
                // MVP: stub — Iterator/Iterable integration not yet available
                let _ = (var, iter, body);
                self.ops.push(JvmOp::StubBody);
            }
            TypedExprKind::Lambda { .. } => {
                // MVP: stub — InvokeDynamic + LambdaMetafactory not yet wired
                self.ops.push(JvmOp::StubBody);
            }
            TypedExprKind::Range { .. } => {
                // MVP: stub — Range type not yet in stdlib
                self.ops.push(JvmOp::StubBody);
            }
            TypedExprKind::StringInterp(parts) => {
                self.lower_string_interp(parts);
            }
            TypedExprKind::Error => {}
        }
    }

    fn lower_call(&mut self, callee: &TypedExpr, args: &[TypedExpr], result_ty: &Ty) {
        let ret_ty = self.ty_to_jvm(result_ty);
        let param_tys: Vec<JvmType> = args.iter().map(|a| self.ty_to_jvm(&a.ty)).collect();

        match &callee.kind {
            TypedExprKind::LocalVar(name) => {
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
                self.lower_expr(callee);
                for arg in args {
                    self.lower_expr(arg);
                }
                self.ops.push(JvmOp::InvokeStatic {
                    owner: self.class_internal.to_string(),
                    name: "apply".to_string(),
                    params: param_tys,
                    ret: ret_ty,
                });
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
                    BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                        unreachable!("ordering comparison on Object/Array types not supported")
                    }
                    _ => unreachable!(),
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
        self.ops.push(JvmOp::PushInt(0));
        self.ops.push(JvmOp::Label(end_label));
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
        self.ops.push(JvmOp::PushInt(0));
        self.ops.push(JvmOp::Label(end_label));
    }

    fn lower_short_circuit_or(&mut self, lhs: &TypedExpr, rhs: &TypedExpr) {
        let true_label = self.alloc_label();
        let end_label = self.alloc_label();

        self.lower_expr(lhs);
        self.ops.push(JvmOp::IfNe(true_label));
        self.lower_expr(rhs);
        self.ops.push(JvmOp::Goto(end_label));
        self.ops.push(JvmOp::Label(true_label));
        self.ops.push(JvmOp::PushInt(1));
        self.ops.push(JvmOp::Label(end_label));
    }

    fn lower_if(
        &mut self,
        cond: &TypedExpr,
        then_branch: &TypedBody,
        else_branch: Option<&TypedExpr>,
    ) {
        let else_label = self.alloc_label();
        let end_label = self.alloc_label();

        self.lower_expr(cond);
        self.ops.push(JvmOp::IfEq(else_label));
        self.push_scope();
        self.lower_body(then_branch);
        self.pop_scope();

        if else_branch.is_some() {
            self.ops.push(JvmOp::Goto(end_label));
        }

        self.ops.push(JvmOp::Label(else_label));

        if let Some(else_expr) = else_branch {
            self.push_scope();
            self.lower_expr(else_expr);
            self.pop_scope();
            self.ops.push(JvmOp::Label(end_label));
        }
    }

    fn lower_match(
        &mut self,
        scrutinee: &TypedExpr,
        arms: &[valen_hir::TypedMatchArm],
        _result_ty: &Ty,
    ) {
        self.lower_expr(scrutinee);
        let scrutinee_ty = self.ty_to_jvm(&scrutinee.ty);
        let temp_slot = self.next_slot;
        self.next_slot += scrutinee_ty.slot_count();
        self.ops
            .push(JvmOp::StoreLocal(temp_slot, scrutinee_ty.clone()));

        let end_label = self.alloc_label();

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
            }
        }

        self.ops.push(JvmOp::Label(end_label));
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
                self.next_slot += 1;
                self.ops.push(JvmOp::StoreLocal(cast_slot, cast_ty.clone()));

                for field in &sp.fields {
                    self.ops.push(JvmOp::LoadLocal(cast_slot, cast_ty.clone()));
                    // TODO(#021): field type is hardcoded to Object — should resolve actual
                    // field types from the variant definition (requires passing enum variant
                    // type information through pattern lowering).
                    let field_ty = JvmType::Object(JVM_OBJECT.to_string());
                    self.ops.push(JvmOp::GetField {
                        owner: variant_internal.clone(),
                        name: field.name.to_string(),
                        descriptor: field_ty.clone(),
                    });
                    if let Some(pat) = &field.pattern {
                        let inner_slot = self.next_slot;
                        self.next_slot += 1;
                        self.ops
                            .push(JvmOp::StoreLocal(inner_slot, field_ty.clone()));
                        self.lower_pattern_check(pat, inner_slot, &field_ty, fail_label);
                    } else {
                        let slot = self.alloc_local(field.name.clone(), field_ty.clone());
                        self.ops.push(JvmOp::StoreLocal(slot, field_ty));
                    }
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
                    }
                }
                self.ops.push(JvmOp::Label(success_label));
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
        }
    }

    fn lower_literal(&mut self, lit: &valen_ast::Literal) {
        match lit {
            valen_ast::Literal::Int(n, _) => {
                let i = i32::try_from(*n)
                    .unwrap_or_else(|_| panic!("integer literal {} out of i32 range", n));
                self.ops.push(JvmOp::PushInt(i));
            }
            valen_ast::Literal::Long(n, _) => self.ops.push(JvmOp::PushLong(*n)),
            valen_ast::Literal::Float(n, _) => self.ops.push(JvmOp::PushFloat(*n)),
            valen_ast::Literal::Double(n, _) => self.ops.push(JvmOp::PushDouble(*n)),
            valen_ast::Literal::Char(c, _) => self.ops.push(JvmOp::PushInt(*c as i32)),
            valen_ast::Literal::String(s, _) => self.ops.push(JvmOp::PushString(s.to_string())),
            valen_ast::Literal::Bool(b, _) => self.ops.push(JvmOp::PushInt(if *b { 1 } else { 0 })),
            valen_ast::Literal::Unit(_) => {}
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
    }

    fn lower_loop(&mut self, body: &TypedBody) {
        let continue_label = self.alloc_label();
        let break_label = self.alloc_label();

        self.ops.push(JvmOp::Label(continue_label));

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
    }

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
