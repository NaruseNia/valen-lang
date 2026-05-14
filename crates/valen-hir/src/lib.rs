//! High-level Intermediate Representation (HIR) for the Valen compiler.
//!
//! This crate defines the core HIR types and provides name resolution, type
//! checking, trait coherence verification, and match exhaustiveness checking.

pub mod classpath;
pub mod coherence;
pub mod exhaustive;
pub mod resolve;
pub mod ty;

use indexmap::IndexMap;
use smol_str::SmolStr;
use valen_ast::{BinaryOp, Span, UnaryOp};

/// Unique identifier for a definition in the HIR.
pub type DefId = u32;

/// The top-level HIR container holding all definitions in a compilation unit.
#[derive(Debug, Default)]
pub struct Hir {
    /// All definitions keyed by their unique `DefId`.
    pub defs: IndexMap<DefId, Def>,
    /// Package path declared via `package` statement, if any.
    pub package: Option<Vec<SmolStr>>,
    /// Methods defined directly in class bodies, indexed by type name.
    pub type_methods: IndexMap<SmolStr, Vec<DefId>>,
    /// Trait impl entries used for method resolution and coherence checking.
    pub trait_impls: Vec<ImplEntry>,
    /// Import path mappings: short name (or alias) → full path segments.
    pub imports: IndexMap<SmolStr, Vec<SmolStr>>,
    /// Metadata for imported Java types loaded from classpath .class files.
    pub foreign_types: IndexMap<SmolStr, ForeignClassInfo>,
    /// DefIds of synthetic prelude types (should not be emitted by codegen).
    pub prelude_ids: Vec<DefId>,
    next_id: DefId,
}

/// A record of a trait impl linking a trait, target type, and implemented methods.
#[derive(Debug, Clone)]
pub struct ImplEntry {
    /// Name of the trait being implemented.
    pub trait_name: SmolStr,
    /// Name of the type the trait is implemented for.
    pub target_name: SmolStr,
    /// `DefId`s of the methods provided by this impl.
    pub methods: Vec<DefId>,
}

impl Hir {
    /// Allocate and return the next unused `DefId`.
    pub fn alloc_id(&mut self) -> DefId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Resolve a method on `type_name`, checking class body methods first, then trait impls.
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

    /// Check whether `def_id` is visible from the given accessor type context.
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

/// Result of resolving a method on a type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MethodResolution {
    /// Exactly one matching method was found.
    Found(DefId),
    /// Multiple trait impls provide the same method name.
    Ambiguous(Vec<DefId>),
    /// No matching method exists.
    NotFound,
}

/// A single top-level definition in the HIR.
#[derive(Debug, Clone)]
pub struct Def {
    pub id: DefId,
    pub name: SmolStr,
    pub kind: DefKind,
    pub vis: Vis,
    pub span: Span,
    /// Package this definition belongs to (from the enclosing `package` declaration).
    pub package: Option<Vec<SmolStr>>,
}

/// Visibility level of a definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vis {
    /// Visible everywhere.
    Pub,
    /// Visible within the same module (currently treated as `Pub`).
    Internal,
    /// Visible only within the defining type.
    Private,
}

/// The kind of a definition, carrying kind-specific data.
#[derive(Debug, Clone)]
pub enum DefKind {
    /// A function or method.
    Fn(FnDef),
    /// A class (final, open, abstract, or sealed).
    Class(ClassDef),
    /// A data class with auto-generated accessors and structural equality.
    DataClass(DataClassDef),
    /// An enum with named variants.
    Enum(EnumDef),
    /// A trait declaration.
    Trait(TraitDef),
    /// A trait impl block (`impl Trait for Type`).
    Impl(ImplDef),
    /// A type alias (`typealias Name<T> = Target<T>;`).
    TypeAlias(TypeAliasDef),
}

/// Function or method definition.
#[derive(Debug, Clone)]
pub struct FnDef {
    pub params: Vec<ParamDef>,
    pub return_ty: Option<TyRef>,
    /// `false` for abstract/trait methods without a default body.
    pub has_body: bool,
}

/// A function parameter definition.
#[derive(Debug, Clone)]
pub struct ParamDef {
    pub name: SmolStr,
    pub ty: TyRef,
    pub mutable: bool,
    /// `true` when this is the implicit `self` receiver parameter.
    pub is_self: bool,
}

/// Class definition including constructor parameters, inheritance, and methods.
#[derive(Debug, Clone)]
pub struct ClassDef {
    pub kind: ClassDefKind,
    pub ctor_params: Vec<CtorParamDef>,
    pub superclass: Option<TyRef>,
    pub trait_impls: Vec<TyRef>,
    pub methods: Vec<DefId>,
}

/// The modifier kind of a class declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassDefKind {
    /// Cannot be extended.
    Final,
    /// Explicitly open for extension.
    Open,
    /// Must be extended; cannot be instantiated directly.
    Abstract,
    /// Can only be extended within the same compilation unit.
    Sealed,
}

/// A constructor parameter, which also becomes a field of the class.
#[derive(Debug, Clone)]
pub struct CtorParamDef {
    pub vis: Vis,
    pub name: SmolStr,
    pub ty: TyRef,
    pub mutable: bool,
}

/// Data class definition with auto-derived equality, hashing, and accessors.
#[derive(Debug, Clone)]
pub struct DataClassDef {
    pub ctor_params: Vec<CtorParamDef>,
}

/// Enum definition containing its variant list.
#[derive(Debug, Clone)]
pub struct EnumDef {
    pub variants: Vec<EnumVariantDef>,
}

/// A single variant of an enum, optionally carrying named fields.
#[derive(Debug, Clone)]
pub struct EnumVariantDef {
    pub name: SmolStr,
    /// Named fields; empty for unit variants.
    pub fields: Vec<(SmolStr, TyRef)>,
}

/// Trait definition listing its method signatures.
#[derive(Debug, Clone)]
pub struct TraitDef {
    pub methods: Vec<DefId>,
}

/// A type alias mapping a name to another type.
#[derive(Debug, Clone)]
pub struct TypeAliasDef {
    /// Generic parameter names (e.g. `T`, `K`, `V`).
    pub generics: Vec<SmolStr>,
    /// The target type this alias expands to.
    pub target: TyRef,
}

/// An `impl Trait for Type` block.
#[derive(Debug, Clone)]
pub struct ImplDef {
    /// The trait being implemented.
    pub trait_ref: TyRef,
    /// The type the trait is implemented for.
    pub target: TyRef,
    /// Methods provided by this impl.
    pub methods: Vec<DefId>,
    /// Generic type parameter names declared on the impl.
    pub generics: Vec<SmolStr>,
}

/// Syntactic type reference as written in source code (not yet resolved to a semantic `Ty`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TyRef {
    /// A built-in primitive type.
    Prim(PrimTy),
    /// A named user-defined type.
    Named(SmolStr),
    /// A generic type with type arguments, e.g. `List<Int>`.
    Generic(SmolStr, Vec<TyRef>),
    /// A nullable type (`T?`).
    Nullable(Box<TyRef>),
    /// A function type (`fn(A, B) -> C`).
    Fn(Vec<TyRef>, Box<TyRef>),
    /// The `Self` type inside trait or impl contexts.
    SelfTy,
    /// A name that could not be resolved during lowering.
    Unresolved(SmolStr),
    /// Placeholder for error recovery.
    Error,
}

/// Built-in primitive types mapped to JVM equivalents.
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
    /// The unit type (void equivalent).
    Unit,
    /// The bottom type (never returns).
    Nothing,
}

/// Semantic type used during type checking (distinct from the syntactic [`TyRef`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ty {
    /// A built-in primitive type.
    Prim(PrimTy),
    /// A resolved named type.
    Named(SmolStr),
    /// A generic type with resolved type arguments.
    Generic(SmolStr, Vec<Ty>),
    /// A nullable type (`T?`).
    Nullable(Box<Ty>),
    /// A function type with parameter types and return type.
    Fn(Vec<Ty>, Box<Ty>),
    /// Placeholder for error recovery.
    Error,
}

impl Ty {
    /// Return the `Unit` type.
    pub fn unit() -> Self {
        Ty::Prim(PrimTy::Unit)
    }
    /// Return the `Nothing` (bottom) type.
    pub fn nothing() -> Self {
        Ty::Prim(PrimTy::Nothing)
    }
    /// `true` if this is the error-recovery sentinel.
    pub fn is_error(&self) -> bool {
        matches!(self, Ty::Error)
    }
    /// `true` for any numeric primitive (Int, Long, Float, Double, Byte, Short).
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
    /// `true` for integer primitives only (Int, Long, Byte, Short).
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            Ty::Prim(PrimTy::Int | PrimTy::Long | PrimTy::Byte | PrimTy::Short)
        )
    }
    /// `true` if this is the `Bool` type.
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

/// Convert a syntactic [`TyRef`] into a semantic [`Ty`], mapping unresolvable refs to `Ty::Error`.
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

/// Map a primitive type name (e.g. `"Int"`) to its [`PrimTy`] variant.
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

/// A type-annotated block body produced by the type checker.
#[derive(Debug, Clone)]
pub struct TypedBody {
    pub stmts: Vec<TypedStmt>,
    /// The trailing expression whose value is the block's result, if any.
    pub tail: Option<Box<TypedExpr>>,
    /// The inferred type of this body.
    pub ty: Ty,
}

/// A type-annotated statement.
#[derive(Debug, Clone)]
pub enum TypedStmt {
    /// A `let` binding with its inferred or annotated type.
    Let {
        name: SmolStr,
        ty: Ty,
        init: TypedExpr,
        mutable: bool,
        span: Span,
    },
    /// An expression whose value is used (tail position).
    Expr(TypedExpr),
    /// An expression followed by a semicolon (value discarded).
    ExprSemi(TypedExpr),
}

/// A type-annotated expression node.
#[derive(Debug, Clone)]
pub struct TypedExpr {
    pub kind: TypedExprKind,
    /// The inferred or checked type of this expression.
    pub ty: Ty,
    pub span: Span,
}

/// The kind of a typed expression.
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
    /// A `safe {}` block that catches JVM exceptions at runtime.
    /// The body is lowered into a try-catch in codegen.
    Safe(TypedBody),
    Error,
}

/// A single arm of a typed `match` expression.
#[derive(Debug, Clone)]
pub struct TypedMatchArm {
    pub pattern: valen_ast::Pattern,
    pub guard: Option<TypedExpr>,
    pub body: TypedExpr,
}

/// A segment of a string interpolation expression.
#[derive(Debug, Clone)]
pub enum TypedStringPart {
    /// A literal text segment.
    Text(SmolStr),
    /// An interpolated expression segment.
    Expr(TypedExpr),
}

// ---------------------------------------------------------------------------
// Foreign (Java) type metadata loaded from classpath .class files
// ---------------------------------------------------------------------------

/// Metadata extracted from a Java .class file for type checking interop.
#[derive(Debug, Clone, Default)]
pub struct ForeignClassInfo {
    /// JVM internal name (e.g. `java/util/ArrayList`).
    pub internal_name: String,
    pub methods: Vec<ForeignMethodInfo>,
    pub constructors: Vec<ForeignCtorInfo>,
    pub fields: Vec<ForeignFieldInfo>,
    pub super_class: Option<String>,
    pub interfaces: Vec<String>,
    /// JVM internal names of permitted subclasses (from `PermittedSubclasses` attribute).
    pub permitted_subclasses: Vec<String>,
    /// Whether `@valen.Closed` annotation is present (enables exhaustive match).
    pub has_valen_closed: bool,
}

/// A method on a foreign Java class.
#[derive(Debug, Clone)]
pub struct ForeignMethodInfo {
    pub name: SmolStr,
    pub params: Vec<TyRef>,
    pub return_ty: TyRef,
    pub is_static: bool,
}

/// A constructor on a foreign Java class.
#[derive(Debug, Clone)]
pub struct ForeignCtorInfo {
    pub params: Vec<TyRef>,
}

/// A field on a foreign Java class.
#[derive(Debug, Clone)]
pub struct ForeignFieldInfo {
    pub name: SmolStr,
    pub ty: TyRef,
}
