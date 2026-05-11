pub mod resolve;
pub mod ty;

use indexmap::IndexMap;
use smol_str::SmolStr;
use valen_ast::Span;

pub type DefId = u32;

#[derive(Debug, Default)]
pub struct Hir {
    pub defs: IndexMap<DefId, Def>,
    pub package: Option<Vec<SmolStr>>,
    next_id: DefId,
}

impl Hir {
    pub fn alloc_id(&mut self) -> DefId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
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
