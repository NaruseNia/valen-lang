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
    resolve_with_classpath(items, &[])
}

/// Run name resolution with classpath entries for Java interop type checking.
pub fn resolve_with_classpath(items: &[Item], classpath: &[std::path::PathBuf]) -> ResolveResult {
    let mut resolver = Resolver {
        hir: Hir::default(),
        scope: Scope::default(),
        diagnostics: Diagnostics::new(),
        current_package: None,
    };
    resolver.resolve_items(items);
    resolver.validate_class_hierarchy();
    resolver.check_naming_conventions();

    if !classpath.is_empty() {
        resolver.hir.foreign_types =
            crate::classpath::scan_classpath(classpath, &resolver.hir.imports);
    }

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
                DiagCode::DUPLICATE_DEFINITION,
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

        // Register stdlib prelude (parsed from core.vln) for names not already defined
        self.register_stdlib_prelude();

        // Second pass: build method indexes and validate
        self.build_method_index();
    }

    /// Parse the embedded `core.vln` and register its definitions as prelude,
    /// skipping any names already defined by user code.
    fn register_stdlib_prelude(&mut self) {
        let stdlib_items = crate::stdlib::parse_core_stdlib();
        let prelude_pkg = Some(vec![SmolStr::from("valen"), SmolStr::from("core")]);

        let saved_package = self.current_package.clone();
        self.current_package = prelude_pkg.clone();

        let mut injected_names = indexmap::IndexSet::<SmolStr>::new();

        // First pass: register named items (types, traits, functions)
        for item in stdlib_items {
            match item {
                Item::Package(_) | Item::Import(_) | Item::Impl(_) => {}
                _ => {
                    let name = item_name(item);
                    if let Some(n) = &name {
                        if self.scope.lookup(n).is_some() {
                            continue;
                        }
                    }
                    let id_before = self.hir.next_id;
                    self.register_item(item);
                    for id in id_before..self.hir.next_id {
                        self.hir.prelude_ids.push(id);
                    }
                    if let Some(n) = &name {
                        injected_names.insert(n.clone());
                        self.hir.imports.insert(
                            SmolStr::from(n.as_str()),
                            vec![
                                SmolStr::from("valen"),
                                SmolStr::from("core"),
                                SmolStr::from(n.as_str()),
                            ],
                        );
                    }
                }
            }
        }

        // Register `Any` (top type, java.lang.Object equivalent) if not already defined.
        if self.scope.lookup("Any").is_none() {
            let any_id = self.hir.alloc_id();
            let any_def = Def {
                id: any_id,
                name: SmolStr::from("Any"),
                kind: DefKind::Class(ClassDef {
                    kind: ClassDefKind::Open,
                    ctor_params: vec![],
                    superclass: None,
                    trait_impls: vec![],
                    methods: vec![],
                }),
                vis: Vis::Pub,
                span: valen_ast::Span::DUMMY,
                package: prelude_pkg.clone(),
            };
            self.hir.defs.insert(any_id, any_def);
            self.hir.prelude_ids.push(any_id);
            self.scope.define(SmolStr::from("Any"), any_id);
            injected_names.insert(SmolStr::from("Any"));
            self.hir.imports.insert(
                SmolStr::from("Any"),
                vec![
                    SmolStr::from("valen"),
                    SmolStr::from("core"),
                    SmolStr::from("Any"),
                ],
            );
        }

        // Second pass: register impl blocks.
        // - Trait impls: only if both trait and target were injected.
        // - Inherent impls (no trait): register if the target is a primitive type or
        //   an injected name, so that stdlib methods like `Int.toLong()` are available.
        for item in stdlib_items {
            if let Item::Impl(imp) = item {
                let trait_name = imp.trait_ref.as_ref().and_then(type_head_name);
                let target_name = type_head_name(&imp.target);
                let is_prim_target = target_name
                    .as_ref()
                    .is_some_and(|n| crate::resolve_prim(n).is_some());
                let should_register = if trait_name.is_some() {
                    // Trait impl: both must be injected
                    trait_name
                        .as_ref()
                        .is_some_and(|n| injected_names.contains(n.as_str()))
                        && target_name
                            .as_ref()
                            .is_some_and(|n| injected_names.contains(n.as_str()))
                } else {
                    // Inherent impl: register if target is primitive or was injected
                    is_prim_target
                        || target_name
                            .as_ref()
                            .is_some_and(|n| injected_names.contains(n.as_str()))
                };
                if !should_register {
                    continue;
                }
                let id_before = self.hir.next_id;
                self.register_item(item);
                for id in id_before..self.hir.next_id {
                    self.hir.prelude_ids.push(id);
                }
            }
        }

        self.current_package = saved_package;
    }

    fn check_naming_conventions(&mut self) {
        for def in self.hir.defs.values() {
            if self.hir.prelude_ids.contains(&def.id) {
                continue;
            }
            match &def.kind {
                DefKind::Fn(_) if !def.name.is_empty() && contains_underscore(&def.name) => {
                    self.diagnostics.warning(
                        DiagCode::NAMING_NOT_CAMEL_CASE,
                        def.span,
                        SmolStr::from(format!(
                            "function `{}` should use camelCase (e.g. `{}`)",
                            def.name,
                            to_camel_case(&def.name)
                        )),
                    );
                }
                DefKind::Class(_)
                | DefKind::DataClass(_)
                | DefKind::Enum(_)
                | DefKind::Trait(_)
                    if !def.name.is_empty()
                        && def
                            .name
                            .chars()
                            .next()
                            .is_some_and(|c| c.is_ascii_lowercase()) =>
                {
                    self.diagnostics.warning(
                        DiagCode::NAMING_NOT_PASCAL_CASE,
                        def.span,
                        SmolStr::from(format!("type `{}` should use PascalCase", def.name)),
                    );
                }
                _ => {}
            }
        }
    }

    fn build_method_index(&mut self) {
        let defs: Vec<_> = self.hir.defs.values().cloned().collect();
        for def in &defs {
            match &def.kind {
                DefKind::Class(c) => {
                    self.hir
                        .type_methods
                        .entry(def.name.clone())
                        .or_default()
                        .extend(c.methods.clone());
                }
                DefKind::Impl(imp) => {
                    let target_name = match &imp.target {
                        TyRef::Named(n) => n.clone(),
                        TyRef::Prim(p) => SmolStr::from(format!("{p:?}")),
                        _ => continue,
                    };
                    if imp.trait_ref == TyRef::Error {
                        // Inherent impl: register methods under the target type
                        self.hir
                            .type_methods
                            .entry(target_name)
                            .or_default()
                            .extend(imp.methods.clone());
                    } else {
                        let trait_name = match &imp.trait_ref {
                            TyRef::Named(n) => n.clone(),
                            TyRef::Generic(n, _) => n.clone(),
                            _ => continue,
                        };
                        self.hir.trait_impls.push(ImplEntry {
                            trait_name,
                            target_name,
                            methods: imp.methods.clone(),
                        });
                    }
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
                self.hir.insert_def(id, def);
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
                        self.hir.insert_def(mid, mdef);
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
                self.hir.insert_def(id, def);
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
                        derives: dc.derives.clone(),
                    }),
                    vis: lower_vis(dc.visibility),
                    span: dc.span,
                    package: self.current_package.clone(),
                };
                self.hir.insert_def(id, def);
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
                    kind: DefKind::Enum(EnumDef {
                        variants,
                        derives: e.derives.clone(),
                    }),
                    vis: lower_vis(e.visibility),
                    span: e.span,
                    package: self.current_package.clone(),
                };
                self.hir.insert_def(id, def);
                self.define_name(e.name.clone(), id, e.span);
            }
            Item::Trait(t) => {
                let id = self.hir.alloc_id();
                let trait_params: Vec<SmolStr> =
                    t.generics.iter().map(|g| g.name.clone()).collect();
                let mut method_ids = Vec::new();
                let mut assoc_types = Vec::new();
                for ti in &t.items {
                    match ti {
                        valen_ast::TraitItem::Fn(m) => {
                            let mid = self.hir.alloc_id();
                            let mdef = Def {
                                id: mid,
                                name: m.name.clone(),
                                kind: DefKind::Fn(self.lower_fn_with_params(m, &trait_params)),
                                vis: Vis::Pub,
                                span: m.span,
                                package: self.current_package.clone(),
                            };
                            self.hir.insert_def(mid, mdef);
                            method_ids.push(mid);
                        }
                        valen_ast::TraitItem::AssociatedType(decl) => {
                            assoc_types.push(crate::HirAssocType {
                                name: decl.name.clone(),
                                default: decl
                                    .default
                                    .as_ref()
                                    .map(|t| lower_type_ref_with_params(t, &trait_params)),
                            });
                        }
                    }
                }
                let def = Def {
                    id,
                    name: t.name.clone(),
                    kind: DefKind::Trait(TraitDef {
                        is_sealed: t.is_sealed,
                        methods: method_ids,
                        associated_types: assoc_types,
                        generics: trait_params.clone(),
                    }),
                    vis: lower_vis(t.visibility),
                    span: t.span,
                    package: self.current_package.clone(),
                };
                self.hir.insert_def(id, def);
                self.define_name(t.name.clone(), id, t.span);
            }
            Item::Impl(imp) => {
                let id = self.hir.alloc_id();
                let impl_params: Vec<SmolStr> =
                    imp.generics.iter().map(|g| g.name.clone()).collect();
                let mut method_ids = Vec::new();
                let mut assoc_types = Vec::new();
                for ii in &imp.items {
                    match ii {
                        valen_ast::ImplItem::Fn(m) => {
                            let mid = self.hir.alloc_id();
                            let mdef = Def {
                                id: mid,
                                name: m.name.clone(),
                                kind: DefKind::Fn(self.lower_fn_with_params(m, &impl_params)),
                                vis: Vis::Pub,
                                span: m.span,
                                package: self.current_package.clone(),
                            };
                            self.hir.insert_def(mid, mdef);
                            method_ids.push(mid);
                        }
                        valen_ast::ImplItem::AssociatedType(def) => {
                            assoc_types.push((
                                def.name.clone(),
                                lower_type_ref_with_params(&def.ty, &impl_params),
                            ));
                        }
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
                        associated_types: assoc_types,
                    }),
                    vis: Vis::Internal,
                    span: imp.span,
                    package: self.current_package.clone(),
                };
                self.hir.insert_def(id, def);
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
                self.hir.insert_def(id, def);
                self.define_name(ta.name.clone(), id, ta.span);
            }
            Item::NewType(nt) => {
                let id = self.hir.alloc_id();
                let def = Def {
                    id,
                    name: nt.name.clone(),
                    kind: DefKind::NewType(crate::NewTypeDef {
                        inner_ty: lower_type_ref_with_params(&nt.inner_ty, &[]),
                    }),
                    vis: lower_vis(nt.visibility),
                    span: nt.span,
                    package: self.current_package.clone(),
                };
                self.hir.insert_def(id, def);
                self.define_name(nt.name.clone(), id, nt.span);
            }
            Item::AnnotationClass(ac) => {
                let id = self.hir.alloc_id();
                let params = ac
                    .params
                    .iter()
                    .map(|p| crate::AnnotationParamDef {
                        name: p.name.clone(),
                        ty: lower_type_ref_with_params(&p.ty, &[]),
                    })
                    .collect();
                let targets = ac
                    .annotations
                    .iter()
                    .filter(|a| a.name == "Target")
                    .flat_map(|a| {
                        a.args.iter().filter_map(|arg| {
                            if let valen_ast::Literal::String(s, _) = &arg.value {
                                Some(s.clone())
                            } else {
                                None
                            }
                        })
                    })
                    .collect();
                let def = Def {
                    id,
                    name: ac.name.clone(),
                    kind: DefKind::AnnotationClass(crate::AnnotationClassDef { params, targets }),
                    vis: lower_vis(ac.visibility),
                    span: ac.span,
                    package: self.current_package.clone(),
                };
                self.hir.insert_def(id, def);
                self.define_name(ac.name.clone(), id, ac.span);
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
                has_default: p.default.is_some(),
            })
            .collect();
        let generic_bounds = f
            .generics
            .iter()
            .map(|g| {
                let bounds = g
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
                (g.name.clone(), bounds)
            })
            .collect();
        FnDef {
            params,
            return_ty: f
                .return_type
                .as_ref()
                .map(|t| lower_type_ref_with_params(t, &all_params)),
            has_body: f.body.is_some(),
            generic_bounds,
            is_unsafe: f.is_unsafe,
            is_open: f.is_open,
            is_override: f.is_override,
            is_abstract: f.is_abstract,
        }
    }

    pub fn lookup(&self, name: &str) -> Option<DefId> {
        self.scope.lookup(name)
    }

    pub fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
    }

    /// Validates class inheritance rules after all definitions are registered.
    fn validate_class_hierarchy(&mut self) {
        let class_entries: Vec<_> = self
            .hir
            .defs
            .iter()
            .filter_map(|(id, def)| {
                if let DefKind::Class(ref cd) = def.kind {
                    Some((*id, def.name.clone(), def.span, cd.clone()))
                } else {
                    None
                }
            })
            .collect();

        for (_id, name, span, class_def) in &class_entries {
            if let Some(ref super_ty) = class_def.superclass {
                let super_name = match super_ty {
                    TyRef::Named(n) => Some(n.as_str()),
                    TyRef::Generic(n, _) => Some(n.as_str()),
                    _ => None,
                };
                if let Some(sname) = super_name {
                    // #003: Validate superclass is open/abstract/sealed
                    let super_class_def = self
                        .hir
                        .defs
                        .values()
                        .find(|d| d.name == sname)
                        .and_then(|d| {
                            if let DefKind::Class(ref cd) = d.kind {
                                Some(cd.clone())
                            } else {
                                None
                            }
                        });

                    if let Some(ref super_cd) = super_class_def {
                        if super_cd.kind == ClassDefKind::Final {
                            self.diagnostics.error(
                                DiagCode::INHERIT_FROM_FINAL,
                                *span,
                                format!(
                                    "cannot inherit from final class `{sname}`; mark it as `open`, `abstract`, or `sealed`"
                                ),
                            );
                        }
                    }

                    // #004: Validate override/open requirements for methods
                    let parent_methods: Vec<_> = super_class_def
                        .iter()
                        .flat_map(|cd| &cd.methods)
                        .filter_map(|mid| self.hir.defs.get(mid))
                        .filter_map(|d| {
                            if let DefKind::Fn(ref fd) = d.kind {
                                Some((d.name.clone(), fd.clone()))
                            } else {
                                None
                            }
                        })
                        .collect();

                    for mid in &class_def.methods {
                        if let Some(mdef) = self.hir.defs.get(mid) {
                            if let DefKind::Fn(ref fd) = mdef.kind {
                                let matching_parent =
                                    parent_methods.iter().find(|(pname, _)| *pname == mdef.name);

                                if let Some((_pname, parent_fd)) = matching_parent {
                                    if fd.is_override
                                        && !parent_fd.is_open
                                        && !parent_fd.is_abstract
                                    {
                                        self.diagnostics.error(
                                            DiagCode::OVERRIDE_PARENT_NOT_OPEN,
                                            mdef.span,
                                            format!(
                                                "cannot override `{}` in `{sname}`: method is not `open` or `abstract`",
                                                mdef.name
                                            ),
                                        );
                                    }
                                    if !fd.is_override {
                                        self.diagnostics.error(
                                            DiagCode::MISSING_OVERRIDE_KEYWORD,
                                            mdef.span,
                                            format!(
                                                "method `{}` in `{name}` shadows `{sname}::{}` but is not declared `override`",
                                                mdef.name, mdef.name
                                            ),
                                        );
                                    }
                                } else if fd.is_override {
                                    self.diagnostics.error(
                                        DiagCode::OVERRIDE_WITHOUT_KEYWORD,
                                        mdef.span,
                                        format!(
                                            "`override fn {}` does not override any method in `{sname}`",
                                            mdef.name
                                        ),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn contains_underscore(name: &str) -> bool {
    name.contains('_')
}

fn to_camel_case(name: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;
    for ch in name.chars() {
        if ch == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.extend(ch.to_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }
    result
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
        has_default: p.default.is_some(),
    }
}

fn type_head_name(ty: &valen_ast::Type) -> Option<SmolStr> {
    match ty {
        valen_ast::Type::Path(tp) if !tp.segments.is_empty() => {
            Some(tp.segments.last().unwrap().name.clone())
        }
        _ => None,
    }
}

fn item_name(item: &Item) -> Option<SmolStr> {
    match item {
        Item::Fn(f) => Some(f.name.clone()),
        Item::Class(c) => Some(c.name.clone()),
        Item::DataClass(dc) => Some(dc.name.clone()),
        Item::Enum(e) => Some(e.name.clone()),
        Item::Trait(t) => Some(t.name.clone()),
        Item::TypeAlias(ta) => Some(ta.name.clone()),
        Item::NewType(nt) => Some(nt.name.clone()),
        Item::AnnotationClass(ac) => Some(ac.name.clone()),
        Item::Impl(_) | Item::Package(_) | Item::Import(_) => None,
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
        valen_ast::Type::RefMut { inner, .. } => {
            TyRef::RefMut(Box::new(lower_type_ref_with_params(inner, type_params)))
        }
        valen_ast::Type::Tuple(..) => TyRef::Error,
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
        let user_impls: Vec<_> = r
            .hir
            .trait_impls
            .iter()
            .filter(|e| e.trait_name == "Show")
            .collect();
        assert_eq!(user_impls.len(), 1);
        assert_eq!(user_impls[0].trait_name, "Show");
        assert_eq!(user_impls[0].target_name, "Dog");
    }

    #[test]
    fn stdlib_prelude_names_injected() {
        let r = resolve_source("fn main() { 0 }");
        for name in [
            "Option",
            "Result",
            "Error",
            "Iterator",
            "Range",
            "JavaException",
        ] {
            assert!(
                r.hir
                    .defs
                    .values()
                    .any(|d| d.name == name && r.hir.prelude_ids.contains(&d.id)),
                "prelude should contain `{name}`"
            );
        }
    }

    #[test]
    fn user_java_exception_blocks_stdlib_error_impl() {
        let r = resolve_source("data class JavaException(pub x: Int);");
        assert!(
            !r.hir
                .trait_impls
                .iter()
                .any(|i| i.trait_name == "Error" && i.target_name == "JavaException"),
            "stdlib impl Error for JavaException should not be injected when user defines JavaException"
        );
    }

    #[test]
    fn user_error_trait_blocks_stdlib_impl() {
        let r = resolve_source("trait Error { fn code(self) -> Int; }");
        assert!(
            !r.hir.trait_impls.iter().any(|i| i.trait_name == "Error"),
            "stdlib impl should not be injected when user redefines Error"
        );
    }

    #[test]
    fn inherent_impl_method_resolution() {
        let r = resolve_source(
            "data class Vec2(pub x: Float, pub y: Float);\nimpl Vec2 { fn length(self) -> Float { 1.0 } }",
        );
        assert!(!r.diagnostics.has_errors());
        let res = r.hir.resolve_method("Vec2", "length");
        assert!(matches!(res, crate::MethodResolution::Found(_)));
    }

    #[test]
    fn inherent_impl_no_trait_impls_entry() {
        let r = resolve_source(
            "data class Vec2(pub x: Float, pub y: Float);\nimpl Vec2 { fn length(self) -> Float { 1.0 } }",
        );
        assert!(
            !r.hir.trait_impls.iter().any(|e| e.target_name == "Vec2"),
            "inherent impl should not appear in trait_impls"
        );
    }
}
