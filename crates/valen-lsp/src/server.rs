//! LSP backend state and LanguageServer omnitrait implementation.

use std::collections::HashMap;
use std::ops::ControlFlow;

use async_lsp::lsp_types::*;
use async_lsp::router::Router;
use async_lsp::{ClientSocket, LanguageClient, LanguageServer, ResponseError};

use valen_ast::token::TokenKind;
use valen_hir::DefKind;

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
}

/// The Valen LSP server state.
pub struct ServerState {
    client: ClientSocket,
    documents: HashMap<Url, DocumentState>,
    workspace_root: Option<std::path::PathBuf>,
}

impl ServerState {
    pub fn new_router(client: ClientSocket) -> Router<Self> {
        let this = Self {
            client,
            documents: HashMap::new(),
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

    fn index_workspace(&mut self, root: &std::path::Path) {
        let vln_files = find_vln_files(root);
        for path in vln_files {
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Ok(uri) = Url::from_file_path(&path) {
                    self.documents.entry(uri).or_insert_with(|| {
                        let (doc_state, _) = analyze_document(&text);
                        doc_state
                    });
                }
            }
        }
    }

    fn analyze_and_publish(&mut self, uri: Url, text: String, version: i32) {
        let (doc_state, diags) = analyze_document(&text);
        self.documents.insert(uri.clone(), doc_state);
        self.client
            .publish_diagnostics(PublishDiagnosticsParams {
                uri,
                diagnostics: diags,
                version: Some(version),
            })
            .ok();
    }

    fn find_definition_at(&self, uri: &Url, position: Position) -> Option<GotoDefinitionResponse> {
        let doc = self.documents.get(uri)?;
        let offset = doc.line_index.position_to_offset(position);
        let name = extract_word_at(&doc.text, offset)?;

        // Search current document first
        if let Some(hir) = doc.hir.as_ref() {
            for def in hir.defs.values() {
                if def.name.as_str() == name {
                    let range = doc.line_index.span_to_range(def.span);
                    return Some(GotoDefinitionResponse::Scalar(Location {
                        uri: uri.clone(),
                        range,
                    }));
                }
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
            CompletionContext::General => self.build_general_completions(doc),
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
        let type_name = self.resolve_receiver_type(doc, hir, receiver);

        if let Some(tn) = &type_name {
            // Fields from class/data class ctor params
            for def in hir.defs.values() {
                match &def.kind {
                    DefKind::Class(c) if def.name.as_str() == tn => {
                        for param in &c.ctor_params {
                            items.push(CompletionItem {
                                label: param.name.to_string(),
                                kind: Some(CompletionItemKind::FIELD),
                                ..Default::default()
                            });
                        }
                    }
                    DefKind::DataClass(dc) if def.name.as_str() == tn => {
                        for param in &dc.ctor_params {
                            items.push(CompletionItem {
                                label: param.name.to_string(),
                                kind: Some(CompletionItemKind::FIELD),
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
                        items.push(CompletionItem {
                            label: mdef.name.to_string(),
                            kind: Some(CompletionItemKind::METHOD),
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
                            items.push(CompletionItem {
                                label: mdef.name.to_string(),
                                kind: Some(CompletionItemKind::METHOD),
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
    ) -> Option<String> {
        if receiver == "self" {
            return self.find_enclosing_class(doc, hir);
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

    fn find_enclosing_class(&self, _doc: &DocumentState, hir: &valen_hir::Hir) -> Option<String> {
        // Heuristic: return the first user-defined class/data class
        for def in hir.defs.values() {
            if hir.prelude_ids.contains(&def.id) {
                continue;
            }
            match &def.kind {
                DefKind::Class(_) | DefKind::DataClass(_) => {
                    return Some(def.name.to_string());
                }
                _ => {}
            }
        }
        None
    }

    fn build_type_completions(&self, doc: &DocumentState) -> Vec<CompletionItem> {
        let mut items = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for ty in BUILTIN_TYPES {
            let label = ty.to_string();
            if seen.insert(label.clone()) {
                items.push(CompletionItem {
                    label,
                    kind: Some(CompletionItemKind::TYPE_PARAMETER),
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

    fn build_general_completions(&self, doc: &DocumentState) -> Vec<CompletionItem> {
        let mut items = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for kw in VALEN_KEYWORDS {
            items.push(CompletionItem {
                label: kw.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                ..Default::default()
            });
            seen.insert(kw.to_string());
        }

        for ty in BUILTIN_TYPES {
            let label = ty.to_string();
            if seen.insert(label.clone()) {
                items.push(CompletionItem {
                    label,
                    kind: Some(CompletionItemKind::TYPE_PARAMETER),
                    ..Default::default()
                });
            }
        }

        if let Some(hir) = doc.hir.as_ref() {
            for def in hir.defs.values() {
                if hir.prelude_ids.contains(&def.id) || def.name.is_empty() {
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
                    DefKind::Enum(_) => (def.name.to_string(), CompletionItemKind::ENUM, None),
                    DefKind::Trait(_) => {
                        (def.name.to_string(), CompletionItemKind::INTERFACE, None)
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
        None
    }

    // -----------------------------------------------------------------------
    // Semantic tokens helpers
    // -----------------------------------------------------------------------

    /// Produce semantic tokens for the entire document using the lexer.
    fn build_semantic_tokens(&self, uri: &Url) -> Option<SemanticTokensResult> {
        let doc = self.documents.get(uri)?;
        let file_id = valen_ast::FileId(0);
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
            let length = span.end.saturating_sub(span.start);
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
pub fn analyze_document(text: &str) -> (DocumentState, Vec<async_lsp::lsp_types::Diagnostic>) {
    let line_index = convert::LineIndex::new(text);
    let file_id = valen_ast::FileId(0);

    let parse_result = valen_parser::parse(text, file_id);
    let mut diags = convert::to_lsp_diagnostics(&parse_result.diagnostics, &line_index);

    let resolve_result = valen_hir::resolve::resolve(&parse_result.items);
    diags.extend(convert::to_lsp_diagnostics(
        &resolve_result.diagnostics,
        &line_index,
    ));

    let hir = if !resolve_result.diagnostics.has_errors() {
        let tc = valen_hir::ty::type_check(&resolve_result.hir, &parse_result.items);
        diags.extend(convert::to_lsp_diagnostics(&tc.diagnostics, &line_index));
        Some(resolve_result.hir)
    } else {
        Some(resolve_result.hir)
    };

    let doc = DocumentState {
        text: text.to_string(),
        line_index,
        items: parse_result.items,
        hir,
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
    let ret = f
        .return_ty
        .as_ref()
        .map(|t| format!(" -> {t}"))
        .unwrap_or_default();
    format!("fn {}({}){}", name, params.join(", "), ret)
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

const BUILTIN_TYPES: &[&str] = &[
    "Int", "Long", "Float", "Double", "Char", "Bool", "Byte", "Short", "String", "Unit", "Nothing",
    "Option", "Result",
];

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
        let rest = trimmed
            .strip_prefix("let mut ")
            .or_else(|| trimmed.strip_prefix("let "))?;
        if !rest.starts_with(var_name) {
            continue;
        }
        let after_name = &rest[var_name.len()..];
        if !after_name.starts_with(':') {
            continue;
        }
        let ty_part = after_name[1..].trim_start();
        let ty_end = ty_part
            .find(|c: char| c == '=' || c == ';' || c == '{')
            .unwrap_or(ty_part.len());
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
    if base.ends_with(",") {
        // Could be in param list type position — check for `:`
        let before_comma = &base[..base.len() - 1].trim_end();
        if before_comma
            .rfind(':')
            .is_some_and(|i| before_comma[i..].find('(').is_none())
        {
            return CompletionContext::TypePosition;
        }
    }

    // After keywords that expect a name — suppress completions
    for kw in &["fn ", "let ", "let mut ", "class ", "data class ", "enum ", "trait ", "typealias "] {
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

fn find_vln_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(find_vln_files(&path));
            } else if path.extension().is_some_and(|e| e == "vln") {
                files.push(path);
            }
        }
    }
    files
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
                        trigger_characters: Some(vec![".".to_string(), ":".to_string()]),
                        ..Default::default()
                    }),
                    hover_provider: Some(HoverProviderCapability::Simple(true)),
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
        if let Some(change) = params.content_changes.into_iter().last() {
            self.analyze_and_publish(uri, change.text, version);
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
}
