//! LSP backend state and LanguageServer omnitrait implementation.

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
}

impl ServerState {
    pub fn new_router(client: ClientSocket) -> Router<Self> {
        let this = Self {
            client,
            documents: HashMap::new(),
            file_ids: HashMap::new(),
            next_file_id: 0,
            workspace_root: None,
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

    // TODO(#041): index_workspace runs synchronously during initialize, blocking
    // the handshake. Move to a background task post-initialization.
    fn index_workspace(&mut self, root: &std::path::Path) {
        let vln_files = find_vln_files(root);
        for path in vln_files {
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Ok(uri) = Url::from_file_path(&path) {
                    if !self.documents.contains_key(&uri) {
                        let file_id = self.file_id_for(&uri);
                        let (doc_state, _) = analyze_document(&text, file_id);
                        self.documents.insert(uri, doc_state);
                    }
                }
            }
        }
    }

    fn analyze_and_publish(&mut self, uri: Url, text: String, version: i32) {
        let file_id = self.file_id_for(&uri);
        let (doc_state, diags) = analyze_document(&text, file_id);
        self.documents.insert(uri.clone(), doc_state);

        // Re-analyze all other open documents so that cross-file dependents
        // pick up changes (e.g. new/renamed definitions).
        let other_uris: Vec<Url> = self
            .documents
            .keys()
            .filter(|u| **u != uri)
            .cloned()
            .collect();
        for other_uri in other_uris {
            if let Some(doc) = self.documents.get(&other_uri) {
                let other_text = doc.text.clone();
                let other_fid = self.file_id_for(&other_uri);
                let (new_state, new_diags) = analyze_document(&other_text, other_fid);
                self.documents.insert(other_uri.clone(), new_state);
                self.client
                    .publish_diagnostics(PublishDiagnosticsParams {
                        uri: other_uri,
                        diagnostics: new_diags,
                        version: None,
                    })
                    .ok();
            }
        }

        self.client
            .publish_diagnostics(PublishDiagnosticsParams {
                uri,
                diagnostics: diags,
                version: Some(version),
            })
            .ok();
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
                        items.push(CompletionItem {
                            label: variant.name.to_string(),
                            kind: Some(CompletionItemKind::ENUM_MEMBER),
                            detail,
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
                                    items.push(CompletionItem {
                                        label: mdef.name.to_string(),
                                        kind: Some(CompletionItemKind::FUNCTION),
                                        detail: Some(format_fn_signature(&mdef.name, f)),
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
                                items.push(CompletionItem {
                                    label: mdef.name.to_string(),
                                    kind: Some(CompletionItemKind::METHOD),
                                    detail: Some(format_fn_signature(&mdef.name, f)),
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
                            items.push(CompletionItem {
                                label: param.name.to_string(),
                                kind: Some(CompletionItemKind::FIELD),
                                detail: Some(format!("{}", param.ty)),
                                ..Default::default()
                            });
                        }
                    }
                    DefKind::DataClass(dc) if def.name.as_str() == tn => {
                        for param in &dc.ctor_params {
                            items.push(CompletionItem {
                                label: param.name.to_string(),
                                kind: Some(CompletionItemKind::FIELD),
                                detail: Some(format!("{}", param.ty)),
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
                        let detail = if let DefKind::Fn(f) = &mdef.kind {
                            Some(format_fn_signature(&mdef.name, f))
                        } else {
                            None
                        };
                        items.push(CompletionItem {
                            label: mdef.name.to_string(),
                            kind: Some(CompletionItemKind::METHOD),
                            detail,
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
                            let detail = if let DefKind::Fn(f) = &mdef.kind {
                                Some(format_fn_signature(&mdef.name, f))
                            } else {
                                None
                            };
                            items.push(CompletionItem {
                                label: mdef.name.to_string(),
                                kind: Some(CompletionItemKind::METHOD),
                                detail,
                                ..Default::default()
                            });
                        }
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
                let kind = match &def.kind {
                    DefKind::Class(_) | DefKind::DataClass(_) => CompletionItemKind::CLASS,
                    DefKind::Enum(_) => CompletionItemKind::ENUM,
                    DefKind::Trait(_) => CompletionItemKind::INTERFACE,
                    DefKind::TypeAlias(_) => CompletionItemKind::CLASS,
                    _ => continue,
                };
                let label = def.name.to_string();
                if seen.insert(label.clone()) {
                    items.push(CompletionItem {
                        label,
                        kind: Some(kind),
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
                            items.push(CompletionItem {
                                label,
                                kind: Some(CompletionItemKind::TYPE_PARAMETER),
                                detail,
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
                        items.push(CompletionItem {
                            label: def.name.to_string(),
                            kind: Some(CompletionItemKind::CLASS),
                            detail: Some(sig),
                            ..Default::default()
                        });
                    }
                    DefKind::DataClass(dc) => {
                        let sig = format_class_signature(&def.name, &dc.ctor_params);
                        items.push(CompletionItem {
                            label: def.name.to_string(),
                            kind: Some(CompletionItemKind::CLASS),
                            detail: Some(sig),
                            ..Default::default()
                        });
                    }
                    _ => {}
                }
            }
        }

        items
    }

    fn build_general_completions(&self, doc: &DocumentState, offset: u32) -> Vec<CompletionItem> {
        let mut items = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // Local variables from typed bodies (sorted above keywords)
        if let Some(bodies) = doc.bodies.as_ref() {
            let locals = collect_local_variables(bodies, offset, doc.hir.as_ref());
            for (name, ty) in locals {
                if seen.insert(name.clone()) {
                    items.push(CompletionItem {
                        label: name,
                        kind: Some(CompletionItemKind::VARIABLE),
                        detail: Some(format!("{ty}")),
                        sort_text: Some(format!("0_{}", items.len())),
                        ..Default::default()
                    });
                }
            }
        }

        for kw in VALEN_KEYWORDS {
            items.push(CompletionItem {
                label: kw.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                ..Default::default()
            });
            seen.insert(kw.to_string());
        }

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
                if def.name.is_empty() {
                    continue;
                }
                // Skip prelude fn defs (operator trait methods) but keep types
                if hir.prelude_ids.contains(&def.id) && matches!(&def.kind, DefKind::Fn(_)) {
                    continue;
                }
                let (label, kind, detail) = match &def.kind {
                    DefKind::Fn(f) => {
                        let sig = format_fn_signature(&def.name, f);
                        (
                            def.name.to_string(),
                            CompletionItemKind::FUNCTION,
                            Some(sig),
                        )
                    }
                    DefKind::Class(c) => {
                        let sig = format_class_signature(&def.name, &c.ctor_params);
                        (def.name.to_string(), CompletionItemKind::CLASS, Some(sig))
                    }
                    DefKind::DataClass(dc) => {
                        let sig = format_class_signature(&def.name, &dc.ctor_params);
                        (def.name.to_string(), CompletionItemKind::CLASS, Some(sig))
                    }
                    DefKind::Enum(e) => {
                        let variants: Vec<&str> =
                            e.variants.iter().map(|v| v.name.as_str()).collect();
                        (
                            def.name.to_string(),
                            CompletionItemKind::ENUM,
                            Some(format!("{{ {} }}", variants.join(", "))),
                        )
                    }
                    DefKind::Trait(t) => {
                        let methods: Vec<String> = t
                            .methods
                            .iter()
                            .filter_map(|&mid| hir.defs.get(&mid).map(|d| d.name.to_string()))
                            .collect();
                        (
                            def.name.to_string(),
                            CompletionItemKind::INTERFACE,
                            Some(format!("trait {{ {} }}", methods.join(", "))),
                        )
                    }
                    DefKind::TypeAlias(ta) => {
                        let detail = format!("typealias {} = {}", def.name, ta.target);
                        (
                            def.name.to_string(),
                            CompletionItemKind::CLASS,
                            Some(detail),
                        )
                    }
                    DefKind::AnnotationClass(_) => {
                        (def.name.to_string(), CompletionItemKind::CLASS, None)
                    }
                    DefKind::Impl(_) => continue,
                };
                if seen.insert(label.clone()) {
                    items.push(CompletionItem {
                        label,
                        kind: Some(kind),
                        detail,
                        ..Default::default()
                    });
                }
            }

            // self keyword (if any fn has a self param)
            let has_self = hir.defs.values().any(|d| {
                if let DefKind::Fn(f) = &d.kind {
                    f.params.first().is_some_and(|p| p.is_self)
                } else {
                    false
                }
            });
            if has_self && seen.insert("self".to_string()) {
                items.push(CompletionItem {
                    label: "self".to_string(),
                    kind: Some(CompletionItemKind::KEYWORD),
                    ..Default::default()
                });
            }

            // Function parameters (from all fn defs in this file)
            for def in hir.defs.values() {
                if let DefKind::Fn(f) = &def.kind {
                    for param in &f.params {
                        if param.is_self || param.name.is_empty() {
                            continue;
                        }
                        let label = param.name.to_string();
                        if seen.insert(label.clone()) {
                            let mut ty_str = format!("{}", param.ty);
                            // Annotate type params with bounds
                            if let valen_hir::TyRef::Unresolved(tp) = &param.ty {
                                for (bn, bounds) in &f.generic_bounds {
                                    if bn == tp && !bounds.is_empty() {
                                        ty_str = format!("{tp}: {}", bounds.join(" + "));
                                        break;
                                    }
                                }
                            }
                            items.push(CompletionItem {
                                label,
                                kind: Some(CompletionItemKind::VARIABLE),
                                detail: Some(ty_str),
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
                            format!("type {name}")
                        } else {
                            format!("{name}: {}", bounds.join(" + "))
                        };
                        return Some(Hover {
                            contents: HoverContents::Scalar(MarkedString::LanguageString(
                                LanguageString {
                                    language: "valen".to_string(),
                                    value,
                                },
                            )),
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
            let value = format_def_hover(def, hir);
            return Some(Hover {
                contents: HoverContents::Scalar(MarkedString::LanguageString(LanguageString {
                    language: "valen".to_string(),
                    value,
                })),
                range: Some(doc.line_index.span_to_range(def.span)),
            });
        }

        // Search typed bodies for expression type info at cursor
        if let Some(bodies) = doc.bodies.as_ref() {
            if let Some(expr) = find_expr_at_offset(bodies, offset) {
                if let Some(hover_text) = format_typed_expr_hover(expr) {
                    return Some(Hover {
                        contents: HoverContents::Scalar(MarkedString::LanguageString(
                            LanguageString {
                                language: "valen".to_string(),
                                value: hover_text,
                            },
                        )),
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

        for (kind, span) in &tokens {
            let token_type = match classify_token(kind) {
                Some(t) => t,
                None => continue,
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
) -> (DocumentState, Vec<async_lsp::lsp_types::Diagnostic>) {
    let line_index = convert::LineIndex::new(text);

    let parse_result = valen_parser::parse(text, file_id);
    let mut diags = convert::to_lsp_diagnostics(&parse_result.diagnostics, &line_index);

    let resolve_result = valen_hir::resolve::resolve(&parse_result.items);
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

    let (hir, bodies) = if !resolve_result.diagnostics.has_errors() {
        let tc = valen_hir::ty::type_check(&resolve_result.hir, &parse_result.items);
        diags.extend(convert::to_lsp_diagnostics(&tc.diagnostics, &line_index));
        (Some(resolve_result.hir), Some(tc.bodies))
    } else {
        (Some(resolve_result.hir), None)
    };

    let doc = DocumentState {
        text: text.to_string(),
        line_index,
        items: parse_result.items,
        hir,
        bodies,
    };

    (doc, diags)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn format_fn_signature(name: &str, f: &valen_hir::FnDef) -> String {
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
                format!("{}: {}", p.name, p.ty)
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
        .map(|t| format!(" -> {t}"))
        .unwrap_or_default();
    format!("fn {}{generics}({}){}", name, params.join(", "), ret)
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

fn tyref_to_type_name(ty: &valen_hir::TyRef) -> Option<String> {
    match ty {
        valen_hir::TyRef::Named(n) => Some(n.to_string()),
        valen_hir::TyRef::Generic(n, _) => Some(n.to_string()),
        valen_hir::TyRef::Prim(p) => Some(format!("{p}")),
        _ => None,
    }
}

fn find_let_type_annotation(source: &str, var_name: &str) -> Option<String> {
    for line in source.lines() {
        let trimmed = line.trim();
        // Match: let [mut] name: Type = ...
        // or:    let [mut] name: Type;
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
        if !after_name.starts_with(':') {
            continue;
        }
        let ty_part = after_name[1..].trim_start();
        let ty_end = ty_part.find(['=', ';', '{']).unwrap_or(ty_part.len());
        let ty_str = ty_part[..ty_end].trim();
        // Strip generics for simple lookup
        let base = ty_str.split('<').next().unwrap_or(ty_str).trim();
        if !base.is_empty() {
            return Some(base.to_string());
        }
    }
    None
}

enum CompletionContext {
    TypePosition,
    ImplTarget,
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

fn is_dot_context(before: &str) -> bool {
    let trimmed = before.trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_');
    trimmed.trim_end().ends_with('.')
}

fn extract_receiver_before_dot(before: &str) -> &str {
    let trimmed = before.trim_end();
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

/// Format a HIR definition for hover display.
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
            format!("fn {}({}){}", def.name, params_str.join(", "), ret)
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
        DefKind::AnnotationClass(_) => {
            format!("annotation class {}", def.name)
        }
    }
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
        | TokenKind::Data
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
        | TokenKind::Suspend
        | TokenKind::Async
        | TokenKind::Await
        | TokenKind::Yield
        | TokenKind::TypeAlias
        | TokenKind::Type
        | TokenKind::Annotation
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

    let mut hints = Vec::new();

    for body in bodies.values() {
        collect_hints_from_body(body, doc, range, &mut hints);
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
            if !ty.is_error() {
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
        TypedExprKind::Block(body) | TypedExprKind::Safe(body) => {
            collect_hints_from_body(body, doc, range, hints);
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
            for a in args {
                collect_hints_from_expr(a, doc, range, hints);
            }
        }
        TypedExprKind::MethodCall { receiver, args, .. } => {
            collect_hints_from_expr(receiver, doc, range, hints);
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
        TypedExprKind::Block(body) | TypedExprKind::Safe(body) => {
            find_expr_in_body(body, offset, best);
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
        TypedExprKind::Block(body) | TypedExprKind::Safe(body) => {
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
        TypedExprKind::For { var, iter, body } => {
            // The iteration variable is in scope inside the body
            if body.stmts.first().is_some_and(|s| match s {
                TypedStmt::Let { span, .. } => span.start <= offset,
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
const VALEN_KEYWORDS: &[&str] = &[
    "fn",
    "let",
    "mut",
    "if",
    "else",
    "match",
    "class",
    "data",
    "enum",
    "trait",
    "impl",
    "pub",
    "return",
    "for",
    "while",
    "loop",
    "break",
    "continue",
    "import",
    "package",
    "safe",
    "sealed",
    "open",
    "abstract",
    "override",
    "annotation",
    "typealias",
    "type",
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
