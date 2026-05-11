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
            Vis::Pub | Vis::Internal => true,
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
