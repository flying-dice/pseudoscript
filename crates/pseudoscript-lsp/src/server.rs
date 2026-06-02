//! The tower-lsp server: a workspace-aware document store plus protocol
//! plumbing.
//!
//! All language logic lives in [`pseudoscript_lsp_core::analysis`]; this layer owns the
//! [`Project`] (the loaded `.pds` modules, with open buffers overlaying disk)
//! and translates LSP notifications/requests into calls against it. Diagnostics
//! are published for every module so cross-file issues surface in the file that
//! causes them.

use std::collections::HashMap;
use std::sync::Mutex;

use pseudoscript_syntax::LineIndex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    CompletionOptions, CompletionParams, CompletionResponse, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DocumentFormattingParams,
    DocumentHighlight, DocumentHighlightKind, DocumentHighlightParams, DocumentSymbolParams,
    DocumentSymbolResponse, FoldingRange, FoldingRangeParams, GotoDefinitionParams,
    GotoDefinitionResponse, Hover, HoverParams, HoverProviderCapability, InitializeParams,
    InitializeResult, InitializedParams, InlayHint, InlayHintParams, Location, MessageType, OneOf,
    PrepareRenameResponse, ReferenceParams, RenameParams, SemanticTokensFullOptions,
    SemanticTokensOptions, SemanticTokensParams, SemanticTokensResult,
    SemanticTokensServerCapabilities, ServerCapabilities, ServerInfo, TextDocumentPositionParams,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit, Url, WorkDoneProgressOptions,
    WorkspaceEdit, WorkspaceSymbolParams,
};
use tower_lsp::{Client, LanguageServer};

use crate::workspace::Project;
use pseudoscript_lsp_core::analysis;

/// The `PseudoScript` language server.
pub struct Backend {
    client: Client,
    /// The loaded workspace: every known `.pds` module, open buffers overlaying
    /// on-disk text. The source of truth for every feature.
    project: Mutex<Project>,
}

impl Backend {
    /// Builds a backend bound to `client` with an empty project.
    #[must_use]
    pub fn new(client: Client) -> Self {
        Self {
            client,
            project: Mutex::new(Project::default()),
        }
    }

    /// Locks the project store.
    fn project(&self) -> std::sync::MutexGuard<'_, Project> {
        self.project.lock().expect("project lock is never poisoned")
    }

    /// The current source for `uri`, cloned out of the store.
    fn source(&self, uri: &Url) -> Option<String> {
        self.project().source(uri).map(ToOwned::to_owned)
    }

    /// The `(source, module-FQN)` for the active document `uri`.
    fn active(&self, uri: &Url) -> Option<(String, String)> {
        let project = self.project();
        Some((
            project.source(uri)?.to_owned(),
            project.fqn(uri)?.to_owned(),
        ))
    }

    /// Maps resolved occurrences back to LSP [`Location`]s via the file index.
    fn locate_occurrences(&self, occurrences: &[analysis::Occurrence]) -> Vec<Location> {
        let project = self.project();
        occurrences
            .iter()
            .filter_map(|occ| {
                let uri = project.uri_of(&occ.fqn)?;
                let source = project.source_of(&occ.fqn)?;
                let index = LineIndex::new(source);
                Some(Location {
                    uri,
                    range: pseudoscript_lsp_core::convert::span_to_range(source, &index, occ.span),
                })
            })
            .collect()
    }

    /// Recomputes diagnostics for the whole workspace and publishes each
    /// module's set against its own file.
    ///
    /// The payload is computed under the lock (reusing the cached parses and
    /// resolved workspace), then the awaited publishes run without holding it.
    async fn publish_all(&self) {
        let payloads = self.project().diagnostics();
        for (uri, diagnostics) in payloads {
            self.client
                .publish_diagnostics(uri, diagnostics, None)
                .await;
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        if let Some(root) = workspace_root(&params) {
            self.project().discover(&root);
        }
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                document_formatting_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".to_owned(), ":".to_owned()]),
                    ..CompletionOptions::default()
                }),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                inlay_hint_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                document_highlight_provider: Some(OneOf::Left(true)),
                folding_range_provider: Some(
                    tower_lsp::lsp_types::FoldingRangeProviderCapability::Simple(true),
                ),
                rename_provider: Some(OneOf::Right(tower_lsp::lsp_types::RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                })),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            work_done_progress_options: WorkDoneProgressOptions::default(),
                            legend: analysis::semantic_legend(),
                            range: Some(false),
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                        },
                    ),
                ),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: "pseudoscript-lsp".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "pseudoscript-lsp ready")
            .await;
        self.publish_all().await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let doc = params.text_document;
        self.project().open(doc.uri, doc.text);
        self.publish_all().await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // FULL sync: the last change carries the entire new document text.
        if let Some(change) = params.content_changes.into_iter().next_back() {
            self.project().change(params.text_document.uri, change.text);
            self.publish_all().await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        // The unsaved overlay is gone; re-sync to disk and reconverge so other
        // files stop seeing the closed buffer's edits.
        self.project().close(&params.text_document.uri);
        self.publish_all().await;
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let Some(text) = self.source(&params.text_document.uri) else {
            return Ok(None);
        };
        // On parse error the buffer is left untouched (no edit).
        Ok(analysis::format_edit(&text).map(|edit| vec![edit]))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let pos = params.text_document_position_params;
        let Some((text, fqn)) = self.active(&pos.text_document.uri) else {
            return Ok(None);
        };
        let mut project = self.project();
        Ok(analysis::hover(
            project.workspace(),
            &fqn,
            &text,
            pos.position,
        ))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let pos = params.text_document_position_params;
        let Some((text, fqn)) = self.active(&pos.text_document.uri) else {
            return Ok(None);
        };
        let target = {
            let mut project = self.project();
            analysis::definition(project.workspace(), &fqn, &text, pos.position)
        };
        let Some(target) = target else {
            return Ok(None);
        };
        // Map the resolved target FQN back to its file and range there.
        let project = self.project();
        let (Some(target_uri), Some(target_src)) =
            (project.uri_of(&target.fqn), project.source_of(&target.fqn))
        else {
            return Ok(None);
        };
        let index = LineIndex::new(target_src);
        let location = Location {
            uri: target_uri,
            range: pseudoscript_lsp_core::convert::span_to_range(target_src, &index, target.span),
        };
        Ok(Some(GotoDefinitionResponse::Scalar(location)))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let pos = params.text_document_position;
        let Some((text, fqn)) = self.active(&pos.text_document.uri) else {
            return Ok(None);
        };
        let items = {
            let mut project = self.project();
            analysis::completion(project.workspace(), &fqn, &text, pos.position)
        };
        Ok((!items.is_empty()).then_some(CompletionResponse::Array(items)))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let Some(text) = self.source(&params.text_document.uri) else {
            return Ok(None);
        };
        Ok(Some(SemanticTokensResult::Tokens(
            analysis::semantic_tokens(&text),
        )))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let Some(text) = self.source(&params.text_document.uri) else {
            return Ok(None);
        };
        let symbols = analysis::document_symbols(&text);
        Ok((!symbols.is_empty()).then_some(DocumentSymbolResponse::Nested(symbols)))
    }

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        let Some(text) = self.source(&params.text_document.uri) else {
            return Ok(None);
        };
        Ok(Some(analysis::folding_ranges(&text)))
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let Some((text, fqn)) = self.active(&params.text_document.uri) else {
            return Ok(None);
        };
        let mut project = self.project();
        Ok(Some(analysis::inlay_hints(
            project.workspace(),
            &fqn,
            &text,
        )))
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<tower_lsp::lsp_types::SymbolInformation>>> {
        let mut project = self.project();
        // Snapshot the file map (owned) before borrowing the workspace, so the
        // `locate` closure does not re-lock the project.
        let locations = project.fqn_locations();
        let symbols = analysis::workspace_symbols(project.workspace(), &params.query, |fqn| {
            locations.get(fqn).cloned()
        });
        Ok(Some(symbols))
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let pos = params.text_document_position;
        let Some((text, fqn)) = self.active(&pos.text_document.uri) else {
            return Ok(None);
        };
        let include_decl = params.context.include_declaration;
        let occurrences = {
            let mut project = self.project();
            let modules = project.module_pairs();
            let offset = pseudoscript_lsp_core::convert::position_to_offset(&text, pos.position);
            analysis::references(
                project.workspace(),
                &modules,
                &fqn,
                &text,
                offset,
                include_decl,
            )
        };
        Ok(Some(self.locate_occurrences(&occurrences)))
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        let pos = params.text_document_position_params;
        let Some((text, fqn)) = self.active(&pos.text_document.uri) else {
            return Ok(None);
        };
        let offset = pseudoscript_lsp_core::convert::position_to_offset(&text, pos.position);
        let spans = {
            let mut project = self.project();
            analysis::highlights(project.workspace(), &fqn, &text, offset)
        };
        let index = LineIndex::new(&text);
        let highlights = spans
            .into_iter()
            .map(|span| DocumentHighlight {
                range: pseudoscript_lsp_core::convert::span_to_range(&text, &index, span),
                kind: Some(DocumentHighlightKind::TEXT),
            })
            .collect();
        Ok(Some(highlights))
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let Some((text, fqn)) = self.active(&params.text_document.uri) else {
            return Ok(None);
        };
        let mut project = self.project();
        Ok(
            analysis::renameable(project.workspace(), &fqn, &text, params.position)
                .map(PrepareRenameResponse::Range),
        )
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let pos = params.text_document_position;
        let Some((text, fqn)) = self.active(&pos.text_document.uri) else {
            return Ok(None);
        };
        let occurrences = {
            let mut project = self.project();
            let modules = project.module_pairs();
            let offset = pseudoscript_lsp_core::convert::position_to_offset(&text, pos.position);
            analysis::references(project.workspace(), &modules, &fqn, &text, offset, true)
        };
        if occurrences.is_empty() {
            return Ok(None);
        }

        let project = self.project();
        let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
        for occ in occurrences {
            let (Some(uri), Some(source)) = (project.uri_of(&occ.fqn), project.source_of(&occ.fqn))
            else {
                continue;
            };
            let index = LineIndex::new(source);
            changes.entry(uri).or_default().push(TextEdit {
                range: pseudoscript_lsp_core::convert::span_to_range(source, &index, occ.span),
                new_text: params.new_name.clone(),
            });
        }
        Ok(Some(WorkspaceEdit {
            changes: Some(changes),
            ..WorkspaceEdit::default()
        }))
    }
}

/// The workspace root URI from the initialize params: the first workspace
/// folder, else the (deprecated but widely sent) `root_uri`.
fn workspace_root(params: &InitializeParams) -> Option<Url> {
    params
        .workspace_folders
        .as_ref()
        .and_then(|folders| folders.first())
        .map(|folder| folder.uri.clone())
        .or_else(|| params.root_uri.clone())
}
