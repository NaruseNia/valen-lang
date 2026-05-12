#![cfg(test)]
#![allow(dead_code)]

use smol_str::SmolStr;
use valen_ast::{FileId, Span};
use valen_hir::*;

fn span() -> Span {
    Span {
        start: 0,
        end: 0,
        file_id: FileId(0),
    }
}

pub fn empty_class(name: &str, kind: ClassDefKind) -> Hir {
    class_with_params(name, kind, vec![], Vis::Pub)
}

pub fn class_with_params(
    name: &str,
    kind: ClassDefKind,
    params: Vec<CtorParamDef>,
    vis: Vis,
) -> Hir {
    let mut hir = Hir::default();
    let id = hir.alloc_id();
    hir.defs.insert(
        id,
        Def {
            id,
            name: SmolStr::from(name),
            kind: DefKind::Class(ClassDef {
                kind,
                ctor_params: params,
                superclass: None,
                trait_impls: vec![],
                methods: vec![],
            }),
            vis,
            span: span(),
        },
    );
    hir
}

pub fn data_class(name: &str, params: Vec<CtorParamDef>) -> Hir {
    let mut hir = Hir::default();
    let id = hir.alloc_id();
    hir.defs.insert(
        id,
        Def {
            id,
            name: SmolStr::from(name),
            kind: DefKind::DataClass(DataClassDef {
                ctor_params: params,
            }),
            vis: Vis::Pub,
            span: span(),
        },
    );
    hir
}

pub fn class_with_method(
    class_name: &str,
    method_name: &str,
    params: Vec<ParamDef>,
    return_ty: Option<TyRef>,
    has_body: bool,
) -> Hir {
    let mut hir = Hir::default();
    let class_id = hir.alloc_id();
    let method_id = hir.alloc_id();

    hir.defs.insert(
        method_id,
        Def {
            id: method_id,
            name: SmolStr::from(method_name),
            kind: DefKind::Fn(FnDef {
                params,
                return_ty,
                has_body,
            }),
            vis: Vis::Pub,
            span: span(),
        },
    );

    hir.defs.insert(
        class_id,
        Def {
            id: class_id,
            name: SmolStr::from(class_name),
            kind: DefKind::Class(ClassDef {
                kind: ClassDefKind::Final,
                ctor_params: vec![],
                superclass: None,
                trait_impls: vec![],
                methods: vec![method_id],
            }),
            vis: Vis::Pub,
            span: span(),
        },
    );
    hir
}

pub fn ctor_param(name: &str, ty: TyRef, vis: Vis, mutable: bool) -> CtorParamDef {
    CtorParamDef {
        vis,
        name: SmolStr::from(name),
        ty,
        mutable,
    }
}

pub fn self_param() -> ParamDef {
    ParamDef {
        name: "self".into(),
        ty: TyRef::SelfTy,
        mutable: false,
        is_self: true,
    }
}

pub fn param(name: &str, ty: TyRef) -> ParamDef {
    ParamDef {
        name: SmolStr::from(name),
        ty,
        mutable: false,
        is_self: false,
    }
}

pub fn enum_def(name: &str, variants: Vec<EnumVariantDef>) -> Hir {
    let mut hir = Hir::default();
    let id = hir.alloc_id();
    hir.defs.insert(
        id,
        Def {
            id,
            name: SmolStr::from(name),
            kind: DefKind::Enum(EnumDef { variants }),
            vis: Vis::Pub,
            span: span(),
        },
    );
    hir
}

pub fn variant(name: &str, fields: Vec<(SmolStr, TyRef)>) -> EnumVariantDef {
    EnumVariantDef {
        name: SmolStr::from(name),
        fields,
    }
}

pub fn unit_variant(name: &str) -> EnumVariantDef {
    variant(name, vec![])
}

pub fn compile_and_verify(hir: &Hir) -> Vec<ristretto_classfile::ClassFile<'static>> {
    let outputs = crate::compile_hir(hir).expect("compile_hir failed");
    outputs
        .iter()
        .map(|o| {
            assert_eq!(&o.bytes[0..4], &[0xCA, 0xFE, 0xBA, 0xBE]);
            ristretto_classfile::ClassFile::from_bytes(&o.bytes)
                .expect("ClassFile::from_bytes failed")
        })
        .collect()
}
