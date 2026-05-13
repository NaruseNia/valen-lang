//! Match exhaustiveness checking for enums, sealed classes, and `Bool`.

use indexmap::{IndexMap, IndexSet};
use smol_str::SmolStr;
use valen_ast::{self, Pattern};
use valen_diagnostics::{DiagCode, Diagnostics};

use crate::{ClassDefKind, DefKind, EnumDef, Hir, TyRef};

/// Output of the exhaustiveness checking pass.
pub struct ExhaustivenessResult {
    pub diagnostics: Diagnostics,
}

/// Walk all match expressions in `items` and report non-exhaustive patterns.
pub fn check_exhaustiveness(hir: &Hir, items: &[valen_ast::Item]) -> ExhaustivenessResult {
    let mut checker = ExhaustivenessChecker {
        hir,
        diags: Diagnostics::new(),
        locals: IndexMap::new(),
    };
    checker.check_items(items);
    ExhaustivenessResult {
        diagnostics: checker.diags,
    }
}

struct ExhaustivenessChecker<'h> {
    hir: &'h Hir,
    diags: Diagnostics,
    locals: IndexMap<SmolStr, SmolStr>,
}

impl<'h> ExhaustivenessChecker<'h> {
    fn check_items(&mut self, items: &[valen_ast::Item]) {
        for item in items {
            match item {
                valen_ast::Item::Fn(f) => self.check_fn(f),
                valen_ast::Item::Class(c) => {
                    for member in &c.body {
                        if let valen_ast::ClassMember::Method(m) = member {
                            self.check_fn(m);
                        }
                    }
                }
                valen_ast::Item::Impl(imp) => {
                    for ii in &imp.items {
                        if let valen_ast::ImplItem::Fn(m) = ii {
                            self.check_fn(m);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn check_fn(&mut self, f: &valen_ast::FnDecl) {
        let Some(body) = &f.body else { return };

        let prev_locals = std::mem::take(&mut self.locals);

        for p in &f.params {
            if let Some(type_name) = self.resolve_type_name(&p.ty) {
                self.locals.insert(p.name.clone(), type_name);
            }
        }

        self.check_block(body);
        self.locals = prev_locals;
    }

    fn resolve_type_name(&self, ty: &valen_ast::Type) -> Option<SmolStr> {
        match ty {
            valen_ast::Type::Path(tp) => {
                if tp.segments.len() == 1 {
                    Some(tp.segments[0].name.clone())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn check_block(&mut self, block: &valen_ast::Block) {
        for stmt in &block.stmts {
            match stmt {
                valen_ast::Stmt::Expr(e) | valen_ast::Stmt::ExprSemi(e) => self.check_expr(e),
                valen_ast::Stmt::Let(ls) => {
                    self.check_expr(&ls.init);
                    if let Some(ty) = &ls.ty {
                        if let Some(tn) = self.resolve_type_name(ty) {
                            self.locals.insert(ls.name.clone(), tn);
                        }
                    }
                }
            }
        }
        if let Some(tail) = &block.tail {
            self.check_expr(tail);
        }
    }

    fn check_expr(&mut self, expr: &valen_ast::Expr) {
        match expr {
            valen_ast::Expr::Match(me) => {
                self.check_match(me);
                self.check_expr(&me.scrutinee);
                for arm in &me.arms {
                    self.check_expr(&arm.body);
                    if let Some(g) = &arm.guard {
                        self.check_expr(g);
                    }
                }
            }
            valen_ast::Expr::If(ife) => {
                self.check_expr(&ife.cond);
                self.check_block(&ife.then_branch);
                if let Some(el) = &ife.else_branch {
                    self.check_expr(el);
                }
            }
            valen_ast::Expr::Block(blk) => self.check_block(blk),
            valen_ast::Expr::Call(c) => {
                self.check_expr(&c.callee);
                for a in &c.args {
                    self.check_expr(&a.value);
                }
            }
            valen_ast::Expr::MethodCall(mc) => {
                self.check_expr(&mc.receiver);
                for a in &mc.args {
                    self.check_expr(&a.value);
                }
            }
            valen_ast::Expr::Binary(b) => {
                self.check_expr(&b.lhs);
                self.check_expr(&b.rhs);
            }
            valen_ast::Expr::Unary(u) => self.check_expr(&u.expr),
            valen_ast::Expr::Assign(a) => {
                self.check_expr(&a.target);
                self.check_expr(&a.value);
            }
            valen_ast::Expr::Return(r) => {
                if let Some(v) = &r.value {
                    self.check_expr(v);
                }
            }
            valen_ast::Expr::For(f) => {
                self.check_expr(&f.iter);
                self.check_block(&f.body);
            }
            valen_ast::Expr::While(w) => {
                self.check_expr(&w.cond);
                self.check_block(&w.body);
            }
            valen_ast::Expr::Loop(l) => self.check_block(&l.body),
            valen_ast::Expr::Lambda(lam) => self.check_expr(&lam.body),
            valen_ast::Expr::Field(fa) => self.check_expr(&fa.receiver),
            valen_ast::Expr::Break(b) => {
                if let Some(v) = &b.value {
                    self.check_expr(v);
                }
            }
            valen_ast::Expr::Safe(s) => self.check_block(&s.block),
            _ => {}
        }
    }

    fn check_match(&mut self, me: &valen_ast::MatchExpr) {
        let scrutinee_type = self.infer_scrutinee_type(&me.scrutinee);

        let Some(type_name) = scrutinee_type else {
            return;
        };

        if let Some(enum_def) = self.find_enum(&type_name) {
            self.check_enum_exhaustive(me, &type_name, &enum_def);
        } else if self.is_sealed_class(&type_name) {
            self.check_sealed_exhaustive(me, &type_name);
        } else if type_name == "Bool" {
            self.check_bool_exhaustive(me);
        }
    }

    // -- enum exhaustiveness ------------------------------------------------

    fn check_enum_exhaustive(
        &mut self,
        me: &valen_ast::MatchExpr,
        enum_name: &SmolStr,
        enum_def: &EnumDef,
    ) {
        let all_variants: IndexSet<SmolStr> =
            enum_def.variants.iter().map(|v| v.name.clone()).collect();

        if has_wildcard_or_binding_excluding(&me.arms, &all_variants) {
            return;
        }

        let mut covered = IndexSet::new();
        for arm in &me.arms {
            if arm.guard.is_some() {
                continue;
            }
            collect_covered_variants(&arm.pattern, enum_name, &mut covered);
            collect_binding_as_type(&arm.pattern, &all_variants, &mut covered);
        }

        let missing: Vec<_> = all_variants.difference(&covered).collect();
        if !missing.is_empty() {
            let names: Vec<&str> = missing.iter().map(|n| n.as_str()).collect();
            self.diags.error(
                DiagCode::MATCH_NOT_EXHAUSTIVE,
                me.span,
                SmolStr::from(format!(
                    "non-exhaustive match on `{enum_name}`: missing variant(s) {}",
                    names.join(", ")
                )),
            );
        }
    }

    // -- sealed class exhaustiveness ----------------------------------------

    fn check_sealed_exhaustive(&mut self, me: &valen_ast::MatchExpr, sealed_name: &SmolStr) {
        let subclasses = self.find_sealed_subclasses(sealed_name);
        if subclasses.is_empty() {
            return;
        }

        if has_wildcard_or_binding_excluding(&me.arms, &subclasses) {
            return;
        }

        let mut covered = IndexSet::new();
        for arm in &me.arms {
            if arm.guard.is_some() {
                continue;
            }
            collect_covered_type_names(&arm.pattern, &mut covered);
            collect_binding_as_type(&arm.pattern, &subclasses, &mut covered);
        }

        let missing: Vec<_> = subclasses.difference(&covered).collect();
        if !missing.is_empty() {
            let names: Vec<&str> = missing.iter().map(|n| n.as_str()).collect();
            self.diags.error(
                DiagCode::MATCH_NOT_EXHAUSTIVE,
                me.span,
                SmolStr::from(format!(
                    "non-exhaustive match on sealed `{sealed_name}`: missing subtype(s) {}",
                    names.join(", ")
                )),
            );
        }
    }

    // -- Bool exhaustiveness ------------------------------------------------

    fn check_bool_exhaustive(&mut self, me: &valen_ast::MatchExpr) {
        if has_wildcard_or_binding(&me.arms) {
            return;
        }

        let mut has_true = false;
        let mut has_false = false;

        for arm in &me.arms {
            if arm.guard.is_some() {
                continue;
            }
            check_bool_pattern(&arm.pattern, &mut has_true, &mut has_false);
        }

        if !has_true || !has_false {
            let mut missing = Vec::new();
            if !has_true {
                missing.push("true");
            }
            if !has_false {
                missing.push("false");
            }
            self.diags.error(
                DiagCode::MATCH_NOT_EXHAUSTIVE,
                me.span,
                SmolStr::from(format!(
                    "non-exhaustive match on `Bool`: missing {}",
                    missing.join(", ")
                )),
            );
        }
    }

    // -- type resolution helpers --------------------------------------------

    fn infer_scrutinee_type(&self, expr: &valen_ast::Expr) -> Option<SmolStr> {
        match expr {
            valen_ast::Expr::Path(path) => {
                if path.segments.len() == 1 {
                    let name = &path.segments[0].name;
                    if let Some(type_name) = self.locals.get(name) {
                        return Some(type_name.clone());
                    }
                }
                None
            }
            valen_ast::Expr::Call(c) => {
                // Constructor call: Shape::Circle(...) → Shape
                if let valen_ast::Expr::Path(path) = c.callee.as_ref() {
                    if path.segments.len() == 1 {
                        let name = &path.segments[0].name;
                        if self.find_enum(name).is_some() || self.is_sealed_class(name) {
                            return Some(name.clone());
                        }
                    }
                }
                None
            }
            valen_ast::Expr::MethodCall(mc) => self.infer_scrutinee_type(&mc.receiver),
            _ => None,
        }
    }

    fn find_enum(&self, name: &str) -> Option<EnumDef> {
        self.hir.defs.values().find_map(|d| {
            if d.name == name {
                if let DefKind::Enum(e) = &d.kind {
                    return Some(e.clone());
                }
            }
            None
        })
    }

    fn is_sealed_class(&self, name: &str) -> bool {
        self.hir.defs.values().any(|d| {
            d.name == name && matches!(&d.kind, DefKind::Class(c) if c.kind == ClassDefKind::Sealed)
        })
    }

    fn find_sealed_subclasses(&self, sealed_name: &SmolStr) -> IndexSet<SmolStr> {
        let mut subs = IndexSet::new();
        for def in self.hir.defs.values() {
            if let DefKind::Class(c) = &def.kind {
                if let Some(TyRef::Named(parent)) = &c.superclass {
                    if parent == sealed_name {
                        subs.insert(def.name.clone());
                    }
                }
            }
        }
        subs
    }
}

// ---------------------------------------------------------------------------
// Pattern analysis helpers
// ---------------------------------------------------------------------------

fn has_wildcard_or_binding(arms: &[valen_ast::MatchArm]) -> bool {
    arms.iter().any(|arm| {
        if arm.guard.is_some() {
            return false;
        }
        pattern_is_catch_all(&arm.pattern)
    })
}

fn has_wildcard_or_binding_excluding(
    arms: &[valen_ast::MatchArm],
    known_types: &IndexSet<SmolStr>,
) -> bool {
    arms.iter().any(|arm| {
        if arm.guard.is_some() {
            return false;
        }
        pattern_is_catch_all_excluding(&arm.pattern, known_types)
    })
}

fn pattern_is_catch_all(pat: &Pattern) -> bool {
    match pat {
        Pattern::Wildcard(_) => true,
        Pattern::Binding(_) => true,
        Pattern::At(at) => pattern_is_catch_all(&at.pattern),
        Pattern::Or(pats, _) => pats.iter().any(pattern_is_catch_all),
        _ => false,
    }
}

fn pattern_is_catch_all_excluding(pat: &Pattern, known_types: &IndexSet<SmolStr>) -> bool {
    match pat {
        Pattern::Wildcard(_) => true,
        Pattern::Binding(b) => !known_types.contains(&b.name),
        Pattern::At(at) => pattern_is_catch_all_excluding(&at.pattern, known_types),
        Pattern::Or(pats, _) => pats
            .iter()
            .any(|p| pattern_is_catch_all_excluding(p, known_types)),
        _ => false,
    }
}

fn collect_binding_as_type(
    pat: &Pattern,
    known_types: &IndexSet<SmolStr>,
    covered: &mut IndexSet<SmolStr>,
) {
    match pat {
        Pattern::Binding(b) if known_types.contains(&b.name) => {
            covered.insert(b.name.clone());
        }
        Pattern::Or(pats, _) => {
            for p in pats {
                collect_binding_as_type(p, known_types, covered);
            }
        }
        Pattern::At(at) => {
            collect_binding_as_type(&at.pattern, known_types, covered);
        }
        _ => {}
    }
}

fn collect_covered_variants(pat: &Pattern, enum_name: &SmolStr, covered: &mut IndexSet<SmolStr>) {
    match pat {
        Pattern::Path(path) => {
            if let Some(variant) = extract_variant_name(path, enum_name) {
                covered.insert(variant);
            }
        }
        Pattern::Struct(sp) => {
            if let Some(variant) = extract_variant_name(&sp.path, enum_name) {
                covered.insert(variant);
            }
        }
        Pattern::Or(pats, _) => {
            for p in pats {
                collect_covered_variants(p, enum_name, covered);
            }
        }
        Pattern::At(at) => {
            collect_covered_variants(&at.pattern, enum_name, covered);
        }
        _ => {}
    }
}

fn extract_variant_name(path: &valen_ast::Path, enum_name: &SmolStr) -> Option<SmolStr> {
    if path.segments.len() == 2 && path.segments[0].name == *enum_name {
        Some(path.segments[1].name.clone())
    } else if path.segments.len() == 1 {
        Some(path.segments[0].name.clone())
    } else {
        None
    }
}

fn collect_covered_type_names(pat: &Pattern, covered: &mut IndexSet<SmolStr>) {
    match pat {
        Pattern::Path(path) => {
            if let Some(seg) = path.segments.last() {
                covered.insert(seg.name.clone());
            }
        }
        Pattern::Struct(sp) => {
            if let Some(seg) = sp.path.segments.last() {
                covered.insert(seg.name.clone());
            }
        }
        Pattern::Or(pats, _) => {
            for p in pats {
                collect_covered_type_names(p, covered);
            }
        }
        Pattern::At(at) => {
            collect_covered_type_names(&at.pattern, covered);
        }
        _ => {}
    }
}

fn check_bool_pattern(pat: &Pattern, has_true: &mut bool, has_false: &mut bool) {
    match pat {
        Pattern::Literal(valen_ast::Literal::Bool(true, _)) => *has_true = true,
        Pattern::Literal(valen_ast::Literal::Bool(false, _)) => *has_false = true,
        Pattern::Or(pats, _) => {
            for p in pats {
                check_bool_pattern(p, has_true, has_false);
            }
        }
        Pattern::At(at) => check_bool_pattern(&at.pattern, has_true, has_false),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve;
    use valen_ast::FileId;
    use valen_parser::parse;

    fn check_source(src: &str) -> ExhaustivenessResult {
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
        check_exhaustiveness(&resolved.hir, &parsed.items)
    }

    fn assert_no_errors(r: &ExhaustivenessResult) {
        assert!(
            !r.diagnostics.has_errors(),
            "exhaustiveness errors: {:?}",
            r.diagnostics
        );
    }

    fn assert_has_error(r: &ExhaustivenessResult, code: DiagCode) {
        assert!(
            r.diagnostics.iter().any(|d| d.code == code),
            "expected error {:?}, got: {:?}",
            code,
            r.diagnostics
        );
    }

    // -- enum exhaustive ----------------------------------------------------

    #[test]
    fn enum_all_variants_covered() {
        let r = check_source(
            r#"
enum Shape { Circle(r: Float), Rect(w: Float, h: Float), Point }
fn describe(s: Shape) -> String {
    match s {
        Shape::Circle(r) => "circle",
        Shape::Rect(w, h) => "rect",
        Shape::Point => "point",
    }
}
"#,
        );
        assert_no_errors(&r);
    }

    #[test]
    fn enum_missing_variant() {
        let r = check_source(
            r#"
enum Shape { Circle(r: Float), Rect(w: Float, h: Float), Point }
fn describe(s: Shape) -> String {
    match s {
        Shape::Circle(r) => "circle",
        Shape::Point => "point",
    }
}
"#,
        );
        assert_has_error(&r, DiagCode::MATCH_NOT_EXHAUSTIVE);
    }

    #[test]
    fn enum_with_wildcard_is_exhaustive() {
        let r = check_source(
            r#"
enum Shape { Circle(r: Float), Rect(w: Float, h: Float), Point }
fn describe(s: Shape) -> String {
    match s {
        Shape::Circle(r) => "circle",
        _ => "other",
    }
}
"#,
        );
        assert_no_errors(&r);
    }

    #[test]
    fn enum_with_binding_is_exhaustive() {
        let r = check_source(
            r#"
enum Shape { Circle(r: Float), Rect(w: Float, h: Float), Point }
fn describe(s: Shape) -> String {
    match s {
        Shape::Circle(r) => "circle",
        other => "other",
    }
}
"#,
        );
        assert_no_errors(&r);
    }

    #[test]
    fn enum_guard_not_counted() {
        let r = check_source(
            r#"
enum Color { Red, Green, Blue }
fn name(c: Color) -> String {
    match c {
        Color::Red => "red",
        Color::Green if true => "green",
        Color::Blue => "blue",
    }
}
"#,
        );
        assert_has_error(&r, DiagCode::MATCH_NOT_EXHAUSTIVE);
    }

    #[test]
    fn enum_or_pattern_covers_multiple() {
        let r = check_source(
            r#"
enum Color { Red, Green, Blue }
fn name(c: Color) -> String {
    match c {
        Color::Red | Color::Green => "warm",
        Color::Blue => "cool",
    }
}
"#,
        );
        assert_no_errors(&r);
    }

    // -- sealed class exhaustive --------------------------------------------

    #[test]
    fn sealed_all_subtypes_covered() {
        let r = check_source(
            r#"
sealed class Animal {}
class Dog : Animal {}
class Cat : Animal {}
fn speak(a: Animal) -> String {
    match a {
        Dog => "woof",
        Cat => "meow",
    }
}
"#,
        );
        assert_no_errors(&r);
    }

    #[test]
    fn sealed_missing_subtype() {
        let r = check_source(
            r#"
sealed class Animal {}
class Dog : Animal {}
class Cat : Animal {}
fn speak(a: Animal) -> String {
    match a {
        Dog => "woof",
    }
}
"#,
        );
        assert_has_error(&r, DiagCode::MATCH_NOT_EXHAUSTIVE);
    }

    #[test]
    fn sealed_with_wildcard() {
        let r = check_source(
            r#"
sealed class Animal {}
class Dog : Animal {}
class Cat : Animal {}
fn speak(a: Animal) -> String {
    match a {
        Dog => "woof",
        _ => "unknown",
    }
}
"#,
        );
        assert_no_errors(&r);
    }

    // -- Bool exhaustive ----------------------------------------------------

    #[test]
    fn bool_both_covered() {
        let r = check_source(
            r#"
fn check(b: Bool) -> String {
    match b {
        true => "yes",
        false => "no",
    }
}
"#,
        );
        assert_no_errors(&r);
    }

    #[test]
    fn bool_missing_false() {
        let r = check_source(
            r#"
fn check(b: Bool) -> String {
    match b {
        true => "yes",
    }
}
"#,
        );
        assert_has_error(&r, DiagCode::MATCH_NOT_EXHAUSTIVE);
    }

    #[test]
    fn bool_with_wildcard() {
        let r = check_source(
            r#"
fn check(b: Bool) -> String {
    match b {
        true => "yes",
        _ => "no",
    }
}
"#,
        );
        assert_no_errors(&r);
    }

    // -- non-enum match (no exhaustiveness required) ------------------------

    #[test]
    fn int_match_no_requirement() {
        let r = check_source(
            r#"
fn classify(n: Int) -> String {
    match n {
        0 => "zero",
        1 => "one",
    }
}
"#,
        );
        assert_no_errors(&r);
    }
}
