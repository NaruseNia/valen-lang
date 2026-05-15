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
}

impl ServerState {
    pub fn new_router(client: ClientSocket) -> Router<Self> {
        let this = Self {
            client,
            documents: HashMap::new(),
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
        let hir = doc.hir.as_ref()?;

        for def in hir.defs.values() {
            if def.name.as_str() == name {
                let range = doc.line_index.span_to_range(def.span);
                return Some(GotoDefinitionResponse::Scalar(Location {
                    uri: uri.clone(),
                    range,
                }));
            }
        }
        None
    }

    // -----------------------------------------------------------------------
    // Completion helpers
    // -----------------------------------------------------------------------

    /// Build completion items from keywords and HIR definitions.
    fn build_completions(&self, uri: &Url) -> Vec<CompletionItem> {
        let mut items = Vec::new();

        // 1) Keyword completions
        for kw in VALEN_KEYWORDS {
            items.push(CompletionItem {
                label: kw.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                ..Default::default()
            });
        }

        // 2) Scope-based completions from HIR
        if let Some(doc) = self.documents.get(uri) {
            if let Some(hir) = doc.hir.as_ref() {
                for def in hir.defs.values() {
                    let (label, kind) = match &def.kind {
                        DefKind::Fn(_) => (def.name.to_string(), CompletionItemKind::FUNCTION),
                        DefKind::Class(_)
                        | DefKind::DataClass(_)
                        | DefKind::Enum(_)
                        | DefKind::TypeAlias(_)
                        | DefKind::AnnotationClass(_) => {
                            (def.name.to_string(), CompletionItemKind::CLASS)
                        }
                        DefKind::Trait(_) => (def.name.to_string(), CompletionItemKind::INTERFACE),
                        DefKind::Impl(_) => continue,
                    };
                    items.push(CompletionItem {
                        label,
                        kind: Some(kind),
                        ..Default::default()
                    });
                }

                // 3) Method completions from type_methods and trait_impls
                for methods in hir.type_methods.values() {
                    for &mid in methods {
                        if let Some(def) = hir.defs.get(&mid) {
                            items.push(CompletionItem {
                                label: def.name.to_string(),
                                kind: Some(CompletionItemKind::METHOD),
                                ..Default::default()
                            });
                        }
                    }
                }
                for entry in &hir.trait_impls {
                    for &mid in &entry.methods {
                        if let Some(def) = hir.defs.get(&mid) {
                            items.push(CompletionItem {
                                label: def.name.to_string(),
                                kind: Some(CompletionItemKind::METHOD),
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

        for def in hir.defs.values() {
            if def.name.as_str() != name {
                continue;
            }
            let value = format_def_hover(def);
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
fn format_def_hover(def: &valen_hir::Def) -> String {
    match &def.kind {
        DefKind::Fn(f) => {
            let params_str: Vec<String> = f
                .params
                .iter()
                .map(|p| {
                    let mutability = if p.mutable { "mut " } else { "" };
                    format!("{}{}: {}", mutability, p.name, p.ty)
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
                .map(|p| format!("{}: {}", p.name, p.ty))
                .collect();
            format!("class {}({})", def.name, params_str.join(", "))
        }
        DefKind::DataClass(dc) => {
            let params_str: Vec<String> = dc
                .ctor_params
                .iter()
                .map(|p| format!("{}: {}", p.name, p.ty))
                .collect();
            format!("data class {}({})", def.name, params_str.join(", "))
        }
        DefKind::Enum(e) => {
            let variants: Vec<&str> = e.variants.iter().map(|v| v.name.as_str()).collect();
            format!("enum {} {{ {} }}", def.name, variants.join(", "))
        }
        DefKind::Trait(t) => {
            let method_names: Vec<String> = t
                .methods
                .iter()
                .map(|&mid| format!("method#{mid}"))
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
        _: InitializeParams,
    ) -> futures::future::BoxFuture<'static, Result<InitializeResult, Self::Error>> {
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
        let items = self.build_completions(&uri);
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
