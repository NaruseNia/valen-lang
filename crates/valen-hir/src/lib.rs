pub mod coherence;
pub mod exhaustive;
pub mod resolve;
pub mod ty;

use indexmap::IndexMap;
use smol_str::SmolStr;
use valen_ast::{BinaryOp, Span, UnaryOp};

pub type DefId = u32;

#[derive(Debug, Default)]
pub struct Hir {
    pub defs: IndexMap<DefId, Def>,
    pub package: Option<Vec<SmolStr>>,
    pub type_methods: IndexMap<SmolStr, Vec<DefId>>,
    pub trait_impls: Vec<ImplEntry>,
    next_id: DefId,
}

#[derive(Debug, Clone)]
pub struct ImplEntry {
    pub trait_name: SmolStr,
    pub target_name: SmolStr,
    pub methods: Vec<DefId>,
}

impl Hir {
    pub fn alloc_id(&mut self) -> DefId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn resolve_method(&self, type_name: &str, method_name: &str) -> MethodResolution {
        if let Some(class_methods) = self.type_methods.get(type_name) {
            for &mid in class_methods {
                if let Some(def) = self.defs.get(&mid) {
                    if def.name == method_name {
                        return MethodResolution::Found(mid);
                    }
                }
            }
        }

        let mut trait_candidates = Vec::new();
        for entry in &self.trait_impls {
            if entry.target_name == type_name {
                for &mid in &entry.methods {
                    if let Some(def) = self.defs.get(&mid) {
                        if def.name == method_name {
                            trait_candidates.push(mid);
                        }
                    }
                }
            }
        }

        match trait_candidates.len() {
            0 => MethodResolution::NotFound,
            1 => MethodResolution::Found(trait_candidates[0]),
            _ => MethodResolution::Ambiguous(trait_candidates),
        }
    }

    pub fn check_visibility(&self, def_id: DefId, accessor_type: Option<&str>) -> bool {
        let Some(def) = self.defs.get(&def_id) else {
            return false;
        };
        match def.vis {
            Vis::Pub => true,
            // TODO: Internal should be restricted to the same module scope.
            // Currently treated identically to Pub because the module system
            // (module DefId / package boundaries) is not yet implemented.
            // Once modules are materialized, check that the accessor is in the
            // same module as the definition.
            Vis::Internal => true,
            Vis::Private => {
                if let Some(accessor) = accessor_type {
                    self.is_member_of(def_id, accessor)
                } else {
                    false
                }
            }
        }
    }

    fn is_member_of(&self, def_id: DefId, type_name: &str) -> bool {
        if let Some(methods) = self.type_methods.get(type_name) {
            return methods.contains(&def_id);
        }
        false
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MethodResolution {
    Found(DefId),
    Ambiguous(Vec<DefId>),
    NotFound,
}

#[derive(Debug, Clone)]
pub struct Def {
    pub id: DefId,
    pub name: SmolStr,
    pub kind: DefKind,
    pub vis: Vis,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vis {
    Pub,
    Internal,
    Private,
}

#[derive(Debug, Clone)]
pub enum DefKind {
    Fn(FnDef),
    Class(ClassDef),
    DataClass(DataClassDef),
    Enum(EnumDef),
    Trait(TraitDef),
    Impl(ImplDef),
    TypeAlias,
}

#[derive(Debug, Clone)]
pub struct FnDef {
    pub params: Vec<ParamDef>,
    pub return_ty: Option<TyRef>,
    pub has_body: bool,
}

#[derive(Debug, Clone)]
pub struct ParamDef {
    pub name: SmolStr,
    pub ty: TyRef,
    pub mutable: bool,
    pub is_self: bool,
}

#[derive(Debug, Clone)]
pub struct ClassDef {
    pub kind: ClassDefKind,
    pub ctor_params: Vec<CtorParamDef>,
    pub superclass: Option<TyRef>,
    pub trait_impls: Vec<TyRef>,
    pub methods: Vec<DefId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassDefKind {
    Final,
    Open,
    Abstract,
    Sealed,
}

#[derive(Debug, Clone)]
pub struct CtorParamDef {
    pub vis: Vis,
    pub name: SmolStr,
    pub ty: TyRef,
    pub mutable: bool,
}

#[derive(Debug, Clone)]
pub struct DataClassDef {
    pub ctor_params: Vec<CtorParamDef>,
}

#[derive(Debug, Clone)]
pub struct EnumDef {
    pub variants: Vec<EnumVariantDef>,
}

#[derive(Debug, Clone)]
pub struct EnumVariantDef {
    pub name: SmolStr,
    pub fields: Vec<(SmolStr, TyRef)>,
}

#[derive(Debug, Clone)]
pub struct TraitDef {
    pub methods: Vec<DefId>,
}

#[derive(Debug, Clone)]
pub struct ImplDef {
    pub trait_ref: TyRef,
    pub target: TyRef,
    pub methods: Vec<DefId>,
    pub generics: Vec<SmolStr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TyRef {
    Prim(PrimTy),
    Named(SmolStr),
    Generic(SmolStr, Vec<TyRef>),
    Nullable(Box<TyRef>),
    Fn(Vec<TyRef>, Box<TyRef>),
    SelfTy,
    Unresolved(SmolStr),
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimTy {
    Int,
    Long,
    Float,
    Double,
    Bool,
    Char,
    Byte,
    Short,
    String,
    Unit,
    Nothing,
}

// ---------------------------------------------------------------------------
// Ty — semantic type used during type checking (distinct from syntactic TyRef)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ty {
    Prim(PrimTy),
    Named(SmolStr),
    Generic(SmolStr, Vec<Ty>),
    Nullable(Box<Ty>),
    Fn(Vec<Ty>, Box<Ty>),
    Error,
}

impl Ty {
    pub fn unit() -> Self {
        Ty::Prim(PrimTy::Unit)
    }
    pub fn nothing() -> Self {
        Ty::Prim(PrimTy::Nothing)
    }
    pub fn is_error(&self) -> bool {
        matches!(self, Ty::Error)
    }
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Ty::Prim(
                PrimTy::Int
                    | PrimTy::Long
                    | PrimTy::Float
                    | PrimTy::Double
                    | PrimTy::Byte
                    | PrimTy::Short
            )
        )
    }
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            Ty::Prim(PrimTy::Int | PrimTy::Long | PrimTy::Byte | PrimTy::Short)
        )
    }
    pub fn is_bool(&self) -> bool {
        matches!(self, Ty::Prim(PrimTy::Bool))
    }
}

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ty::Prim(p) => write!(f, "{p}"),
            Ty::Named(n) => write!(f, "{n}"),
            Ty::Generic(n, args) => {
                write!(f, "{n}<")?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{a}")?;
                }
                write!(f, ">")
            }
            Ty::Nullable(inner) => write!(f, "{inner}?"),
            Ty::Fn(params, ret) => {
                write!(f, "fn(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{p}")?;
                }
                write!(f, ") -> {ret}")
            }
            Ty::Error => write!(f, "<error>"),
        }
    }
}

impl std::fmt::Display for PrimTy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrimTy::Int => write!(f, "Int"),
            PrimTy::Long => write!(f, "Long"),
            PrimTy::Float => write!(f, "Float"),
            PrimTy::Double => write!(f, "Double"),
            PrimTy::Bool => write!(f, "Bool"),
            PrimTy::Char => write!(f, "Char"),
            PrimTy::Byte => write!(f, "Byte"),
            PrimTy::Short => write!(f, "Short"),
            PrimTy::String => write!(f, "String"),
            PrimTy::Unit => write!(f, "Unit"),
            PrimTy::Nothing => write!(f, "Nothing"),
        }
    }
}

impl std::fmt::Display for TyRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TyRef::Prim(p) => write!(f, "{p}"),
            TyRef::Named(n) => write!(f, "{n}"),
            TyRef::Generic(n, args) => {
                write!(f, "{n}<")?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{a}")?;
                }
                write!(f, ">")
            }
            TyRef::Nullable(inner) => write!(f, "{inner}?"),
            TyRef::Fn(params, ret) => {
                write!(f, "fn(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{p}")?;
                }
                write!(f, ") -> {ret}")
            }
            TyRef::SelfTy => write!(f, "Self"),
            TyRef::Unresolved(n) => write!(f, "{n}"),
            TyRef::Error => write!(f, "<error>"),
        }
    }
}

pub fn tyref_to_ty(tyref: &TyRef) -> Ty {
    match tyref {
        TyRef::Prim(p) => Ty::Prim(*p),
        TyRef::Named(n) => Ty::Named(n.clone()),
        TyRef::Generic(n, args) => Ty::Generic(n.clone(), args.iter().map(tyref_to_ty).collect()),
        TyRef::Nullable(inner) => Ty::Nullable(Box::new(tyref_to_ty(inner))),
        TyRef::Fn(params, ret) => Ty::Fn(
            params.iter().map(tyref_to_ty).collect(),
            Box::new(tyref_to_ty(ret)),
        ),
        TyRef::SelfTy | TyRef::Unresolved(_) | TyRef::Error => Ty::Error,
    }
}

/// Map a primitive type name to `PrimTy`. Shared by resolve and type-check.
pub fn resolve_prim(name: &str) -> Option<PrimTy> {
    match name {
        "Int" => Some(PrimTy::Int),
        "Long" => Some(PrimTy::Long),
        "Float" => Some(PrimTy::Float),
        "Double" => Some(PrimTy::Double),
        "Bool" => Some(PrimTy::Bool),
        "Char" => Some(PrimTy::Char),
        "Byte" => Some(PrimTy::Byte),
        "Short" => Some(PrimTy::Short),
        "String" => Some(PrimTy::String),
        "Unit" => Some(PrimTy::Unit),
        "Nothing" => Some(PrimTy::Nothing),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Typed HIR — expression/statement trees with Ty annotations
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TypedBody {
    pub stmts: Vec<TypedStmt>,
    pub tail: Option<Box<TypedExpr>>,
    pub ty: Ty,
}

#[derive(Debug, Clone)]
pub enum TypedStmt {
    Let {
        name: SmolStr,
        ty: Ty,
        init: TypedExpr,
        mutable: bool,
        span: Span,
    },
    Expr(TypedExpr),
    ExprSemi(TypedExpr),
}

#[derive(Debug, Clone)]
pub struct TypedExpr {
    pub kind: TypedExprKind,
    pub ty: Ty,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum TypedExprKind {
    IntLit(i64),
    LongLit(i64),
    FloatLit(f64),
    Float32Lit(f32),
    CharLit(char),
    StringLit(SmolStr),
    BoolLit(bool),
    UnitLit,
    LocalVar(SmolStr),
    FieldAccess {
        receiver: Box<TypedExpr>,
        field: SmolStr,
    },
    Call {
        callee: Box<TypedExpr>,
        args: Vec<TypedExpr>,
    },
    MethodCall {
        receiver: Box<TypedExpr>,
        method: SmolStr,
        args: Vec<TypedExpr>,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<TypedExpr>,
        rhs: Box<TypedExpr>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<TypedExpr>,
    },
    If {
        cond: Box<TypedExpr>,
        then_branch: TypedBody,
        else_branch: Option<Box<TypedExpr>>,
    },
    Match {
        scrutinee: Box<TypedExpr>,
        arms: Vec<TypedMatchArm>,
    },
    Block(TypedBody),
    Return(Option<Box<TypedExpr>>),
    Break(Option<Box<TypedExpr>>),
    Continue,
    Assign {
        target: Box<TypedExpr>,
        value: Box<TypedExpr>,
    },
    For {
        var: SmolStr,
        iter: Box<TypedExpr>,
        body: TypedBody,
    },
    While {
        cond: Box<TypedExpr>,
        body: TypedBody,
    },
    Loop {
        body: TypedBody,
    },
    Lambda {
        params: Vec<(SmolStr, Ty)>,
        body: Box<TypedExpr>,
    },
    Range {
        start: Option<Box<TypedExpr>>,
        end: Option<Box<TypedExpr>>,
        inclusive: bool,
    },
    StringInterp(Vec<TypedStringPart>),
    Error,
}

#[derive(Debug, Clone)]
pub struct TypedMatchArm {
    pub pattern: valen_ast::Pattern,
    pub guard: Option<TypedExpr>,
    pub body: TypedExpr,
}

#[derive(Debug, Clone)]
pub enum TypedStringPart {
    Text(SmolStr),
    Expr(TypedExpr),
}
