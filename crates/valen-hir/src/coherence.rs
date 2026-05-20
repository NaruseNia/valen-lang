//! Trait coherence checking: orphan rule, blanket impl rejection, duplicate impl
//! detection, and trait satisfaction verification.

use indexmap::IndexSet;
use smol_str::SmolStr;
use valen_diagnostics::{DiagCode, Diagnostics};

use crate::{DefKind, FnDef, Hir, TyRef};

/// Output of the coherence checking pass.
pub struct CoherenceResult {
    pub diagnostics: Diagnostics,
}

/// Check all trait impls in the HIR for coherence violations.
pub fn check_coherence(hir: &Hir, imports: &[SmolStr]) -> CoherenceResult {
    // Issue #021: Build the set of locally-defined type/trait names from HIR defs
    // instead of relying solely on import names. A name is "local" if it exists
    // in the HIR defs and is NOT a prelude-injected synthetic type.
    let local_defs: IndexSet<SmolStr> = hir
        .defs
        .values()
        .filter(|d| {
            !d.name.is_empty()
                && !hir.prelude_ids.contains(&d.id)
                && matches!(
                    d.kind,
                    DefKind::Class(_)
                        | DefKind::DataClass(_)
                        | DefKind::Enum(_)
                        | DefKind::Trait(_)
                )
        })
        .map(|d| d.name.clone())
        .collect();

    let mut checker = CoherenceChecker {
        hir,
        foreign: imports.iter().cloned().collect(),
        local_defs,
        diags: Diagnostics::new(),
    };
    checker.run();
    CoherenceResult {
        diagnostics: checker.diags,
    }
}

struct CoherenceChecker<'h> {
    hir: &'h Hir,
    foreign: IndexSet<SmolStr>,
    /// Names of types/traits defined locally in the current compilation unit.
    local_defs: IndexSet<SmolStr>,
    diags: Diagnostics,
}

impl<'h> CoherenceChecker<'h> {
    fn run(&mut self) {
        let impls: Vec<_> = self
            .hir
            .defs
            .iter()
            .filter_map(|(id, d)| {
                if self.hir.prelude_ids.contains(id) {
                    return None;
                }
                if let DefKind::Impl(imp) = &d.kind {
                    Some((d.span, d.package.clone(), imp.clone()))
                } else {
                    None
                }
            })
            .collect();

        let mut seen_pairs: Vec<(SmolStr, SmolStr, valen_ast::Span)> = Vec::new();

        for (span, pkg, imp) in &impls {
            let trait_name = tyref_name(&imp.trait_ref);
            let target_name = tyref_name(&imp.target);

            // Inherent impl (no trait ref) — skip coherence checks
            if imp.trait_ref == TyRef::Error && target_name.is_some() {
                continue;
            }

            let Some(tn) = &trait_name else { continue };
            let Some(tgt) = &target_name else { continue };

            // Blanket impl detection: target is a generic type parameter
            let generic_names: IndexSet<SmolStr> = imp.generics.iter().cloned().collect();
            if is_type_param(&imp.target, &generic_names, self.hir, &self.foreign) {
                self.diags.error(
                    DiagCode::BLANKET_IMPL_NOT_ALLOWED,
                    *span,
                    SmolStr::from(format!(
                        "blanket impl `impl {tn} for {tgt}` is not allowed in MVP"
                    )),
                );
                continue;
            }

            // Issue #021: Orphan rule — check HIR defs for local origin, not just import names.
            // A name is local if it appears in local_defs (user-defined types/traits).
            let trait_local = self.local_defs.contains(tn.as_str());
            let type_local = self.local_defs.contains(tgt.as_str());
            if !trait_local && !type_local && !is_stdlib_package(pkg) {
                self.diags.error(
                    DiagCode::ORPHAN_RULE_VIOLATION,
                    *span,
                    SmolStr::from(format!(
                        "orphan rule: cannot implement foreign trait `{tn}` for foreign type `{tgt}`"
                    )),
                );
            }

            // Duplicate impl detection
            for (prev_tn, prev_tgt, prev_span) in &seen_pairs {
                if prev_tn == tn && prev_tgt == tgt {
                    self.diags.error(
                        DiagCode::IMPL_CONFLICT,
                        *span,
                        SmolStr::from(format!(
                            "conflicting impl: `impl {tn} for {tgt}` is already defined at {:?}",
                            prev_span
                        )),
                    );
                }
            }
            seen_pairs.push((tn.clone(), tgt.clone(), *span));

            // Sealed trait: reject enum implementors
            if self.is_sealed_trait(tn) && self.is_enum_type(tgt) {
                self.diags.error(
                    DiagCode::SEALED_TRAIT_IMPL_BY_ENUM,
                    *span,
                    SmolStr::from(format!(
                        "enum `{tgt}` cannot implement sealed trait `{tn}`; only class and data class are permitted"
                    )),
                );
            }

            // Trait satisfaction: check all required methods are implemented
            self.check_trait_satisfaction(tn, imp, *span);
        }
    }

    fn is_sealed_trait(&self, name: &SmolStr) -> bool {
        self.hir
            .defs
            .values()
            .any(|d| d.name == *name && matches!(&d.kind, DefKind::Trait(t) if t.is_sealed))
    }

    fn is_enum_type(&self, name: &SmolStr) -> bool {
        self.hir
            .defs
            .values()
            .any(|d| d.name == *name && matches!(&d.kind, DefKind::Enum(_)))
    }

    fn check_trait_satisfaction(
        &mut self,
        trait_name: &SmolStr,
        imp: &crate::ImplDef,
        impl_span: valen_ast::Span,
    ) {
        let trait_def = self
            .hir
            .defs
            .values()
            .find(|d| d.name == *trait_name && matches!(d.kind, DefKind::Trait(_)));

        let Some(trait_def) = trait_def else {
            if !self.foreign.contains(trait_name.as_str()) {
                self.diags.error(
                    DiagCode::UNKNOWN_TRAIT,
                    impl_span,
                    SmolStr::from(format!("trait `{trait_name}` not found")),
                );
            }
            return;
        };

        let DefKind::Trait(tdef) = &trait_def.kind else {
            return;
        };

        // Build substitution map: trait generic params → impl type args
        // e.g. trait Satisfied<T> + impl Satisfied<String> → {T: String}
        let trait_type_args = match &imp.trait_ref {
            TyRef::Generic(_, args) => args.clone(),
            _ => vec![],
        };
        let subst: Vec<(SmolStr, TyRef)> = tdef
            .generics
            .iter()
            .zip(trait_type_args.iter())
            .map(|(name, arg)| (name.clone(), arg.clone()))
            .collect();

        for &required_id in &tdef.methods {
            let Some(required_def) = self.hir.defs.get(&required_id) else {
                continue;
            };
            let DefKind::Fn(required_fn) = &required_def.kind else {
                continue;
            };

            let substituted = substitute_fn_def(required_fn, &subst);

            if substituted.has_body {
                if let Some((_, impl_fn)) = self.find_impl_method(imp, &required_def.name) {
                    self.check_signature_match(
                        &required_def.name,
                        &substituted,
                        &impl_fn,
                        impl_span,
                    );
                }
                continue;
            }

            if let Some((_, impl_fn)) = self.find_impl_method(imp, &required_def.name) {
                self.check_signature_match(&required_def.name, &substituted, &impl_fn, impl_span);
            } else {
                self.diags.error(
                    DiagCode::MISSING_TRAIT_METHOD,
                    impl_span,
                    SmolStr::from(format!(
                        "missing method `{}` required by trait `{trait_name}`",
                        required_def.name
                    )),
                );
            }
        }
    }

    fn find_impl_method(&self, imp: &crate::ImplDef, name: &str) -> Option<(crate::DefId, FnDef)> {
        for &mid in &imp.methods {
            if let Some(def) = self.hir.defs.get(&mid) {
                if def.name == name {
                    if let DefKind::Fn(fdef) = &def.kind {
                        return Some((mid, fdef.clone()));
                    }
                }
            }
        }
        None
    }

    fn check_signature_match(
        &mut self,
        method_name: &SmolStr,
        required: &FnDef,
        actual: &FnDef,
        impl_span: valen_ast::Span,
    ) {
        let req_non_self: Vec<_> = required.params.iter().filter(|p| !p.is_self).collect();
        let act_non_self: Vec<_> = actual.params.iter().filter(|p| !p.is_self).collect();

        if req_non_self.len() != act_non_self.len() {
            self.diags.error(
                DiagCode::TRAIT_METHOD_SIG_MISMATCH,
                impl_span,
                SmolStr::from(format!(
                    "method `{method_name}` has {} parameter(s), trait requires {}",
                    act_non_self.len(),
                    req_non_self.len()
                )),
            );
            return;
        }

        for (req_p, act_p) in req_non_self.iter().zip(act_non_self.iter()) {
            if req_p.ty != act_p.ty {
                self.diags.error(
                    DiagCode::TRAIT_METHOD_SIG_MISMATCH,
                    impl_span,
                    SmolStr::from(format!(
                        "method `{method_name}`: parameter `{}` type mismatch — trait requires `{}`, found `{}`",
                        req_p.name, req_p.ty, act_p.ty
                    )),
                );
            }
        }

        let req_ret = &required.return_ty;
        let act_ret = &actual.return_ty;
        if req_ret != act_ret {
            self.diags.error(
                DiagCode::TRAIT_METHOD_SIG_MISMATCH,
                impl_span,
                SmolStr::from(format!(
                    "method `{method_name}`: return type mismatch — trait requires `{}`, found `{}`",
                    format_opt_tyref(req_ret), format_opt_tyref(act_ret)
                )),
            );
        }
    }
}

fn format_opt_tyref(ty: &Option<TyRef>) -> String {
    match ty {
        Some(t) => format!("{t}"),
        None => "Unit".to_string(),
    }
}

fn substitute_fn_def(f: &FnDef, subst: &[(SmolStr, TyRef)]) -> FnDef {
    if subst.is_empty() {
        return f.clone();
    }
    FnDef {
        params: f
            .params
            .iter()
            .map(|p| crate::ParamDef {
                ty: substitute_tyref(&p.ty, subst),
                ..p.clone()
            })
            .collect(),
        return_ty: f.return_ty.as_ref().map(|t| substitute_tyref(t, subst)),
        has_body: f.has_body,
        generic_bounds: f.generic_bounds.clone(),
        is_unsafe: f.is_unsafe,
        is_open: f.is_open,
        is_override: f.is_override,
        is_abstract: f.is_abstract,
    }
}

fn substitute_tyref(ty: &TyRef, subst: &[(SmolStr, TyRef)]) -> TyRef {
    match ty {
        TyRef::Named(n) | TyRef::Unresolved(n) => {
            for (param, replacement) in subst {
                if n == param {
                    return replacement.clone();
                }
            }
            ty.clone()
        }
        TyRef::Generic(n, args) => {
            for (param, replacement) in subst {
                if n == param {
                    return replacement.clone();
                }
            }
            TyRef::Generic(
                n.clone(),
                args.iter().map(|a| substitute_tyref(a, subst)).collect(),
            )
        }
        TyRef::Nullable(inner) => TyRef::Nullable(Box::new(substitute_tyref(inner, subst))),
        TyRef::Fn(params, ret) => TyRef::Fn(
            params.iter().map(|p| substitute_tyref(p, subst)).collect(),
            Box::new(substitute_tyref(ret, subst)),
        ),
        _ => ty.clone(),
    }
}

fn tyref_name(ty: &TyRef) -> Option<SmolStr> {
    match ty {
        TyRef::Named(n) => Some(n.clone()),
        TyRef::Prim(p) => Some(SmolStr::from(format!("{p:?}"))),
        TyRef::Generic(n, _) => Some(n.clone()),
        _ => None,
    }
}

/// Returns true if the package is a stdlib package (valen.core or valen.std.*),
/// which is exempt from the orphan rule for foreign-foreign impl.
fn is_stdlib_package(pkg: &Option<Vec<SmolStr>>) -> bool {
    let Some(segs) = pkg else { return false };
    if segs.len() >= 2 && segs[0] == "valen" && (segs[1] == "core" || segs[1] == "std") {
        return true;
    }
    false
}

fn is_type_param(
    ty: &TyRef,
    generic_names: &IndexSet<SmolStr>,
    hir: &Hir,
    foreign: &IndexSet<SmolStr>,
) -> bool {
    match ty {
        TyRef::Named(n) => {
            // Explicitly listed in the impl's generic parameters
            if generic_names.contains(n) {
                return true;
            }
            // Fallback heuristic for impls without explicit generics:
            // a single-uppercase-letter name that is neither a locally defined type
            // nor a known import is treated as a type parameter.
            if n.len() == 1 && n.chars().next().unwrap_or('a').is_uppercase() {
                let locally_defined = hir.defs.values().any(|d| d.name == *n);
                let is_import = foreign.contains(n.as_str());
                return !locally_defined && !is_import;
            }
            false
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve;
    use valen_ast::FileId;
    use valen_parser::parse;

    fn check_source(src: &str) -> CoherenceResult {
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
        let imports: Vec<SmolStr> = parsed
            .items
            .iter()
            .filter_map(|i| {
                if let valen_ast::Item::Import(imp) = i {
                    imp.alias.clone().or_else(|| imp.path.last().cloned())
                } else {
                    None
                }
            })
            .collect();
        check_coherence(&resolved.hir, &imports)
    }

    fn assert_no_errors(r: &CoherenceResult) {
        assert!(
            !r.diagnostics.has_errors(),
            "coherence errors: {:?}",
            r.diagnostics
        );
    }

    fn assert_has_error(r: &CoherenceResult, code: DiagCode) {
        assert!(
            r.diagnostics.iter().any(|d| d.code == code),
            "expected error {:?}, got: {:?}",
            code,
            r.diagnostics
        );
    }

    // -- valid impls --------------------------------------------------------

    #[test]
    fn local_trait_local_type() {
        let r = check_source(
            "trait Show { fn show(self) -> String; }\nclass Dog {}\nimpl Show for Dog { fn show(self) -> String { \"Dog\" } }",
        );
        assert_no_errors(&r);
    }

    #[test]
    fn local_trait_for_imported_type() {
        let r = check_source(
            "import java.util.List;\ntrait Show { fn show(self) -> String; }\nimpl Show for List { fn show(self) -> String { \"list\" } }",
        );
        assert_no_errors(&r);
    }

    #[test]
    fn imported_trait_for_local_type() {
        // Simulated: import a "trait" name, then impl it for a local type
        // In real code the import would bring in a trait, but for test purposes
        // the orphan rule just checks name locality
        let r = check_source(
            "import java.io.Serializable;\nclass Dog {}\nimpl Serializable for Dog { }",
        );
        assert_no_errors(&r);
    }

    // -- orphan rule violations ---------------------------------------------

    #[test]
    fn foreign_trait_foreign_type() {
        let r = check_source(
            "import java.io.Serializable;\nimport java.util.List;\nimpl Serializable for List { }",
        );
        assert_has_error(&r, DiagCode::ORPHAN_RULE_VIOLATION);
    }

    #[test]
    fn stdlib_package_exempt_from_orphan_rule() {
        let r = check_source(
            "package valen.core;\nimport java.util.List;\ntrait MyTrait { fn foo(self) -> Int; }\nimpl MyTrait for List { fn foo(self) -> Int { 0 } }",
        );
        assert_no_errors(&r);
    }

    #[test]
    fn non_stdlib_package_not_exempt() {
        let r = check_source(
            "package com.example;\nimport java.io.Serializable;\nimport java.util.List;\nimpl Serializable for List { }",
        );
        assert_has_error(&r, DiagCode::ORPHAN_RULE_VIOLATION);
    }

    // -- blanket impl -------------------------------------------------------

    #[test]
    fn blanket_impl_rejected() {
        let r = check_source(
            "trait Show { fn show(self) -> String; }\nimpl Show for T { fn show(self) -> String { \"any\" } }",
        );
        assert_has_error(&r, DiagCode::BLANKET_IMPL_NOT_ALLOWED);
    }

    // -- duplicate impl -----------------------------------------------------

    #[test]
    fn duplicate_impl_rejected() {
        let r = check_source(
            "trait Show { fn show(self) -> String; }\nclass Dog {}\nimpl Show for Dog { fn show(self) -> String { \"a\" } }\nimpl Show for Dog { fn show(self) -> String { \"b\" } }",
        );
        assert_has_error(&r, DiagCode::IMPL_CONFLICT);
    }

    // -- trait satisfaction --------------------------------------------------

    #[test]
    fn missing_required_method() {
        let r = check_source(
            "trait Area { fn area(self) -> Float; }\nclass Circle {}\nimpl Area for Circle { }",
        );
        assert_has_error(&r, DiagCode::MISSING_TRAIT_METHOD);
    }

    #[test]
    fn all_methods_implemented() {
        let r = check_source(
            "trait Area { fn area(self) -> Float; }\nclass Circle {}\nimpl Area for Circle { fn area(self) -> Float { 0.0 } }",
        );
        assert_no_errors(&r);
    }

    #[test]
    fn method_param_count_mismatch() {
        let r = check_source(
            "trait Foo { fn bar(self, x: Int) -> Int; }\nclass Baz {}\nimpl Foo for Baz { fn bar(self) -> Int { 0 } }",
        );
        assert_has_error(&r, DiagCode::TRAIT_METHOD_SIG_MISMATCH);
    }

    #[test]
    fn method_return_type_mismatch() {
        let r = check_source(
            "trait Foo { fn bar(self) -> Int; }\nclass Baz {}\nimpl Foo for Baz { fn bar(self) -> String { \"oops\" } }",
        );
        assert_has_error(&r, DiagCode::TRAIT_METHOD_SIG_MISMATCH);
    }

    #[test]
    fn method_param_type_mismatch() {
        let r = check_source(
            "trait Foo { fn bar(self, x: Int) -> Int; }\nclass Baz {}\nimpl Foo for Baz { fn bar(self, x: String) -> Int { 0 } }",
        );
        assert_has_error(&r, DiagCode::TRAIT_METHOD_SIG_MISMATCH);
    }

    // -- default method not required ----------------------------------------

    #[test]
    fn default_method_not_required() {
        let r = check_source(
            "trait Greet { fn greet(self) -> String { \"hello\" } }\nclass Dog {}\nimpl Greet for Dog { }",
        );
        assert_no_errors(&r);
    }

    // -- multiple traits with different types -------------------------------

    // -- sealed trait: enum impl rejection ----------------------------------

    #[test]
    fn sealed_trait_enum_impl_rejected() {
        let r = check_source(
            "sealed trait Marker {}\nenum Color { Red, Green }\nimpl Marker for Color {}",
        );
        assert_has_error(&r, DiagCode::SEALED_TRAIT_IMPL_BY_ENUM);
    }

    #[test]
    fn sealed_trait_class_impl_ok() {
        let r = check_source("sealed trait Marker {}\nclass Dog {}\nimpl Marker for Dog {}");
        assert_no_errors(&r);
    }

    // -- multiple traits with different types -------------------------------

    #[test]
    fn two_different_impls_ok() {
        let r = check_source(
            "trait A { fn a(self) -> Int; }\ntrait B { fn b(self) -> Int; }\nclass X {}\nimpl A for X { fn a(self) -> Int { 1 } }\nimpl B for X { fn b(self) -> Int { 2 } }",
        );
        assert_no_errors(&r);
    }
}
