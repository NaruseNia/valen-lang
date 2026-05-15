//! AST pretty-printer with comment interleaving.

use valen_ast::*;

use crate::comment::Comment;

#[derive(Clone, Copy, PartialEq, Eq)]
enum FnCtx {
    TopLevel,
    ClassMethod,
    TraitMethod,
    ImplMethod,
}

/// Walks the AST and emits formatted source, interleaving recovered comments.
pub struct Printer<'a> {
    source: &'a str,
    comments: Vec<Comment>,
    buf: String,
    indent: usize,
    next_comment: usize,
}

impl<'a> Printer<'a> {
    pub fn new(source: &'a str, comments: Vec<Comment>) -> Self {
        Self {
            source,
            comments,
            buf: String::with_capacity(source.len()),
            indent: 0,
            next_comment: 0,
        }
    }

    fn sort_imports(&self, items: &[Item]) -> Vec<Item> {
        let mut result: Vec<Item> = Vec::with_capacity(items.len());
        let mut i = 0;
        while i < items.len() {
            if matches!(&items[i], Item::Import(_)) {
                let start = i;
                while i < items.len() && matches!(&items[i], Item::Import(_)) {
                    i += 1;
                }
                let mut group: Vec<&Item> = items[start..i].iter().collect();
                group.sort_by(|a, b| {
                    let ka = import_sort_key(a);
                    let kb = import_sort_key(b);
                    ka.cmp(&kb)
                });
                result.extend(group.iter().map(|&item| item.clone()));
            } else {
                result.push(items[i].clone());
                i += 1;
            }
        }
        result
    }

    pub fn print(mut self, items: &[Item]) -> String {
        let sorted_items = self.sort_imports(items);
        let items = &sorted_items;
        let mut prev_kind: Option<ItemKind> = None;

        for item in items {
            let kind = item_kind(item);
            let span = item_span(item);

            // Blank line between items, except between consecutive imports
            if let Some(pk) = prev_kind {
                if !(pk == ItemKind::Import && kind == ItemKind::Import) {
                    self.newline();
                }
            }

            self.flush_comments_before(span.start);
            self.print_item(item);
            prev_kind = Some(kind);
        }

        let last_end = items.last().map_or(0, |i| item_span(i).end);
        self.flush_remaining_comments(last_end);

        // Ensure trailing newline
        if !self.buf.ends_with('\n') {
            self.newline();
        }

        self.buf
    }

    // ── Output helpers ──────────────────────────────────────────────

    fn w(&mut self, s: &str) {
        self.buf.push_str(s);
    }

    fn newline(&mut self) {
        self.buf.push('\n');
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent {
            self.buf.push_str("    ");
        }
    }

    fn src(&self, span: &Span) -> &'a str {
        &self.source[span.start as usize..span.end as usize]
    }

    // ── Comment handling ────────────────────────────────────────────

    fn flush_comments_before(&mut self, pos: u32) {
        while self.next_comment < self.comments.len() {
            if self.comments[self.next_comment].start >= pos {
                break;
            }
            let text = self.comments[self.next_comment].text.clone();
            let end = self.comments[self.next_comment].end;
            self.write_indent();
            self.w(&text);
            self.newline();
            self.next_comment += 1;

            // Preserve blank lines after comments from the original source
            let next_pos = if self.next_comment < self.comments.len()
                && self.comments[self.next_comment].start < pos
            {
                self.comments[self.next_comment].start
            } else {
                pos
            };
            if has_blank_line(self.source, end, next_pos) {
                self.newline();
            }
        }
    }

    fn flush_remaining_comments(&mut self, after: u32) {
        let mut prev_end = after;
        while self.next_comment < self.comments.len() {
            let start = self.comments[self.next_comment].start;
            let end = self.comments[self.next_comment].end;
            let text = self.comments[self.next_comment].text.clone();

            if has_blank_line(self.source, prev_end, start) && !self.buf.ends_with("\n\n") {
                self.newline();
            }
            if !self.buf.ends_with('\n') {
                self.newline();
            }
            self.write_indent();
            self.w(&text);
            self.newline();

            prev_end = end;
            self.next_comment += 1;

            // Preserve blank lines between consecutive trailing comments
            if self.next_comment < self.comments.len()
                && has_blank_line(self.source, end, self.comments[self.next_comment].start)
            {
                self.newline();
            }
        }
    }

    // ── Items ───────────────────────────────────────────────────────

    fn print_item(&mut self, item: &Item) {
        match item {
            Item::Package(p) => self.print_package(p),
            Item::Import(i) => self.print_import(i),
            Item::Fn(f) => self.print_fn_decl(f, FnCtx::TopLevel),
            Item::Class(c) => self.print_class(c),
            Item::DataClass(d) => self.print_data_class(d),
            Item::Enum(e) => self.print_enum(e),
            Item::Trait(t) => self.print_trait(t),
            Item::Impl(i) => self.print_impl_block(i),
            Item::TypeAlias(t) => self.print_type_alias(t),
            Item::AnnotationClass(a) => self.print_annotation_class(a),
        }
    }

    fn print_package(&mut self, p: &PackageDecl) {
        self.write_indent();
        self.w("package ");
        self.w(&p.path.join("."));
        self.w(";");
        self.newline();
    }

    fn print_import(&mut self, i: &ImportDecl) {
        self.write_indent();
        self.w("import ");
        self.w(&i.path.join("."));
        if let Some(alias) = &i.alias {
            self.w(" as ");
            self.w(alias);
        }
        self.w(";");
        self.newline();
    }

    fn print_fn_decl(&mut self, f: &FnDecl, ctx: FnCtx) {
        self.write_indent();
        let show_vis = ctx == FnCtx::TopLevel || ctx == FnCtx::ClassMethod;
        if show_vis {
            self.print_visibility(&f.visibility);
        }
        // `abstract` is implicit for bodyless trait methods
        if f.is_abstract && ctx != FnCtx::TraitMethod {
            self.w("abstract ");
        }
        if f.is_open {
            self.w("open ");
        }
        if f.is_override {
            self.w("override ");
        }
        self.w("fn ");
        self.w(&f.name);
        self.print_generics(&f.generics);
        self.w("(");
        self.print_params(&f.params);
        self.w(")");
        if let Some(ret) = &f.return_type {
            self.w(" -> ");
            self.print_type(ret);
        }
        match &f.body {
            Some(block) => {
                self.w(" ");
                self.print_block(block);
                self.newline();
            }
            None => {
                self.w(";");
                self.newline();
            }
        }
    }

    fn print_params(&mut self, params: &[Param]) {
        for (i, p) in params.iter().enumerate() {
            if i > 0 {
                self.w(", ");
            }
            // `self` / `mut self` shorthand
            if p.name.as_str() == "self" && is_self_type(&p.ty) {
                if p.mutable {
                    self.w("mut self");
                } else {
                    self.w("self");
                }
                continue;
            }
            if p.mutable {
                self.w("mut ");
            }
            self.w(&p.name);
            self.w(": ");
            self.print_type(&p.ty);
            if let Some(default) = &p.default {
                self.w(" = ");
                self.print_expr(default);
            }
        }
    }

    fn print_class(&mut self, c: &ClassDecl) {
        self.write_indent();
        self.print_visibility(&c.visibility);
        match c.kind {
            ClassKind::Open => self.w("open "),
            ClassKind::Abstract => self.w("abstract "),
            ClassKind::Sealed => self.w("sealed "),
            ClassKind::Final => {}
        }
        self.w("class ");
        self.w(&c.name);
        self.print_generics(&c.generics);
        if !c.ctor_params.is_empty() {
            self.w("(");
            self.print_ctor_params(&c.ctor_params);
            self.w(")");
        }
        if !c.supertypes.is_empty() {
            self.w(" : ");
            for (i, s) in c.supertypes.iter().enumerate() {
                if i > 0 {
                    self.w(", ");
                }
                self.print_type(s);
            }
        }
        if c.body.is_empty() {
            self.w(" {}");
            self.newline();
        } else {
            self.w(" {");
            self.newline();
            self.indent += 1;
            let mut prev_end: Option<u32> = None;
            for member in &c.body {
                let span = class_member_span(member);
                // Preserve blank lines between members
                if let Some(pe) = prev_end {
                    if has_blank_line(self.source, pe, span.start) {
                        self.newline();
                    }
                }
                self.flush_comments_before(span.start);
                self.print_class_member(member);
                prev_end = Some(span.end);
            }
            self.indent -= 1;
            self.write_indent();
            self.w("}");
            self.newline();
        }
    }

    fn print_ctor_params(&mut self, params: &[CtorParam]) {
        for (i, p) in params.iter().enumerate() {
            if i > 0 {
                self.w(", ");
            }
            self.print_visibility(&p.visibility);
            if p.mutable {
                self.w("mut ");
            }
            self.w(&p.name);
            self.w(": ");
            self.print_type(&p.ty);
            if let Some(default) = &p.default {
                self.w(" = ");
                self.print_expr(default);
            }
        }
    }

    fn print_class_member(&mut self, member: &ClassMember) {
        match member {
            ClassMember::Field(f) => self.print_field(f),
            ClassMember::Method(m) => self.print_fn_decl(m, FnCtx::ClassMethod),
        }
    }

    fn print_field(&mut self, f: &FieldDecl) {
        self.write_indent();
        self.print_visibility(&f.visibility);
        if f.mutable {
            self.w("let mut ");
        } else {
            self.w("let ");
        }
        self.w(&f.name);
        self.w(": ");
        self.print_type(&f.ty);
        if let Some(init) = &f.init {
            self.w(" = ");
            self.print_expr(init);
        }
        self.w(";");
        self.newline();
    }

    fn print_data_class(&mut self, d: &DataClassDecl) {
        self.write_indent();
        self.print_visibility(&d.visibility);
        self.w("data class ");
        self.w(&d.name);
        self.print_generics(&d.generics);
        self.w("(");
        self.print_ctor_params(&d.ctor_params);
        self.w(");");
        self.newline();
    }

    fn print_enum(&mut self, e: &EnumDecl) {
        self.write_indent();
        self.print_visibility(&e.visibility);
        self.w("enum ");
        self.w(&e.name);
        self.print_generics(&e.generics);
        self.w(" {");
        self.newline();
        self.indent += 1;
        for v in &e.variants {
            self.flush_comments_before(v.span.start);
            self.print_enum_variant(v);
        }
        self.indent -= 1;
        self.write_indent();
        self.w("}");
        self.newline();
    }

    fn print_enum_variant(&mut self, v: &EnumVariant) {
        self.write_indent();
        self.w(&v.name);
        match &v.fields {
            EnumVariantFields::Unit => {}
            EnumVariantFields::Named(fields) => {
                self.w("(");
                for (i, f) in fields.iter().enumerate() {
                    if i > 0 {
                        self.w(", ");
                    }
                    self.w(&f.name);
                    self.w(": ");
                    self.print_type(&f.ty);
                }
                self.w(")");
            }
        }
        self.w(",");
        self.newline();
    }

    fn print_trait(&mut self, t: &TraitDecl) {
        self.write_indent();
        self.print_visibility(&t.visibility);
        if t.is_sealed {
            self.w("sealed ");
        }
        self.w("trait ");
        self.w(&t.name);
        self.print_generics(&t.generics);
        self.w(" {");
        self.newline();
        self.indent += 1;
        for item in &t.items {
            self.flush_comments_before(trait_item_span(item).start);
            self.print_trait_item(item);
        }
        self.indent -= 1;
        self.write_indent();
        self.w("}");
        self.newline();
    }

    fn print_trait_item(&mut self, item: &TraitItem) {
        match item {
            TraitItem::Fn(f) => self.print_fn_decl(f, FnCtx::TraitMethod),
            TraitItem::AssociatedType(a) => {
                self.write_indent();
                self.w("type ");
                self.w(&a.name);
                if let Some(default) = &a.default {
                    self.w(" = ");
                    self.print_type(default);
                }
                self.w(";");
                self.newline();
            }
        }
    }

    fn print_annotations(&mut self, annotations: &[valen_ast::Annotation]) {
        for ann in annotations {
            self.write_indent();
            self.w("@");
            self.w(&ann.name);
            if !ann.args.is_empty() {
                self.w("(");
                for (i, arg) in ann.args.iter().enumerate() {
                    if i > 0 {
                        self.w(", ");
                    }
                    if let Some(name) = &arg.name {
                        self.w(name);
                        self.w(" = ");
                    }
                    self.print_literal(&arg.value);
                }
                self.w(")");
            }
            self.newline();
        }
    }

    fn print_annotation_class(&mut self, a: &valen_ast::AnnotationClassDecl) {
        self.print_annotations(&a.annotations);
        self.write_indent();
        self.print_visibility(&a.visibility);
        self.w("annotation class ");
        self.w(&a.name);
        if !a.params.is_empty() {
            self.w("(");
            for (i, p) in a.params.iter().enumerate() {
                if i > 0 {
                    self.w(", ");
                }
                self.print_visibility(&p.visibility);
                self.w(&p.name);
                self.w(": ");
                self.print_type(&p.ty);
            }
            self.w(")");
        }
        self.newline();
    }

    fn print_impl_block(&mut self, i: &ImplBlock) {
        self.write_indent();
        self.w("impl");
        self.print_generics(&i.generics);
        self.w(" ");
        if let Some(tr) = &i.trait_ref {
            self.print_type(tr);
            self.w(" for ");
        }
        self.print_type(&i.target);
        self.w(" {");
        self.newline();
        self.indent += 1;
        for item in &i.items {
            self.flush_comments_before(impl_item_span(item).start);
            self.print_impl_item(item);
        }
        self.indent -= 1;
        self.write_indent();
        self.w("}");
        self.newline();
    }

    fn print_impl_item(&mut self, item: &ImplItem) {
        match item {
            ImplItem::Fn(f) => self.print_fn_decl(f, FnCtx::ImplMethod),
            ImplItem::AssociatedType(a) => {
                self.write_indent();
                self.w("type ");
                self.w(&a.name);
                self.w(" = ");
                self.print_type(&a.ty);
                self.w(";");
                self.newline();
            }
        }
    }

    fn print_type_alias(&mut self, t: &TypeAliasDecl) {
        self.write_indent();
        self.print_visibility(&t.visibility);
        self.w("typealias ");
        self.w(&t.name);
        self.print_generics(&t.generics);
        self.w(" = ");
        self.print_type(&t.ty);
        self.w(";");
        self.newline();
    }

    // ── Shared helpers ──────────────────────────────────────────────

    fn print_visibility(&mut self, v: &Visibility) {
        match v {
            Visibility::Pub => self.w("pub "),
            // `internal` is the default visibility — omit for conciseness
            Visibility::Internal => {}
            Visibility::Private => self.w("private "),
        }
    }

    fn print_generics(&mut self, generics: &[GenericParam]) {
        if generics.is_empty() {
            return;
        }
        self.w("<");
        for (i, g) in generics.iter().enumerate() {
            if i > 0 {
                self.w(", ");
            }
            match g.variance {
                Variance::Covariant => self.w("out "),
                Variance::Contravariant => self.w("in "),
                Variance::Invariant => {}
            }
            self.w(&g.name);
            if !g.bounds.is_empty() {
                self.w(": ");
                for (j, b) in g.bounds.iter().enumerate() {
                    if j > 0 {
                        self.w(" + ");
                    }
                    self.print_type(b);
                }
            }
        }
        self.w(">");
    }

    fn print_type(&mut self, ty: &Type) {
        match ty {
            Type::Path(p) => self.print_type_path(p),
            Type::Nullable { inner, .. } => {
                self.print_type(inner);
                self.w("?");
            }
            Type::Fn(f) => {
                self.w("fn(");
                for (i, p) in f.params.iter().enumerate() {
                    if i > 0 {
                        self.w(", ");
                    }
                    self.print_type(p);
                }
                self.w(") -> ");
                self.print_type(&f.return_type);
            }
            Type::Tuple(types) => {
                self.w("(");
                for (i, t) in types.iter().enumerate() {
                    if i > 0 {
                        self.w(", ");
                    }
                    self.print_type(t);
                }
                self.w(")");
            }
        }
    }

    fn print_type_path(&mut self, p: &TypePath) {
        for (i, seg) in p.segments.iter().enumerate() {
            if i > 0 {
                self.w(".");
            }
            self.w(&seg.name);
            if !seg.generics.is_empty() {
                self.w("<");
                for (j, g) in seg.generics.iter().enumerate() {
                    if j > 0 {
                        self.w(", ");
                    }
                    self.print_type(g);
                }
                self.w(">");
            }
        }
    }

    // ── Blocks and statements ───────────────────────────────────────

    fn print_block(&mut self, block: &Block) {
        self.w("{");
        if block.stmts.is_empty() && block.tail.is_none() {
            self.w("}");
            return;
        }
        self.newline();
        self.indent += 1;
        for stmt in &block.stmts {
            self.flush_comments_before(stmt_span(stmt).start);
            self.print_stmt(stmt);
        }
        if let Some(tail) = &block.tail {
            self.flush_comments_before(expr_span(tail).start);
            self.write_indent();
            self.print_expr(tail);
            self.newline();
        }
        self.indent -= 1;
        self.write_indent();
        self.w("}");
    }

    /// Compact block: `{ expr }` on one line for simple single-expression blocks,
    /// falls back to multi-line otherwise.
    fn print_block_compact(&mut self, block: &Block) {
        if block.stmts.is_empty() {
            if let Some(tail) = &block.tail {
                if is_simple_expr(tail) {
                    self.w("{ ");
                    self.print_expr(tail);
                    self.w(" }");
                    return;
                }
            }
        }
        self.print_block(block);
    }

    fn print_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let(l) => {
                self.write_indent();
                if l.mutable {
                    self.w("let mut ");
                } else {
                    self.w("let ");
                }
                self.w(&l.name);
                if let Some(ty) = &l.ty {
                    self.w(": ");
                    self.print_type(ty);
                }
                self.w(" = ");
                self.print_expr(&l.init);
                self.w(";");
                self.newline();
            }
            Stmt::Expr(e) => {
                self.write_indent();
                self.print_expr(e);
                self.newline();
            }
            Stmt::ExprSemi(e) => {
                self.write_indent();
                self.print_expr(e);
                if !expr_ends_with_block(e) {
                    self.w(";");
                }
                self.newline();
            }
        }
    }

    // ── Expressions ─────────────────────────────────────────────────

    fn print_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal(lit) => self.print_literal(lit),
            Expr::Path(p) => self.print_path(p),
            Expr::Call(c) => self.print_call(c),
            Expr::MethodCall(m) => self.print_method_call(m),
            Expr::Field(f) => self.print_field_access(f),
            Expr::Binary(b) => self.print_binary(b),
            Expr::Unary(u) => self.print_unary(u),
            Expr::Assign(a) => self.print_assign(a),
            Expr::If(i) => self.print_if(i),
            Expr::Match(m) => self.print_match(m),
            Expr::Block(b) => self.print_block(b),
            Expr::Return(r) => self.print_return(r),
            Expr::Break(b) => self.print_break(b),
            Expr::Continue(_) => self.w("continue"),
            Expr::For(f) => self.print_for(f),
            Expr::While(wh) => self.print_while(wh),
            Expr::Loop(l) => self.print_loop(l),
            Expr::Lambda(l) => self.print_lambda(l),
            Expr::Range(r) => self.print_range(r),
            Expr::Try(t) => self.print_try(t),
            Expr::StringInterp(s) => self.print_string_interp(s),
            Expr::Safe(s) => self.print_safe(s),
        }
    }

    fn print_literal(&mut self, lit: &Literal) {
        let span = literal_span(lit);
        // Prefer original source text when available to preserve formatting
        // (e.g. `1_000`, hex literals, escape sequences).
        let src = self.src(&span);
        if !src.is_empty() && span.start != span.end {
            self.w(src);
            return;
        }
        // Fallback: reconstruct from parsed value
        match lit {
            Literal::Int(v, _) => self.w(&v.to_string()),
            Literal::Long(v, _) => {
                self.w(&v.to_string());
                self.w("L");
            }
            Literal::Float(v, _) => self.w(&format!("{v}")),
            Literal::Double(v, _) => self.w(&format!("{v}")),
            Literal::Char(c, _) => self.w(&format!("'{c}'")),
            Literal::String(s, _) => {
                self.w("\"");
                self.w(&escape_string(s));
                self.w("\"");
            }
            Literal::Bool(b, _) => self.w(if *b { "true" } else { "false" }),
            Literal::Unit(_) => self.w("()"),
        }
    }

    fn print_path(&mut self, p: &Path) {
        for (i, seg) in p.segments.iter().enumerate() {
            if i > 0 {
                if seg.double_colon {
                    self.w("::");
                } else {
                    self.w(".");
                }
            }
            self.w(&seg.name);
            if !seg.generics.is_empty() {
                self.w("<");
                for (j, g) in seg.generics.iter().enumerate() {
                    if j > 0 {
                        self.w(", ");
                    }
                    self.print_type(g);
                }
                self.w(">");
            }
        }
    }

    fn print_call(&mut self, c: &CallExpr) {
        self.print_expr(&c.callee);
        self.w("(");
        self.print_call_args(&c.args);
        self.w(")");
    }

    fn print_call_args(&mut self, args: &[CallArg]) {
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                self.w(", ");
            }
            if let Some(name) = &arg.name {
                self.w(name);
                self.w(" = ");
            }
            self.print_expr(&arg.value);
        }
    }

    fn print_method_call(&mut self, m: &MethodCallExpr) {
        self.print_expr(&m.receiver);
        self.w(".");
        self.w(&m.method);
        if !m.generics.is_empty() {
            self.w("<");
            for (i, g) in m.generics.iter().enumerate() {
                if i > 0 {
                    self.w(", ");
                }
                self.print_type(g);
            }
            self.w(">");
        }
        self.w("(");
        self.print_call_args(&m.args);
        self.w(")");
    }

    fn print_field_access(&mut self, f: &FieldAccess) {
        self.print_expr(&f.receiver);
        self.w(".");
        self.w(&f.field);
    }

    fn print_binary(&mut self, b: &BinaryExpr) {
        let prec = bin_precedence(b.op);
        self.print_expr_with_prec(&b.lhs, prec, false);
        self.w(" ");
        self.w(bin_op_str(b.op));
        self.w(" ");
        self.print_expr_with_prec(&b.rhs, prec, true);
    }

    fn print_expr_with_prec(&mut self, expr: &Expr, parent_prec: u8, is_rhs: bool) {
        let needs_parens = match expr {
            Expr::Binary(b) => {
                let child_prec = bin_precedence(b.op);
                child_prec < parent_prec || (child_prec == parent_prec && is_rhs)
            }
            _ => false,
        };
        if needs_parens {
            self.w("(");
            self.print_expr(expr);
            self.w(")");
        } else {
            self.print_expr(expr);
        }
    }

    fn print_unary(&mut self, u: &UnaryExpr) {
        match u.op {
            UnaryOp::Neg => self.w("-"),
            UnaryOp::Not => self.w("!"),
        }
        self.print_expr(&u.expr);
    }

    fn print_assign(&mut self, a: &AssignExpr) {
        self.print_expr(&a.target);
        match &a.op {
            None => self.w(" = "),
            Some(op) => {
                self.w(" ");
                self.w(bin_op_str(*op));
                self.w("= ");
            }
        }
        self.print_expr(&a.value);
    }

    fn print_if(&mut self, i: &IfExpr) {
        self.w("if ");
        self.print_expr(&i.cond);
        self.w(" ");
        self.print_block(&i.then_branch);
        if let Some(else_branch) = &i.else_branch {
            self.w(" else ");
            match else_branch.as_ref() {
                Expr::If(nested) => self.print_if(nested),
                Expr::Block(b) => self.print_block(b),
                other => self.print_expr(other),
            }
        }
    }

    fn print_match(&mut self, m: &MatchExpr) {
        self.w("match ");
        self.print_expr(&m.scrutinee);
        self.w(" {");
        self.newline();
        self.indent += 1;
        for arm in &m.arms {
            self.flush_comments_before(arm.span.start);
            self.print_match_arm(arm);
        }
        self.indent -= 1;
        self.write_indent();
        self.w("}");
    }

    fn print_match_arm(&mut self, arm: &MatchArm) {
        self.write_indent();
        self.print_pattern(&arm.pattern);
        if let Some(guard) = &arm.guard {
            self.w(" if ");
            self.print_expr(guard);
        }
        self.w(" => ");
        match &arm.body {
            Expr::Block(b) => {
                self.print_block(b);
                self.w(",");
            }
            other => {
                self.print_expr(other);
                self.w(",");
            }
        }
        self.newline();
    }

    fn print_return(&mut self, r: &ReturnExpr) {
        self.w("return");
        if let Some(val) = &r.value {
            self.w(" ");
            self.print_expr(val);
        }
    }

    fn print_break(&mut self, b: &BreakExpr) {
        self.w("break");
        if let Some(val) = &b.value {
            self.w(" ");
            self.print_expr(val);
        }
    }

    fn print_for(&mut self, f: &ForExpr) {
        self.w("for ");
        self.w(&f.var);
        self.w(" in ");
        self.print_expr(&f.iter);
        self.w(" ");
        self.print_block(&f.body);
    }

    fn print_while(&mut self, wh: &WhileExpr) {
        self.w("while ");
        self.print_expr(&wh.cond);
        self.w(" ");
        self.print_block(&wh.body);
    }

    fn print_loop(&mut self, l: &LoopExpr) {
        self.w("loop ");
        self.print_block(&l.body);
    }

    fn print_lambda(&mut self, l: &LambdaExpr) {
        self.w("|");
        for (i, p) in l.params.iter().enumerate() {
            if i > 0 {
                self.w(", ");
            }
            self.w(&p.name);
            if let Some(ty) = &p.ty {
                self.w(": ");
                self.print_type(ty);
            }
        }
        self.w("|");
        if let Some(ret) = &l.return_type {
            self.w(" -> ");
            self.print_type(ret);
        }
        self.w(" ");
        match l.body.as_ref() {
            Expr::Block(b) => self.print_block_compact(b),
            other => self.print_expr(other),
        }
    }

    fn print_range(&mut self, r: &RangeExpr) {
        if let Some(start) = &r.start {
            self.print_expr(start);
        }
        if r.inclusive {
            self.w("..=");
        } else {
            self.w("..");
        }
        if let Some(end) = &r.end {
            self.print_expr(end);
        }
    }

    fn print_try(&mut self, t: &TryExpr) {
        self.print_expr(&t.expr);
        self.w("?");
    }

    fn print_string_interp(&mut self, s: &StringInterpExpr) {
        self.w("f\"");
        for part in &s.parts {
            match part {
                StringInterpPart::Text(t) => self.w(&escape_string(t)),
                StringInterpPart::Expr(e) => {
                    self.w("{");
                    self.print_expr(e);
                    self.w("}");
                }
            }
        }
        self.w("\"");
    }

    fn print_safe(&mut self, s: &SafeExpr) {
        self.w("safe ");
        self.print_block(&s.block);
    }

    // ── Patterns ────────────────────────────────────────────────────

    fn print_pattern(&mut self, pat: &Pattern) {
        match pat {
            Pattern::Wildcard(_) => self.w("_"),
            Pattern::Literal(lit) => self.print_literal(lit),
            Pattern::Binding(b) => {
                if b.mutable {
                    self.w("mut ");
                }
                self.w(&b.name);
            }
            Pattern::Path(p) => self.print_path(p),
            Pattern::Struct(s) => {
                self.print_path(&s.path);
                self.w("(");
                for (i, f) in s.fields.iter().enumerate() {
                    if i > 0 {
                        self.w(", ");
                    }
                    self.w(&f.name);
                    if let Some(sub) = &f.pattern {
                        self.w(": ");
                        self.print_pattern(sub);
                    }
                }
                if s.rest {
                    if !s.fields.is_empty() {
                        self.w(", ");
                    }
                    self.w("..");
                }
                self.w(")");
            }
            Pattern::Tuple(pats, _) => {
                self.w("(");
                for (i, p) in pats.iter().enumerate() {
                    if i > 0 {
                        self.w(", ");
                    }
                    self.print_pattern(p);
                }
                self.w(")");
            }
            Pattern::Range(r) => {
                if let Some(start) = &r.start {
                    self.print_literal(start);
                }
                if r.inclusive {
                    self.w("..=");
                } else {
                    self.w("..");
                }
                if let Some(end) = &r.end {
                    self.print_literal(end);
                }
            }
            Pattern::Or(pats, _) => {
                for (i, p) in pats.iter().enumerate() {
                    if i > 0 {
                        self.w(" | ");
                    }
                    self.print_pattern(p);
                }
            }
            Pattern::At(a) => {
                self.w(&a.name);
                self.w(" @ ");
                self.print_pattern(&a.pattern);
            }
        }
    }
}

// ── Free functions ──────────────────────────────────────────────────

fn escape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\0' => out.push_str("\\0"),
            _ => out.push(c),
        }
    }
    out
}

fn bin_op_str(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Rem => "%",
        BinaryOp::Eq => "==",
        BinaryOp::Ne => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::Le => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::Ge => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::Shl => "<<",
        BinaryOp::Shr => ">>",
        BinaryOp::RefEq => "===",
        BinaryOp::RefNe => "!==",
    }
}

fn bin_precedence(op: BinaryOp) -> u8 {
    match op {
        BinaryOp::Or => 1,
        BinaryOp::And => 2,
        BinaryOp::BitOr => 3,
        BinaryOp::BitXor => 4,
        BinaryOp::BitAnd => 5,
        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::RefEq | BinaryOp::RefNe => 6,
        BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => 7,
        BinaryOp::Shl | BinaryOp::Shr => 8,
        BinaryOp::Add | BinaryOp::Sub => 9,
        BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => 10,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ItemKind {
    Package,
    Import,
    Other,
}

fn item_kind(item: &Item) -> ItemKind {
    match item {
        Item::Package(_) => ItemKind::Package,
        Item::Import(_) => ItemKind::Import,
        _ => ItemKind::Other,
    }
}

fn import_sort_key(item: &Item) -> String {
    match item {
        Item::Import(i) => {
            let mut key = i
                .path
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(".");
            if let Some(alias) = &i.alias {
                key.push_str(" as ");
                key.push_str(alias);
            }
            key
        }
        _ => String::new(),
    }
}

fn item_span(item: &Item) -> Span {
    match item {
        Item::Package(p) => p.span,
        Item::Import(i) => i.span,
        Item::Fn(f) => f.span,
        Item::Class(c) => c.span,
        Item::DataClass(d) => d.span,
        Item::Enum(e) => e.span,
        Item::Trait(t) => t.span,
        Item::Impl(i) => i.span,
        Item::TypeAlias(t) => t.span,
        Item::AnnotationClass(a) => a.span,
    }
}

fn stmt_span(stmt: &Stmt) -> Span {
    match stmt {
        Stmt::Let(l) => l.span,
        Stmt::Expr(e) | Stmt::ExprSemi(e) => expr_span(e),
    }
}

fn expr_span(expr: &Expr) -> Span {
    match expr {
        Expr::Literal(l) => literal_span(l),
        Expr::Path(p) => p.span,
        Expr::Call(c) => c.span,
        Expr::MethodCall(m) => m.span,
        Expr::Field(f) => f.span,
        Expr::Binary(b) => b.span,
        Expr::Unary(u) => u.span,
        Expr::Assign(a) => a.span,
        Expr::If(i) => i.span,
        Expr::Match(m) => m.span,
        Expr::Block(b) => b.span,
        Expr::Return(r) => r.span,
        Expr::Break(b) => b.span,
        Expr::Continue(c) => c.span,
        Expr::For(f) => f.span,
        Expr::While(w) => w.span,
        Expr::Loop(l) => l.span,
        Expr::Lambda(l) => l.span,
        Expr::Range(r) => r.span,
        Expr::Try(t) => t.span,
        Expr::StringInterp(s) => s.span,
        Expr::Safe(s) => s.span,
    }
}

fn literal_span(lit: &Literal) -> Span {
    match lit {
        Literal::Int(_, s)
        | Literal::Long(_, s)
        | Literal::Float(_, s)
        | Literal::Double(_, s)
        | Literal::Char(_, s)
        | Literal::String(_, s)
        | Literal::Bool(_, s)
        | Literal::Unit(s) => *s,
    }
}

fn class_member_span(member: &ClassMember) -> Span {
    match member {
        ClassMember::Field(f) => f.span,
        ClassMember::Method(m) => m.span,
    }
}

fn trait_item_span(item: &TraitItem) -> Span {
    match item {
        TraitItem::Fn(f) => f.span,
        TraitItem::AssociatedType(a) => a.span,
    }
}

fn impl_item_span(item: &ImplItem) -> Span {
    match item {
        ImplItem::Fn(f) => f.span,
        ImplItem::AssociatedType(a) => a.span,
    }
}

fn has_blank_line(source: &str, from: u32, to: u32) -> bool {
    let slice = &source[from as usize..to as usize];
    slice.chars().filter(|&c| c == '\n').count() >= 2
}

fn expr_ends_with_block(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::If(_)
            | Expr::Match(_)
            | Expr::For(_)
            | Expr::While(_)
            | Expr::Loop(_)
            | Expr::Block(_)
            | Expr::Safe(_)
    )
}

fn is_simple_expr(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Literal(_)
            | Expr::Path(_)
            | Expr::Binary(_)
            | Expr::Unary(_)
            | Expr::Call(_)
            | Expr::MethodCall(_)
            | Expr::Field(_)
            | Expr::Try(_)
            | Expr::Range(_)
            | Expr::StringInterp(_)
            | Expr::Continue(_)
    )
}

fn is_self_type(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Path(TypePath { segments, .. })
            if segments.len() == 1
                && segments[0].name == "Self"
                && segments[0].generics.is_empty()
    )
}
