//! LSP backend state and LanguageServer omnitrait implementation.
//!
//! TODO(#063): This file is a monolith (~3 000 lines). Split into focused modules:
//!   - `completion.rs`   — completion provider (~700 lines)
//!   - `hover.rs`        — hover provider
//!   - `semantic_tokens.rs` — semantic token encoding
//!   - `inlay_hints.rs`  — inlay hints (~230 lines)
//!   - `helpers.rs`      — shared utilities (~600 lines)

use std::collections::HashMap;
use std::ops::ControlFlow;

use async_lsp::lsp_types::*;
use async_lsp::router::Router;
use async_lsp::{ClientSocket, LanguageClient, LanguageServer, ResponseError};

use valen_ast::token::TokenKind;
use valen_hir::{DefKind, Ty, TypedBody, TypedExpr, TypedExprKind, TypedStmt};

use crate::convert;

// ---------------------------------------------------------------------------
// Semantic token type indices (must match the legend order in `initialize`)
// ---------------------------------------------------------------------------
const ST_KEYWORD: u32 = 0;
const ST_TYPE: u32 = 1;
#[allow(dead_code)]
const ST_FUNCTION: u32 = 2;
const ST_VARIABLE: u32 = 3;
const ST_STRING: u32 = 4;
const ST_NUMBER: u32 = 5;
const ST_COMMENT: u32 = 6;
#[allow(dead_code)]
const ST_PARAMETER: u32 = 7;

/// Per-document analysis state.
pub struct DocumentState {
    pub text: String,
    pub line_index: convert::LineIndex,
    pub items: Vec<valen_ast::Item>,
    pub hir: Option<valen_hir::Hir>,
    /// Typed bodies from type checking, indexed by DefId.
    pub bodies: Option<indexmap::IndexMap<valen_hir::DefId, valen_hir::TypedBody>>,
    /// FileId for this document, used to filter cross-file data.
    pub file_id: valen_ast::FileId,
}

/// The Valen LSP server state.
pub struct ServerState {
    client: ClientSocket,
    documents: HashMap<Url, DocumentState>,
    /// Maps document URIs to unique FileIds for per-document identification.
    file_ids: HashMap<Url, valen_ast::FileId>,
    /// Counter for allocating new FileIds.
    next_file_id: u32,
    workspace_root: Option<std::path::PathBuf>,
    /// Classpath entries for Java interop resolution (JDK jmods/jars).
    classpath: Vec<std::path::PathBuf>,
    /// Semantic diagnostics from the last cross-file analysis pass.
    last_semantic_diags: Vec<valen_diagnostics::Diagnostic>,
}

impl ServerState {
    pub fn new_router(client: ClientSocket) -> Router<Self> {
        let this = Self {
            client,
            documents: HashMap::new(),
            file_ids: HashMap::new(),
            next_file_id: 0,
            workspace_root: None,
            classpath: valen_hir::classpath::detect_jdk_classpath(),
            last_semantic_diags: Vec::new(),
        };
        let mut router = Router::from_language_server(this);
        router.event(Self::on_event);
        router
    }

    fn on_event(
        _state: &mut Self,
        _event: async_lsp::AnyEvent,
    ) -> ControlFlow<async_lsp::Result<()>> {
        ControlFlow::Continue(())
    }

    /// Allocate or retrieve a stable FileId for the given URI.
    fn file_id_for(&mut self, uri: &Url) -> valen_ast::FileId {
        if let Some(&id) = self.file_ids.get(uri) {
            return id;
        }
        let id = valen_ast::FileId(self.next_file_id);
        self.next_file_id += 1;
        self.file_ids.insert(uri.clone(), id);
        id
    }

    fn index_workspace(&mut self, root: &std::path::Path) {
        let vln_files = find_vln_files(root);
        for path in vln_files {
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Ok(uri) = Url::from_file_path(&path) {
                    if !self.documents.contains_key(&uri) {
                        let file_id = self.file_id_for(&uri);
                        let line_index = convert::LineIndex::new(&text);
                        let parse_result = valen_parser::parse(&text, file_id);
                        self.documents.insert(
                            uri,
                            DocumentState {
                                text,
                                line_index,
                                items: parse_result.items,
                                hir: None,
                                bodies: None,
                                file_id,
                            },
                        );
                    }
                }
            }
        }
        self.run_cross_file_analysis();
    }

    /// Re-parse the changed file, then run a combined resolve + type-check
    /// across ALL workspace documents so cross-file types are visible.
    fn analyze_and_publish(&mut self, uri: Url, text: String, version: i32) {
        let file_id = self.file_id_for(&uri);
        let line_index = convert::LineIndex::new(&text);
        let parse_result = valen_parser::parse(&text, file_id);
        let parse_diags = convert::to_lsp_diagnostics(&parse_result.diagnostics, &line_index);
        self.documents.insert(
            uri.clone(),
            DocumentState {
                text,
                line_index,
                items: parse_result.items,
                hir: None,
                bodies: None,
                file_id,
            },
        );

        self.run_cross_file_analysis();

        // Publish diagnostics for changed document
        let mut diags = parse_diags;
        diags.extend(self.semantic_diags_for(&uri));
        self.client
            .publish_diagnostics(PublishDiagnosticsParams {
                uri: uri.clone(),
                diagnostics: diags,
                version: Some(version),
            })
            .ok();

        // Re-publish diagnostics for other documents
        let other_uris: Vec<Url> = self
            .documents
            .keys()
            .filter(|u| **u != uri)
            .cloned()
            .collect();
        for other_uri in other_uris {
            let other_fid = self.file_id_for(&other_uri);
            let other_diags = if let Some(doc) = self.documents.get(&other_uri) {
                let mut d = convert::to_lsp_diagnostics(
                    &valen_parser::parse(&doc.text, other_fid).diagnostics,
                    &doc.line_index,
                );
                d.extend(self.semantic_diags_for(&other_uri));
                d
            } else {
                vec![]
            };
            self.client
                .publish_diagnostics(PublishDiagnosticsParams {
                    uri: other_uri,
                    diagnostics: other_diags,
                    version: None,
                })
                .ok();
        }
    }

    /// Combine all workspace document items and run a single resolve + type check pass.
    fn run_cross_file_analysis(&mut self) {
        let mut all_items: Vec<valen_ast::Item> = Vec::new();
        for doc in self.documents.values() {
            all_items.extend(doc.items.iter().cloned());
        }

        let resolve_result =
            valen_hir::resolve::resolve_with_classpath(&all_items, &self.classpath);
        let coherence_result = valen_hir::coherence::check_coherence(&resolve_result.hir, &[]);
        let exhaustive_result =
            valen_hir::exhaustive::check_exhaustiveness(&resolve_result.hir, &all_items);
        let tc = valen_hir::ty::type_check(&resolve_result.hir, &all_items);

        let mut all_semantic_diags = Vec::new();
        all_semantic_diags.extend(resolve_result.diagnostics.iter().cloned());
        all_semantic_diags.extend(coherence_result.diagnostics.iter().cloned());
        all_semantic_diags.extend(exhaustive_result.diagnostics.iter().cloned());
        all_semantic_diags.extend(tc.diagnostics.iter().cloned());
        self.last_semantic_diags = all_semantic_diags;

        let hir = resolve_result.hir;
        let bodies = tc.bodies;
        for doc in self.documents.values_mut() {
            doc.hir = Some(hir.clone());
            doc.bodies = Some(bodies.clone());
        }
    }

    /// Get LSP diagnostics from the last semantic analysis that belong to a specific file.
    fn semantic_diags_for(&self, uri: &Url) -> Vec<async_lsp::lsp_types::Diagnostic> {
        let Some(fid) = self.file_ids.get(uri) else {
            return vec![];
        };
        let Some(doc) = self.documents.get(uri) else {
            return vec![];
        };
        let filtered: Vec<_> = self
            .last_semantic_diags
            .iter()
            .filter(|d| d.primary.file_id == *fid)
            .cloned()
            .collect();
        let temp = valen_diagnostics::Diagnostics::from_vec(filtered);
        convert::to_lsp_diagnostics(&temp, &doc.line_index)
    }

    // TODO(#039): Goto definition currently resolves by name only via linear
    // scan. Should prioritize definitions whose scope contains the cursor
    // position using HIR scope info once available.
    fn find_definition_at(&self, uri: &Url, position: Position) -> Option<GotoDefinitionResponse> {
        let doc = self.documents.get(uri)?;
        let offset = doc.line_index.position_to_offset(position);
        let name = extract_word_at(&doc.text, offset)?;

        // Search current document first, preferring definitions whose span
        // encloses the cursor (heuristic for scope proximity).
        if let Some(hir) = doc.hir.as_ref() {
            let mut best: Option<&valen_hir::Def> = None;
            for def in hir.defs.values() {
                if def.name.as_str() == name {
                    match best {
                        None => best = Some(def),
                        Some(prev) => {
                            // Prefer the def whose span is closest to (and contains) the cursor
                            let prev_contains =
                                prev.span.start <= offset && offset <= prev.span.end;
                            let this_contains = def.span.start <= offset && offset <= def.span.end;
                            if this_contains && !prev_contains {
                                best = Some(def);
                            } else if this_contains
                                && prev_contains
                                && def.span.len() < prev.span.len()
                            {
                                // Tighter enclosing scope wins
                                best = Some(def);
                            }
                        }
                    }
                }
            }
            if let Some(def) = best {
                let range = doc.line_index.span_to_range(def.span);
                return Some(GotoDefinitionResponse::Scalar(Location {
                    uri: uri.clone(),
                    range,
                }));
            }
        }

        // Cross-file: search other documents
        for (other_uri, other_doc) in &self.documents {
            if other_uri == uri {
                continue;
            }
            if let Some(hir) = other_doc.hir.as_ref() {
                for def in hir.defs.values() {
                    if def.name.as_str() == name {
                        let range = other_doc.line_index.span_to_range(def.span);
                        return Some(GotoDefinitionResponse::Scalar(Location {
                            uri: other_uri.clone(),
                            range,
                        }));
                    }
                }
            }
        }
        None
    }

    // -----------------------------------------------------------------------
    // Completion helpers
    // -----------------------------------------------------------------------

    fn build_completions(&self, uri: &Url, pos: Position) -> Vec<CompletionItem> {
        let doc = match self.documents.get(uri) {
            Some(d) => d,
            None => return Vec::new(),
        };

        let offset = doc.line_index.position_to_offset(pos) as usize;
        let before = &doc.text[..offset.min(doc.text.len())];

        if is_double_colon_context(before) {
            return self.build_path_completions(doc, before);
        }

        if is_dot_context(before) {
            return self.build_dot_completions(doc, before);
        }

        match detect_context(before) {
            CompletionContext::TypePosition => self.build_type_completions(doc),
            CompletionContext::ImplTarget => self.build_impl_target_completions(doc),
            CompletionContext::ImplTraitPosition => self.build_impl_trait_completions(doc),
            CompletionContext::ImportPath => self.build_import_path_completions(doc, before),
            CompletionContext::NamingPosition => Vec::new(),
            CompletionContext::General => self.build_general_completions(doc, offset as u32),
        }
    }

    fn build_path_completions(&self, doc: &DocumentState, before: &str) -> Vec<CompletionItem> {
        let mut items = Vec::new();
        let hir = match doc.hir.as_ref() {
            Some(h) => h,
            None => return items,
        };

        let name = extract_name_before_double_colon(before);

        for def in hir.defs.values() {
            if def.name.as_str() != name {
                continue;
            }
            match &def.kind {
                // Enum variants
                DefKind::Enum(e) => {
                    for variant in &e.variants {
                        let detail = if variant.fields.is_empty() {
                            None
                        } else {
                            let fs: Vec<String> = variant
                                .fields
                                .iter()
                                .map(|(n, t)| format!("{n}: {t}"))
                                .collect();
                            Some(format!("({})", fs.join(", ")))
                        };
                        // Variant documentation: show which enum it belongs to
                        let documentation = {
                            let mut md = format!("```valen\n{name}::{}", variant.name);
                            if !variant.fields.is_empty() {
                                let fs: Vec<String> = variant
                                    .fields
                                    .iter()
                                    .map(|(n, t)| format!("{n}: {t}"))
                                    .collect();
                                md.push_str(&format!("({})", fs.join(", ")));
                            }
                            md.push_str("\n```\n");
                            Some(Documentation::MarkupContent(MarkupContent {
                                kind: MarkupKind::Markdown,
                                value: md,
                            }))
                        };
                        items.push(CompletionItem {
                            label: variant.name.to_string(),
                            kind: Some(CompletionItemKind::ENUM_MEMBER),
                            detail,
                            documentation,
                            ..Default::default()
                        });
                    }
                }
                // Class associated functions (no self param)
                DefKind::Class(c) => {
                    for &mid in &c.methods {
                        if let Some(mdef) = hir.defs.get(&mid) {
                            if let DefKind::Fn(f) = &mdef.kind {
                                let has_self = f.params.first().is_some_and(|p| p.is_self);
                                if !has_self {
                                    let documentation = build_fn_documentation(
                                        &mdef.name,
                                        f,
                                        &doc.text,
                                        mdef.span.start,
                                    );
                                    items.push(CompletionItem {
                                        label: mdef.name.to_string(),
                                        kind: Some(CompletionItemKind::FUNCTION),
                                        detail: Some(format_fn_signature(&mdef.name, f)),
                                        documentation,
                                        ..Default::default()
                                    });
                                }
                            }
                        }
                    }
                }
                // Trait methods (UFCS)
                DefKind::Trait(t) => {
                    for &mid in &t.methods {
                        if let Some(mdef) = hir.defs.get(&mid) {
                            if let DefKind::Fn(f) = &mdef.kind {
                                let documentation = build_fn_documentation(
                                    &mdef.name,
                                    f,
                                    &doc.text,
                                    mdef.span.start,
                                );
                                items.push(CompletionItem {
                                    label: mdef.name.to_string(),
                                    kind: Some(CompletionItemKind::METHOD),
                                    detail: Some(format_fn_signature(&mdef.name, f)),
                                    documentation,
                                    ..Default::default()
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        items
    }

    fn build_dot_completions(&self, doc: &DocumentState, before: &str) -> Vec<CompletionItem> {
        let mut items = Vec::new();
        let hir = match doc.hir.as_ref() {
            Some(h) => h,
            None => return items,
        };

        let receiver = extract_receiver_before_dot(before);
        let type_name = self.resolve_receiver_type(doc, hir, receiver, before);

        if let Some(tn) = &type_name {
            // Fields from class/data class ctor params
            for def in hir.defs.values() {
                match &def.kind {
                    DefKind::Class(c) if def.name.as_str() == tn => {
                        for param in &c.ctor_params {
                            let documentation =
                                build_variable_documentation(&param.name, &param.ty);
                            items.push(CompletionItem {
                                label: param.name.to_string(),
                                kind: Some(CompletionItemKind::FIELD),
                                detail: Some(format!("{}", param.ty)),
                                documentation,
                                ..Default::default()
                            });
                        }
                    }
                    DefKind::DataClass(dc) if def.name.as_str() == tn => {
                        for param in &dc.ctor_params {
                            let documentation =
                                build_variable_documentation(&param.name, &param.ty);
                            items.push(CompletionItem {
                                label: param.name.to_string(),
                                kind: Some(CompletionItemKind::FIELD),
                                detail: Some(format!("{}", param.ty)),
                                documentation,
                                ..Default::default()
                            });
                        }
                    }
                    _ => {}
                }
            }

            // Methods from class body
            if let Some(methods) = hir.type_methods.get(tn.as_str()) {
                for &mid in methods {
                    if let Some(mdef) = hir.defs.get(&mid) {
                        let (detail, documentation) = if let DefKind::Fn(f) = &mdef.kind {
                            (
                                Some(format_fn_signature_with_owner(
                                    &mdef.name,
                                    f,
                                    Some(tn.as_str()),
                                )),
                                build_fn_documentation(&mdef.name, f, &doc.text, mdef.span.start),
                            )
                        } else {
                            (None, None)
                        };
                        items.push(CompletionItem {
                            label: mdef.name.to_string(),
                            kind: Some(CompletionItemKind::METHOD),
                            detail,
                            documentation,
                            ..Default::default()
                        });
                    }
                }
            }

            // Methods from trait impls
            for entry in &hir.trait_impls {
                if entry.target_name.as_str() == tn {
                    for &mid in &entry.methods {
                        if let Some(mdef) = hir.defs.get(&mid) {
                            let (detail, documentation) = if let DefKind::Fn(f) = &mdef.kind {
                                (
                                    Some(format_fn_signature_with_owner(
                                        &mdef.name,
                                        f,
                                        Some(tn.as_str()),
                                    )),
                                    build_fn_documentation(
                                        &mdef.name,
                                        f,
                                        &doc.text,
                                        mdef.span.start,
                                    ),
                                )
                            } else {
                                (None, None)
                            };
                            items.push(CompletionItem {
                                label: mdef.name.to_string(),
                                kind: Some(CompletionItemKind::METHOD),
                                detail,
                                documentation,
                                ..Default::default()
                            });
                        }
                    }
                }
            }
        }

        // Variant shorthand: when `.` has no receiver (e.g., `= .` or `=> .`),
        // suggest enum variants filtered by expected type when determinable.
        if type_name.is_none() && is_variant_shorthand_context(before) {
            let expected_enum = infer_expected_enum_type(before, hir);
            for def in hir.defs.values() {
                if let DefKind::Enum(e) = &def.kind {
                    if let Some(ref expected) = expected_enum {
                        if def.name.as_str() != expected {
                            continue;
                        }
                    }
                    for v in &e.variants {
                        let detail = if v.fields.is_empty() {
                            format!("{}.{}", def.name, v.name)
                        } else {
                            let fields: Vec<String> = v
                                .fields
                                .iter()
                                .map(|(name, ty)| format!("{name}: {ty}"))
                                .collect();
                            format!("{}.{}({})", def.name, v.name, fields.join(", "))
                        };
                        items.push(CompletionItem {
                            label: v.name.to_string(),
                            kind: Some(CompletionItemKind::ENUM_MEMBER),
                            detail: Some(detail),
                            ..Default::default()
                        });
                    }
                }
            }
        }

        // Deduplicate
        items.sort_by(|a, b| a.label.cmp(&b.label));
        items.dedup_by(|a, b| a.label == b.label);
        items
    }

    fn resolve_receiver_type(
        &self,
        doc: &DocumentState,
        hir: &valen_hir::Hir,
        receiver: &str,
        before: &str,
    ) -> Option<String> {
        if receiver == "self" {
            return self.find_enclosing_type(doc, before);
        }

        // Check if receiver is a type name directly
        for def in hir.defs.values() {
            if def.name.as_str() == receiver {
                match &def.kind {
                    DefKind::Class(_) | DefKind::DataClass(_) | DefKind::Enum(_) => {
                        return Some(receiver.to_string());
                    }
                    _ => {}
                }
            }
        }

        // Look up variable type from typed bodies (handles inferred types)
        let offset = before.len() as u32;
        if let Some(bodies) = doc.bodies.as_ref() {
            let locals = collect_local_variables(bodies, offset, doc.hir.as_ref());
            for (name, ty) in &locals {
                if name == receiver {
                    return ty_to_type_name(ty);
                }
            }
        }

        // Look up variable type from fn params
        for def in hir.defs.values() {
            if let DefKind::Fn(f) = &def.kind {
                for param in &f.params {
                    if param.name.as_str() == receiver {
                        return tyref_to_type_name(&param.ty);
                    }
                }
            }
        }

        // Look up variable type from let bindings in source (heuristic)
        if let Some(ty) = find_let_type_annotation(&doc.text, receiver) {
            return Some(ty);
        }

        None
    }

    fn find_enclosing_type(&self, _doc: &DocumentState, before: &str) -> Option<String> {
        find_enclosing_type_from_source(before)
    }

    fn build_type_completions(&self, doc: &DocumentState) -> Vec<CompletionItem> {
        let mut items = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for &(ty, desc) in BUILTIN_TYPES {
            let label = ty.to_string();
            if seen.insert(label.clone()) {
                items.push(CompletionItem {
                    label,
                    kind: Some(CompletionItemKind::TYPE_PARAMETER),
                    detail: Some(desc.to_string()),
                    ..Default::default()
                });
            }
        }

        if let Some(hir) = doc.hir.as_ref() {
            for def in hir.defs.values() {
                if hir.prelude_ids.contains(&def.id) || def.name.is_empty() {
                    continue;
                }
                let (kind, documentation) = match &def.kind {
                    DefKind::Class(c) => (
                        CompletionItemKind::CLASS,
                        build_class_documentation(
                            &def.name,
                            &c.ctor_params,
                            c.superclass.as_ref(),
                            hir,
                            &doc.text,
                            def.span.start,
                        ),
                    ),
                    DefKind::DataClass(dc) => (
                        CompletionItemKind::CLASS,
                        build_class_documentation(
                            &def.name,
                            &dc.ctor_params,
                            None,
                            hir,
                            &doc.text,
                            def.span.start,
                        ),
                    ),
                    DefKind::Enum(e) => (
                        CompletionItemKind::ENUM,
                        build_enum_documentation(&def.name, e, &doc.text, def.span.start),
                    ),
                    DefKind::Trait(t) => (
                        CompletionItemKind::INTERFACE,
                        build_trait_documentation(&def.name, t, hir, &doc.text, def.span.start),
                    ),
                    DefKind::TypeAlias(_) | DefKind::NewType(_) => {
                        (CompletionItemKind::CLASS, None)
                    }
                    _ => continue,
                };
                let label = def.name.to_string();
                if seen.insert(label.clone()) {
                    items.push(CompletionItem {
                        label,
                        kind: Some(kind),
                        documentation,
                        ..Default::default()
                    });
                }
            }

            // Generic type params from fn defs
            for def in hir.defs.values() {
                if let DefKind::Fn(f) = &def.kind {
                    for (tp, bounds) in &f.generic_bounds {
                        let label = tp.to_string();
                        if seen.insert(label.clone()) {
                            let detail = if bounds.is_empty() {
                                None
                            } else {
                                Some(format!("{tp}: {}", bounds.join(" + ")))
                            };
                            let bounds_str = bounds.join(" + ");
                            let documentation = build_type_param_documentation(tp, &bounds_str);
                            items.push(CompletionItem {
                                label,
                                kind: Some(CompletionItemKind::TYPE_PARAMETER),
                                detail,
                                documentation,
                                ..Default::default()
                            });
                        }
                    }
                }
            }
        }

        items
    }

    fn build_impl_target_completions(&self, doc: &DocumentState) -> Vec<CompletionItem> {
        let mut items = Vec::new();

        if let Some(hir) = doc.hir.as_ref() {
            for def in hir.defs.values() {
                if hir.prelude_ids.contains(&def.id) || def.name.is_empty() {
                    continue;
                }
                match &def.kind {
                    DefKind::Class(c) => {
                        let sig = format_class_signature(&def.name, &c.ctor_params);
                        let documentation = build_class_documentation(
                            &def.name,
                            &c.ctor_params,
                            c.superclass.as_ref(),
                            hir,
                            &doc.text,
                            def.span.start,
                        );
                        items.push(CompletionItem {
                            label: def.name.to_string(),
                            kind: Some(CompletionItemKind::CLASS),
                            detail: Some(sig),
                            documentation,
                            ..Default::default()
                        });
                    }
                    DefKind::DataClass(dc) => {
                        let sig = format_class_signature(&def.name, &dc.ctor_params);
                        let documentation = build_class_documentation(
                            &def.name,
                            &dc.ctor_params,
                            None,
                            hir,
                            &doc.text,
                            def.span.start,
                        );
                        items.push(CompletionItem {
                            label: def.name.to_string(),
                            kind: Some(CompletionItemKind::CLASS),
                            detail: Some(sig),
                            documentation,
                            ..Default::default()
                        });
                    }
                    DefKind::Enum(e) => {
                        let variants: Vec<String> =
                            e.variants.iter().map(|v| v.name.to_string()).collect();
                        items.push(CompletionItem {
                            label: def.name.to_string(),
                            kind: Some(CompletionItemKind::ENUM),
                            detail: Some(format!(
                                "enum {} {{ {} }}",
                                def.name,
                                variants.join(", ")
                            )),
                            ..Default::default()
                        });
                    }
                    _ => {}
                }
            }
        }

        items
    }

    /// Issue #038: Show only trait definitions after `impl `.
    fn build_impl_trait_completions(&self, doc: &DocumentState) -> Vec<CompletionItem> {
        let mut items = Vec::new();

        if let Some(hir) = doc.hir.as_ref() {
            for def in hir.defs.values() {
                if hir.prelude_ids.contains(&def.id) || def.name.is_empty() {
                    continue;
                }
                if let DefKind::Trait(t) = &def.kind {
                    let documentation =
                        build_trait_documentation(&def.name, t, hir, &doc.text, def.span.start);
                    items.push(CompletionItem {
                        label: def.name.to_string(),
                        kind: Some(CompletionItemKind::INTERFACE),
                        detail: Some(format!("trait {}", def.name)),
                        documentation,
                        ..Default::default()
                    });
                }
            }
        }

        items
    }

    /// Issue #039: Provide completions inside `import` statements.
    ///
    /// TODO(#039): Currently only suggests known package segments from the
    /// current file's HIR imports and foreign types. A full implementation
    /// would scan the workspace for all available packages, walk the classpath
    /// for Java packages, and support wildcard imports.
    ///
    /// TODO(#040): Java stdlib completion should integrate here, providing
    /// `java.lang.*`, `java.util.*`, `java.io.*` etc. as import candidates.
    /// This requires building a Java stdlib index (class names, methods,
    /// fields) from JDK class files or a pre-computed database.
    fn build_import_path_completions(
        &self,
        doc: &DocumentState,
        _before: &str,
    ) -> Vec<CompletionItem> {
        let mut items = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // Suggest known import paths from the HIR
        if let Some(hir) = doc.hir.as_ref() {
            for (short_name, segments) in &hir.imports {
                let full_path = segments
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                if seen.insert(full_path.clone()) {
                    items.push(CompletionItem {
                        label: short_name.to_string(),
                        kind: Some(CompletionItemKind::MODULE),
                        detail: Some(full_path),
                        ..Default::default()
                    });
                }
            }

            // Suggest foreign types as potential imports
            for (name, _info) in &hir.foreign_types {
                if seen.insert(name.to_string()) {
                    items.push(CompletionItem {
                        label: name.to_string(),
                        kind: Some(CompletionItemKind::CLASS),
                        detail: Some(format!("(Java) {name}")),
                        ..Default::default()
                    });
                }
            }
        }

        // Also suggest imports from other open documents
        for (other_uri, other_doc) in &self.documents {
            let _ = other_uri;
            if let Some(hir) = other_doc.hir.as_ref() {
                if let Some(pkg) = &hir.package {
                    let pkg_path = pkg.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(".");
                    if seen.insert(pkg_path.clone()) {
                        items.push(CompletionItem {
                            label: pkg_path,
                            kind: Some(CompletionItemKind::MODULE),
                            detail: Some("(package)".to_string()),
                            ..Default::default()
                        });
                    }
                }
            }
        }

        items
    }

    // TODO(#040): Add Java stdlib completion. Needs a pre-built index of
    // common Java types and methods (java.lang.*, java.util.*, java.io.*)
    // that can be offered as completions alongside Valen definitions.
    // The index could be generated from JDK class files at build time
    // or loaded from a bundled JSON/binary database.
    //
    // TODO(#041): Include package information in completion items. The HIR
    // already stores `package: Option<Vec<SmolStr>>` per document. For
    // cross-file completions, the detail field should include the source
    // package (e.g., `com.example.MyClass`) so users can distinguish
    // identically-named types from different packages.
    //
    // TODO(#043): Generate trait method stubs inside `impl Trait for Type {}`
    // blocks. Needs: (1) detect that cursor is inside an impl block body,
    // (2) look up the trait being implemented, (3) find which methods are
    // already implemented, (4) offer snippet completions for the remaining
    // unimplemented methods with full signatures.
    fn build_general_completions(&self, doc: &DocumentState, offset: u32) -> Vec<CompletionItem> {
        let mut items = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // Local variables from typed bodies (sorted above keywords)
        if let Some(bodies) = doc.bodies.as_ref() {
            let locals = collect_local_variables(bodies, offset, doc.hir.as_ref());
            for (name, ty) in locals {
                if seen.insert(name.clone()) {
                    let documentation = build_variable_documentation(&name, &ty);
                    items.push(CompletionItem {
                        label: name,
                        kind: Some(CompletionItemKind::VARIABLE),
                        detail: Some(format!("{ty}")),
                        sort_text: Some(format!("0_{}", items.len())),
                        documentation,
                        ..Default::default()
                    });
                }
            }
        }

        // Fallback: extract let bindings from source text before the cursor.
        // Covers the case where parse errors cause the enclosing function to
        // be dropped from the AST (no typed body available).
        let before = &doc.text[..offset.min(doc.text.len() as u32) as usize];
        for local in extract_let_names_from_text(before) {
            if seen.insert(local.name.clone()) {
                items.push(CompletionItem {
                    label: local.name,
                    kind: Some(CompletionItemKind::VARIABLE),
                    detail: local.ty,
                    sort_text: Some(format!("0_{}", items.len())),
                    ..Default::default()
                });
            }
        }

        for kw in EXPR_KEYWORDS {
            if seen.insert(kw.to_string()) {
                items.push(CompletionItem {
                    label: kw.to_string(),
                    kind: Some(CompletionItemKind::KEYWORD),
                    ..Default::default()
                });
            }
        }

        if let Some(hir) = doc.hir.as_ref() {
            for def in hir.defs.values() {
                if def.name.is_empty() {
                    continue;
                }
                // Issue #033: Include prelude functions (println, print, etc.)
                // in completions with a lower sort priority so users can
                // discover built-in functions.
                let is_prelude =
                    hir.prelude_ids.contains(&def.id) && matches!(&def.kind, DefKind::Fn(_));
                let (label, kind, detail, documentation, sort_prefix) = match &def.kind {
                    DefKind::Fn(f) => {
                        let sig = format_fn_signature(&def.name, f);
                        let doc_md =
                            build_fn_documentation(&def.name, f, &doc.text, def.span.start);
                        (
                            def.name.to_string(),
                            CompletionItemKind::FUNCTION,
                            Some(sig),
                            doc_md,
                            "1",
                        )
                    }
                    DefKind::Class(c) => {
                        let sig = format_class_signature(&def.name, &c.ctor_params);
                        let doc_md = build_class_documentation(
                            &def.name,
                            &c.ctor_params,
                            c.superclass.as_ref(),
                            hir,
                            &doc.text,
                            def.span.start,
                        );
                        (
                            def.name.to_string(),
                            CompletionItemKind::CLASS,
                            Some(sig),
                            doc_md,
                            "1",
                        )
                    }
                    DefKind::DataClass(dc) => {
                        let sig = format_class_signature(&def.name, &dc.ctor_params);
                        let doc_md = build_class_documentation(
                            &def.name,
                            &dc.ctor_params,
                            None,
                            hir,
                            &doc.text,
                            def.span.start,
                        );
                        (
                            def.name.to_string(),
                            CompletionItemKind::CLASS,
                            Some(sig),
                            doc_md,
                            "1",
                        )
                    }
                    DefKind::Enum(e) => {
                        let variants: Vec<&str> =
                            e.variants.iter().map(|v| v.name.as_str()).collect();
                        let doc_md =
                            build_enum_documentation(&def.name, e, &doc.text, def.span.start);
                        (
                            def.name.to_string(),
                            CompletionItemKind::ENUM,
                            Some(format!("{{ {} }}", variants.join(", "))),
                            doc_md,
                            "1",
                        )
                    }
                    DefKind::Trait(t) => {
                        let doc_md =
                            build_trait_documentation(&def.name, t, hir, &doc.text, def.span.start);
                        (
                            def.name.to_string(),
                            CompletionItemKind::INTERFACE,
                            Some(format!("trait {}", def.name)),
                            doc_md,
                            "8",
                        )
                    }
                    // typealias / annotation not usable as expressions
                    DefKind::TypeAlias(_) | DefKind::NewType(_) | DefKind::AnnotationClass(_) => {
                        continue
                    }
                    DefKind::Impl(_) => continue,
                };
                if seen.insert(label.clone()) {
                    // Issue #033: Prelude functions get sort prefix "9" so
                    // they appear after user-defined items but before nothing.
                    let effective_prefix = if is_prelude { "9" } else { sort_prefix };
                    items.push(CompletionItem {
                        label,
                        kind: Some(kind),
                        detail,
                        documentation,
                        sort_text: Some(format!("{effective_prefix}_{}", def.name)),
                        ..Default::default()
                    });
                }
            }

            // Issue #045: Only offer `self` when the cursor is inside a
            // method that actually has a self parameter.
            let has_self = hir.defs.values().any(|d| {
                d.span.start <= offset
                    && offset < d.span.end
                    && matches!(&d.kind, DefKind::Fn(f) if f.params.first().is_some_and(|p| p.is_self))
            });
            if has_self && seen.insert("self".to_string()) {
                items.push(CompletionItem {
                    label: "self".to_string(),
                    kind: Some(CompletionItemKind::KEYWORD),
                    ..Default::default()
                });
            }

            // Issue #045: Only show function parameters from the enclosing
            // function (whose span contains the cursor), not from all fns.
            for def in hir.defs.values() {
                if !(def.span.start <= offset && offset < def.span.end) {
                    continue;
                }
                if let DefKind::Fn(f) = &def.kind {
                    for param in &f.params {
                        if param.is_self || param.name.is_empty() {
                            continue;
                        }
                        let label = param.name.to_string();
                        if seen.insert(label.clone()) {
                            let mut ty_str = format!("{}", param.ty);
                            let mut is_type_param = false;
                            // Annotate type params with bounds
                            if let valen_hir::TyRef::Unresolved(tp) = &param.ty {
                                for (bn, bounds) in &f.generic_bounds {
                                    if bn == tp && !bounds.is_empty() {
                                        ty_str = format!("{tp}: {}", bounds.join(" + "));
                                        is_type_param = true;
                                        break;
                                    }
                                }
                            }
                            let documentation = if is_type_param {
                                None
                            } else {
                                build_variable_documentation(&label, &param.ty)
                            };
                            items.push(CompletionItem {
                                label,
                                kind: Some(CompletionItemKind::VARIABLE),
                                detail: Some(ty_str),
                                documentation,
                                ..Default::default()
                            });
                        }
                    }
                }
            }
        }

        items
    }

    // -----------------------------------------------------------------------
    // Hover helpers
    // -----------------------------------------------------------------------

    /// Build hover content for the word at the given position.
    fn build_hover(&self, uri: &Url, position: Position) -> Option<Hover> {
        let doc = self.documents.get(uri)?;
        let offset = doc.line_index.position_to_offset(position);
        let name = extract_word_at(&doc.text, offset)?;
        let hir = doc.hir.as_ref()?;

        // Check if it's a type parameter with bounds
        for def in hir.defs.values() {
            if let DefKind::Fn(f) = &def.kind {
                for (param_name, bounds) in &f.generic_bounds {
                    if param_name.as_str() == name {
                        let value = if bounds.is_empty() {
                            format!("```valen\ntype {name}\n```\n")
                        } else {
                            format!("```valen\n{name}: {}\n```\n", bounds.join(" + "))
                        };
                        return Some(Hover {
                            contents: HoverContents::Markup(MarkupContent {
                                kind: MarkupKind::Markdown,
                                value,
                            }),
                            range: None,
                        });
                    }
                }
            }
        }

        for def in hir.defs.values() {
            if def.name.as_str() != name {
                continue;
            }
            let value = format_def_hover_markdown(def, hir, &doc.text);
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value,
                }),
                range: Some(doc.line_index.span_to_range(def.span)),
            });
        }

        // Check foreign (Java) types
        if let Some(info) = hir.foreign_types.get(name) {
            let mut lines = vec![format!("```java\n// {}\nclass {name}", info.internal_name)];
            if !info.type_params.is_empty() {
                lines[0] = format!(
                    "```java\n// {}\nclass {name}<{}>",
                    info.internal_name,
                    info.type_params.join(", ")
                );
            }
            lines.push("```\n".to_string());
            if !info.constructors.is_empty() {
                lines.push(format!("**Constructors:** {}", info.constructors.len()));
            }
            if !info.methods.is_empty() {
                let method_names: Vec<&str> = info
                    .methods
                    .iter()
                    .map(|m| m.name.as_str())
                    .take(10)
                    .collect();
                lines.push(format!("**Methods:** {}", method_names.join(", ")));
                if info.methods.len() > 10 {
                    lines.push(format!("… and {} more", info.methods.len() - 10));
                }
            }
            let value = lines.join("\n");
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value,
                }),
                range: None,
            });
        }

        // Search typed bodies for expression type info at cursor
        if let Some(bodies) = doc.bodies.as_ref() {
            if let Some(expr) = find_expr_at_offset(bodies, offset) {
                // Issue #036: Provide rich hover info for variables including
                // declaration context and enclosing function.
                if let TypedExprKind::LocalVar(var_name) = &expr.kind {
                    if !expr.ty.is_error() {
                        let value =
                            build_rich_variable_hover(var_name, &expr.ty, &doc.text, hir, offset);
                        return Some(Hover {
                            contents: HoverContents::Markup(MarkupContent {
                                kind: MarkupKind::Markdown,
                                value,
                            }),
                            range: Some(doc.line_index.span_to_range(expr.span)),
                        });
                    }
                }
                if let Some(hover_text) = format_typed_expr_hover(expr) {
                    return Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: format!("```valen\n{hover_text}\n```\n"),
                        }),
                        range: Some(doc.line_index.span_to_range(expr.span)),
                    });
                }
            }
        }

        None
    }

    // -----------------------------------------------------------------------
    // Semantic tokens helpers
    // -----------------------------------------------------------------------

    /// Produce semantic tokens for the entire document using the lexer.
    fn build_semantic_tokens(&self, uri: &Url) -> Option<SemanticTokensResult> {
        let doc = self.documents.get(uri)?;
        let file_id = self
            .file_ids
            .get(uri)
            .copied()
            .unwrap_or(valen_ast::FileId(0));
        let (tokens, _) = valen_parser::lexer::lex(&doc.text, file_id);

        let mut result: Vec<SemanticToken> = Vec::new();
        let mut prev_line: u32 = 0;
        let mut prev_start: u32 = 0;

        for (idx, (kind, span)) in tokens.iter().enumerate() {
            let token_type = if matches!(kind, TokenKind::Data) {
                let next_is_class = tokens
                    .get(idx + 1)
                    .is_some_and(|(next, _)| matches!(next, TokenKind::Class));
                if next_is_class {
                    ST_KEYWORD
                } else {
                    ST_VARIABLE
                }
            } else {
                match classify_token(kind) {
                    Some(t) => t,
                    None => continue,
                }
            };

            let start_pos = doc.line_index.offset_to_position(span.start);
            // LSP requires token length in UTF-16 code units, not bytes.
            let token_text =
                &doc.text[span.start as usize..(span.end as usize).min(doc.text.len())];
            let length = token_text.encode_utf16().count() as u32;
            if length == 0 {
                continue;
            }

            let delta_line = start_pos.line.saturating_sub(prev_line);
            let delta_start = if delta_line == 0 {
                start_pos.character.saturating_sub(prev_start)
            } else {
                start_pos.character
            };

            result.push(SemanticToken {
                delta_line,
                delta_start,
                length,
                token_type,
                token_modifiers_bitset: 0,
            });

            prev_line = start_pos.line;
            prev_start = start_pos.character;
        }

        Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: result,
        }))
    }
}

// ---------------------------------------------------------------------------
// Analysis pipeline
// ---------------------------------------------------------------------------

/// Run the full analysis pipeline on a source text, returning document state and LSP diagnostics.
///
/// TODO(#040): Full re-parse on every keystroke with no debounce. Parse, resolve,
/// coherence, and type check all run synchronously. Known MVP limitation.
///
/// TODO(#047): Each document is analyzed in isolation — single-file parse+resolve
/// cannot reference definitions from other files in the workspace.
pub fn analyze_document(
    text: &str,
    file_id: valen_ast::FileId,
    classpath: &[std::path::PathBuf],
) -> (DocumentState, Vec<async_lsp::lsp_types::Diagnostic>) {
    let line_index = convert::LineIndex::new(text);

    let parse_result = valen_parser::parse(text, file_id);
    let mut diags = convert::to_lsp_diagnostics(&parse_result.diagnostics, &line_index);

    let resolve_result = valen_hir::resolve::resolve_with_classpath(&parse_result.items, classpath);
    diags.extend(convert::to_lsp_diagnostics(
        &resolve_result.diagnostics,
        &line_index,
    ));

    let coherence_result = valen_hir::coherence::check_coherence(&resolve_result.hir, &[]);
    diags.extend(convert::to_lsp_diagnostics(
        &coherence_result.diagnostics,
        &line_index,
    ));

    let exhaustive_result =
        valen_hir::exhaustive::check_exhaustiveness(&resolve_result.hir, &parse_result.items);
    diags.extend(convert::to_lsp_diagnostics(
        &exhaustive_result.diagnostics,
        &line_index,
    ));

    let tc = valen_hir::ty::type_check(&resolve_result.hir, &parse_result.items);
    diags.extend(convert::to_lsp_diagnostics(&tc.diagnostics, &line_index));
    let (hir, bodies) = (Some(resolve_result.hir), Some(tc.bodies));

    let doc = DocumentState {
        text: text.to_string(),
        line_index,
        items: parse_result.items,
        hir,
        bodies,
        file_id,
    };

    (doc, diags)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

// TODO(#034): Completion and hover documentation are built by separate code
// paths (~300 lines duplicated). Unify into a single `build_def_documentation()`
// function that produces consistent Markdown for both contexts, parameterized
// by whether it needs a code-block header (hover) or compact detail (completion).

/// Extract `///` doc comments from lines immediately before `span_start` in the source.
///
/// Walks backwards from the line preceding the definition, collecting consecutive
/// `///` comment lines. Strips the `/// ` (or `///`) prefix and joins with newlines.
/// Returns `None` if no doc comments are found.
fn extract_doc_comment(source: &str, span_start: u32) -> Option<String> {
    if span_start as usize > source.len() {
        return None;
    }
    let before = &source[..span_start as usize];
    // Find the line containing span_start; we want the lines *before* it.
    let lines: Vec<&str> = before.lines().collect();
    if lines.is_empty() {
        return None;
    }
    // Start from the line just before the definition line.
    // The last line in `lines` is the one containing span_start (or partial).
    let start_idx = if lines.len() >= 2 {
        lines.len() - 2
    } else {
        return None;
    };

    let mut doc_lines: Vec<&str> = Vec::new();
    for i in (0..=start_idx).rev() {
        let trimmed = lines[i].trim();
        if let Some(rest) = trimmed.strip_prefix("///") {
            // Strip optional leading space after `///`
            let content = rest.strip_prefix(' ').unwrap_or(rest);
            doc_lines.push(content);
        } else if trimmed.is_empty() {
            // Allow blank lines between doc comments and definition
            if doc_lines.is_empty() {
                continue;
            }
            break;
        } else {
            break;
        }
    }

    if doc_lines.is_empty() {
        return None;
    }

    doc_lines.reverse();
    Some(doc_lines.join("\n"))
}

/// Build rich Markdown documentation for a function completion item.
fn build_fn_documentation(
    name: &str,
    f: &valen_hir::FnDef,
    source: &str,
    span_start: u32,
) -> Option<Documentation> {
    let mut md = String::new();
    md.push_str("```valen\n");
    md.push_str(&format_fn_signature(name, f));
    md.push_str("\n```\n");

    // Type parameters section
    if !f.generic_bounds.is_empty() {
        md.push_str("\n**Type Parameters:**\n");
        for (tp, bounds) in &f.generic_bounds {
            if bounds.is_empty() {
                md.push_str(&format!("- `{tp}`\n"));
            } else {
                md.push_str(&format!("- `{tp}` \u{2014} `{}`\n", bounds.join(" + ")));
            }
        }
    }

    if let Some(doc) = extract_doc_comment(source, span_start) {
        md.push_str("\n---\n");
        md.push_str(&doc);
        md.push('\n');
    }

    Some(Documentation::MarkupContent(MarkupContent {
        kind: MarkupKind::Markdown,
        value: md,
    }))
}

/// Build rich Markdown documentation for a class/data class completion item.
fn build_class_documentation(
    name: &str,
    params: &[valen_hir::CtorParamDef],
    superclass: Option<&valen_hir::TyRef>,
    hir: &valen_hir::Hir,
    source: &str,
    span_start: u32,
) -> Option<Documentation> {
    let mut md = String::new();
    md.push_str("```valen\n");

    // Build signature line
    let ps: Vec<String> = params
        .iter()
        .map(|p| {
            let vis = match p.vis {
                valen_hir::Vis::Pub => "pub ",
                _ => "",
            };
            let m = if p.mutable { "mut " } else { "" };
            format!("{vis}{m}{}: {}", p.name, p.ty)
        })
        .collect();
    let mut sig = format!("class {name}({})", ps.join(", "));
    if let Some(sup) = superclass {
        sig.push_str(&format!(" : {sup}"));
    }
    md.push_str(&sig);
    md.push_str("\n```\n");

    // Implements section: find traits implemented by this class
    let trait_names: Vec<&str> = hir
        .trait_impls
        .iter()
        .filter(|e| e.target_name.as_str() == name)
        .map(|e| e.trait_name.as_str())
        .collect();
    if !trait_names.is_empty() {
        md.push_str(&format!("\n**Implements:** {}\n", trait_names.join(", ")));
    }

    // Fields section
    if !params.is_empty() {
        md.push_str("\n**Fields:**\n");
        for p in params {
            md.push_str(&format!("- `{}: {}`\n", p.name, p.ty));
        }
    }

    if let Some(doc) = extract_doc_comment(source, span_start) {
        md.push_str("\n---\n");
        md.push_str(&doc);
        md.push('\n');
    }

    Some(Documentation::MarkupContent(MarkupContent {
        kind: MarkupKind::Markdown,
        value: md,
    }))
}

/// Build rich Markdown documentation for an enum completion item.
fn build_enum_documentation(
    name: &str,
    e: &valen_hir::EnumDef,
    source: &str,
    span_start: u32,
) -> Option<Documentation> {
    let mut md = String::new();
    md.push_str("```valen\n");

    let variants_short: Vec<String> = e
        .variants
        .iter()
        .map(|v| {
            if v.fields.is_empty() {
                v.name.to_string()
            } else {
                let fs: Vec<String> = v.fields.iter().map(|(n, t)| format!("{n}: {t}")).collect();
                format!("{}({})", v.name, fs.join(", "))
            }
        })
        .collect();
    md.push_str(&format!("enum {name} {{ {} }}", variants_short.join(", ")));
    md.push_str("\n```\n");

    // Variants section
    md.push_str("\n**Variants:**\n");
    for v in &e.variants {
        if v.fields.is_empty() {
            md.push_str(&format!("- `{}`\n", v.name));
        } else {
            let fs: Vec<String> = v.fields.iter().map(|(n, t)| format!("{n}: {t}")).collect();
            md.push_str(&format!("- `{}({})`\n", v.name, fs.join(", ")));
        }
    }

    if let Some(doc) = extract_doc_comment(source, span_start) {
        md.push_str("\n---\n");
        md.push_str(&doc);
        md.push('\n');
    }

    Some(Documentation::MarkupContent(MarkupContent {
        kind: MarkupKind::Markdown,
        value: md,
    }))
}

/// Build rich Markdown documentation for a trait completion item.
fn build_trait_documentation(
    name: &str,
    t: &valen_hir::TraitDef,
    hir: &valen_hir::Hir,
    source: &str,
    span_start: u32,
) -> Option<Documentation> {
    let mut md = String::new();
    md.push_str("```valen\n");
    md.push_str(&format!("trait {name} {{\n"));

    let mut method_sigs: Vec<String> = Vec::new();
    for &mid in &t.methods {
        if let Some(mdef) = hir.defs.get(&mid) {
            if let DefKind::Fn(f) = &mdef.kind {
                let sig = format_fn_signature(&mdef.name, f);
                md.push_str(&format!("    {sig};\n"));
                method_sigs.push(sig);
            }
        }
    }

    md.push_str("}\n");
    md.push_str("```\n");

    // Methods section
    if !method_sigs.is_empty() {
        md.push_str("\n**Methods:**\n");
        for sig in &method_sigs {
            md.push_str(&format!("- `{sig}`\n"));
        }
    }

    if let Some(doc) = extract_doc_comment(source, span_start) {
        md.push_str("\n---\n");
        md.push_str(&doc);
        md.push('\n');
    }

    Some(Documentation::MarkupContent(MarkupContent {
        kind: MarkupKind::Markdown,
        value: md,
    }))
}

/// Issue #036: Build rich Markdown hover content for a local variable.
///
/// Shows the variable type in a code block, the enclosing function name,
/// and any doc comment attached to the let binding.
fn build_rich_variable_hover(
    name: &str,
    ty: &valen_hir::Ty,
    _source: &str,
    hir: &valen_hir::Hir,
    cursor_offset: u32,
) -> String {
    let mut md = String::new();
    md.push_str(&format!("```valen\nlet {name}: {ty}\n```\n"));

    // Find the enclosing function for context
    for def in hir.defs.values() {
        if def.span.start <= cursor_offset
            && cursor_offset < def.span.end
            && matches!(&def.kind, DefKind::Fn(_))
        {
            md.push_str(&format!("\n*in* `fn {}`\n", def.name));
            break;
        }
    }

    // Check if this is a function parameter and show parameter info
    for def in hir.defs.values() {
        if def.span.start <= cursor_offset && cursor_offset < def.span.end {
            if let DefKind::Fn(f) = &def.kind {
                for param in &f.params {
                    if param.name.as_str() == name {
                        md.clear();
                        md.push_str(&format!(
                            "```valen\n(parameter) {name}: {}\n```\n",
                            param.ty
                        ));
                        md.push_str(&format!("\n*in* `fn {}`\n", def.name));
                        return md;
                    }
                }
            }
        }
    }

    md
}

/// Build Markdown documentation for a variable completion item.
fn build_variable_documentation(name: &str, ty: &impl std::fmt::Display) -> Option<Documentation> {
    let md = format!("```valen\nlet {name}: {ty}\n```\n");
    Some(Documentation::MarkupContent(MarkupContent {
        kind: MarkupKind::Markdown,
        value: md,
    }))
}

/// Build Markdown documentation for a type parameter with its bounds.
fn build_type_param_documentation(name: &str, bounds_joined: &str) -> Option<Documentation> {
    let md = if bounds_joined.is_empty() {
        format!("Type parameter `{name}`\n")
    } else {
        format!("Type parameter with bounds: `{bounds_joined}`\n")
    };
    Some(Documentation::MarkupContent(MarkupContent {
        kind: MarkupKind::Markdown,
        value: md,
    }))
}

fn format_fn_signature(name: &str, f: &valen_hir::FnDef) -> String {
    format_fn_signature_with_owner(name, f, None)
}

fn format_fn_signature_with_owner(name: &str, f: &valen_hir::FnDef, owner: Option<&str>) -> String {
    let format_ty = |ty: &valen_hir::TyRef| -> String {
        if *ty == valen_hir::TyRef::SelfTy {
            owner.unwrap_or("Self").to_string()
        } else {
            format!("{ty}")
        }
    };
    let params: Vec<String> = f
        .params
        .iter()
        .map(|p| {
            if p.is_self {
                if p.mutable {
                    "mut self".into()
                } else {
                    "self".into()
                }
            } else {
                format!("{}: {}", p.name, format_ty(&p.ty))
            }
        })
        .collect();
    let generics = if f.generic_bounds.is_empty() {
        String::new()
    } else {
        let gs: Vec<String> = f
            .generic_bounds
            .iter()
            .map(|(name, bounds)| {
                if bounds.is_empty() {
                    name.to_string()
                } else {
                    format!("{}: {}", name, bounds.join(" + "))
                }
            })
            .collect();
        format!("<{}>", gs.join(", "))
    };
    let ret = f
        .return_ty
        .as_ref()
        .map(|t| format!(" -> {}", format_ty(t)))
        .unwrap_or_default();
    let prefix = if f.is_unsafe { "unsafe " } else { "" };
    format!(
        "{prefix}fn {}{generics}({}){}",
        name,
        params.join(", "),
        ret
    )
}

fn format_class_signature(name: &str, params: &[valen_hir::CtorParamDef]) -> String {
    let ps: Vec<String> = params
        .iter()
        .map(|p| {
            let vis = match p.vis {
                valen_hir::Vis::Pub => "pub ",
                _ => "",
            };
            let m = if p.mutable { "mut " } else { "" };
            format!("{vis}{m}{}: {}", p.name, p.ty)
        })
        .collect();
    format!("{}({})", name, ps.join(", "))
}

const BUILTIN_TYPES: &[(&str, &str)] = &[
    ("Int", "JVM integer (32-bit)"),
    ("Long", "JVM long (64-bit)"),
    ("Float", "JVM float (32-bit)"),
    ("Double", "JVM double (64-bit)"),
    ("Char", "Unicode character"),
    ("Bool", "Boolean (true/false)"),
    ("Byte", "JVM byte (8-bit)"),
    ("Short", "JVM short (16-bit)"),
    ("String", "UTF-16 string"),
    ("Unit", "Unit type (void)"),
    ("Nothing", "Bottom type (never returns)"),
    ("Option", "Option<T> — Some(value) | None"),
    ("Result", "Result<T, E> — Ok(value) | Err(error)"),
];

fn find_enclosing_type_from_source(source: &str) -> Option<String> {
    // Walk backwards from end of source (where cursor is during completion)
    // looking for the nearest `impl ... for TypeName` or `class TypeName`
    for line in source.lines().rev() {
        let trimmed = line.trim();

        // impl Trait for TypeName {
        if let Some(rest) = trimmed.strip_prefix("impl ") {
            if let Some(for_idx) = rest.find(" for ") {
                let after_for = rest[for_idx + 5..].trim();
                let type_name = after_for
                    .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                    .next()
                    .unwrap_or("");
                if !type_name.is_empty() {
                    return Some(type_name.to_string());
                }
            }
        }

        // [pub] [open|abstract|sealed] class TypeName
        let rest = trimmed.strip_prefix("pub ").unwrap_or(trimmed);
        let rest = rest
            .strip_prefix("open ")
            .or_else(|| rest.strip_prefix("abstract "))
            .or_else(|| rest.strip_prefix("sealed "))
            .unwrap_or(rest);
        if let Some(after_class) = rest.strip_prefix("class ") {
            let type_name = after_class
                .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .next()
                .unwrap_or("");
            if !type_name.is_empty() {
                return Some(type_name.to_string());
            }
        }
        if let Some(after_data) = rest.strip_prefix("data class ") {
            let type_name = after_data
                .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .next()
                .unwrap_or("");
            if !type_name.is_empty() {
                return Some(type_name.to_string());
            }
        }
    }
    None
}

struct TextLocal {
    name: String,
    ty: Option<String>,
}

fn extract_let_names_from_text(before: &str) -> Vec<TextLocal> {
    let mut results = Vec::new();
    for line in before.lines() {
        let trimmed = line.trim();
        let rest = match trimmed
            .strip_prefix("let mut ")
            .or_else(|| trimmed.strip_prefix("let "))
        {
            Some(r) => r,
            None => continue,
        };
        let name_end = rest
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .unwrap_or(rest.len());
        let name = &rest[..name_end];
        if name.is_empty() {
            continue;
        }
        let after = rest[name_end..].trim_start();

        // Try explicit annotation: let name: Type
        if let Some(stripped) = after.strip_prefix(':') {
            let ty_part = stripped.trim_start();
            let ty_end = ty_part.find(['=', ';', '{']).unwrap_or(ty_part.len());
            let ty_str = ty_part[..ty_end].trim();
            let base = ty_str.split('<').next().unwrap_or(ty_str).trim();
            if !base.is_empty() {
                results.push(TextLocal {
                    name: name.to_string(),
                    ty: Some(base.to_string()),
                });
                continue;
            }
        }

        // Try RHS constructor: let name = TypeName(...)
        if let Some(rhs) = after.strip_prefix('=') {
            let rhs = rhs.trim_start();
            let ctor_end = rhs
                .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .unwrap_or(rhs.len());
            let ctor_name = &rhs[..ctor_end];
            if !ctor_name.is_empty() && ctor_name.starts_with(|c: char| c.is_ascii_uppercase()) {
                results.push(TextLocal {
                    name: name.to_string(),
                    ty: Some(ctor_name.to_string()),
                });
                continue;
            }
        }

        results.push(TextLocal {
            name: name.to_string(),
            ty: None,
        });
    }
    results
}

fn tyref_to_type_name(ty: &valen_hir::TyRef) -> Option<String> {
    match ty {
        valen_hir::TyRef::Named(n) => Some(n.to_string()),
        valen_hir::TyRef::Generic(n, _) => Some(n.to_string()),
        valen_hir::TyRef::Prim(p) => Some(format!("{p}")),
        _ => None,
    }
}

fn ty_to_type_name(ty: &Ty) -> Option<String> {
    match ty {
        Ty::Named(n) => Some(n.to_string()),
        Ty::Generic(n, _) => Some(n.to_string()),
        Ty::Prim(p) => Some(format!("{p}")),
        Ty::Nullable(inner) => ty_to_type_name(inner),
        _ => None,
    }
}

pub fn find_let_type_annotation_pub(source: &str, var_name: &str) -> Option<String> {
    find_let_type_annotation(source, var_name)
}

fn find_let_type_annotation(source: &str, var_name: &str) -> Option<String> {
    for line in source.lines() {
        let trimmed = line.trim();
        let rest = match trimmed
            .strip_prefix("let mut ")
            .or_else(|| trimmed.strip_prefix("let "))
        {
            Some(r) => r,
            None => continue,
        };
        if !rest.starts_with(var_name) {
            continue;
        }
        let after_name = &rest[var_name.len()..];

        // Match: let [mut] name: Type = ... / let [mut] name: Type;
        if let Some(stripped) = after_name.strip_prefix(':') {
            let ty_part = stripped.trim_start();
            let ty_end = ty_part.find(['=', ';', '{']).unwrap_or(ty_part.len());
            let ty_str = ty_part[..ty_end].trim();
            let base = ty_str.split('<').next().unwrap_or(ty_str).trim();
            if !base.is_empty() {
                return Some(base.to_string());
            }
        }

        // Match: let [mut] name = ConstructorName(...) — infer from RHS
        let after_eq = after_name.trim_start();
        if let Some(rhs) = after_eq.strip_prefix('=') {
            let rhs = rhs.trim_start();
            let ctor_end = rhs
                .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .unwrap_or(rhs.len());
            let ctor_name = &rhs[..ctor_end];
            if !ctor_name.is_empty() && ctor_name.starts_with(|c: char| c.is_ascii_uppercase()) {
                return Some(ctor_name.to_string());
            }
        }
    }
    None
}

enum CompletionContext {
    TypePosition,
    ImplTarget,
    /// Issue #038: After `impl `, show only traits (not all definitions).
    ImplTraitPosition,
    /// Issue #039: Inside `import` statements, show package paths.
    ImportPath,
    NamingPosition,
    General,
}

fn detect_context(before: &str) -> CompletionContext {
    let trimmed = before.trim_end();
    // Strip partial identifier being typed
    let base = trimmed.trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_');
    let base = base.trim_end();

    if base.ends_with("for") || base.ends_with("for ") {
        // Check if this is `impl Trait for ` pattern
        let prefix = base.trim_end_matches("for").trim_end();
        if prefix.ends_with(|c: char| c.is_ascii_alphanumeric() || c == '>' || c == '_') {
            // Looks like `impl Something for`
            return CompletionContext::ImplTarget;
        }
    }

    // Issue #038: After `impl ` (without a following trait name + ` for`),
    // show only traits since `impl` expects a trait name next.
    if base.ends_with("impl") {
        return CompletionContext::ImplTraitPosition;
    }

    // Issue #039: Inside `import` statement, show package paths.
    if base.ends_with("import") {
        return CompletionContext::ImportPath;
    }
    // Also detect import with partial dotted path (e.g., `import java.`)
    if is_import_path_context(before) {
        return CompletionContext::ImportPath;
    }

    if base.ends_with(':') && !base.ends_with("::") {
        return CompletionContext::TypePosition;
    }
    if base.ends_with("->") {
        return CompletionContext::TypePosition;
    }
    if base.ends_with("<") {
        return CompletionContext::TypePosition;
    }
    if base.ends_with(',') {
        let before_comma = base.strip_suffix(',').unwrap_or(base).trim_end();
        if before_comma
            .rfind(':')
            .is_some_and(|i| before_comma[i..].find('(').is_none())
        {
            return CompletionContext::TypePosition;
        }
    }

    // After keywords that expect a name — suppress completions
    for kw in &[
        "fn ",
        "let ",
        "let mut ",
        "class ",
        "data class ",
        "enum ",
        "trait ",
        "typealias ",
    ] {
        if base.ends_with(kw) {
            return CompletionContext::NamingPosition;
        }
    }

    CompletionContext::General
}

/// Issue #039: Check if the cursor is in an `import path.to.module` context.
fn is_import_path_context(before: &str) -> bool {
    let trimmed = before.trim_end();
    if let Some(import_pos) = trimmed.rfind("import ") {
        let after_import = &trimmed[import_pos + 7..];
        // If the content after `import ` contains a dot and only identifier
        // chars/dots, we are in an import path context.
        !after_import.is_empty()
            && after_import.contains('.')
            && after_import
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == ' ')
    } else {
        false
    }
}

fn is_double_colon_context(before: &str) -> bool {
    let trimmed = before.trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_');
    trimmed.trim_end().ends_with("::")
}

fn extract_name_before_double_colon(before: &str) -> &str {
    let trimmed = before.trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_');
    let without_colons = trimmed.trim_end().strip_suffix("::").unwrap_or(trimmed);
    let without_colons = without_colons.trim_end();
    let start = without_colons
        .rfind(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);
    &without_colons[start..]
}

/// Attempt to infer the expected enum type from surrounding text context.
///
/// Handles patterns like:
/// - `let x: Color = .`  → "Color"
/// - `fn foo() -> Color { .` → "Color"
/// - `param: Color` in a call argument position
/// - `match expr { .` → uses match scrutinee type (via `let x: Type = ...; match x`)
fn infer_expected_enum_type(before: &str, hir: &valen_hir::Hir) -> Option<String> {
    let enum_names: std::collections::HashSet<&str> = hir
        .defs
        .values()
        .filter_map(|d| {
            if matches!(d.kind, DefKind::Enum(_)) {
                Some(d.name.as_str())
            } else {
                None
            }
        })
        .collect();

    // Pattern: `let name: Type = .` or `let name: Type = expr; ... .`
    // Scan backwards for `: TypeName` before `=`
    let trimmed = before.trim_end();
    let base = trimmed
        .strip_suffix('.')
        .unwrap_or(trimmed)
        .trim_end()
        .trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_')
        .trim_end();

    // `let x: Color = .` → look for `: Color =`
    // `let x: Color = someExpr; ... .` → too far, skip
    // Find the last `: TypeName` followed by `=` on the same line-ish
    if let Some(eq_pos) = base.rfind('=') {
        let before_eq = base[..eq_pos].trim_end();
        // Don't match `==`, `!=`, `>=`, `<=`, `=>`
        if !before_eq.ends_with('!')
            && !before_eq.ends_with('>')
            && !before_eq.ends_with('<')
            && !before_eq.ends_with('=')
        {
            if let Some(colon_pos) = before_eq.rfind(':') {
                let type_str = before_eq[colon_pos + 1..].trim();
                let type_name = type_str
                    .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                    .next()
                    .unwrap_or("");
                if enum_names.contains(type_name) {
                    return Some(type_name.to_string());
                }
            }
        }
    }

    // Pattern: `-> Type { ... .` (return type)
    if let Some(arrow_pos) = before.rfind("->") {
        let after_arrow = before[arrow_pos + 2..].trim_start();
        let type_name = after_arrow
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .next()
            .unwrap_or("");
        if enum_names.contains(type_name) {
            return Some(type_name.to_string());
        }
    }

    // Pattern: `match expr {` → try to find scrutinee type from let bindings
    if let Some(match_pos) = before.rfind("match ") {
        let after_match = &before[match_pos + 6..];
        let scrutinee = after_match
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .next()
            .unwrap_or("");
        if !scrutinee.is_empty() {
            let let_pattern = format!("let {scrutinee}:");
            if let Some(let_pos) = before.rfind(&let_pattern) {
                let after_colon = &before[let_pos + let_pattern.len()..];
                let type_name = after_colon
                    .trim_start()
                    .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                    .next()
                    .unwrap_or("");
                if enum_names.contains(type_name) {
                    return Some(type_name.to_string());
                }
            }
        }
    }

    // Pattern: `funcName(arg1, .` → find function, determine param index, get param type
    if let Some(result) = infer_enum_from_call_arg(before, hir, &enum_names) {
        return Some(result);
    }

    None
}

/// Infer enum type from function call argument position.
/// e.g. `foo(x, .` → find `foo`, param index 1, check if param type is an enum.
fn infer_enum_from_call_arg(
    before: &str,
    hir: &valen_hir::Hir,
    enum_names: &std::collections::HashSet<&str>,
) -> Option<String> {
    let trimmed = before.trim_end();
    let base = trimmed
        .strip_suffix('.')
        .unwrap_or(trimmed)
        .trim_end()
        .trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_')
        .trim_end();

    // Walk backwards to find matching `(`, counting commas for arg index
    let mut depth = 0i32;
    let mut arg_index = 0usize;
    let mut paren_pos = None;
    for (i, ch) in base.char_indices().rev() {
        match ch {
            ')' => depth += 1,
            '(' => {
                if depth == 0 {
                    paren_pos = Some(i);
                    break;
                }
                depth -= 1;
            }
            ',' if depth == 0 => arg_index += 1,
            _ => {}
        }
    }
    let paren_pos = paren_pos?;

    // Extract function name before `(`
    let before_paren = base[..paren_pos].trim_end();
    let fn_name_start = before_paren
        .rfind(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);
    let fn_name = &before_paren[fn_name_start..];
    if fn_name.is_empty() {
        return None;
    }

    // Look up function in HIR and get param type at arg_index
    for def in hir.defs.values() {
        if def.name.as_str() != fn_name {
            continue;
        }
        if let DefKind::Fn(f) = &def.kind {
            let params: Vec<_> = f.params.iter().filter(|p| !p.is_self).collect();
            if let Some(param) = params.get(arg_index) {
                let type_name = match &param.ty {
                    valen_hir::TyRef::Named(n) => n.as_str(),
                    valen_hir::TyRef::Generic(n, _) => n.as_str(),
                    _ => continue,
                };
                if enum_names.contains(type_name) {
                    return Some(type_name.to_string());
                }
            }
        }
    }

    None
}

fn is_variant_shorthand_context(before: &str) -> bool {
    let stripped = before.trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_');
    let trimmed = stripped.trim_end();
    let without_dot = match trimmed.strip_suffix('.') {
        Some(s) => s.trim_end(),
        None => return false,
    };
    without_dot.is_empty()
        || without_dot.ends_with('=')
        || without_dot.ends_with('(')
        || without_dot.ends_with(',')
        || without_dot.ends_with('{')
        || without_dot.ends_with("=>")
}

fn is_dot_context(before: &str) -> bool {
    let trimmed = before.trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_');
    trimmed.trim_end().ends_with('.')
}

fn extract_receiver_before_dot(before: &str) -> &str {
    let stripped = before.trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_');
    let trimmed = stripped.trim_end();
    let without_dot = trimmed.strip_suffix('.').unwrap_or(trimmed);
    let without_dot = without_dot.trim_end();
    let start = without_dot
        .rfind(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);
    &without_dot[start..]
}

const SKIP_DIRS: &[&str] = &[".git", "target", "build", "node_modules", ".gradle"];

fn find_vln_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let canonical_root = match root.canonicalize() {
        Ok(p) => p,
        Err(_) => return files,
    };
    let mut seen = std::collections::HashSet::new();
    find_vln_files_inner(&canonical_root, &canonical_root, &mut seen, &mut files);
    files
}

fn find_vln_files_inner(
    dir: &std::path::Path,
    workspace_root: &std::path::Path,
    seen: &mut std::collections::HashSet<std::path::PathBuf>,
    files: &mut Vec<std::path::PathBuf>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();

        // Use symlink_metadata to detect symlinks before following them
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if meta.is_symlink() {
            // Resolve the symlink target and verify it stays within workspace
            let canonical = match path.canonicalize() {
                Ok(p) => p,
                Err(_) => continue,
            };
            if !canonical.starts_with(workspace_root) {
                continue; // Symlink points outside workspace — skip
            }
            if !seen.insert(canonical.clone()) {
                continue; // Cycle detection — already visited
            }
            if canonical.is_dir() {
                let name = canonical.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !SKIP_DIRS.contains(&name) {
                    find_vln_files_inner(&canonical, workspace_root, seen, files);
                }
            } else if canonical.extension().is_some_and(|e| e == "vln") {
                files.push(canonical);
            }
        } else if meta.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if SKIP_DIRS.contains(&name) {
                continue;
            }
            find_vln_files_inner(&path, workspace_root, seen, files);
        } else if path.extension().is_some_and(|e| e == "vln") {
            files.push(path);
        }
    }
}

/// Extract the identifier word at the given byte offset.
pub fn extract_word_at(text: &str, offset: u32) -> Option<&str> {
    let bytes = text.as_bytes();
    let pos = offset as usize;
    if pos >= bytes.len() || (!bytes[pos].is_ascii_alphanumeric() && bytes[pos] != b'_') {
        return None;
    }
    let start = text[..pos]
        .rfind(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);
    let end = text[pos..]
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .map(|i| i + pos)
        .unwrap_or(text.len());
    Some(&text[start..end])
}

/// Format a HIR definition for hover display (plain text).
///
/// Retained as a utility for potential non-Markdown consumers; the LSP hover
/// path now uses [`format_def_hover_markdown`] instead.
#[allow(dead_code)]
fn format_def_hover(def: &valen_hir::Def, hir: &valen_hir::Hir) -> String {
    match &def.kind {
        DefKind::Fn(f) => {
            let params_str: Vec<String> = f
                .params
                .iter()
                .map(|p| {
                    if p.is_self {
                        if p.mutable {
                            "mut self".to_string()
                        } else {
                            "self".to_string()
                        }
                    } else {
                        format!("{}: {}", p.name, p.ty)
                    }
                })
                .collect();
            let ret = f
                .return_ty
                .as_ref()
                .map(|t| format!(" -> {t}"))
                .unwrap_or_default();
            let prefix = if f.is_unsafe { "unsafe " } else { "" };
            format!("{prefix}fn {}({}){}", def.name, params_str.join(", "), ret)
        }
        DefKind::Class(c) => {
            let params_str: Vec<String> = c
                .ctor_params
                .iter()
                .map(|p| {
                    let vis = match p.vis {
                        valen_hir::Vis::Pub => "pub ",
                        _ => "",
                    };
                    let m = if p.mutable { "mut " } else { "" };
                    format!("{vis}{m}{}: {}", p.name, p.ty)
                })
                .collect();
            format!("class {}({})", def.name, params_str.join(", "))
        }
        DefKind::DataClass(dc) => {
            let params_str: Vec<String> = dc
                .ctor_params
                .iter()
                .map(|p| {
                    let vis = match p.vis {
                        valen_hir::Vis::Pub => "pub ",
                        _ => "",
                    };
                    format!("{vis}{}: {}", p.name, p.ty)
                })
                .collect();
            format!("data class {}({})", def.name, params_str.join(", "))
        }
        DefKind::Enum(e) => {
            let variants: Vec<String> = e
                .variants
                .iter()
                .map(|v| {
                    if v.fields.is_empty() {
                        v.name.to_string()
                    } else {
                        let fields: Vec<String> =
                            v.fields.iter().map(|(n, t)| format!("{n}: {t}")).collect();
                        format!("{}({})", v.name, fields.join(", "))
                    }
                })
                .collect();
            format!("enum {} {{ {} }}", def.name, variants.join(", "))
        }
        DefKind::Trait(t) => {
            let method_names: Vec<String> = t
                .methods
                .iter()
                .filter_map(|&mid| hir.defs.get(&mid).map(|d| d.name.to_string()))
                .collect();
            format!("trait {} {{ {} }}", def.name, method_names.join(", "))
        }
        DefKind::Impl(im) => {
            format!("impl {} for {}", im.trait_ref, im.target)
        }
        DefKind::TypeAlias(ta) => {
            format!("typealias {} = {}", def.name, ta.target)
        }
        DefKind::NewType(nt) => {
            format!("newtype {} = {}", def.name, nt.inner_ty)
        }
        DefKind::AnnotationClass(_) => {
            format!("annotation class {}", def.name)
        }
    }
}

/// Format a HIR definition for hover display as rich Markdown with doc comments.
fn format_def_hover_markdown(def: &valen_hir::Def, hir: &valen_hir::Hir, source: &str) -> String {
    let mut md = String::new();
    md.push_str("```valen\n");

    match &def.kind {
        DefKind::Fn(f) => {
            md.push_str(&format_fn_signature(&def.name, f));
        }
        DefKind::Class(c) => {
            let ps: Vec<String> = c
                .ctor_params
                .iter()
                .map(|p| {
                    let vis = match p.vis {
                        valen_hir::Vis::Pub => "pub ",
                        _ => "",
                    };
                    let m = if p.mutable { "mut " } else { "" };
                    format!("{vis}{m}{}: {}", p.name, p.ty)
                })
                .collect();
            let mut sig = format!("class {}({})", def.name, ps.join(", "));
            if let Some(sup) = &c.superclass {
                sig.push_str(&format!(" : {sup}"));
            }
            md.push_str(&sig);
        }
        DefKind::DataClass(dc) => {
            let ps: Vec<String> = dc
                .ctor_params
                .iter()
                .map(|p| {
                    let vis = match p.vis {
                        valen_hir::Vis::Pub => "pub ",
                        _ => "",
                    };
                    format!("{vis}{}: {}", p.name, p.ty)
                })
                .collect();
            md.push_str(&format!("data class {}({})", def.name, ps.join(", ")));
        }
        DefKind::Enum(e) => {
            let variants: Vec<String> = e
                .variants
                .iter()
                .map(|v| {
                    if v.fields.is_empty() {
                        v.name.to_string()
                    } else {
                        let fields: Vec<String> =
                            v.fields.iter().map(|(n, t)| format!("{n}: {t}")).collect();
                        format!("{}({})", v.name, fields.join(", "))
                    }
                })
                .collect();
            md.push_str(&format!("enum {} {{ {} }}", def.name, variants.join(", ")));
        }
        DefKind::Trait(t) => {
            md.push_str(&format!("trait {} {{\n", def.name));
            for &mid in &t.methods {
                if let Some(mdef) = hir.defs.get(&mid) {
                    if let DefKind::Fn(f) = &mdef.kind {
                        md.push_str(&format!("    {};\n", format_fn_signature(&mdef.name, f)));
                    }
                }
            }
            md.push('}');
        }
        DefKind::Impl(im) => {
            md.push_str(&format!("impl {} for {}", im.trait_ref, im.target));
        }
        DefKind::TypeAlias(ta) => {
            md.push_str(&format!("typealias {} = {}", def.name, ta.target));
        }
        DefKind::NewType(nt) => {
            md.push_str(&format!("newtype {} = {}", def.name, nt.inner_ty));
        }
        DefKind::AnnotationClass(_) => {
            md.push_str(&format!("annotation class {}", def.name));
        }
    }

    md.push_str("\n```\n");

    // Append doc comment if present
    if let Some(doc) = extract_doc_comment(source, def.span.start) {
        md.push_str("\n---\n");
        md.push_str(&doc);
        md.push('\n');
    }

    md
}

/// Classify a `TokenKind` into a semantic token type index, or `None` to skip.
fn classify_token(kind: &TokenKind) -> Option<u32> {
    match kind {
        // Keywords
        TokenKind::Fn
        | TokenKind::Let
        | TokenKind::Mut
        | TokenKind::SelfKw
        | TokenKind::Return
        | TokenKind::If
        | TokenKind::Else
        | TokenKind::Match
        | TokenKind::Class
        | TokenKind::Enum
        | TokenKind::Trait
        | TokenKind::Impl
        | TokenKind::Pub
        | TokenKind::Internal
        | TokenKind::Private
        | TokenKind::Open
        | TokenKind::Override
        | TokenKind::Abstract
        | TokenKind::Sealed
        | TokenKind::Package
        | TokenKind::Import
        | TokenKind::For
        | TokenKind::In
        | TokenKind::While
        | TokenKind::Loop
        | TokenKind::Break
        | TokenKind::Continue
        | TokenKind::As
        | TokenKind::Safe
        | TokenKind::Unsafe
        | TokenKind::Ref
        | TokenKind::Suspend
        | TokenKind::Async
        | TokenKind::Await
        | TokenKind::Yield
        | TokenKind::TypeAlias
        | TokenKind::Type
        | TokenKind::Annotation
        | TokenKind::Static
        | TokenKind::Void
        | TokenKind::This
        | TokenKind::Super
        | TokenKind::Null
        | TokenKind::Throw
        | TokenKind::Try
        | TokenKind::Catch
        | TokenKind::Finally
        | TokenKind::Extends
        | TokenKind::Implements
        | TokenKind::True
        | TokenKind::False
        | TokenKind::BoolLit(_) => Some(ST_KEYWORD),

        // Identifiers — heuristic: starts with uppercase → TYPE, otherwise VARIABLE
        TokenKind::Ident(name) => {
            if name.starts_with(|c: char| c.is_uppercase()) {
                Some(ST_TYPE)
            } else {
                Some(ST_VARIABLE)
            }
        }

        // Strings
        TokenKind::StringLit(_) | TokenKind::FStringLit(_) | TokenKind::CharLit(_) => {
            Some(ST_STRING)
        }

        // Numbers
        TokenKind::IntLit(_)
        | TokenKind::LongLit(_)
        | TokenKind::FloatLit(_)
        | TokenKind::DoubleLit(_) => Some(ST_NUMBER),

        // Comments (the logos lexer skips comments, but handle if present)
        TokenKind::LineComment | TokenKind::BlockComment | TokenKind::DocComment(_) => {
            Some(ST_COMMENT)
        }

        // Punctuation, operators, whitespace, EOF, errors — skip
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Inlay hints
// ---------------------------------------------------------------------------

/// Build inlay hints for the requested range of a document.
fn build_inlay_hints(doc: &DocumentState, range: Range) -> Vec<InlayHint> {
    let bodies = match doc.bodies.as_ref() {
        Some(b) => b,
        None => return Vec::new(),
    };
    let hir = match doc.hir.as_ref() {
        Some(h) => h,
        None => return Vec::new(),
    };

    let mut hints = Vec::new();

    // Only process bodies whose owning Def belongs to this file.
    for (def_id, body) in bodies {
        let belongs_to_file = hir
            .defs
            .get(def_id)
            .map(|d| d.span.file_id == doc.file_id)
            .unwrap_or(false);
        if belongs_to_file {
            collect_hints_from_body(body, doc, range, &mut hints);
        }
    }

    hints
}

/// Recursively collect inlay hints from a typed body.
fn collect_hints_from_body(
    body: &TypedBody,
    doc: &DocumentState,
    range: Range,
    hints: &mut Vec<InlayHint>,
) {
    for stmt in &body.stmts {
        collect_hints_from_stmt(stmt, doc, range, hints);
    }
    if let Some(tail) = &body.tail {
        collect_hints_from_expr(tail, doc, range, hints);
    }
}

/// Collect inlay hints from a single typed statement.
fn collect_hints_from_stmt(
    stmt: &TypedStmt,
    doc: &DocumentState,
    range: Range,
    hints: &mut Vec<InlayHint>,
) {
    match stmt {
        TypedStmt::Let {
            has_annotation: false,
            name,
            ty,
            span,
            init,
            ..
        } => {
            if !ty.is_error() && (span.start as usize) < doc.text.len() {
                // Position hint after the variable name.
                // The span covers the whole let statement; we find the name end
                // by scanning from span.start for `let [mut] <name>`.
                let let_text =
                    &doc.text[span.start as usize..(span.end as usize).min(doc.text.len())];
                if let Some(name_offset) = find_name_end_in_let(let_text, name) {
                    let abs_offset = span.start + name_offset as u32;
                    let pos = doc.line_index.offset_to_position(abs_offset);
                    if position_in_range(pos, range) {
                        hints.push(InlayHint {
                            position: pos,
                            label: InlayHintLabel::String(format!(": {ty}")),
                            kind: Some(InlayHintKind::TYPE),
                            text_edits: None,
                            tooltip: None,
                            padding_left: None,
                            padding_right: None,
                            data: None,
                        });
                    }
                }
            }
            collect_hints_from_expr(init, doc, range, hints);
        }
        TypedStmt::Let { init, .. } => {
            collect_hints_from_expr(init, doc, range, hints);
        }
        TypedStmt::LetElse { scrutinee, .. } => {
            collect_hints_from_expr(scrutinee, doc, range, hints);
        }
        TypedStmt::Expr(e) | TypedStmt::ExprSemi(e) => {
            collect_hints_from_expr(e, doc, range, hints);
        }
    }
}

/// Collect inlay hints from a typed expression (recursively).
fn collect_hints_from_expr(
    expr: &TypedExpr,
    doc: &DocumentState,
    range: Range,
    hints: &mut Vec<InlayHint>,
) {
    match &expr.kind {
        TypedExprKind::Lambda { params, body } => {
            // Emit type hints for lambda params that lack an explicit annotation.
            let lam_text =
                &doc.text[expr.span.start as usize..(expr.span.end as usize).min(doc.text.len())];
            for (pname, pty) in params {
                if !pty.is_error() {
                    if let Some(rel_end) = find_param_name_end(lam_text, pname) {
                        // Heuristic: if the source text immediately after the param
                        // name (before the next `,` or `|`) contains `:`, the user
                        // wrote an explicit type annotation — skip the hint.
                        if lambda_param_has_annotation(lam_text, rel_end) {
                            continue;
                        }
                        let abs_offset = expr.span.start + rel_end as u32;
                        let pos = doc.line_index.offset_to_position(abs_offset);
                        if position_in_range(pos, range) {
                            hints.push(InlayHint {
                                position: pos,
                                label: InlayHintLabel::String(format!(": {pty}")),
                                kind: Some(InlayHintKind::TYPE),
                                text_edits: None,
                                tooltip: None,
                                padding_left: None,
                                padding_right: None,
                                data: None,
                            });
                        }
                    }
                }
            }
            collect_hints_from_expr(body, doc, range, hints);
        }
        TypedExprKind::Block(body) | TypedExprKind::Safe(body) | TypedExprKind::Unsafe(body) => {
            collect_hints_from_body(body, doc, range, hints);
        }
        TypedExprKind::Cast { expr: inner, .. }
        | TypedExprKind::Deref { expr: inner }
        | TypedExprKind::RefMutCreate { expr: inner } => {
            collect_hints_from_expr(inner, doc, range, hints);
        }
        TypedExprKind::DerefAssign { target, value } => {
            collect_hints_from_expr(target, doc, range, hints);
            collect_hints_from_expr(value, doc, range, hints);
        }
        TypedExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_hints_from_expr(cond, doc, range, hints);
            collect_hints_from_body(then_branch, doc, range, hints);
            if let Some(eb) = else_branch {
                collect_hints_from_expr(eb, doc, range, hints);
            }
        }
        TypedExprKind::IfLet {
            expr,
            then_branch,
            else_branch,
            ..
        } => {
            collect_hints_from_expr(expr, doc, range, hints);
            collect_hints_from_body(then_branch, doc, range, hints);
            if let Some(eb) = else_branch {
                collect_hints_from_expr(eb, doc, range, hints);
            }
        }
        TypedExprKind::WhileLet { expr, body, .. } => {
            collect_hints_from_expr(expr, doc, range, hints);
            collect_hints_from_body(body, doc, range, hints);
        }
        TypedExprKind::Match { scrutinee, arms } => {
            collect_hints_from_expr(scrutinee, doc, range, hints);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    collect_hints_from_expr(g, doc, range, hints);
                }
                collect_hints_from_expr(&arm.body, doc, range, hints);
            }
        }
        TypedExprKind::For { iter, body, .. } => {
            collect_hints_from_expr(iter, doc, range, hints);
            collect_hints_from_body(body, doc, range, hints);
        }
        TypedExprKind::While { cond, body } => {
            collect_hints_from_expr(cond, doc, range, hints);
            collect_hints_from_body(body, doc, range, hints);
        }
        TypedExprKind::Loop { body } => {
            collect_hints_from_body(body, doc, range, hints);
        }
        TypedExprKind::Call { callee, args } => {
            collect_hints_from_expr(callee, doc, range, hints);
            // Issue #037: Emit parameter name hints for call arguments.
            if let Some(hir) = doc.hir.as_ref() {
                let fn_name = match &callee.kind {
                    TypedExprKind::LocalVar(n) => Some(n.as_str()),
                    _ => None,
                };
                if let Some(name) = fn_name {
                    emit_param_name_hints(name, args, hir, doc, range, hints);
                }
            }
            for a in args {
                collect_hints_from_expr(a, doc, range, hints);
            }
        }
        TypedExprKind::MethodCall {
            receiver,
            method,
            args,
        } => {
            collect_hints_from_expr(receiver, doc, range, hints);
            // Issue #037: Emit parameter name hints for method call arguments.
            if let Some(hir) = doc.hir.as_ref() {
                emit_param_name_hints(method.as_str(), args, hir, doc, range, hints);
            }
            for a in args {
                collect_hints_from_expr(a, doc, range, hints);
            }
        }
        TypedExprKind::Binary { lhs, rhs, .. } => {
            collect_hints_from_expr(lhs, doc, range, hints);
            collect_hints_from_expr(rhs, doc, range, hints);
        }
        TypedExprKind::Unary { expr: inner, .. } => {
            collect_hints_from_expr(inner, doc, range, hints);
        }
        TypedExprKind::FieldAccess { receiver, .. } => {
            collect_hints_from_expr(receiver, doc, range, hints);
        }
        TypedExprKind::Assign { target, value } => {
            collect_hints_from_expr(target, doc, range, hints);
            collect_hints_from_expr(value, doc, range, hints);
        }
        TypedExprKind::Return(Some(inner)) | TypedExprKind::Break(Some(inner)) => {
            collect_hints_from_expr(inner, doc, range, hints);
        }
        TypedExprKind::Try { inner, .. } => {
            collect_hints_from_expr(inner, doc, range, hints);
        }
        TypedExprKind::Range { start, end, .. } => {
            if let Some(s) = start {
                collect_hints_from_expr(s, doc, range, hints);
            }
            if let Some(e) = end {
                collect_hints_from_expr(e, doc, range, hints);
            }
        }
        TypedExprKind::StringInterp(parts) => {
            for part in parts {
                if let valen_hir::TypedStringPart::Expr(e) = part {
                    collect_hints_from_expr(e, doc, range, hints);
                }
            }
        }
        _ => {}
    }
}

/// Issue #037: Emit parameter name inlay hints before each call argument.
///
/// Looks up the function/method by name in the HIR and pairs each argument
/// with its parameter name. Skips the hint if the argument already uses a
/// named argument syntax or if the argument text matches the parameter name.
fn emit_param_name_hints(
    fn_name: &str,
    args: &[TypedExpr],
    hir: &valen_hir::Hir,
    doc: &DocumentState,
    range: Range,
    hints: &mut Vec<InlayHint>,
) {
    // Find the function definition to get parameter names
    let params = hir.defs.values().find_map(|def| {
        if def.name.as_str() == fn_name {
            if let DefKind::Fn(f) = &def.kind {
                let non_self: Vec<&valen_hir::ParamDef> =
                    f.params.iter().filter(|p| !p.is_self).collect();
                return Some(non_self);
            }
        }
        None
    });

    if let Some(params) = params {
        for (arg, param) in args.iter().zip(params.iter()) {
            let param_name = param.name.as_str();
            // Skip if param name is empty or a single char (not informative)
            if param_name.is_empty() || param_name.len() <= 1 {
                continue;
            }
            // Skip if the argument source text matches the param name (redundant)
            let arg_start = arg.span.start as usize;
            let arg_end = (arg.span.end as usize).min(doc.text.len());
            if arg_start < arg_end {
                let arg_text = doc.text[arg_start..arg_end].trim();
                if arg_text == param_name {
                    continue;
                }
            }
            let pos = doc.line_index.offset_to_position(arg.span.start);
            if position_in_range(pos, range) {
                hints.push(InlayHint {
                    position: pos,
                    label: InlayHintLabel::String(format!("{param_name}: ")),
                    kind: Some(InlayHintKind::PARAMETER),
                    text_edits: None,
                    tooltip: None,
                    padding_left: None,
                    padding_right: Some(false),
                    data: None,
                });
            }
        }
    }
}

/// Find the byte offset (relative to the let statement text) where the variable name ends.
fn find_name_end_in_let(let_text: &str, name: &str) -> Option<usize> {
    // Skip `let` keyword and optional `mut`
    let rest = let_text.strip_prefix("let")?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix("mut ").unwrap_or(rest);
    let rest = rest.trim_start();
    if rest.starts_with(name.as_bytes().first().copied().unwrap_or(0) as char)
        && rest.starts_with(name)
    {
        let after_name = &rest[name.len()..];
        // Make sure the name isn't a prefix of a longer identifier
        if after_name.is_empty()
            || !after_name.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
        {
            let offset_in_let = let_text.len() - rest.len() + name.len();
            return Some(offset_in_let);
        }
    }
    None
}

/// Find the byte offset (relative to the lambda text) where a param name ends.
fn find_param_name_end(lambda_text: &str, name: &str) -> Option<usize> {
    // Search for the param name preceded by `|`, `(`, `,`, or whitespace
    let mut search_from = 0;
    while search_from < lambda_text.len() {
        if let Some(pos) =
            lambda_text[search_from..].find(name.as_bytes().first().copied().unwrap_or(0) as char)
        {
            let abs_pos = search_from + pos;
            let candidate = &lambda_text[abs_pos..];
            if let Some(after) = candidate.strip_prefix(name) {
                let before_ok = abs_pos == 0
                    || lambda_text.as_bytes()[abs_pos - 1].is_ascii_whitespace()
                    || lambda_text.as_bytes()[abs_pos - 1] == b'|'
                    || lambda_text.as_bytes()[abs_pos - 1] == b'('
                    || lambda_text.as_bytes()[abs_pos - 1] == b',';
                let after_ok = after.is_empty()
                    || !after.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_');
                if before_ok && after_ok {
                    return Some(abs_pos + name.len());
                }
            }
            search_from = abs_pos + 1;
        } else {
            break;
        }
    }
    None
}

/// Check whether a lambda parameter at the given position has an explicit type annotation.
///
/// Looks at the source text after the param name (up to the next `,` or `|`) for a `:`.
fn lambda_param_has_annotation(lambda_text: &str, name_end: usize) -> bool {
    let rest = &lambda_text[name_end..];
    for ch in rest.chars() {
        match ch {
            ':' => return true,
            ',' | '|' | ')' => return false,
            _ => {}
        }
    }
    false
}

/// Check whether a position falls within the given range (inclusive start, exclusive end).
fn position_in_range(pos: Position, range: Range) -> bool {
    if pos.line < range.start.line || pos.line > range.end.line {
        return false;
    }
    if pos.line == range.start.line && pos.character < range.start.character {
        return false;
    }
    if pos.line == range.end.line && pos.character > range.end.character {
        return false;
    }
    true
}

// ---------------------------------------------------------------------------
// Typed expression search (for hover / completion)
// ---------------------------------------------------------------------------

/// Find the narrowest typed expression whose span contains the given byte offset.
fn find_expr_at_offset(
    bodies: &indexmap::IndexMap<valen_hir::DefId, TypedBody>,
    offset: u32,
) -> Option<&TypedExpr> {
    let mut best: Option<&TypedExpr> = None;
    for body in bodies.values() {
        find_expr_in_body(body, offset, &mut best);
    }
    best
}

fn find_expr_in_body<'a>(body: &'a TypedBody, offset: u32, best: &mut Option<&'a TypedExpr>) {
    for stmt in &body.stmts {
        match stmt {
            TypedStmt::Let { init, .. } => find_expr_in_expr(init, offset, best),
            TypedStmt::LetElse { scrutinee, .. } => find_expr_in_expr(scrutinee, offset, best),
            TypedStmt::Expr(e) | TypedStmt::ExprSemi(e) => find_expr_in_expr(e, offset, best),
        }
    }
    if let Some(tail) = &body.tail {
        find_expr_in_expr(tail, offset, best);
    }
}

fn find_expr_in_expr<'a>(expr: &'a TypedExpr, offset: u32, best: &mut Option<&'a TypedExpr>) {
    // Spans are half-open intervals [start, end)
    if offset < expr.span.start || offset >= expr.span.end {
        return;
    }
    // This expression contains the offset — check if it's narrower than current best
    let dominated = match best {
        Some(prev) => expr.span.len() < prev.span.len(),
        None => true,
    };
    if dominated {
        *best = Some(expr);
    }
    // Recurse into sub-expressions
    match &expr.kind {
        TypedExprKind::Block(body) | TypedExprKind::Safe(body) | TypedExprKind::Unsafe(body) => {
            find_expr_in_body(body, offset, best);
        }
        TypedExprKind::Cast { expr: inner, .. }
        | TypedExprKind::Deref { expr: inner }
        | TypedExprKind::RefMutCreate { expr: inner } => {
            find_expr_in_expr(inner, offset, best);
        }
        TypedExprKind::DerefAssign { target, value } => {
            find_expr_in_expr(target, offset, best);
            find_expr_in_expr(value, offset, best);
        }
        TypedExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            find_expr_in_expr(cond, offset, best);
            find_expr_in_body(then_branch, offset, best);
            if let Some(eb) = else_branch {
                find_expr_in_expr(eb, offset, best);
            }
        }
        TypedExprKind::IfLet {
            expr,
            then_branch,
            else_branch,
            ..
        } => {
            find_expr_in_expr(expr, offset, best);
            find_expr_in_body(then_branch, offset, best);
            if let Some(eb) = else_branch {
                find_expr_in_expr(eb, offset, best);
            }
        }
        TypedExprKind::WhileLet { expr, body, .. } => {
            find_expr_in_expr(expr, offset, best);
            find_expr_in_body(body, offset, best);
        }
        TypedExprKind::Match { scrutinee, arms } => {
            find_expr_in_expr(scrutinee, offset, best);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    find_expr_in_expr(g, offset, best);
                }
                find_expr_in_expr(&arm.body, offset, best);
            }
        }
        TypedExprKind::For { iter, body, .. } => {
            find_expr_in_expr(iter, offset, best);
            find_expr_in_body(body, offset, best);
        }
        TypedExprKind::While { cond, body } => {
            find_expr_in_expr(cond, offset, best);
            find_expr_in_body(body, offset, best);
        }
        TypedExprKind::Loop { body } => {
            find_expr_in_body(body, offset, best);
        }
        TypedExprKind::Lambda { body, .. } => {
            find_expr_in_expr(body, offset, best);
        }
        TypedExprKind::Call { callee, args } => {
            find_expr_in_expr(callee, offset, best);
            for a in args {
                find_expr_in_expr(a, offset, best);
            }
        }
        TypedExprKind::MethodCall { receiver, args, .. } => {
            find_expr_in_expr(receiver, offset, best);
            for a in args {
                find_expr_in_expr(a, offset, best);
            }
        }
        TypedExprKind::Binary { lhs, rhs, .. } => {
            find_expr_in_expr(lhs, offset, best);
            find_expr_in_expr(rhs, offset, best);
        }
        TypedExprKind::Unary { expr: inner, .. } => {
            find_expr_in_expr(inner, offset, best);
        }
        TypedExprKind::FieldAccess { receiver, .. } => {
            find_expr_in_expr(receiver, offset, best);
        }
        TypedExprKind::Assign { target, value } => {
            find_expr_in_expr(target, offset, best);
            find_expr_in_expr(value, offset, best);
        }
        TypedExprKind::Return(Some(inner)) | TypedExprKind::Break(Some(inner)) => {
            find_expr_in_expr(inner, offset, best);
        }
        TypedExprKind::Try { inner, .. } => {
            find_expr_in_expr(inner, offset, best);
        }
        TypedExprKind::Range { start, end, .. } => {
            if let Some(s) = start {
                find_expr_in_expr(s, offset, best);
            }
            if let Some(e) = end {
                find_expr_in_expr(e, offset, best);
            }
        }
        TypedExprKind::StringInterp(parts) => {
            for part in parts {
                if let valen_hir::TypedStringPart::Expr(e) = part {
                    find_expr_in_expr(e, offset, best);
                }
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Local variable collection (for completion)
// ---------------------------------------------------------------------------

/// Collect local variables visible at the given offset from typed bodies.
///
/// Only walks the body whose enclosing function's span contains `offset`,
/// so variables from unrelated functions are excluded.
pub fn collect_local_variables_pub(
    bodies: &indexmap::IndexMap<valen_hir::DefId, TypedBody>,
    offset: u32,
    hir: Option<&valen_hir::Hir>,
) -> Vec<(String, Ty)> {
    collect_local_variables(bodies, offset, hir)
}

fn collect_local_variables(
    bodies: &indexmap::IndexMap<valen_hir::DefId, TypedBody>,
    offset: u32,
    hir: Option<&valen_hir::Hir>,
) -> Vec<(String, Ty)> {
    let mut vars: indexmap::IndexMap<String, Ty> = indexmap::IndexMap::new();

    // Find which DefId's span contains the cursor, then walk only that body.
    if let Some(hir) = hir {
        for (&def_id, body) in bodies {
            if let Some(def) = hir.defs.get(&def_id) {
                if offset >= def.span.start && offset < def.span.end {
                    let mut raw = Vec::new();
                    collect_vars_from_body(body, offset, &mut raw);
                    // IndexMap::insert overwrites, so the last (innermost) binding wins
                    for (name, ty) in raw {
                        vars.insert(name, ty);
                    }
                }
            }
        }
    } else {
        // Fallback: no HIR, walk all bodies (best effort)
        for body in bodies.values() {
            let mut raw = Vec::new();
            collect_vars_from_body(body, offset, &mut raw);
            for (name, ty) in raw {
                vars.insert(name, ty);
            }
        }
    }

    vars.into_iter().collect()
}

/// Extract variable bindings from a pattern, pairing each binding name with `ty`.
///
/// For simple `Binding` patterns the name maps directly to `ty`.  Nested
/// patterns (struct destructuring, tuples, or-patterns, `@`-patterns) are
/// walked recursively; inner fields all receive the outer `ty` as a
/// conservative approximation since the HIR does not carry per-field types
/// for patterns today.
fn extract_pattern_bindings(pattern: &valen_ast::Pattern, ty: &Ty) -> Vec<(String, Ty)> {
    let mut out = Vec::new();
    collect_pattern_bindings(pattern, ty, &mut out);
    out
}

fn collect_pattern_bindings(pattern: &valen_ast::Pattern, ty: &Ty, out: &mut Vec<(String, Ty)>) {
    use valen_ast::Pattern;
    match pattern {
        Pattern::Binding(bp) => {
            out.push((bp.name.to_string(), ty.clone()));
        }
        Pattern::At(at) => {
            // `name @ sub` — the outer name binds the whole value.
            out.push((at.name.to_string(), ty.clone()));
            collect_pattern_bindings(&at.pattern, ty, out);
        }
        Pattern::Struct(sp) => {
            for field in &sp.fields {
                if let Some(sub) = &field.pattern {
                    collect_pattern_bindings(sub, ty, out);
                } else {
                    // Shorthand field: `Foo { x }` — field name = variable name.
                    out.push((field.name.to_string(), ty.clone()));
                }
            }
        }
        Pattern::Tuple(pats, _) => {
            for p in pats {
                collect_pattern_bindings(p, ty, out);
            }
        }
        Pattern::Or(pats, _) => {
            // All alternatives must bind the same names, so just take the first.
            if let Some(first) = pats.first() {
                collect_pattern_bindings(first, ty, out);
            }
        }
        Pattern::VariantShorthand(vs) => {
            for field in &vs.fields {
                if let Some(sub) = &field.pattern {
                    collect_pattern_bindings(sub, ty, out);
                } else {
                    out.push((field.name.to_string(), ty.clone()));
                }
            }
        }
        Pattern::Wildcard(_) | Pattern::Literal(_) | Pattern::Path(_) | Pattern::Range(_) => {
            // No variable bindings introduced.
        }
    }
}

fn collect_vars_from_body(body: &TypedBody, offset: u32, vars: &mut Vec<(String, Ty)>) {
    for stmt in &body.stmts {
        match stmt {
            TypedStmt::Let {
                name,
                ty,
                span,
                init,
                ..
            } => {
                // Only include let bindings after the initializer has ended,
                // so the variable is not visible inside its own initializer.
                if span.start < offset && offset >= init.span.end {
                    vars.push((name.to_string(), ty.clone()));
                }
                // Also recurse into init expression for nested blocks
                collect_vars_from_expr(init, offset, vars);
            }
            TypedStmt::LetElse {
                pattern,
                scrutinee,
                ty,
                span,
                ..
            } => {
                collect_vars_from_expr(scrutinee, offset, vars);
                // Pattern bindings are visible after the let-else statement
                if span.start < offset {
                    let bindings = extract_pattern_bindings(pattern, ty);
                    vars.extend(bindings);
                }
            }
            TypedStmt::Expr(e) | TypedStmt::ExprSemi(e) => {
                collect_vars_from_expr(e, offset, vars);
            }
        }
    }
    if let Some(tail) = &body.tail {
        collect_vars_from_expr(tail, offset, vars);
    }
}

fn collect_vars_from_expr(expr: &TypedExpr, offset: u32, vars: &mut Vec<(String, Ty)>) {
    // Only recurse into blocks/bodies that contain the offset (half-open [start, end))
    if offset < expr.span.start || offset >= expr.span.end {
        return;
    }
    match &expr.kind {
        TypedExprKind::Block(body) | TypedExprKind::Safe(body) | TypedExprKind::Unsafe(body) => {
            collect_vars_from_body(body, offset, vars);
        }
        TypedExprKind::If {
            then_branch,
            else_branch,
            ..
        } => {
            collect_vars_from_body(then_branch, offset, vars);
            if let Some(eb) = else_branch {
                collect_vars_from_expr(eb, offset, vars);
            }
        }
        TypedExprKind::IfLet {
            then_branch,
            else_branch,
            ..
        } => {
            collect_vars_from_body(then_branch, offset, vars);
            if let Some(eb) = else_branch {
                collect_vars_from_expr(eb, offset, vars);
            }
        }
        TypedExprKind::WhileLet { body, .. } => {
            collect_vars_from_body(body, offset, vars);
        }
        TypedExprKind::For { var, iter, body } => {
            // The iteration variable is in scope inside the body
            if body.stmts.first().is_some_and(|s| match s {
                TypedStmt::Let { span, .. } | TypedStmt::LetElse { span, .. } => {
                    span.start <= offset
                }
                TypedStmt::Expr(e) | TypedStmt::ExprSemi(e) => e.span.start <= offset,
            }) || body.tail.as_ref().is_some_and(|t| t.span.start <= offset)
            {
                vars.push((var.to_string(), iter.ty.clone()));
            }
            collect_vars_from_body(body, offset, vars);
        }
        TypedExprKind::While { body, .. } => {
            collect_vars_from_body(body, offset, vars);
        }
        TypedExprKind::Loop { body } => {
            collect_vars_from_body(body, offset, vars);
        }
        TypedExprKind::Match { arms, .. } => {
            for arm in arms {
                collect_vars_from_expr(&arm.body, offset, vars);
            }
        }
        TypedExprKind::Lambda { body, .. } => {
            collect_vars_from_expr(body, offset, vars);
        }
        _ => {}
    }
}

/// Format a typed expression for hover display.
fn format_typed_expr_hover(expr: &TypedExpr) -> Option<String> {
    if expr.ty.is_error() {
        return None;
    }
    match &expr.kind {
        TypedExprKind::LocalVar(name) => Some(format!("(variable) {name}: {}", expr.ty)),
        TypedExprKind::IntLit(v) => Some(format!("{v}: {}", expr.ty)),
        TypedExprKind::LongLit(v) => Some(format!("{v}: {}", expr.ty)),
        TypedExprKind::FloatLit(v) => Some(format!("{v}: {}", expr.ty)),
        TypedExprKind::Float32Lit(v) => Some(format!("{v}: {}", expr.ty)),
        TypedExprKind::CharLit(v) => Some(format!("'{v}': {}", expr.ty)),
        TypedExprKind::StringLit(v) => Some(format!("\"{v}\": {}", expr.ty)),
        TypedExprKind::BoolLit(v) => Some(format!("{v}: {}", expr.ty)),
        TypedExprKind::UnitLit => Some(format!("(): {}", expr.ty)),
        _ => Some(format!("{}", expr.ty)),
    }
}

/// Valen language keywords offered for completion.
/// Keywords shown in the General context (expression + statement start).
///
/// Issue #044: `override`, `open`, `abstract`, `sealed`, `internal`, `private`,
/// `typealias`, and `annotation` are included so they appear in completions.
const EXPR_KEYWORDS: &[&str] = &[
    "if",
    "else",
    "match",
    "for",
    "while",
    "loop",
    "return",
    "break",
    "continue",
    "true",
    "false",
    "safe",
    "unsafe",
    "ref mut",
    "let",
    "mut",
    "fn",
    "pub",
    "class",
    "data",
    "enum",
    "trait",
    "impl",
    "import",
    "override",
    "open",
    "abstract",
    "sealed",
    "internal",
    "private",
    "typealias",
    "annotation",
];

// ---------------------------------------------------------------------------
// LanguageServer trait implementation
// ---------------------------------------------------------------------------

impl LanguageServer for ServerState {
    type Error = ResponseError;
    type NotifyResult = ControlFlow<async_lsp::Result<()>>;

    fn initialize(
        &mut self,
        params: InitializeParams,
    ) -> futures::future::BoxFuture<'static, Result<InitializeResult, Self::Error>> {
        let root_uri = params
            .workspace_folders
            .as_ref()
            .and_then(|folders| folders.first())
            .map(|f| f.uri.clone())
            .or_else(|| {
                #[allow(deprecated)]
                params.root_uri.clone()
            });
        if let Some(root) = root_uri.as_ref().and_then(|u| u.to_file_path().ok()) {
            self.workspace_root = Some(root.clone());
            self.index_workspace(&root);
        }
        Box::pin(async {
            Ok(InitializeResult {
                capabilities: ServerCapabilities {
                    text_document_sync: Some(TextDocumentSyncCapability::Kind(
                        TextDocumentSyncKind::FULL,
                    )),
                    definition_provider: Some(OneOf::Left(true)),
                    completion_provider: Some(CompletionOptions {
                        trigger_characters: Some(vec![
                            ".".to_string(),
                            ":".to_string(),
                            " ".to_string(),
                        ]),
                        ..Default::default()
                    }),
                    hover_provider: Some(HoverProviderCapability::Simple(true)),
                    inlay_hint_provider: Some(OneOf::Left(true)),
                    semantic_tokens_provider: Some(
                        SemanticTokensServerCapabilities::SemanticTokensOptions(
                            SemanticTokensOptions {
                                legend: SemanticTokensLegend {
                                    token_types: vec![
                                        SemanticTokenType::KEYWORD,
                                        SemanticTokenType::TYPE,
                                        SemanticTokenType::FUNCTION,
                                        SemanticTokenType::VARIABLE,
                                        SemanticTokenType::STRING,
                                        SemanticTokenType::NUMBER,
                                        SemanticTokenType::COMMENT,
                                        SemanticTokenType::PARAMETER,
                                    ],
                                    token_modifiers: vec![],
                                },
                                full: Some(SemanticTokensFullOptions::Bool(true)),
                                range: None,
                                ..Default::default()
                            },
                        ),
                    ),
                    ..Default::default()
                },
                server_info: Some(ServerInfo {
                    name: "valen-lsp".to_string(),
                    version: Some(env!("CARGO_PKG_VERSION").to_string()),
                }),
            })
        })
    }

    fn shutdown(&mut self, _: ()) -> futures::future::BoxFuture<'static, Result<(), Self::Error>> {
        Box::pin(async { Ok(()) })
    }

    fn did_open(&mut self, params: DidOpenTextDocumentParams) -> Self::NotifyResult {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        let version = params.text_document.version;
        self.analyze_and_publish(uri, text, version);
        ControlFlow::Continue(())
    }

    fn did_change(&mut self, params: DidChangeTextDocumentParams) -> Self::NotifyResult {
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        // We advertise TextDocumentSyncKind::FULL, so each content change
        // should be a full-document replacement (range == None).
        // Reject incremental (range-based) changes to avoid silent corruption.
        if let Some(change) = params.content_changes.into_iter().last() {
            if change.range.is_some() {
                // Incremental change received despite FULL sync mode — log and
                // skip to avoid corruption.
                tracing::warn!("ignoring incremental text change; server advertises FULL sync");
            } else {
                self.analyze_and_publish(uri, change.text, version);
            }
        }
        ControlFlow::Continue(())
    }

    fn did_save(&mut self, _params: DidSaveTextDocumentParams) -> Self::NotifyResult {
        ControlFlow::Continue(())
    }

    fn did_close(&mut self, params: DidCloseTextDocumentParams) -> Self::NotifyResult {
        let uri = params.text_document.uri;
        self.documents.remove(&uri);
        self.client
            .publish_diagnostics(PublishDiagnosticsParams {
                uri,
                diagnostics: vec![],
                version: None,
            })
            .ok();
        ControlFlow::Continue(())
    }

    fn definition(
        &mut self,
        params: GotoDefinitionParams,
    ) -> futures::future::BoxFuture<'static, Result<Option<GotoDefinitionResponse>, Self::Error>>
    {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let result = self.find_definition_at(&uri, pos);
        Box::pin(async { Ok(result) })
    }

    fn completion(
        &mut self,
        params: CompletionParams,
    ) -> futures::future::BoxFuture<'static, Result<Option<CompletionResponse>, Self::Error>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let items = self.build_completions(&uri, pos);
        Box::pin(async { Ok(Some(CompletionResponse::Array(items))) })
    }

    fn hover(
        &mut self,
        params: HoverParams,
    ) -> futures::future::BoxFuture<'static, Result<Option<Hover>, Self::Error>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let result = self.build_hover(&uri, pos);
        Box::pin(async { Ok(result) })
    }

    fn semantic_tokens_full(
        &mut self,
        params: SemanticTokensParams,
    ) -> futures::future::BoxFuture<'static, Result<Option<SemanticTokensResult>, Self::Error>>
    {
        let uri = params.text_document.uri;
        let result = self.build_semantic_tokens(&uri);
        Box::pin(async { Ok(result) })
    }

    fn inlay_hint(
        &mut self,
        params: InlayHintParams,
    ) -> futures::future::BoxFuture<'static, Result<Option<Vec<InlayHint>>, Self::Error>> {
        let uri = params.text_document.uri;
        let result = self
            .documents
            .get(&uri)
            .map(|doc| build_inlay_hints(doc, params.range));
        Box::pin(async { Ok(result) })
    }
}
