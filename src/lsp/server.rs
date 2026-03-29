//! LSP server implementation using tower-lsp.

use std::collections::HashMap;
use std::sync::Mutex;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use crate::analyzer::{SemanticError, analyze};
use crate::lexer::token::Span;
use crate::lexer::{LexError, tokenize};
use crate::parser::{ParseError, parse};

/// Per-document state cached by the server.
struct DocumentState {
    /// Full source text.
    source: String,
    /// Line start offsets (byte offset of each line start).
    line_starts: Vec<usize>,
    /// Cached function definitions: (name, span_start, span_end).
    /// Built from AST on open/change for scope-aware goto-definition.
    fn_defs: Vec<(String, usize, usize)>,
    /// Cached struct definitions: (name, span_start, span_end).
    struct_defs: Vec<(String, usize, usize)>,
    /// Cached variable bindings: (name, scope_depth, span_start, span_end).
    /// Reserved for future scope-aware variable goto-definition.
    #[allow(dead_code)]
    var_defs: Vec<(String, u32, usize, usize)>,
}

impl DocumentState {
    fn new(source: String) -> Self {
        let line_starts: Vec<usize> = std::iter::once(0)
            .chain(source.match_indices('\n').map(|(i, _)| i + 1))
            .collect();

        // Build symbol index from AST (best-effort — ignore parse errors).
        let mut fn_defs = Vec::new();
        let mut struct_defs = Vec::new();
        let var_defs = Vec::new();

        if let Ok(tokens) = crate::lexer::tokenize(&source) {
            if let Ok(program) = crate::parser::parse(tokens) {
                for item in &program.items {
                    match item {
                        crate::parser::ast::Item::FnDef(f) => {
                            let span = f.body.span();
                            fn_defs.push((f.name.clone(), span.start, span.end));
                        }
                        crate::parser::ast::Item::StructDef(s) => {
                            struct_defs.push((s.name.clone(), s.span.start, s.span.end));
                        }
                        _ => {}
                    }
                }
            }
        }

        Self {
            source,
            line_starts,
            fn_defs,
            struct_defs,
            var_defs,
        }
    }

    /// Converts a byte offset to an LSP Position (0-based line, 0-based UTF-16 char).
    fn offset_to_position(&self, offset: usize) -> Position {
        let line = self
            .line_starts
            .partition_point(|&start| start <= offset)
            .saturating_sub(1);
        let line_start = self.line_starts[line];
        let col = offset.saturating_sub(line_start);
        Position::new(line as u32, col as u32)
    }

    /// Converts a Fajar Span to an LSP Range.
    fn span_to_range(&self, span: Span) -> Range {
        Range::new(
            self.offset_to_position(span.start),
            self.offset_to_position(span.end),
        )
    }
}

/// The Fajar Lang LSP backend.
struct FajarLspBackend {
    client: Client,
    documents: Mutex<HashMap<Url, DocumentState>>,
}

impl FajarLspBackend {
    fn new(client: Client) -> Self {
        Self {
            client,
            documents: Mutex::new(HashMap::new()),
        }
    }

    /// Runs the full analysis pipeline and publishes diagnostics.
    async fn publish_diagnostics(&self, uri: Url) {
        // Collect diagnostics synchronously under the lock, then publish async after dropping it.
        let diagnostics = {
            let docs = self.documents.lock().expect("lsp state lock");
            let doc = match docs.get(&uri) {
                Some(d) => d,
                None => return,
            };
            collect_diagnostics(&doc.source, doc)
        };

        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for FajarLspBackend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".into(), ":".into()]),
                    ..Default::default()
                }),
                definition_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".into(), ",".into()]),
                    retrigger_characters: None,
                    work_done_progress_options: Default::default(),
                }),
                rename_provider: Some(OneOf::Left(true)),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: SemanticTokensLegend {
                                token_types: vec![
                                    SemanticTokenType::KEYWORD,
                                    SemanticTokenType::COMMENT,
                                    SemanticTokenType::STRING,
                                    SemanticTokenType::NUMBER,
                                    SemanticTokenType::FUNCTION,
                                    SemanticTokenType::VARIABLE,
                                    SemanticTokenType::TYPE,
                                    SemanticTokenType::OPERATOR,
                                    SemanticTokenType::PARAMETER,
                                    SemanticTokenType::PROPERTY,
                                    SemanticTokenType::NAMESPACE,
                                    SemanticTokenType::MACRO,
                                    SemanticTokenType::DECORATOR,
                                ],
                                token_modifiers: vec![
                                    SemanticTokenModifier::DECLARATION,
                                    SemanticTokenModifier::DEFINITION,
                                    SemanticTokenModifier::READONLY,
                                ],
                            },
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            range: None,
                            ..Default::default()
                        },
                    ),
                ),
                inlay_hint_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
                code_lens_provider: Some(CodeLensOptions {
                    resolve_provider: Some(false),
                }),
                selection_range_provider: Some(SelectionRangeProviderCapability::Simple(true)),
                linked_editing_range_provider: Some(LinkedEditingRangeServerCapabilities::Simple(
                    true,
                )),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                call_hierarchy_provider: Some(CallHierarchyServerCapability::Simple(true)),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "fajar-lang-lsp".into(),
                version: Some("0.3.0".into()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Fajar Lang LSP server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let doc = DocumentState::new(params.text_document.text);
        self.documents
            .lock()
            .expect("lsp state lock")
            .insert(uri.clone(), doc);
        self.publish_diagnostics(uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        if let Some(change) = params.content_changes.into_iter().last() {
            let doc = DocumentState::new(change.text);
            self.documents
                .lock()
                .expect("lsp state lock")
                .insert(uri.clone(), doc);
            self.publish_diagnostics(uri).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        self.documents.lock().expect("lsp state lock").remove(&uri);
        // Clear diagnostics
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let docs = self.documents.lock().expect("lsp state lock");
        let doc = match docs.get(uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        // Find the word at the cursor position
        let word = word_at_position(&doc.source, &doc.line_starts, pos);
        if word.is_empty() {
            return Ok(None);
        }

        // Check if it's a keyword, builtin, or type
        if let Some(info) = keyword_info(&word) {
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: info,
                }),
                range: None,
            }));
        }

        if let Some(info) = builtin_info(&word) {
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: info,
                }),
                range: None,
            }));
        }

        if let Some(info) = type_info(&word) {
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: info,
                }),
                range: None,
            }));
        }

        Ok(None)
    }

    async fn completion(&self, _params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let mut items = Vec::new();

        // Keywords
        for kw in KEYWORDS {
            items.push(CompletionItem {
                label: kw.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("keyword".into()),
                ..Default::default()
            });
        }

        // Built-in functions
        for (name, sig) in BUILTINS {
            items.push(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some(sig.to_string()),
                ..Default::default()
            });
        }

        // Types
        for ty in TYPES {
            items.push(CompletionItem {
                label: ty.to_string(),
                kind: Some(CompletionItemKind::TYPE_PARAMETER),
                detail: Some("type".into()),
                ..Default::default()
            });
        }

        // Annotations
        for ann in ANNOTATIONS {
            items.push(CompletionItem {
                label: ann.to_string(),
                kind: Some(CompletionItemKind::SNIPPET),
                detail: Some("context annotation".into()),
                ..Default::default()
            });
        }

        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let docs = self.documents.lock().expect("lsp state lock");
        let doc = match docs.get(uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        let word = word_at_position(&doc.source, &doc.line_starts, pos);
        if word.is_empty() {
            return Ok(None);
        }

        // AST-based definition lookup: search cached fn/struct definitions first.
        // This is more accurate than text search (won't match inside comments/strings).
        for (name, span_start, _span_end) in &doc.fn_defs {
            if name == &word {
                let start = doc.offset_to_position(*span_start);
                let end = doc.offset_to_position(*span_start + word.len());
                return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                    uri: uri.clone(),
                    range: Range::new(start, end),
                })));
            }
        }
        for (name, span_start, _span_end) in &doc.struct_defs {
            if name == &word {
                let start = doc.offset_to_position(*span_start);
                let end = doc.offset_to_position(*span_start + word.len());
                return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                    uri: uri.clone(),
                    range: Range::new(start, end),
                })));
            }
        }

        // Fallback: text search for let/const/enum/trait definitions
        let fallback_patterns = [
            format!("let {word}"),
            format!("let mut {word}"),
            format!("enum {word}"),
            format!("const {word}"),
            format!("trait {word}"),
        ];
        for pat in &fallback_patterns {
            if let Some(offset) = doc.source.find(pat.as_str()) {
                let name_offset = offset + pat.len() - word.len();
                let start = doc.offset_to_position(name_offset);
                let end = doc.offset_to_position(name_offset + word.len());
                return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                    uri: uri.clone(),
                    range: Range::new(start, end),
                })));
            }
        }

        Ok(None)
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = &params.text_document.uri;
        let docs = self.documents.lock().expect("lsp state lock");
        let doc = match docs.get(uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        let symbols = extract_symbols(&doc.source, doc);
        Ok(Some(DocumentSymbolResponse::Flat(symbols)))
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = &params.text_document.uri;
        let docs = self.documents.lock().expect("lsp state lock");
        let doc = match docs.get(uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        let mut actions = Vec::new();

        for diag in &params.context.diagnostics {
            let code = match &diag.code {
                Some(NumberOrString::String(c)) => c.as_str(),
                _ => continue,
            };

            match code {
                "SE007" => {
                    // ImmutableAssignment — suggest adding mut
                    let line = diag.range.start.line as usize;
                    if line < doc.line_starts.len() {
                        let line_start = doc.line_starts[line];
                        let line_end = if line + 1 < doc.line_starts.len() {
                            doc.line_starts[line + 1]
                        } else {
                            doc.source.len()
                        };
                        let line_text = &doc.source[line_start..line_end];
                        if let Some(pos) = line_text.find("let ") {
                            let insert_pos = Position::new(diag.range.start.line, (pos + 4) as u32);
                            let edit =
                                TextEdit::new(Range::new(insert_pos, insert_pos), "mut ".into());
                            let mut changes = HashMap::new();
                            changes.insert(uri.clone(), vec![edit]);
                            actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                                title: "Add `mut` to make variable mutable".into(),
                                kind: Some(CodeActionKind::QUICKFIX),
                                diagnostics: Some(vec![diag.clone()]),
                                edit: Some(WorkspaceEdit::new(changes)),
                                ..Default::default()
                            }));
                        }
                    }
                }
                "SE009" => {
                    // UnusedVariable — suggest prefixing with _
                    let start = diag.range.start;
                    let end = diag.range.end;
                    let start_offset =
                        doc.line_starts[start.line as usize] + start.character as usize;
                    let end_offset = doc.line_starts[end.line as usize] + end.character as usize;
                    if end_offset <= doc.source.len() {
                        let var_name = &doc.source[start_offset..end_offset];
                        if !var_name.starts_with('_') {
                            let edit = TextEdit::new(diag.range, format!("_{var_name}"));
                            let mut changes = HashMap::new();
                            changes.insert(uri.clone(), vec![edit]);
                            actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                                title: "Prefix with `_` to suppress warning".to_string(),
                                kind: Some(CodeActionKind::QUICKFIX),
                                diagnostics: Some(vec![diag.clone()]),
                                edit: Some(WorkspaceEdit::new(changes)),
                                ..Default::default()
                            }));
                        }
                    }
                }
                "SE001" | "SE002" => {
                    // UndefinedVariable/Function — suggest typo fix using lsp_v3.
                    // Extract the undefined name from the diagnostic message.
                    let msg = &diag.message;
                    // Messages like "SE001: undefined variable 'foo'" or "SE002: undefined function 'bar'"
                    if let Some(start_q) = msg.find('\'') {
                        if let Some(end_q) = msg[start_q + 1..].find('\'') {
                            let typo_name = &msg[start_q + 1..start_q + 1 + end_q];
                            // Collect known names from AST cache
                            let candidates: Vec<String> = doc
                                .fn_defs
                                .iter()
                                .map(|(n, _, _)| n.clone())
                                .chain(doc.struct_defs.iter().map(|(n, _, _)| n.clone()))
                                .collect();
                            if let Some(suggestion) = crate::lsp_v3::diagnostics::suggest_typo_fix(
                                typo_name,
                                &candidates.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                                2,
                            ) {
                                let edit = TextEdit::new(diag.range, suggestion.clone());
                                let mut changes = HashMap::new();
                                changes.insert(uri.clone(), vec![edit]);
                                actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                                    title: format!("Did you mean `{suggestion}`?"),
                                    kind: Some(CodeActionKind::QUICKFIX),
                                    diagnostics: Some(vec![diag.clone()]),
                                    edit: Some(WorkspaceEdit::new(changes)),
                                    ..Default::default()
                                }));
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if actions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(actions))
        }
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let docs = self.documents.lock().expect("lsp state lock");
        let doc = match docs.get(uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        // Find the function name before the cursor (look back for identifier before '(')
        let line = pos.line as usize;
        if line >= doc.line_starts.len() {
            return Ok(None);
        }
        let line_start = doc.line_starts[line];
        let col = pos.character as usize;
        let line_text = &doc.source[line_start..];
        let before_cursor = if col <= line_text.len() {
            &line_text[..col]
        } else {
            line_text
        };

        // Find the matching function call — look for last "name(" pattern
        let fn_name = extract_fn_name_before_paren(before_cursor);
        if fn_name.is_empty() {
            return Ok(None);
        }

        // Look up the builtin signature
        if let Some((_, sig)) = BUILTINS.iter().find(|(n, _)| *n == fn_name) {
            return Ok(Some(SignatureHelp {
                signatures: vec![SignatureInformation {
                    label: sig.to_string(),
                    documentation: None,
                    parameters: None,
                    active_parameter: None,
                }],
                active_signature: Some(0),
                active_parameter: None,
            }));
        }

        // Search for user-defined function signatures
        if let Some(sig) = find_fn_signature(&doc.source, &fn_name) {
            return Ok(Some(SignatureHelp {
                signatures: vec![SignatureInformation {
                    label: sig,
                    documentation: None,
                    parameters: None,
                    active_parameter: None,
                }],
                active_signature: Some(0),
                active_parameter: None,
            }));
        }

        Ok(None)
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = &params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let new_name = &params.new_name;

        // Validate the new name using lsp_v3 refactoring rules (keyword/naming checks).
        // Done before acquiring the lock to avoid holding MutexGuard across .await.
        if let Err(reason) = crate::lsp_v3::refactoring::validate_rename(new_name) {
            self.client
                .log_message(MessageType::WARNING, format!("rename rejected: {reason}"))
                .await;
            return Ok(None);
        }

        let docs = self.documents.lock().expect("lsp state lock");
        let doc = match docs.get(uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        let word = word_at_position(&doc.source, &doc.line_starts, pos);
        if word.is_empty() {
            return Ok(None);
        }

        // Find all occurrences of the word in the document
        let mut edits = Vec::new();
        for (i, line_text) in doc.source.lines().enumerate() {
            let mut col = 0;
            while let Some(found) = line_text[col..].find(&word) {
                let start_col = col + found;
                let end_col = start_col + word.len();
                // Verify it's a whole word match (not part of a larger identifier)
                let before_ok = start_col == 0
                    || !line_text.as_bytes()[start_col - 1].is_ascii_alphanumeric()
                        && line_text.as_bytes()[start_col - 1] != b'_';
                let after_ok = end_col >= line_text.len()
                    || !line_text.as_bytes()[end_col].is_ascii_alphanumeric()
                        && line_text.as_bytes()[end_col] != b'_';
                if before_ok && after_ok {
                    edits.push(TextEdit {
                        range: Range {
                            start: Position {
                                line: i as u32,
                                character: start_col as u32,
                            },
                            end: Position {
                                line: i as u32,
                                character: end_col as u32,
                            },
                        },
                        new_text: new_name.clone(),
                    });
                }
                col = end_col;
            }
        }

        if edits.is_empty() {
            return Ok(None);
        }

        let mut changes = HashMap::new();
        changes.insert(uri.clone(), edits);
        Ok(Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let docs = self.documents.lock().expect("lsp state lock");
        let doc = match docs.get(&uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        let tokens = generate_semantic_tokens(&doc.source);
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: tokens,
        })))
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let uri = params.text_document.uri;
        let docs = self.documents.lock().expect("lsp state lock");
        let doc = match docs.get(&uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        let hints = generate_inlay_hints(&doc.source, doc);
        Ok(Some(hints))
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let docs = self.documents.lock().expect("lsp state lock");
        let doc = match docs.get(&uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        let word = word_at_position(&doc.source, &doc.line_starts, pos);
        if word.is_empty() {
            return Ok(None);
        }

        let locations = find_all_references(&doc.source, &word, &uri);
        if locations.is_empty() {
            Ok(None)
        } else {
            Ok(Some(locations))
        }
    }

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        let uri = params.text_document.uri;
        let docs = self.documents.lock().expect("lsp state lock");
        let doc = match docs.get(&uri) {
            Some(d) => d,
            None => return Ok(None),
        };
        Ok(Some(compute_folding_ranges(&doc.source)))
    }

    async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        let uri = params.text_document.uri;
        let docs = self.documents.lock().expect("lsp state lock");
        let doc = match docs.get(&uri) {
            Some(d) => d,
            None => return Ok(None),
        };
        Ok(Some(compute_code_lenses(&doc.source, &uri)))
    }

    async fn selection_range(
        &self,
        params: SelectionRangeParams,
    ) -> Result<Option<Vec<SelectionRange>>> {
        let uri = &params.text_document.uri;
        let docs = self.documents.lock().expect("lsp state lock");
        let doc = match docs.get(uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        let mut ranges = Vec::new();
        for pos in &params.positions {
            let offset = position_to_offset(&doc.line_starts, *pos);
            let (ws, we) = find_word_range_at(&doc.source, offset);
            let ls = doc.line_starts.get(pos.line as usize).copied().unwrap_or(0);
            let le = doc
                .line_starts
                .get(pos.line as usize + 1)
                .copied()
                .unwrap_or(doc.source.len());

            let inner = SelectionRange {
                range: Range::new(doc.offset_to_position(ws), doc.offset_to_position(we)),
                parent: Some(Box::new(SelectionRange {
                    range: Range::new(doc.offset_to_position(ls), doc.offset_to_position(le)),
                    parent: Some(Box::new(SelectionRange {
                        range: Range::new(
                            Position::new(0, 0),
                            doc.offset_to_position(doc.source.len()),
                        ),
                        parent: None,
                    })),
                })),
            };
            ranges.push(inner);
        }
        Ok(Some(ranges))
    }

    async fn linked_editing_range(
        &self,
        params: LinkedEditingRangeParams,
    ) -> Result<Option<LinkedEditingRanges>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let docs = self.documents.lock().expect("lsp state lock");
        let doc = match docs.get(uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        let word = word_at_position(&doc.source, &doc.line_starts, pos);
        if word.is_empty() {
            return Ok(None);
        }

        let mut ranges = Vec::new();
        let bytes = doc.source.as_bytes();
        let mut i = 0;
        while i < doc.source.len() {
            if let Some(idx) = doc.source[i..].find(&word) {
                let abs = i + idx;
                let before_ok =
                    abs == 0 || (!bytes[abs - 1].is_ascii_alphanumeric() && bytes[abs - 1] != b'_');
                let end = abs + word.len();
                let after_ok = end >= bytes.len()
                    || (!bytes[end].is_ascii_alphanumeric() && bytes[end] != b'_');
                if before_ok && after_ok {
                    ranges.push(Range::new(
                        doc.offset_to_position(abs),
                        doc.offset_to_position(end),
                    ));
                }
                i = end;
            } else {
                break;
            }
        }

        if ranges.len() <= 1 {
            return Ok(None);
        }
        Ok(Some(LinkedEditingRanges {
            ranges,
            word_pattern: None,
        }))
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let query = params.query.to_lowercase();
        let docs = self.documents.lock().expect("lsp state lock");
        let mut symbols = Vec::new();

        for (uri, doc) in docs.iter() {
            for (line_idx, line) in doc.source.lines().enumerate() {
                let trimmed = line.trim();
                let (kind, name) = if trimmed.starts_with("fn ") {
                    (SymbolKind::FUNCTION, extract_name(trimmed, "fn "))
                } else if trimmed.starts_with("pub fn ") {
                    (SymbolKind::FUNCTION, extract_name(trimmed, "pub fn "))
                } else if trimmed.starts_with("struct ") {
                    (SymbolKind::STRUCT, extract_name(trimmed, "struct "))
                } else if trimmed.starts_with("enum ") {
                    (SymbolKind::ENUM, extract_name(trimmed, "enum "))
                } else if trimmed.starts_with("trait ") {
                    (SymbolKind::INTERFACE, extract_name(trimmed, "trait "))
                } else if trimmed.starts_with("const ") {
                    (SymbolKind::CONSTANT, extract_name(trimmed, "const "))
                } else {
                    continue;
                };

                if !query.is_empty() && !name.to_lowercase().contains(&query) {
                    continue;
                }

                #[allow(deprecated)]
                symbols.push(SymbolInformation {
                    name,
                    kind,
                    tags: None,
                    deprecated: None,
                    location: Location {
                        uri: uri.clone(),
                        range: Range::new(
                            Position::new(line_idx as u32, 0),
                            Position::new(line_idx as u32, line.len() as u32),
                        ),
                    },
                    container_name: None,
                });
            }
        }
        Ok(Some(symbols))
    }

    async fn prepare_call_hierarchy(
        &self,
        params: CallHierarchyPrepareParams,
    ) -> Result<Option<Vec<CallHierarchyItem>>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let docs = self.documents.lock().expect("lsp state lock");
        let doc = match docs.get(uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        let word = word_at_position(&doc.source, &doc.line_starts, pos);
        if word.is_empty() {
            return Ok(None);
        }

        // Check if the word is a function name at its definition
        let line = doc.source.lines().nth(pos.line as usize).unwrap_or("");
        let trimmed = line.trim();
        if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") {
            let range = Range::new(
                Position::new(pos.line, 0),
                Position::new(pos.line, line.len() as u32),
            );
            Ok(Some(vec![CallHierarchyItem {
                name: word,
                kind: SymbolKind::FUNCTION,
                tags: None,
                detail: Some(trimmed.to_string()),
                uri: uri.clone(),
                range,
                selection_range: range,
                data: None,
            }]))
        } else {
            Ok(None)
        }
    }

    async fn incoming_calls(
        &self,
        params: CallHierarchyIncomingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyIncomingCall>>> {
        let uri = &params.item.uri;
        let fn_name = &params.item.name;
        let docs = self.documents.lock().expect("lsp state lock");
        let doc = match docs.get(uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        let mut calls = Vec::new();
        for (line_idx, line) in doc.source.lines().enumerate() {
            let trimmed = line.trim();
            if line_idx as u32 == params.item.range.start.line {
                continue; // Skip the definition itself
            }
            if line.contains(fn_name) && line.contains('(') {
                // Find the enclosing function
                let caller_name = find_enclosing_function(&doc.source, line_idx);
                let range = Range::new(
                    Position::new(line_idx as u32, 0),
                    Position::new(line_idx as u32, line.len() as u32),
                );
                calls.push(CallHierarchyIncomingCall {
                    from: CallHierarchyItem {
                        name: caller_name,
                        kind: SymbolKind::FUNCTION,
                        tags: None,
                        detail: Some(trimmed.to_string()),
                        uri: uri.clone(),
                        range,
                        selection_range: range,
                        data: None,
                    },
                    from_ranges: vec![range],
                });
            }
        }
        Ok(Some(calls))
    }

    async fn outgoing_calls(
        &self,
        params: CallHierarchyOutgoingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyOutgoingCall>>> {
        let uri = &params.item.uri;
        let docs = self.documents.lock().expect("lsp state lock");
        let doc = match docs.get(uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        // Find function body and extract called functions
        let start_line = params.item.range.start.line as usize;
        let mut calls = Vec::new();
        let mut in_body = false;
        let mut brace_depth = 0i32;

        for (line_idx, line) in doc.source.lines().enumerate().skip(start_line) {
            for ch in line.chars() {
                if ch == '{' {
                    brace_depth += 1;
                    in_body = true;
                }
                if ch == '}' {
                    brace_depth -= 1;
                }
            }
            if in_body && brace_depth <= 0 && line_idx > start_line {
                break;
            }
            if in_body && line_idx > start_line {
                // Look for function calls: name(
                for cap in find_function_calls(line) {
                    let range = Range::new(
                        Position::new(line_idx as u32, 0),
                        Position::new(line_idx as u32, line.len() as u32),
                    );
                    calls.push(CallHierarchyOutgoingCall {
                        to: CallHierarchyItem {
                            name: cap,
                            kind: SymbolKind::FUNCTION,
                            tags: None,
                            detail: None,
                            uri: uri.clone(),
                            range,
                            selection_range: range,
                            data: None,
                        },
                        from_ranges: vec![range],
                    });
                }
            }
        }
        Ok(Some(calls))
    }

    async fn prepare_type_hierarchy(
        &self,
        params: TypeHierarchyPrepareParams,
    ) -> Result<Option<Vec<TypeHierarchyItem>>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let docs = self.documents.lock().expect("lsp state lock");
        let doc = match docs.get(uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        let word = word_at_position(&doc.source, &doc.line_starts, pos);
        if word.is_empty() {
            return Ok(None);
        }

        let line = doc.source.lines().nth(pos.line as usize).unwrap_or("");
        let trimmed = line.trim();
        if trimmed.starts_with("struct ")
            || trimmed.starts_with("enum ")
            || trimmed.starts_with("trait ")
        {
            let kind = if trimmed.starts_with("struct ") {
                SymbolKind::STRUCT
            } else if trimmed.starts_with("enum ") {
                SymbolKind::ENUM
            } else {
                SymbolKind::INTERFACE
            };
            let range = Range::new(
                Position::new(pos.line, 0),
                Position::new(pos.line, line.len() as u32),
            );
            Ok(Some(vec![TypeHierarchyItem {
                name: word,
                kind,
                tags: None,
                detail: Some(trimmed.to_string()),
                uri: uri.clone(),
                range,
                selection_range: range,
                data: None,
            }]))
        } else {
            Ok(None)
        }
    }

    async fn supertypes(
        &self,
        params: TypeHierarchySupertypesParams,
    ) -> Result<Option<Vec<TypeHierarchyItem>>> {
        let uri = &params.item.uri;
        let type_name = &params.item.name;
        let docs = self.documents.lock().expect("lsp state lock");
        let doc = match docs.get(uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        // Find "impl TraitName for TypeName" patterns
        let mut supertypes = Vec::new();
        for (line_idx, line) in doc.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("impl ")
                && trimmed.contains(" for ")
                && trimmed.contains(type_name)
            {
                let trait_name = trimmed
                    .trim_start_matches("impl ")
                    .split(" for ")
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !trait_name.is_empty() {
                    let range = Range::new(
                        Position::new(line_idx as u32, 0),
                        Position::new(line_idx as u32, line.len() as u32),
                    );
                    supertypes.push(TypeHierarchyItem {
                        name: trait_name,
                        kind: SymbolKind::INTERFACE,
                        tags: None,
                        detail: Some(trimmed.to_string()),
                        uri: uri.clone(),
                        range,
                        selection_range: range,
                        data: None,
                    });
                }
            }
        }
        Ok(Some(supertypes))
    }

    async fn subtypes(
        &self,
        params: TypeHierarchySubtypesParams,
    ) -> Result<Option<Vec<TypeHierarchyItem>>> {
        let uri = &params.item.uri;
        let trait_name = &params.item.name;
        let docs = self.documents.lock().expect("lsp state lock");
        let doc = match docs.get(uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        let mut subtypes = Vec::new();
        for (line_idx, line) in doc.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("impl ")
                && trimmed.contains(trait_name)
                && trimmed.contains(" for ")
            {
                let impl_type = trimmed
                    .split(" for ")
                    .nth(1)
                    .unwrap_or("")
                    .trim()
                    .trim_end_matches('{')
                    .trim()
                    .to_string();
                if !impl_type.is_empty() {
                    let range = Range::new(
                        Position::new(line_idx as u32, 0),
                        Position::new(line_idx as u32, line.len() as u32),
                    );
                    subtypes.push(TypeHierarchyItem {
                        name: impl_type,
                        kind: SymbolKind::STRUCT,
                        tags: None,
                        detail: Some(trimmed.to_string()),
                        uri: uri.clone(),
                        range,
                        selection_range: range,
                        data: None,
                    });
                }
            }
        }
        Ok(Some(subtypes))
    }
}

// ── Folding Ranges ──────────────────────────────────────────────────

/// Compute folding ranges for functions, structs, enums, impl blocks, and comments.
fn compute_folding_ranges(source: &str) -> Vec<FoldingRange> {
    let mut ranges = Vec::new();
    let mut brace_stack: Vec<u32> = Vec::new();

    for (line_idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        let line_num = line_idx as u32;

        // Block comment regions
        if trimmed.starts_with("/*") {
            brace_stack.push(line_num);
        }
        if trimmed.ends_with("*/") || trimmed.contains("*/") {
            if let Some(start) = brace_stack.pop() {
                if line_num > start {
                    ranges.push(FoldingRange {
                        start_line: start,
                        start_character: None,
                        end_line: line_num,
                        end_character: None,
                        kind: Some(FoldingRangeKind::Comment),
                        collapsed_text: None,
                    });
                }
            }
        }

        // Opening braces start a fold
        if trimmed.ends_with('{') {
            brace_stack.push(line_num);
        }
        // Closing braces end a fold
        if trimmed == "}" || trimmed.starts_with('}') {
            if let Some(start) = brace_stack.pop() {
                if line_num > start {
                    ranges.push(FoldingRange {
                        start_line: start,
                        start_character: None,
                        end_line: line_num,
                        end_character: None,
                        kind: Some(FoldingRangeKind::Region),
                        collapsed_text: None,
                    });
                }
            }
        }
    }

    ranges
}

// ── Code Lens ───────────────────────────────────────────────────────

/// Compute code lenses for test functions and reference counts.
fn compute_code_lenses(source: &str, uri: &Url) -> Vec<CodeLens> {
    let mut lenses = Vec::new();

    for (line_idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        // @test annotation → "Run Test" lens
        if trimmed.starts_with("@test") {
            lenses.push(CodeLens {
                range: Range {
                    start: Position {
                        line: line_idx as u32,
                        character: 0,
                    },
                    end: Position {
                        line: line_idx as u32,
                        character: trimmed.len() as u32,
                    },
                },
                command: Some(Command {
                    title: "Run Test".into(),
                    command: "fajar.runTest".into(),
                    arguments: Some(vec![
                        serde_json::Value::String(uri.to_string()),
                        serde_json::Value::Number((line_idx as u64).into()),
                    ]),
                }),
                data: None,
            });
        }

        // fn definitions → "N references" lens
        if (trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ")) && trimmed.contains('(') {
            let fn_name = trimmed
                .trim_start_matches("pub ")
                .trim_start_matches("fn ")
                .split('(')
                .next()
                .unwrap_or("")
                .trim();

            if !fn_name.is_empty() && fn_name != "main" {
                let ref_count = source
                    .lines()
                    .filter(|l| l.contains(fn_name) && !l.trim().starts_with("//"))
                    .count()
                    .saturating_sub(1); // subtract the definition itself

                lenses.push(CodeLens {
                    range: Range {
                        start: Position {
                            line: line_idx as u32,
                            character: 0,
                        },
                        end: Position {
                            line: line_idx as u32,
                            character: trimmed.len() as u32,
                        },
                    },
                    command: Some(Command {
                        title: format!("{ref_count} references"),
                        command: "fajar.showReferences".into(),
                        arguments: None,
                    }),
                    data: None,
                });
            }
        }
    }

    lenses
}

// ── Semantic Tokens ─────────────────────────────────────────────────

/// Generates semantic tokens for syntax highlighting via LSP.
fn generate_semantic_tokens(source: &str) -> Vec<SemanticToken> {
    let tokens = match tokenize(source) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };

    let mut absolute_tokens = Vec::new();

    for token in &tokens {
        let token_type = match &token.kind {
            // Keywords (type 0)
            crate::lexer::token::TokenKind::If
            | crate::lexer::token::TokenKind::Else
            | crate::lexer::token::TokenKind::Match
            | crate::lexer::token::TokenKind::While
            | crate::lexer::token::TokenKind::For
            | crate::lexer::token::TokenKind::Loop
            | crate::lexer::token::TokenKind::In
            | crate::lexer::token::TokenKind::Return
            | crate::lexer::token::TokenKind::Break
            | crate::lexer::token::TokenKind::Continue
            | crate::lexer::token::TokenKind::Let
            | crate::lexer::token::TokenKind::Mut
            | crate::lexer::token::TokenKind::Fn
            | crate::lexer::token::TokenKind::Struct
            | crate::lexer::token::TokenKind::Enum
            | crate::lexer::token::TokenKind::Impl
            | crate::lexer::token::TokenKind::Trait
            | crate::lexer::token::TokenKind::Pub
            | crate::lexer::token::TokenKind::Use
            | crate::lexer::token::TokenKind::Mod
            | crate::lexer::token::TokenKind::Const
            | crate::lexer::token::TokenKind::Static
            | crate::lexer::token::TokenKind::Async
            | crate::lexer::token::TokenKind::Await
            | crate::lexer::token::TokenKind::Linear
            | crate::lexer::token::TokenKind::Comptime
            | crate::lexer::token::TokenKind::Effect
            | crate::lexer::token::TokenKind::Dyn
            | crate::lexer::token::TokenKind::Extern
            | crate::lexer::token::TokenKind::Type
            | crate::lexer::token::TokenKind::Where
            | crate::lexer::token::TokenKind::True
            | crate::lexer::token::TokenKind::False
            | crate::lexer::token::TokenKind::Null => Some(0u32), // KEYWORD

            // Strings (type 2)
            crate::lexer::token::TokenKind::StringLit(_)
            | crate::lexer::token::TokenKind::RawStringLit(_)
            | crate::lexer::token::TokenKind::CharLit(_) => Some(2), // STRING

            // Numbers (type 3)
            crate::lexer::token::TokenKind::IntLit(_)
            | crate::lexer::token::TokenKind::FloatLit(_) => Some(3), // NUMBER

            // Types (type 6)
            crate::lexer::token::TokenKind::BoolType
            | crate::lexer::token::TokenKind::I8
            | crate::lexer::token::TokenKind::I16
            | crate::lexer::token::TokenKind::I32
            | crate::lexer::token::TokenKind::I64
            | crate::lexer::token::TokenKind::I128
            | crate::lexer::token::TokenKind::U8
            | crate::lexer::token::TokenKind::U16
            | crate::lexer::token::TokenKind::U32
            | crate::lexer::token::TokenKind::U64
            | crate::lexer::token::TokenKind::U128
            | crate::lexer::token::TokenKind::F32Type
            | crate::lexer::token::TokenKind::F64Type
            | crate::lexer::token::TokenKind::StrType
            | crate::lexer::token::TokenKind::CharType
            | crate::lexer::token::TokenKind::Void
            | crate::lexer::token::TokenKind::Never
            | crate::lexer::token::TokenKind::Tensor => Some(6), // TYPE

            // Operators (type 7)
            crate::lexer::token::TokenKind::Plus
            | crate::lexer::token::TokenKind::Minus
            | crate::lexer::token::TokenKind::Star
            | crate::lexer::token::TokenKind::Slash
            | crate::lexer::token::TokenKind::Percent
            | crate::lexer::token::TokenKind::StarStar
            | crate::lexer::token::TokenKind::EqEq
            | crate::lexer::token::TokenKind::BangEq
            | crate::lexer::token::TokenKind::Lt
            | crate::lexer::token::TokenKind::Gt
            | crate::lexer::token::TokenKind::LtEq
            | crate::lexer::token::TokenKind::GtEq
            | crate::lexer::token::TokenKind::AmpAmp
            | crate::lexer::token::TokenKind::PipePipe
            | crate::lexer::token::TokenKind::PipeGt
            | crate::lexer::token::TokenKind::Arrow
            | crate::lexer::token::TokenKind::FatArrow => Some(7), // OPERATOR

            // Annotations (type 12 = DECORATOR)
            crate::lexer::token::TokenKind::AtKernel
            | crate::lexer::token::TokenKind::AtDevice
            | crate::lexer::token::TokenKind::AtSafe
            | crate::lexer::token::TokenKind::AtUnsafe
            | crate::lexer::token::TokenKind::AtNpu
            | crate::lexer::token::TokenKind::AtFfi
            | crate::lexer::token::TokenKind::AtTest
            | crate::lexer::token::TokenKind::AtDerive
            | crate::lexer::token::TokenKind::AtPure
            | crate::lexer::token::TokenKind::AtEntry => Some(12), // DECORATOR

            // Doc comments (type 1 = COMMENT)
            crate::lexer::token::TokenKind::DocComment(_) => Some(1), // COMMENT

            _ => None,
        };

        if let Some(tt) = token_type {
            let line = token.line.saturating_sub(1);
            let start = token.col.saturating_sub(1);
            let length = token.span.len() as u32;

            absolute_tokens.push(crate::lsp_v3::semantic::AbsoluteToken {
                line,
                start,
                length,
                token_type: tt,
                modifiers: 0,
            });
        }
    }

    // Use lsp_v3 delta encoder for correct semantic token encoding.
    crate::lsp_v3::semantic::encode_semantic_tokens(&absolute_tokens)
        .into_iter()
        .map(|t| SemanticToken {
            delta_line: t.delta_line,
            delta_start: t.delta_start,
            length: t.length,
            token_type: t.token_type,
            token_modifiers_bitset: t.token_modifiers,
        })
        .collect()
}

// ── Inlay Hints ─────────────────────────────────────────────────────

/// Generates inlay hints for type annotations on let bindings.
fn generate_inlay_hints(source: &str, _doc: &DocumentState) -> Vec<InlayHint> {
    let mut hints = Vec::new();

    for (line_idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        // Pattern: `let name = value` without explicit type annotation
        if let Some(rest) = trimmed.strip_prefix("let ") {
            let rest = rest
                .trim_start_matches("mut ")
                .trim_start_matches("linear ");
            // Check if there's no `:` before `=` (no type annotation)
            if let Some(eq_pos) = rest.find('=') {
                let before_eq = &rest[..eq_pos].trim();
                if !before_eq.contains(':') {
                    let name = before_eq.trim();
                    let after_eq = rest[eq_pos + 1..].trim();
                    if let Some(inferred) = infer_type_hint(after_eq) {
                        // Position: after the variable name
                        let col = line.find(name).unwrap_or(0) + name.len();
                        hints.push(InlayHint {
                            position: Position {
                                line: line_idx as u32,
                                character: col as u32,
                            },
                            label: InlayHintLabel::String(format!(": {inferred}")),
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
    }

    hints
}

/// Simple type inference for inlay hints.
fn infer_type_hint(expr: &str) -> Option<&'static str> {
    let expr = expr.trim();
    if expr.starts_with('"') || expr.starts_with("f\"") {
        Some("str")
    } else if expr == "true" || expr == "false" {
        Some("bool")
    } else if expr.contains('.') && expr.chars().all(|c| c.is_ascii_digit() || c == '.') {
        Some("f64")
    } else if expr.chars().all(|c| c.is_ascii_digit() || c == '-') && !expr.is_empty() {
        Some("i64")
    } else if expr.starts_with('[') || expr.starts_with("vec![") {
        Some("Array")
    } else {
        None
    }
}

// ── References ──────────────────────────────────────────────────────

/// Finds all references to a symbol in the document.
fn find_all_references(source: &str, word: &str, uri: &Url) -> Vec<Location> {
    let mut locations = Vec::new();

    for (line_idx, line) in source.lines().enumerate() {
        let mut search_from = 0;
        while let Some(col) = line[search_from..].find(word) {
            let abs_col = search_from + col;
            // Check word boundaries
            let before_ok = abs_col == 0
                || !line.as_bytes()[abs_col - 1].is_ascii_alphanumeric()
                    && line.as_bytes()[abs_col - 1] != b'_';
            let after_pos = abs_col + word.len();
            let after_ok = after_pos >= line.len()
                || !line.as_bytes()[after_pos].is_ascii_alphanumeric()
                    && line.as_bytes()[after_pos] != b'_';

            if before_ok && after_ok {
                let start = Position {
                    line: line_idx as u32,
                    character: abs_col as u32,
                };
                let end = Position {
                    line: line_idx as u32,
                    character: after_pos as u32,
                };
                locations.push(Location {
                    uri: uri.clone(),
                    range: Range { start, end },
                });
            }
            search_from = abs_col + word.len();
        }
    }

    locations
}

// ── Diagnostic pipeline ─────────────────────────────────────────────

/// Run lex + parse + analyze and collect all diagnostics.
fn collect_diagnostics(source: &str, doc: &DocumentState) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // Lex
    let tokens = match tokenize(source) {
        Ok(t) => t,
        Err(errors) => {
            for e in &errors {
                diagnostics.push(lex_error_to_diagnostic(e, doc));
            }
            return diagnostics;
        }
    };

    // Parse
    let program = match parse(tokens) {
        Ok(p) => p,
        Err(errors) => {
            for e in &errors {
                diagnostics.push(parse_error_to_diagnostic(e, doc));
            }
            return diagnostics;
        }
    };

    // Analyze
    if let Err(errors) = analyze(&program) {
        for e in &errors {
            diagnostics.push(semantic_error_to_diagnostic(e, doc));
        }
    }

    diagnostics
}

// ── Diagnostic converters ───────────────────────────────────────────

fn lex_error_to_diagnostic(e: &LexError, doc: &DocumentState) -> Diagnostic {
    let span = e.span();
    let range = doc.span_to_range(span);
    let code = match e {
        LexError::UnexpectedChar { .. } => "LE001",
        LexError::UnterminatedString { .. } => "LE002",
        LexError::UnterminatedBlockComment { .. } => "LE003",
        LexError::InvalidNumber { .. } => "LE004",
        LexError::InvalidEscape { .. } => "LE005",
        LexError::NumberOverflow { .. } => "LE006",
        LexError::EmptyCharLiteral { .. } => "LE007",
        LexError::MultiCharLiteral { .. } => "LE008",
        LexError::InvalidCharLiteral { .. } => "LE004",
        LexError::UnknownAnnotation { .. } => "LE001",
    };
    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(code.into())),
        source: Some("fajar-lang".into()),
        message: e.to_string(),
        ..Default::default()
    }
}

fn parse_error_to_diagnostic(e: &ParseError, doc: &DocumentState) -> Diagnostic {
    let span = e.span();
    let range = doc.span_to_range(span);
    let code = match e {
        ParseError::UnexpectedToken { .. } => "PE001",
        ParseError::ExpectedExpression { .. } => "PE002",
        ParseError::ExpectedType { .. } => "PE003",
        ParseError::ExpectedPattern { .. } => "PE004",
        ParseError::ExpectedIdentifier { .. } => "PE005",
        ParseError::UnexpectedEof { .. } => "PE006",
        ParseError::InvalidPattern { .. } => "PE007",
        ParseError::DuplicateField { .. } => "PE008",
        ParseError::TrailingSeparator { .. } => "PE009",
        ParseError::InvalidAnnotation { .. } => "PE010",
        ParseError::ModuleFileNotFound { .. } => "PE011",
    };
    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(code.into())),
        source: Some("fajar-lang".into()),
        message: e.to_string(),
        ..Default::default()
    }
}

fn semantic_error_to_diagnostic(e: &SemanticError, doc: &DocumentState) -> Diagnostic {
    let span = e.span();
    let range = doc.span_to_range(span);
    let severity = if e.is_warning() {
        DiagnosticSeverity::WARNING
    } else {
        DiagnosticSeverity::ERROR
    };
    let code = match e {
        SemanticError::UndefinedVariable { .. } => "SE001",
        SemanticError::UndefinedFunction { .. } => "SE002",
        SemanticError::UndefinedType { .. } => "SE003",
        SemanticError::TypeMismatch { .. } => "SE004",
        SemanticError::ArgumentCountMismatch { .. } => "SE005",
        SemanticError::DuplicateDefinition { .. } => "SE006",
        SemanticError::ImmutableAssignment { .. } => "SE007",
        SemanticError::MissingReturn { .. } => "SE008",
        SemanticError::UnusedVariable { .. } => "SE009",
        SemanticError::UnreachableCode { .. } => "SE010",
        SemanticError::NonExhaustiveMatch { .. } => "SE011",
        SemanticError::MissingField { .. } => "SE012",
        SemanticError::BreakOutsideLoop { .. } => "SE007",
        SemanticError::ReturnOutsideFunction { .. } => "SE007",
        SemanticError::HeapAllocInKernel { .. } => "KE001",
        SemanticError::TensorInKernel { .. } => "KE002",
        SemanticError::DeviceCallInKernel { .. } => "KE003",
        SemanticError::RawPointerInDevice { .. } => "DE001",
        SemanticError::KernelCallInDevice { .. } => "DE002",
        SemanticError::FfiUnsafeType { .. } => "SE013",
        SemanticError::UseAfterMove { .. } => "ME001",
        SemanticError::MoveWhileBorrowed { .. } => "ME003",
        SemanticError::MutBorrowConflict { .. } => "ME004",
        SemanticError::ImmBorrowConflict { .. } => "ME005",
        SemanticError::TraitBoundNotSatisfied { .. } => "SE014",
        SemanticError::UnknownTrait { .. } => "SE015",
        SemanticError::CannotInferType { .. } => "SE013",
        SemanticError::TraitMethodSignatureMismatch { .. } => "SE016",
        SemanticError::TensorShapeMismatch { .. } => "TE001",
        SemanticError::HardwareAccessInSafe { .. } => "SE020",
        SemanticError::KernelCallInSafe { .. } => "SE021",
        SemanticError::DeviceCallInSafe { .. } => "SE022",
        SemanticError::AsmInSafeContext { .. } => "KE005",
        SemanticError::AsmInDeviceContext { .. } => "KE006",
        SemanticError::AwaitOutsideAsync { .. } => "SE017",
        SemanticError::NotSendType { .. } => "SE018",
        SemanticError::UnusedImport { .. } => "SE019",
        SemanticError::UnreachablePattern { .. } => "SE020",
        SemanticError::LifetimeMismatch { .. } => "SE021",
        SemanticError::LifetimeConflict { .. } => "ME009",
        SemanticError::DanglingReference { .. } => "ME010",
        SemanticError::RawPointerInNpu { .. } => "NE001",
        SemanticError::HeapAllocInNpu { .. } => "NE002",
        SemanticError::OsPrimitiveInNpu { .. } => "NE003",
        SemanticError::KernelCallInNpu { .. } => "NE004",
        SemanticError::LinearNotConsumed { .. } => "ME010",
        SemanticError::UndeclaredEffect { .. } => "EE001",
        SemanticError::UnknownEffect { .. } => "EE002",
        SemanticError::EffectForbiddenInContext { .. } => "EE006",
        SemanticError::ResumeOutsideHandler { .. } => "EE005",
        SemanticError::DuplicateEffectDecl { .. } => "EE004",
        SemanticError::MessageTooLarge { .. } => "IPC001",
        SemanticError::IpcTypeMismatch { .. } => "IPC002",
        SemanticError::IndexOutOfBounds { .. } => "SE022",
    };
    Diagnostic {
        range,
        severity: Some(severity),
        code: Some(NumberOrString::String(code.into())),
        source: Some("fajar-lang".into()),
        message: e.to_string(),
        ..Default::default()
    }
}

// ── Hover helpers ───────────────────────────────────────────────────

/// Extracts the word at the given cursor position.
fn word_at_position(source: &str, line_starts: &[usize], pos: Position) -> String {
    let line = pos.line as usize;
    if line >= line_starts.len() {
        return String::new();
    }
    let line_start = line_starts[line];
    let col = pos.character as usize;
    let offset = line_start + col;
    if offset >= source.len() {
        return String::new();
    }

    let bytes = source.as_bytes();
    // Walk backward to find word start
    let mut start = offset;
    while start > 0 && is_ident_char(bytes[start - 1]) {
        start -= 1;
    }
    // Handle @ prefix for annotations
    if start > 0 && bytes[start - 1] == b'@' {
        start -= 1;
    }
    // Walk forward to find word end
    let mut end = offset;
    while end < bytes.len() && is_ident_char(bytes[end]) {
        end += 1;
    }
    source[start..end].to_string()
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Convert an LSP Position to a byte offset.
fn position_to_offset(line_starts: &[usize], pos: Position) -> usize {
    let line = pos.line as usize;
    if line >= line_starts.len() {
        return 0;
    }
    line_starts[line] + pos.character as usize
}

/// Find word boundaries at a byte offset.
fn find_word_range_at(source: &str, offset: usize) -> (usize, usize) {
    let bytes = source.as_bytes();
    let mut start = offset;
    while start > 0 && is_ident_char(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = offset;
    while end < bytes.len() && is_ident_char(bytes[end]) {
        end += 1;
    }
    (start, end.max(start))
}

/// Extract the identifier name after a prefix (e.g., "fn " → name).
fn extract_name(line: &str, prefix: &str) -> String {
    let rest = line.trim_start_matches(prefix);
    rest.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .next()
        .unwrap_or("")
        .to_string()
}

/// Find the name of the function enclosing a given line index.
fn find_enclosing_function(source: &str, target_line: usize) -> String {
    let mut last_fn = "<module>".to_string();
    for (i, line) in source.lines().enumerate() {
        if i >= target_line {
            break;
        }
        let trimmed = line.trim();
        if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") {
            let prefix = if trimmed.starts_with("pub fn ") {
                "pub fn "
            } else {
                "fn "
            };
            last_fn = extract_name(trimmed, prefix);
        }
    }
    last_fn
}

/// Find function call names in a line (identifiers followed by `(`).
fn find_function_calls(line: &str) -> Vec<String> {
    let mut calls = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'(' && i > 0 {
            // Walk backward to find function name
            let end = i;
            let mut start = i;
            while start > 0 && is_ident_char(bytes[start - 1]) {
                start -= 1;
            }
            if start < end {
                let name = &line[start..end];
                // Skip keywords
                if !matches!(
                    name,
                    "if" | "while" | "for" | "match" | "return" | "let" | "mut"
                ) {
                    calls.push(name.to_string());
                }
            }
        }
        i += 1;
    }
    calls
}

fn keyword_info(word: &str) -> Option<String> {
    let info = match word {
        "let" => "**let** — Variable binding\n```fajar\nlet x: i32 = 42\nlet mut y = 0\n```",
        "mut" => {
            "**mut** — Mutable variable modifier\n```fajar\nlet mut counter = 0\ncounter = counter + 1\n```"
        }
        "fn" => {
            "**fn** — Function definition\n```fajar\nfn add(a: i32, b: i32) -> i32 { a + b }\n```"
        }
        "if" => {
            "**if** — Conditional expression\n```fajar\nlet max = if a > b { a } else { b }\n```"
        }
        "else" => "**else** — Alternative branch of if expression",
        "while" => {
            "**while** — Loop while condition is true\n```fajar\nwhile x < 10 { x = x + 1 }\n```"
        }
        "for" => {
            "**for** — Iterate over a range or collection\n```fajar\nfor i in 0..10 { println(i) }\n```"
        }
        "match" => {
            "**match** — Pattern matching expression\n```fajar\nmatch x { 0 => \"zero\", _ => \"other\" }\n```"
        }
        "return" => "**return** — Return a value from a function",
        "break" => "**break** — Exit a loop, optionally with a value",
        "continue" => "**continue** — Skip to next loop iteration",
        "struct" => {
            "**struct** — Define a data structure\n```fajar\nstruct Point { x: f64, y: f64 }\n```"
        }
        "enum" => {
            "**enum** — Define a sum type\n```fajar\nenum Shape { Circle(f64), Rect(f64, f64) }\n```"
        }
        "impl" => {
            "**impl** — Implement methods on a type\n```fajar\nimpl Point { fn origin() -> Point { ... } }\n```"
        }
        "trait" => "**trait** — Define a shared interface",
        "const" => "**const** — Compile-time constant\n```fajar\nconst MAX: usize = 1024\n```",
        "use" => "**use** — Import items from a module\n```fajar\nuse math::sqrt\n```",
        "mod" => "**mod** — Define a module",
        "pub" => "**pub** — Make item publicly visible",
        "as" => "**as** — Type cast\n```fajar\nlet y = x as f64\n```",
        "loop" => {
            "**loop** — Infinite loop (exit with break)\n```fajar\nloop { if done { break } }\n```"
        }
        "in" => "**in** — Part of for-in loop syntax",
        "true" | "false" => &format!("**{word}** — Boolean literal (`bool`)"),
        "null" => "**null** — Null value (void type)",
        "@kernel" => "**@kernel** — Kernel context: OS primitives, no heap, no tensor",
        "@device" => "**@device** — Device context: tensor ops, no raw pointer, no IRQ",
        "@safe" => "**@safe** — Safe context (default): no hardware, no raw pointer",
        "@unsafe" => "**@unsafe** — Unsafe context: full access to all features",
        "@ffi" => "**@ffi** — Foreign function interface annotation",
        _ => return None,
    };
    Some(info.to_string())
}

fn builtin_info(name: &str) -> Option<String> {
    let info = match name {
        "print" => "```fajar\nfn print(value: any) -> void\n```\nPrint a value without newline.",
        "println" => "```fajar\nfn println(value: any) -> void\n```\nPrint a value with newline.",
        "len" => {
            "```fajar\nfn len(collection: Array | Str) -> i64\n```\nReturn the length of an array or string."
        }
        "type_of" => {
            "```fajar\nfn type_of(value: any) -> str\n```\nReturn the type name of a value as a string."
        }
        "to_string" => {
            "```fajar\nfn to_string(value: any) -> str\n```\nConvert any value to its string representation."
        }
        "to_int" => {
            "```fajar\nfn to_int(value: any) -> i64\n```\nConvert to integer (from float, string, bool)."
        }
        "to_float" => {
            "```fajar\nfn to_float(value: any) -> f64\n```\nConvert to float (from int, string)."
        }
        "abs" => "```fajar\nfn abs(x: i64 | f64) -> i64 | f64\n```\nAbsolute value.",
        "sqrt" => "```fajar\nfn sqrt(x: f64) -> f64\n```\nSquare root.",
        "pow" => "```fajar\nfn pow(base: f64, exp: f64) -> f64\n```\nRaise base to power.",
        "log" | "log2" | "log10" => {
            &format!("```fajar\nfn {name}(x: f64) -> f64\n```\nLogarithm function.")
        }
        "sin" | "cos" | "tan" => {
            &format!("```fajar\nfn {name}(x: f64) -> f64\n```\nTrigonometric function (radians).")
        }
        "floor" | "ceil" | "round" => {
            &format!("```fajar\nfn {name}(x: f64) -> f64\n```\nRounding function.")
        }
        "min" => "```fajar\nfn min(a: f64, b: f64) -> f64\n```\nReturn the smaller value.",
        "max" => "```fajar\nfn max(a: f64, b: f64) -> f64\n```\nReturn the larger value.",
        "clamp" => {
            "```fajar\nfn clamp(x: f64, lo: f64, hi: f64) -> f64\n```\nClamp value to range [lo, hi]."
        }
        "assert" => {
            "```fajar\nfn assert(condition: bool) -> void\n```\nPanic if condition is false."
        }
        "assert_eq" => "```fajar\nfn assert_eq(a: any, b: any) -> void\n```\nPanic if a != b.",
        "panic" => "```fajar\nfn panic(msg: str) -> never\n```\nAbort with error message.",
        "todo" => "```fajar\nfn todo(msg: str) -> never\n```\nMark unimplemented code.",
        "dbg" => "```fajar\nfn dbg(value: any) -> any\n```\nDebug-print and return the value.",
        "push" => {
            "```fajar\nfn push(arr: Array, value: any) -> void\n```\nAppend element to array."
        }
        "pop" => "```fajar\nfn pop(arr: Array) -> any\n```\nRemove and return last element.",
        "PI" => "**PI** — `3.141592653589793` (`f64`)",
        "E" => "**E** — `2.718281828459045` (`f64`)",
        _ => return None,
    };
    Some(info.to_string())
}

fn type_info(name: &str) -> Option<String> {
    let info = match name {
        "i8" | "i16" | "i32" | "i64" | "i128" => {
            &format!("**{name}** — Signed integer type ({} bits)", &name[1..])
        }
        "u8" | "u16" | "u32" | "u64" | "u128" => {
            &format!("**{name}** — Unsigned integer type ({} bits)", &name[1..])
        }
        "isize" => "**isize** — Pointer-sized signed integer",
        "usize" => "**usize** — Pointer-sized unsigned integer",
        "f32" => "**f32** — 32-bit floating point",
        "f64" => "**f64** — 64-bit floating point",
        "bool" => "**bool** — Boolean type (`true` or `false`)",
        "str" => "**str** — String type",
        "char" => "**char** — Unicode character",
        "void" => "**void** — Unit type (no value)",
        "never" => "**never** — Bottom type (function never returns)",
        _ => return None,
    };
    Some(info.to_string())
}

// ── Document symbol extraction ──────────────────────────────────────

/// Extracts document symbols (functions, structs, enums) from source.
fn extract_symbols(source: &str, doc: &DocumentState) -> Vec<SymbolInformation> {
    let mut symbols = Vec::new();

    for (i, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        // Strip annotation prefix
        let trimmed = if trimmed.starts_with('@') {
            if let Some(rest) = trimmed.split_whitespace().nth(1) {
                &trimmed[trimmed.find(rest).unwrap_or(0)..]
            } else {
                trimmed
            }
        } else {
            trimmed
        };

        if let Some(name) = extract_def_name(trimmed, "fn ") {
            let offset = doc.line_starts[i];
            symbols.push(SymbolInformation {
                name: name.to_string(),
                kind: SymbolKind::FUNCTION,
                #[allow(deprecated)]
                deprecated: None,
                location: Location {
                    uri: Url::parse("file:///")
                        .unwrap_or_else(|_| Url::parse("file:///tmp").expect("static URL")),
                    range: Range::new(
                        Position::new(i as u32, 0),
                        doc.offset_to_position(offset + line.len()),
                    ),
                },
                tags: None,
                container_name: None,
            });
        } else if let Some(name) = extract_def_name(trimmed, "struct ") {
            let offset = doc.line_starts[i];
            symbols.push(SymbolInformation {
                name: name.to_string(),
                kind: SymbolKind::STRUCT,
                #[allow(deprecated)]
                deprecated: None,
                location: Location {
                    uri: Url::parse("file:///")
                        .unwrap_or_else(|_| Url::parse("file:///tmp").expect("static URL")),
                    range: Range::new(
                        Position::new(i as u32, 0),
                        doc.offset_to_position(offset + line.len()),
                    ),
                },
                tags: None,
                container_name: None,
            });
        } else if let Some(name) = extract_def_name(trimmed, "enum ") {
            let offset = doc.line_starts[i];
            symbols.push(SymbolInformation {
                name: name.to_string(),
                kind: SymbolKind::ENUM,
                #[allow(deprecated)]
                deprecated: None,
                location: Location {
                    uri: Url::parse("file:///")
                        .unwrap_or_else(|_| Url::parse("file:///tmp").expect("static URL")),
                    range: Range::new(
                        Position::new(i as u32, 0),
                        doc.offset_to_position(offset + line.len()),
                    ),
                },
                tags: None,
                container_name: None,
            });
        } else if let Some(name) = extract_def_name(trimmed, "trait ") {
            let offset = doc.line_starts[i];
            symbols.push(SymbolInformation {
                name: name.to_string(),
                kind: SymbolKind::INTERFACE,
                #[allow(deprecated)]
                deprecated: None,
                location: Location {
                    uri: Url::parse("file:///")
                        .unwrap_or_else(|_| Url::parse("file:///tmp").expect("static URL")),
                    range: Range::new(
                        Position::new(i as u32, 0),
                        doc.offset_to_position(offset + line.len()),
                    ),
                },
                tags: None,
                container_name: None,
            });
        } else if let Some(name) = extract_def_name(trimmed, "const ") {
            let offset = doc.line_starts[i];
            symbols.push(SymbolInformation {
                name: name.to_string(),
                kind: SymbolKind::CONSTANT,
                #[allow(deprecated)]
                deprecated: None,
                location: Location {
                    uri: Url::parse("file:///")
                        .unwrap_or_else(|_| Url::parse("file:///tmp").expect("static URL")),
                    range: Range::new(
                        Position::new(i as u32, 0),
                        doc.offset_to_position(offset + line.len()),
                    ),
                },
                tags: None,
                container_name: None,
            });
        }
    }

    symbols
}

/// Extracts a definition name from a line starting with a keyword prefix.
fn extract_def_name<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(prefix)?;
    let end = rest.find(|c: char| !c.is_ascii_alphanumeric() && c != '_')?;
    if end == 0 {
        return None;
    }
    Some(&rest[..end])
}

// ── Signature help helpers ──────────────────────────────────────────

/// Extracts the function name immediately before the last `(`.
fn extract_fn_name_before_paren(text: &str) -> String {
    // Find the last '(' in text
    let paren_pos = match text.rfind('(') {
        Some(p) => p,
        None => return String::new(),
    };
    // Walk backward from paren to find the identifier
    let before = &text[..paren_pos];
    let trimmed = before.trim_end();
    let name_start = trimmed
        .rfind(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .map(|p| p + 1)
        .unwrap_or(0);
    trimmed[name_start..].to_string()
}

/// Finds a function signature in the source code by name.
fn find_fn_signature(source: &str, name: &str) -> Option<String> {
    let pattern = format!("fn {name}(");
    let pos = source.find(&pattern)?;
    // Extract from "fn" to the end of the signature (closing paren + return type)
    let rest = &source[pos..];
    // Find the closing `{` or end of line
    let end = rest
        .find('{')
        .or_else(|| rest.find('\n'))
        .unwrap_or(rest.len());
    Some(rest[..end].trim().to_string())
}

// ── Static data ─────────────────────────────────────────────────────

const KEYWORDS: &[&str] = &[
    "let", "mut", "fn", "if", "else", "while", "for", "in", "match", "return", "break", "continue",
    "struct", "enum", "impl", "trait", "type", "const", "use", "mod", "pub", "extern", "as",
    "loop", "true", "false", "null",
];

const BUILTINS: &[(&str, &str)] = &[
    ("print", "fn print(value: any) -> void"),
    ("println", "fn println(value: any) -> void"),
    ("len", "fn len(collection) -> i64"),
    ("type_of", "fn type_of(value) -> str"),
    ("to_string", "fn to_string(value) -> str"),
    ("to_int", "fn to_int(value) -> i64"),
    ("to_float", "fn to_float(value) -> f64"),
    ("abs", "fn abs(x) -> i64 | f64"),
    ("sqrt", "fn sqrt(x: f64) -> f64"),
    ("pow", "fn pow(base: f64, exp: f64) -> f64"),
    ("log", "fn log(x: f64) -> f64"),
    ("log2", "fn log2(x: f64) -> f64"),
    ("log10", "fn log10(x: f64) -> f64"),
    ("sin", "fn sin(x: f64) -> f64"),
    ("cos", "fn cos(x: f64) -> f64"),
    ("tan", "fn tan(x: f64) -> f64"),
    ("floor", "fn floor(x: f64) -> f64"),
    ("ceil", "fn ceil(x: f64) -> f64"),
    ("round", "fn round(x: f64) -> f64"),
    ("min", "fn min(a: f64, b: f64) -> f64"),
    ("max", "fn max(a: f64, b: f64) -> f64"),
    ("clamp", "fn clamp(x: f64, lo: f64, hi: f64) -> f64"),
    ("assert", "fn assert(condition: bool) -> void"),
    ("assert_eq", "fn assert_eq(a: any, b: any) -> void"),
    ("panic", "fn panic(msg: str) -> never"),
    ("todo", "fn todo(msg: str) -> never"),
    ("dbg", "fn dbg(value: any) -> any"),
    ("push", "fn push(arr: Array, value: any) -> void"),
    ("pop", "fn pop(arr: Array) -> any"),
];

const TYPES: &[&str] = &[
    "bool", "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64", "u128", "isize", "usize",
    "f32", "f64", "str", "char", "void", "never",
];

const ANNOTATIONS: &[&str] = &["@kernel", "@device", "@safe", "@unsafe", "@ffi"];

// ── LexError/ParseError span helpers ────────────────────────────────

trait HasSpan {
    fn span(&self) -> Span;
}

impl HasSpan for LexError {
    fn span(&self) -> Span {
        match self {
            LexError::UnexpectedChar { span, .. }
            | LexError::UnterminatedString { span, .. }
            | LexError::UnterminatedBlockComment { span, .. }
            | LexError::InvalidNumber { span, .. }
            | LexError::InvalidEscape { span, .. }
            | LexError::NumberOverflow { span, .. }
            | LexError::EmptyCharLiteral { span, .. }
            | LexError::MultiCharLiteral { span, .. }
            | LexError::InvalidCharLiteral { span, .. }
            | LexError::UnknownAnnotation { span, .. } => *span,
        }
    }
}

impl HasSpan for ParseError {
    fn span(&self) -> Span {
        match self {
            ParseError::UnexpectedToken { span, .. }
            | ParseError::ExpectedExpression { span, .. }
            | ParseError::ExpectedType { span, .. }
            | ParseError::ExpectedPattern { span, .. }
            | ParseError::ExpectedIdentifier { span, .. }
            | ParseError::UnexpectedEof { span }
            | ParseError::InvalidPattern { span, .. }
            | ParseError::DuplicateField { span, .. }
            | ParseError::TrailingSeparator { span, .. }
            | ParseError::InvalidAnnotation { span, .. }
            | ParseError::ModuleFileNotFound { span, .. } => *span,
        }
    }
}

// ── Public entry point ──────────────────────────────────────────────

/// Starts the LSP server on stdin/stdout.
pub async fn run_lsp() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(FajarLspBackend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_state_offset_to_position_first_line() {
        let doc = DocumentState::new("hello world".into());
        let pos = doc.offset_to_position(6);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.character, 6);
    }

    #[test]
    fn document_state_offset_to_position_second_line() {
        let doc = DocumentState::new("line1\nline2\nline3".into());
        // 'l' of line2 is at offset 6
        let pos = doc.offset_to_position(6);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.character, 0);
        // 'n' of line2 is at offset 8
        let pos = doc.offset_to_position(8);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.character, 2);
    }

    #[test]
    fn document_state_span_to_range() {
        let doc = DocumentState::new("let x = 42\nlet y = 10".into());
        let span = Span::new(4, 5); // 'x'
        let range = doc.span_to_range(span);
        assert_eq!(range.start.line, 0);
        assert_eq!(range.start.character, 4);
        assert_eq!(range.end.line, 0);
        assert_eq!(range.end.character, 5);
    }

    #[test]
    fn collect_diagnostics_clean_source() {
        let source = "let x: i64 = 42";
        let doc = DocumentState::new(source.into());
        let diags = collect_diagnostics(source, &doc);
        assert!(diags.is_empty());
    }

    #[test]
    fn collect_diagnostics_lex_error() {
        let source = "let x = \"unterminated";
        let doc = DocumentState::new(source.into());
        let diags = collect_diagnostics(source, &doc);
        assert!(!diags.is_empty());
        assert_eq!(diags[0].code, Some(NumberOrString::String("LE002".into())));
    }

    #[test]
    fn collect_diagnostics_parse_error() {
        let source = "let = 42";
        let doc = DocumentState::new(source.into());
        let diags = collect_diagnostics(source, &doc);
        assert!(!diags.is_empty());
    }

    #[test]
    fn collect_diagnostics_semantic_error() {
        let source = "fn main() -> i64 { unknown_var }";
        let doc = DocumentState::new(source.into());
        let diags = collect_diagnostics(source, &doc);
        assert!(
            diags
                .iter()
                .any(|d| { d.code == Some(NumberOrString::String("SE001".into())) })
        );
    }

    #[test]
    fn word_at_position_simple() {
        let source = "let counter = 42";
        let line_starts = vec![0];
        let word = word_at_position(source, &line_starts, Position::new(0, 5));
        assert_eq!(word, "counter");
    }

    #[test]
    fn word_at_position_annotation() {
        let source = "@kernel fn main() {}";
        let line_starts = vec![0];
        let word = word_at_position(source, &line_starts, Position::new(0, 3));
        assert_eq!(word, "@kernel");
    }

    #[test]
    fn keyword_info_returns_some_for_keywords() {
        assert!(keyword_info("fn").is_some());
        assert!(keyword_info("let").is_some());
        assert!(keyword_info("@kernel").is_some());
    }

    #[test]
    fn keyword_info_returns_none_for_non_keywords() {
        assert!(keyword_info("myvar").is_none());
        assert!(keyword_info("xyz").is_none());
    }

    #[test]
    fn builtin_info_returns_some_for_builtins() {
        assert!(builtin_info("println").is_some());
        assert!(builtin_info("len").is_some());
        assert!(builtin_info("PI").is_some());
    }

    #[test]
    fn type_info_returns_some_for_types() {
        assert!(type_info("i64").is_some());
        assert!(type_info("bool").is_some());
        assert!(type_info("f32").is_some());
    }

    #[test]
    fn type_info_returns_none_for_non_types() {
        assert!(type_info("mytype").is_none());
    }

    // ── Document symbols ──

    #[test]
    fn extract_symbols_finds_functions() {
        let source = "fn add(a: i64, b: i64) -> i64 { a + b }\nfn main() -> void {}";
        let doc = DocumentState::new(source.into());
        let symbols = extract_symbols(source, &doc);
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "add");
        assert_eq!(symbols[0].kind, SymbolKind::FUNCTION);
        assert_eq!(symbols[1].name, "main");
    }

    #[test]
    fn extract_symbols_finds_structs_enums() {
        let source = "struct Point { x: f64, y: f64 }\nenum Shape { Circle(f64) }";
        let doc = DocumentState::new(source.into());
        let symbols = extract_symbols(source, &doc);
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "Point");
        assert_eq!(symbols[0].kind, SymbolKind::STRUCT);
        assert_eq!(symbols[1].name, "Shape");
        assert_eq!(symbols[1].kind, SymbolKind::ENUM);
    }

    #[test]
    fn extract_symbols_with_annotations() {
        let source = "@kernel fn init() -> void {}";
        let doc = DocumentState::new(source.into());
        let symbols = extract_symbols(source, &doc);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "init");
    }

    // ── Signature help ──

    #[test]
    fn extract_fn_name_before_paren_simple() {
        assert_eq!(extract_fn_name_before_paren("println("), "println");
        assert_eq!(extract_fn_name_before_paren("  add(1, "), "add");
        assert_eq!(extract_fn_name_before_paren("let x = sqrt("), "sqrt");
        assert_eq!(extract_fn_name_before_paren(""), "");
    }

    #[test]
    fn find_fn_signature_in_source() {
        let source = "fn greet(name: str) -> void {\n    println(name)\n}";
        let sig = find_fn_signature(source, "greet");
        assert!(sig.is_some());
        assert!(sig.unwrap().contains("fn greet(name: str) -> void"));
    }

    #[test]
    fn find_fn_signature_not_found() {
        let source = "fn main() -> void {}";
        assert!(find_fn_signature(source, "nonexistent").is_none());
    }

    // ═════════════════════════════════════════════════════════════════
    // V8 IDE1: Folding ranges + Code lens tests
    // ═════════════════════════════════════════════════════════════════

    #[test]
    fn ide1_folding_range_function() {
        let source = "fn main() {\n    let x = 42\n    println(x)\n}";
        let ranges = compute_folding_ranges(source);
        assert!(!ranges.is_empty(), "should find at least 1 fold");
        assert_eq!(ranges[0].start_line, 0);
        assert_eq!(ranges[0].end_line, 3);
    }

    #[test]
    fn ide1_folding_range_nested() {
        let source = "fn f() {\n    if true {\n        1\n    }\n}";
        let ranges = compute_folding_ranges(source);
        assert!(ranges.len() >= 2, "should find 2+ folds (fn + if)");
    }

    #[test]
    fn ide1_folding_range_empty() {
        let source = "let x = 42";
        let ranges = compute_folding_ranges(source);
        assert!(ranges.is_empty(), "no braces = no folds");
    }

    #[test]
    fn ide1_code_lens_test_annotation() {
        let source = "@test\nfn test_add() {\n    assert_eq(1 + 1, 2)\n}";
        let uri = Url::parse("file:///test.fj").unwrap();
        let lenses = compute_code_lenses(source, &uri);
        assert!(
            lenses.iter().any(|l| l
                .command
                .as_ref()
                .is_some_and(|c| c.title.contains("Run Test"))),
            "should have Run Test lens"
        );
    }

    #[test]
    fn ide1_code_lens_references() {
        let source = "fn add(a: i64, b: i64) -> i64 { a + b }\nfn main() { add(1, 2) }";
        let uri = Url::parse("file:///test.fj").unwrap();
        let lenses = compute_code_lenses(source, &uri);
        assert!(
            lenses.iter().any(|l| l
                .command
                .as_ref()
                .is_some_and(|c| c.title.contains("reference"))),
            "should have references lens for add()"
        );
    }

    #[test]
    fn ide1_code_lens_main_excluded() {
        let source = "fn main() { }";
        let uri = Url::parse("file:///test.fj").unwrap();
        let lenses = compute_code_lenses(source, &uri);
        // main() should not get a references lens (excluded by convention)
        assert!(
            !lenses.iter().any(|l| l
                .command
                .as_ref()
                .is_some_and(|c| c.title.contains("reference"))),
            "main() should not get references lens"
        );
    }
}
