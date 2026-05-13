//! LSP backend state and LanguageServer omnitrait implementation.

use std::collections::HashMap;
use std::ops::ControlFlow;

use async_lsp::lsp_types::*;
use async_lsp::router::Router;
use async_lsp::{ClientSocket, LanguageClient, LanguageServer, ResponseError};

use crate::convert;

/// Per-document analysis state.
#[allow(dead_code)]
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
        let line_index = convert::LineIndex::new(&text);
        let file_id = valen_ast::FileId(0);

        let parse_result = valen_parser::parse(&text, file_id);
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

        self.documents.insert(
            uri.clone(),
            DocumentState {
                text,
                line_index,
                items: parse_result.items,
                hir,
            },
        );

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
}

fn extract_word_at(text: &str, offset: u32) -> Option<&str> {
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
}
