//! Name resolution -- registers top-level definitions and builds method indexes.

use indexmap::IndexMap;
use smol_str::SmolStr;
use valen_ast::{self, Item, Visibility};
use valen_diagnostics::Diagnostics;

use valen_diagnostics::DiagCode;

use crate::{
    ClassDef, ClassDefKind, CtorParamDef, DataClassDef, Def, DefId, DefKind, EnumDef,
    EnumVariantDef, FnDef, Hir, ImplDef, ImplEntry, ParamDef, TraitDef, TyRef, Vis,
};

/// Name resolver that lowers AST items into HIR definitions and builds scope tables.
pub struct Resolver {
    hir: Hir,
    scope: Scope,
    diagnostics: Diagnostics,
    current_package: Option<Vec<SmolStr>>,
}

#[derive(Debug, Default)]
struct Scope {
    names: IndexMap<SmolStr, DefId>,
    imports: IndexMap<SmolStr, Vec<SmolStr>>,
}

impl Scope {
    fn define(&mut self, name: SmolStr, id: DefId) -> Option<DefId> {
        self.names.insert(name, id)
    }

    fn lookup(&self, name: &str) -> Option<DefId> {
        self.names.get(name).copied()
    }
}

/// Output of the name resolution pass.
pub struct ResolveResult {
    /// The populated HIR with all registered definitions.
    pub hir: Hir,
    pub diagnostics: Diagnostics,
}

/// Run name resolution on a list of AST items, producing an HIR with definitions and method indexes.
pub fn resolve(items: &[Item]) -> ResolveResult {
    let mut resolver = Resolver {
        hir: Hir::default(),
        scope: Scope::default(),
        diagnostics: Diagnostics::new(),
        current_package: None,
    };
    resolver.resolve_items(items);
    ResolveResult {
        hir: resolver.hir,
        diagnostics: resolver.diagnostics,
    }
}

impl Resolver {
    fn define_name(&mut self, name: SmolStr, id: DefId, span: valen_ast::Span) {
        if let Some(prev_id) = self.scope.define(name.clone(), id) {
            let prev_span = self.hir.defs.get(&prev_id).map(|d| d.span).unwrap_or(span);
            self.diagnostics.error(
                DiagCode::NAME_NOT_FOUND,
                span,
                SmolStr::from(format!(
                    "duplicate definition `{}` (previously defined at {:?})",
                    name, prev_span
                )),
            );
        }
    }

    fn resolve_items(&mut self, items: &[Item]) {
        // Collect imports (from all files)
        for item in items {
            if let Item::Import(imp) = item {
                let short = imp
                    .alias
                    .clone()
                    .or_else(|| imp.path.last().cloned())
                    .unwrap_or_default();
                self.scope.imports.insert(short, imp.path.clone());
            }
        }

        self.hir.imports = self.scope.imports.clone();

        // First pass: register all top-level names, tracking current package
        for item in items {
            if let Item::Package(pkg) = item {
                self.current_package = Some(pkg.path.clone());
                self.hir.package = Some(pkg.path.clone());
            }
            self.register_item(item);
        }

        // Inject prelude types for names not already defined by user/stdlib code
        self.inject_prelude();

        // Second pass: build method indexes and validate
        self.build_method_index();
    }

    fn inject_prelude(&mut self) {
        let prelude_pkg = Some(vec![SmolStr::from("valen"), SmolStr::from("core")]);

        self.inject_prelude_option(&prelude_pkg);
        self.inject_prelude_result(&prelude_pkg);
        self.inject_prelude_error_trait(&prelude_pkg);
        self.inject_prelude_iterator_trait(&prelude_pkg);
        self.inject_prelude_range(&prelude_pkg);
        self.inject_prelude_java_exception(&prelude_pkg);
    }

    fn inject_prelude_option(&mut self, pkg: &Option<Vec<SmolStr>>) {
        if self.scope.lookup("Option").is_some() {
            return;
        }
        let id = self.hir.alloc_id();
        let def = Def {
            id,
            name: SmolStr::from("Option"),
            kind: DefKind::Enum(EnumDef {
                variants: vec![
                    EnumVariantDef {
                        name: SmolStr::from("Some"),
                        fields: vec![(
                            SmolStr::from("value"),
                            TyRef::Unresolved(SmolStr::from("T")),
                        )],
                    },
                    EnumVariantDef {
                        name: SmolStr::from("None"),
                        fields: vec![],
                    },
                ],
            }),
            vis: Vis::Pub,
            span: valen_ast::Span::DUMMY,
            package: pkg.clone(),
        };
        self.hir.defs.insert(id, def);
        self.hir.prelude_ids.push(id);
        self.scope.names.insert(SmolStr::from("Option"), id);
        self.hir.imports.insert(
            SmolStr::from("Option"),
            vec![
                SmolStr::from("valen"),
                SmolStr::from("core"),
                SmolStr::from("Option"),
            ],
        );
    }

    fn inject_prelude_result(&mut self, pkg: &Option<Vec<SmolStr>>) {
        if self.scope.lookup("Result").is_some() {
            return;
        }
        let id = self.hir.alloc_id();
        let def = Def {
            id,
            name: SmolStr::from("Result"),
            kind: DefKind::Enum(EnumDef {
                variants: vec![
                    EnumVariantDef {
                        name: SmolStr::from("Ok"),
                        fields: vec![(
                            SmolStr::from("value"),
                            TyRef::Unresolved(SmolStr::from("T")),
                        )],
                    },
                    EnumVariantDef {
                        name: SmolStr::from("Err"),
                        fields: vec![(
                            SmolStr::from("error"),
                            TyRef::Unresolved(SmolStr::from("E")),
                        )],
                    },
                ],
            }),
            vis: Vis::Pub,
            span: valen_ast::Span::DUMMY,
            package: pkg.clone(),
        };
        self.hir.defs.insert(id, def);
        self.hir.prelude_ids.push(id);
        self.scope.names.insert(SmolStr::from("Result"), id);
        self.hir.imports.insert(
            SmolStr::from("Result"),
            vec![
                SmolStr::from("valen"),
                SmolStr::from("core"),
                SmolStr::from("Result"),
            ],
        );
    }

    fn inject_prelude_error_trait(&mut self, pkg: &Option<Vec<SmolStr>>) {
        if self.scope.lookup("Error").is_some() {
            return;
        }
        let mid = self.hir.alloc_id();
        let mdef = Def {
            id: mid,
            name: SmolStr::from("message"),
            kind: DefKind::Fn(FnDef {
                params: vec![ParamDef {
                    name: SmolStr::from("self"),
                    ty: TyRef::SelfTy,
                    mutable: false,
                    is_self: true,
                }],
                return_ty: Some(TyRef::Prim(crate::PrimTy::String)),
                has_body: false,
            }),
            vis: Vis::Pub,
            span: valen_ast::Span::DUMMY,
            package: pkg.clone(),
        };
        self.hir.defs.insert(mid, mdef);
        self.hir.prelude_ids.push(mid);

        let id = self.hir.alloc_id();
        let def = Def {
            id,
            name: SmolStr::from("Error"),
            kind: DefKind::Trait(TraitDef { methods: vec![mid] }),
            vis: Vis::Pub,
            span: valen_ast::Span::DUMMY,
            package: pkg.clone(),
        };
        self.hir.defs.insert(id, def);
        self.hir.prelude_ids.push(id);
        self.scope.names.insert(SmolStr::from("Error"), id);
        self.hir.imports.insert(
            SmolStr::from("Error"),
            vec![
                SmolStr::from("valen"),
                SmolStr::from("core"),
                SmolStr::from("Error"),
            ],
        );
    }

    fn inject_prelude_iterator_trait(&mut self, pkg: &Option<Vec<SmolStr>>) {
        if self.scope.lookup("Iterator").is_some() {
            return;
        }
        let mid = self.hir.alloc_id();
        let mdef = Def {
            id: mid,
            name: SmolStr::from("next"),
            kind: DefKind::Fn(FnDef {
                params: vec![ParamDef {
                    name: SmolStr::from("self"),
                    ty: TyRef::SelfTy,
                    mutable: true,
                    is_self: true,
                }],
                return_ty: Some(TyRef::Generic(
                    SmolStr::from("Option"),
                    vec![TyRef::Unresolved(SmolStr::from("T"))],
                )),
                has_body: false,
            }),
            vis: Vis::Pub,
            span: valen_ast::Span::DUMMY,
            package: pkg.clone(),
        };
        self.hir.defs.insert(mid, mdef);
        self.hir.prelude_ids.push(mid);

        let id = self.hir.alloc_id();
        let def = Def {
            id,
            name: SmolStr::from("Iterator"),
            kind: DefKind::Trait(TraitDef { methods: vec![mid] }),
            vis: Vis::Pub,
            span: valen_ast::Span::DUMMY,
            package: pkg.clone(),
        };
        self.hir.defs.insert(id, def);
        self.hir.prelude_ids.push(id);
        self.scope.names.insert(SmolStr::from("Iterator"), id);
        self.hir.imports.insert(
            SmolStr::from("Iterator"),
            vec![
                SmolStr::from("valen"),
                SmolStr::from("core"),
                SmolStr::from("Iterator"),
            ],
        );
    }

    fn inject_prelude_range(&mut self, pkg: &Option<Vec<SmolStr>>) {
        if self.scope.lookup("Range").is_some() {
            return;
        }
        let id = self.hir.alloc_id();
        let def = Def {
            id,
            name: SmolStr::from("Range"),
            kind: DefKind::DataClass(DataClassDef {
                ctor_params: vec![
                    CtorParamDef {
                        vis: Vis::Pub,
                        name: SmolStr::from("start"),
                        ty: TyRef::Unresolved(SmolStr::from("T")),
                        mutable: false,
                    },
                    CtorParamDef {
                        vis: Vis::Pub,
                        name: SmolStr::from("end"),
                        ty: TyRef::Unresolved(SmolStr::from("T")),
                        mutable: false,
                    },
                    CtorParamDef {
                        vis: Vis::Pub,
                        name: SmolStr::from("inclusive"),
                        ty: TyRef::Prim(crate::PrimTy::Bool),
                        mutable: false,
                    },
                ],
            }),
            vis: Vis::Pub,
            span: valen_ast::Span::DUMMY,
            package: pkg.clone(),
        };
        self.hir.defs.insert(id, def);
        self.hir.prelude_ids.push(id);
        self.scope.names.insert(SmolStr::from("Range"), id);
        self.hir.imports.insert(
            SmolStr::from("Range"),
            vec![
                SmolStr::from("valen"),
                SmolStr::from("core"),
                SmolStr::from("Range"),
            ],
        );
    }

    fn inject_prelude_java_exception(&mut self, pkg: &Option<Vec<SmolStr>>) {
        if self.scope.lookup("JavaException").is_some() {
            return;
        }
        let id = self.hir.alloc_id();
        let def = Def {
            id,
            name: SmolStr::from("JavaException"),
            kind: DefKind::DataClass(DataClassDef {
                ctor_params: vec![
                    CtorParamDef {
                        vis: Vis::Pub,
                        name: SmolStr::from("message"),
                        ty: TyRef::Prim(crate::PrimTy::String),
                        mutable: false,
                    },
                    CtorParamDef {
                        vis: Vis::Pub,
                        name: SmolStr::from("class_name"),
                        ty: TyRef::Prim(crate::PrimTy::String),
                        mutable: false,
                    },
                ],
            }),
            vis: Vis::Pub,
            span: valen_ast::Span::DUMMY,
            package: pkg.clone(),
        };
        self.hir.defs.insert(id, def);
        self.hir.prelude_ids.push(id);
        self.scope.names.insert(SmolStr::from("JavaException"), id);
        self.hir.imports.insert(
            SmolStr::from("JavaException"),
            vec![
                SmolStr::from("valen"),
                SmolStr::from("core"),
                SmolStr::from("JavaException"),
            ],
        );
    }

    fn build_method_index(&mut self) {
        let defs: Vec<_> = self.hir.defs.values().cloned().collect();
        for def in &defs {
            match &def.kind {
                DefKind::Class(c) => {
                    self.hir
                        .type_methods
                        .insert(def.name.clone(), c.methods.clone());
                }
                DefKind::Impl(imp) => {
                    let target_name = match &imp.target {
                        TyRef::Named(n) => n.clone(),
                        TyRef::Prim(p) => SmolStr::from(format!("{p:?}")),
                        _ => continue,
                    };
                    let trait_name = match &imp.trait_ref {
                        TyRef::Named(n) => n.clone(),
                        _ => continue,
                    };
                    self.hir.trait_impls.push(ImplEntry {
                        trait_name,
                        target_name,
                        methods: imp.methods.clone(),
                    });
                }
                _ => {}
            }
        }
    }

    fn register_item(&mut self, item: &Item) {
        match item {
            Item::Package(_) | Item::Import(_) => {}
            Item::Fn(f) => {
                let id = self.hir.alloc_id();
                let def = Def {
                    id,
                    name: f.name.clone(),
                    kind: DefKind::Fn(self.lower_fn_with_params(f, &[])),
                    vis: lower_vis(f.visibility),
                    span: f.span,
                    package: self.current_package.clone(),
                };
                self.hir.defs.insert(id, def);
                self.define_name(f.name.clone(), id, f.span);
            }
            Item::Class(c) => {
                let id = self.hir.alloc_id();
                let type_params: Vec<SmolStr> = c.generics.iter().map(|g| g.name.clone()).collect();
                let mut method_ids = Vec::new();
                for member in &c.body {
                    if let valen_ast::ClassMember::Method(m) = member {
                        let mid = self.hir.alloc_id();
                        let mdef = Def {
                            id: mid,
                            name: m.name.clone(),
                            kind: DefKind::Fn(self.lower_fn_with_params(m, &type_params)),
                            vis: lower_vis(m.visibility),
                            span: m.span,
                            package: self.current_package.clone(),
                        };
                        self.hir.defs.insert(mid, mdef);
                        method_ids.push(mid);
                    }
                }
                let def = Def {
                    id,
                    name: c.name.clone(),
                    kind: DefKind::Class(ClassDef {
                        kind: lower_class_kind(c.kind),
                        ctor_params: c
                            .ctor_params
                            .iter()
                            .map(|p| lower_ctor_param_with_params(p, &type_params))
                            .collect(),
                        superclass: c
                            .supertypes
                            .first()
                            .map(|t| lower_type_ref_with_params(t, &type_params)),
                        trait_impls: c
                            .supertypes
                            .iter()
                            .skip(1)
                            .map(|t| lower_type_ref_with_params(t, &type_params))
                            .collect(),
                        methods: method_ids,
                    }),
                    vis: lower_vis(c.visibility),
                    span: c.span,
                    package: self.current_package.clone(),
                };
                self.hir.defs.insert(id, def);
                self.define_name(c.name.clone(), id, c.span);
            }
            Item::DataClass(dc) => {
                let id = self.hir.alloc_id();
                let dc_params: Vec<SmolStr> = dc.generics.iter().map(|g| g.name.clone()).collect();
                let def = Def {
                    id,
                    name: dc.name.clone(),
                    kind: DefKind::DataClass(DataClassDef {
                        ctor_params: dc
                            .ctor_params
                            .iter()
                            .map(|p| lower_ctor_param_with_params(p, &dc_params))
                            .collect(),
                    }),
                    vis: lower_vis(dc.visibility),
                    span: dc.span,
                    package: self.current_package.clone(),
                };
                self.hir.defs.insert(id, def);
                self.define_name(dc.name.clone(), id, dc.span);
            }
            Item::Enum(e) => {
                let id = self.hir.alloc_id();
                let type_params: Vec<SmolStr> = e.generics.iter().map(|g| g.name.clone()).collect();
                let variants = e
                    .variants
                    .iter()
                    .map(|v| {
                        let fields = match &v.fields {
                            valen_ast::EnumVariantFields::Unit => Vec::new(),
                            valen_ast::EnumVariantFields::Named(fs) => fs
                                .iter()
                                .map(|f| {
                                    (
                                        f.name.clone(),
                                        lower_type_ref_with_params(&f.ty, &type_params),
                                    )
                                })
                                .collect(),
                        };
                        EnumVariantDef {
                            name: v.name.clone(),
                            fields,
                        }
                    })
                    .collect();
                let def = Def {
                    id,
                    name: e.name.clone(),
                    kind: DefKind::Enum(EnumDef { variants }),
                    vis: lower_vis(e.visibility),
                    span: e.span,
                    package: self.current_package.clone(),
                };
                self.hir.defs.insert(id, def);
                self.define_name(e.name.clone(), id, e.span);
            }
            Item::Trait(t) => {
                let id = self.hir.alloc_id();
                let trait_params: Vec<SmolStr> =
                    t.generics.iter().map(|g| g.name.clone()).collect();
                let mut method_ids = Vec::new();
                for ti in &t.items {
                    if let valen_ast::TraitItem::Fn(m) = ti {
                        let mid = self.hir.alloc_id();
                        let mdef = Def {
                            id: mid,
                            name: m.name.clone(),
                            kind: DefKind::Fn(self.lower_fn_with_params(m, &trait_params)),
                            vis: Vis::Pub,
                            span: m.span,
                            package: self.current_package.clone(),
                        };
                        self.hir.defs.insert(mid, mdef);
                        method_ids.push(mid);
                    }
                }
                let def = Def {
                    id,
                    name: t.name.clone(),
                    kind: DefKind::Trait(TraitDef {
                        methods: method_ids,
                    }),
                    vis: lower_vis(t.visibility),
                    span: t.span,
                    package: self.current_package.clone(),
                };
                self.hir.defs.insert(id, def);
                self.define_name(t.name.clone(), id, t.span);
            }
            Item::Impl(imp) => {
                let id = self.hir.alloc_id();
                let impl_params: Vec<SmolStr> =
                    imp.generics.iter().map(|g| g.name.clone()).collect();
                let mut method_ids = Vec::new();
                for ii in &imp.items {
                    if let valen_ast::ImplItem::Fn(m) = ii {
                        let mid = self.hir.alloc_id();
                        let mdef = Def {
                            id: mid,
                            name: m.name.clone(),
                            kind: DefKind::Fn(self.lower_fn_with_params(m, &impl_params)),
                            vis: Vis::Pub,
                            span: m.span,
                            package: self.current_package.clone(),
                        };
                        self.hir.defs.insert(mid, mdef);
                        method_ids.push(mid);
                    }
                }
                let trait_ref = imp
                    .trait_ref
                    .as_ref()
                    .map(|t| lower_type_ref_with_params(t, &impl_params))
                    .unwrap_or(TyRef::Error);
                let generics = imp.generics.iter().map(|g| g.name.clone()).collect();
                let def = Def {
                    id,
                    name: SmolStr::from(""),
                    kind: DefKind::Impl(ImplDef {
                        trait_ref,
                        target: lower_type_ref_with_params(&imp.target, &impl_params),
                        methods: method_ids,
                        generics,
                    }),
                    vis: Vis::Internal,
                    span: imp.span,
                    package: self.current_package.clone(),
                };
                self.hir.defs.insert(id, def);
            }
            Item::TypeAlias(ta) => {
                let id = self.hir.alloc_id();
                let ta_params: Vec<SmolStr> = ta.generics.iter().map(|g| g.name.clone()).collect();
                let def = Def {
                    id,
                    name: ta.name.clone(),
                    kind: DefKind::TypeAlias(crate::TypeAliasDef {
                        generics: ta_params.clone(),
                        target: lower_type_ref_with_params(&ta.ty, &ta_params),
                    }),
                    vis: lower_vis(ta.visibility),
                    span: ta.span,
                    package: self.current_package.clone(),
                };
                self.hir.defs.insert(id, def);
                self.define_name(ta.name.clone(), id, ta.span);
            }
        }
    }

    fn lower_fn_with_params(&self, f: &valen_ast::FnDecl, outer_params: &[SmolStr]) -> FnDef {
        let mut all_params: Vec<SmolStr> = outer_params.to_vec();
        all_params.extend(f.generics.iter().map(|g| g.name.clone()));
        let params = f
            .params
            .iter()
            .map(|p| ParamDef {
                name: p.name.clone(),
                ty: lower_type_ref_with_params(&p.ty, &all_params),
                mutable: p.mutable,
                is_self: p.name == "self",
            })
            .collect();
        FnDef {
            params,
            return_ty: f
                .return_type
                .as_ref()
                .map(|t| lower_type_ref_with_params(t, &all_params)),
            has_body: f.body.is_some(),
        }
    }

    pub fn lookup(&self, name: &str) -> Option<DefId> {
        self.scope.lookup(name)
    }

    pub fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
    }
}

fn lower_vis(v: Visibility) -> Vis {
    match v {
        Visibility::Pub => Vis::Pub,
        Visibility::Internal => Vis::Internal,
        Visibility::Private => Vis::Private,
    }
}

fn lower_class_kind(k: valen_ast::ClassKind) -> ClassDefKind {
    match k {
        valen_ast::ClassKind::Final => ClassDefKind::Final,
        valen_ast::ClassKind::Open => ClassDefKind::Open,
        valen_ast::ClassKind::Abstract => ClassDefKind::Abstract,
        valen_ast::ClassKind::Sealed => ClassDefKind::Sealed,
    }
}

fn lower_ctor_param_with_params(p: &valen_ast::CtorParam, type_params: &[SmolStr]) -> CtorParamDef {
    CtorParamDef {
        vis: lower_vis(p.visibility),
        name: p.name.clone(),
        ty: lower_type_ref_with_params(&p.ty, type_params),
        mutable: p.mutable,
    }
}

fn lower_type_ref_with_params(ty: &valen_ast::Type, type_params: &[SmolStr]) -> TyRef {
    match ty {
        valen_ast::Type::Path(tp) => {
            if tp.segments.len() == 1 {
                let seg = &tp.segments[0];
                let name = &seg.name;
                if let Some(prim) = crate::resolve_prim(name) {
                    return TyRef::Prim(prim);
                }
                if seg.generics.is_empty() {
                    if name == "Self" {
                        return TyRef::SelfTy;
                    }
                    if type_params.iter().any(|p| p == name) {
                        return TyRef::Unresolved(name.clone());
                    }
                    return TyRef::Named(name.clone());
                }
                let args = seg
                    .generics
                    .iter()
                    .map(|g| lower_type_ref_with_params(g, type_params))
                    .collect();
                return TyRef::Generic(name.clone(), args);
            }
            let full: String = tp
                .segments
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
                .join(".");
            TyRef::Named(SmolStr::from(full))
        }
        valen_ast::Type::Nullable { inner, .. } => {
            TyRef::Nullable(Box::new(lower_type_ref_with_params(inner, type_params)))
        }
        valen_ast::Type::Fn(ft) => {
            let params = ft
                .params
                .iter()
                .map(|p| lower_type_ref_with_params(p, type_params))
                .collect();
            let ret = Box::new(lower_type_ref_with_params(&ft.return_type, type_params));
            TyRef::Fn(params, ret)
        }
        valen_ast::Type::Tuple(_) => TyRef::Error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PrimTy;
    use valen_ast::FileId;
    use valen_parser::parse;

    fn resolve_source(src: &str) -> ResolveResult {
        let parsed = parse(src, FileId(0));
        assert!(
            !parsed.diagnostics.has_errors(),
            "parse errors: {:?}",
            parsed.diagnostics
        );
        resolve(&parsed.items)
    }

    #[test]
    fn resolve_simple_fn() {
        let r = resolve_source("fn main() { 42 }");
        assert!(!r.diagnostics.has_errors());
        let def = r.hir.defs.values().find(|d| d.name == "main").unwrap();
        assert!(matches!(def.kind, DefKind::Fn(_)));
    }

    #[test]
    fn resolve_fn_with_params() {
        let r = resolve_source("fn add(a: Int, b: Int) -> Int { a }");
        let (_, def) = r.hir.defs.iter().next().unwrap();
        if let DefKind::Fn(f) = &def.kind {
            assert_eq!(f.params.len(), 2);
            assert_eq!(f.params[0].ty, TyRef::Prim(PrimTy::Int));
            assert_eq!(f.return_ty, Some(TyRef::Prim(PrimTy::Int)));
        } else {
            panic!("expected FnDef");
        }
    }

    #[test]
    fn resolve_class_with_methods() {
        let r = resolve_source(
            "class Dog(pub name: String) { fn greet(self) -> String { self.name } }",
        );
        assert!(!r.diagnostics.has_errors());
        let class_def = r.hir.defs.values().find(|d| d.name == "Dog").unwrap();
        if let DefKind::Class(c) = &class_def.kind {
            assert_eq!(c.ctor_params.len(), 1);
            assert_eq!(c.ctor_params[0].name, "name");
            assert_eq!(c.ctor_params[0].ty, TyRef::Prim(PrimTy::String));
            assert_eq!(c.methods.len(), 1);
        } else {
            panic!("expected ClassDef");
        }
    }

    #[test]
    fn resolve_enum() {
        let r = resolve_source("enum Shape { Circle(r: Float), Point }");
        let def = r.hir.defs.values().find(|d| d.name == "Shape").unwrap();
        if let DefKind::Enum(e) = &def.kind {
            assert_eq!(e.variants.len(), 2);
            assert_eq!(e.variants[0].name, "Circle");
            assert_eq!(e.variants[0].fields.len(), 1);
            assert_eq!(e.variants[1].name, "Point");
            assert!(e.variants[1].fields.is_empty());
        } else {
            panic!("expected EnumDef");
        }
    }

    #[test]
    fn resolve_trait_and_impl() {
        let r = resolve_source(
            "trait Area { fn area(self) -> Float; }\nimpl Area for Circle { fn area(self) -> Float { 0.0 } }",
        );
        assert!(!r.diagnostics.has_errors());
        let trait_def = r.hir.defs.values().find(|d| d.name == "Area").unwrap();
        assert!(matches!(trait_def.kind, DefKind::Trait(_)));
        let impl_def = r
            .hir
            .defs
            .values()
            .find(|d| matches!(d.kind, DefKind::Impl(_)))
            .unwrap();
        if let DefKind::Impl(i) = &impl_def.kind {
            assert_eq!(i.target, TyRef::Named(SmolStr::from("Circle")));
            assert_eq!(i.methods.len(), 1);
        } else {
            panic!("expected ImplDef");
        }
    }

    #[test]
    fn resolve_package_and_import() {
        let r = resolve_source("package com.example;\nimport java.util.List;\nfn main() { 42 }");
        assert!(!r.diagnostics.has_errors());
        assert_eq!(
            r.hir.package,
            Some(vec![SmolStr::from("com"), SmolStr::from("example")])
        );
    }

    #[test]
    fn resolve_data_class() {
        let r = resolve_source("data class Point(x: Float, y: Float);");
        let def = r.hir.defs.values().find(|d| d.name == "Point").unwrap();
        if let DefKind::DataClass(dc) = &def.kind {
            assert_eq!(dc.ctor_params.len(), 2);
        } else {
            panic!("expected DataClassDef");
        }
    }

    #[test]
    fn resolve_nullable_type() {
        let r = resolve_source("fn find(id: Int) -> String? { id }");
        let (_, def) = r.hir.defs.iter().next().unwrap();
        if let DefKind::Fn(f) = &def.kind {
            assert_eq!(
                f.return_ty,
                Some(TyRef::Nullable(Box::new(TyRef::Prim(PrimTy::String))))
            );
        } else {
            panic!("expected FnDef");
        }
    }

    #[test]
    fn resolve_generic_type() {
        let r = resolve_source("fn first(xs: List<Int>) -> Option<Int> { xs }");
        let (_, def) = r.hir.defs.iter().next().unwrap();
        if let DefKind::Fn(f) = &def.kind {
            assert_eq!(
                f.params[0].ty,
                TyRef::Generic(SmolStr::from("List"), vec![TyRef::Prim(PrimTy::Int)])
            );
            assert_eq!(
                f.return_ty,
                Some(TyRef::Generic(
                    SmolStr::from("Option"),
                    vec![TyRef::Prim(PrimTy::Int)]
                ))
            );
        } else {
            panic!("expected FnDef");
        }
    }

    #[test]
    fn scope_lookup() {
        let r = resolve_source("fn foo() { 1 }\nfn bar() { 2 }");
        let user_defs: Vec<_> = r
            .hir
            .defs
            .values()
            .filter(|d| !r.hir.prelude_ids.contains(&d.id))
            .collect();
        assert_eq!(user_defs.len(), 2);
    }

    // --- TASK-004 後半: method resolution + visibility ---

    #[test]
    fn method_resolution_class_body() {
        let r = resolve_source(
            "class Dog(pub name: String) { fn greet(self) -> String { self.name } }",
        );
        let res = r.hir.resolve_method("Dog", "greet");
        assert!(matches!(res, crate::MethodResolution::Found(_)));
    }

    #[test]
    fn method_resolution_not_found() {
        let r = resolve_source("class Dog(pub name: String) {}");
        let res = r.hir.resolve_method("Dog", "bark");
        assert!(matches!(res, crate::MethodResolution::NotFound));
    }

    #[test]
    fn method_resolution_trait_impl() {
        let r = resolve_source(
            "class Circle {}\ntrait Area { fn area(self) -> Float; }\nimpl Area for Circle { fn area(self) -> Float { 0.0 } }",
        );
        let res = r.hir.resolve_method("Circle", "area");
        assert!(matches!(res, crate::MethodResolution::Found(_)));
    }

    #[test]
    fn method_resolution_class_body_priority() {
        let r = resolve_source(
            "class Foo { fn show(self) -> String { self } }\ntrait Display { fn show(self) -> String; }\nimpl Display for Foo { fn show(self) -> String { self } }",
        );
        let res = r.hir.resolve_method("Foo", "show");
        if let crate::MethodResolution::Found(id) = res {
            let def = r.hir.defs.get(&id).unwrap();
            assert_eq!(def.vis, Vis::Internal);
        } else {
            panic!("expected class body method to win");
        }
    }

    #[test]
    fn visibility_pub_accessible() {
        let r = resolve_source("pub fn main() { 42 }");
        let (_, def) = r.hir.defs.iter().next().unwrap();
        assert!(r.hir.check_visibility(def.id, None));
    }

    #[test]
    fn visibility_private_blocked() {
        let r = resolve_source("class Foo { private fn secret(self) -> Int { 42 } }");
        let secret = r.hir.defs.values().find(|d| d.name == "secret").unwrap();
        assert!(!r.hir.check_visibility(secret.id, None));
        assert!(r.hir.check_visibility(secret.id, Some("Foo")));
    }

    #[test]
    fn type_method_index_built() {
        let r = resolve_source(
            "class Dog { fn bark(self) -> String { self } fn fetch(self) -> String { self } }",
        );
        let methods = r.hir.type_methods.get("Dog").unwrap();
        assert_eq!(methods.len(), 2);
    }

    #[test]
    fn impl_index_built() {
        let r = resolve_source(
            "trait Show { fn show(self) -> String; }\nimpl Show for Dog { fn show(self) -> String { self } }",
        );
        assert_eq!(r.hir.trait_impls.len(), 1);
        assert_eq!(r.hir.trait_impls[0].trait_name, "Show");
        assert_eq!(r.hir.trait_impls[0].target_name, "Dog");
    }
}
