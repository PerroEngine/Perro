use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use crate::completion::get_completions;
use crate::diagnostics::{diagnose_fur, diagnose_pup};
use crate::hover::get_hover;
use crate::types::{DocumentCache, ParsedDocument};

use perro_core::nodes::ui::parser::FurParser;
use perro_core::scripting::ast::Script;
use perro_core::scripting::lang::pup::parser::PupParser;
use std::collections::HashMap;

pub struct PerroLspServer {
    client: Client,
    documents: Arc<RwLock<DocumentCache>>,
}

impl PerroLspServer {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(DocumentCache::new())),
        }
    }

    /// Extract `@script Name extends NodeType` info from raw source.
    /// This is intentionally lenient so it works while the user is mid-typing (e.g. `self.`).
    fn extract_pup_header(source: &str) -> (Option<String>, Option<String>) {
        // Only look at the first few lines to keep it cheap.
        for line in source.lines().take(10) {
            let l = line.trim();
            if !l.starts_with("@script") {
                continue;
            }
            // Expected-ish: @script Player extends Sprite2D
            let mut parts = l.split_whitespace();
            let script_kw = parts.next();
            if script_kw != Some("@script") {
                continue;
            }
            let script_name = parts.next().map(|s| s.to_string());
            // Find `extends` and take the next token.
            let mut node_type: Option<String> = None;
            while let Some(p) = parts.next() {
                if p == "extends" {
                    node_type = parts.next().map(|s| s.to_string());
                    break;
                }
            }
            return (script_name, node_type);
        }
        (None, None)
    }

    fn fallback_pup_script(uri: &str, source: &str) -> Script {
        let (script_name, node_type_opt) = Self::extract_pup_header(source);
        Script {
            script_name,
            node_type: node_type_opt.unwrap_or_default(),
            variables: Vec::new(),
            functions: Vec::new(),
            structs: Vec::new(),
            verbose: false,
            attributes: HashMap::new(),
            source_file: Some(uri.to_string()),
            language: Some("pup".to_string()),
            module_names: std::collections::HashSet::new(), // LSP doesn't track modules
            module_name_to_identifier: std::collections::HashMap::new(), // LSP doesn't track modules
            module_functions: std::collections::HashMap::new(),
            module_variables: std::collections::HashMap::new(),
            module_scope_variables: None,
            is_global: false,
            global_names: std::collections::HashSet::new(),
            global_name_to_node_id: std::collections::HashMap::new(),
            rust_struct_name: None,
        }
    }

    async fn parse_document_with_fallback(
        &self,
        uri: &str,
        text: &str,
        previous: Option<&ParsedDocument>,
    ) -> Option<ParsedDocument> {
        if uri.ends_with(".pup") {
            let mut parser = PupParser::new(text);
            parser.set_source_file(uri.to_string());
            // LSP runs incrementally while the user is mid-typing (e.g. `self.`),
            // so we need error-tolerant parsing to avoid dropping completions.
            parser.set_error_tolerant(true);

            match parser.parse_script() {
                Ok(script) => Some(ParsedDocument::Pup {
                    script,
                    source: text.to_string(),
                    uri: uri.to_string(),
                }),
                Err(_) => {
                    // IMPORTANT:
                    // PUP parsing fails on incomplete member access like `self.`.
                    // For LSP we must still store the *latest* source text so incremental edits
                    // and completions are computed against the right document version.
                    //
                    // Prefer keeping the last-good AST if we have one, but always refresh
                    // `script_name`/`node_type` from the current header so `self.` resolves.
                    if let Some(ParsedDocument::Pup { script, .. }) = previous {
                        let mut script = script.clone();
                        let (script_name, node_type_opt) = Self::extract_pup_header(text);
                        if script_name.is_some() {
                            script.script_name = script_name;
                        }
                        if let Some(nt) = node_type_opt {
                            script.node_type = nt;
                        }
                        Some(ParsedDocument::Pup {
                            script,
                            source: text.to_string(),
                            uri: uri.to_string(),
                        })
                    } else {
                        Some(ParsedDocument::Pup {
                            script: Self::fallback_pup_script(uri, text),
                            source: text.to_string(),
                            uri: uri.to_string(),
                        })
                    }
                }
            }
        } else if uri.ends_with(".fur") {
            match FurParser::new(text) {
                Ok(mut parser) => match parser.parse() {
                    Ok(ast) => Some(ParsedDocument::Fur {
                        ast,
                        source: text.to_string(),
                        uri: uri.to_string(),
                    }),
                    Err(_) => {
                        // Keep latest source on parse errors for better incremental behavior.
                        if let Some(ParsedDocument::Fur { ast, .. }) = previous {
                            Some(ParsedDocument::Fur {
                                ast: ast.clone(),
                                source: text.to_string(),
                                uri: uri.to_string(),
                            })
                        } else {
                            Some(ParsedDocument::Fur {
                                ast: Vec::new(),
                                source: text.to_string(),
                                uri: uri.to_string(),
                            })
                        }
                    }
                },
                Err(_) => Some(ParsedDocument::Fur {
                    ast: Vec::new(),
                    source: text.to_string(),
                    uri: uri.to_string(),
                }),
            }
        } else {
            None
        }
    }

    async fn update_diagnostics(&self, uri: &str, source: Option<&str>) {
        let diagnostics = if let Some(source_text) = source {
            // Use provided source text
            if uri.ends_with(".pup") {
                diagnose_pup(source_text, uri)
            } else if uri.ends_with(".fur") {
                diagnose_fur(source_text, uri)
            } else {
                Vec::new()
            }
        } else {
            // Try to get from cache
            let documents = self.documents.read().await;
            if let Some(doc) = documents.get(uri) {
                match doc {
                    ParsedDocument::Pup { source, .. } => diagnose_pup(source, uri),
                    ParsedDocument::Fur { source, .. } => diagnose_fur(source, uri),
                }
            } else {
                Vec::new()
            }
        };

        if let Ok(url) = url::Url::parse(uri) {
            self.client
                .publish_diagnostics(url, diagnostics, None)
                .await;
        }
    }
}

#[async_trait]
impl LanguageServer for PerroLspServer {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "perro-lsp".to_string(),
                version: Some("0.1.0".to_string()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::INCREMENTAL,
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string(), ":".to_string()]),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                // Diagnostics are pushed via publish_diagnostics, so we don't need to declare
                // diagnostic_provider capability (which would require pull-based diagnostics)
                ..Default::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Perro LSP server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.to_string();
        let text = params.text_document.text;

        {
            let documents = self.documents.read().await;
            let prev = documents.get(&uri).cloned();
            drop(documents);
            if let Some(doc) = self
                .parse_document_with_fallback(&uri, &text, prev.as_ref())
                .await
            {
                let mut documents = self.documents.write().await;
                documents.insert(uri.clone(), doc);
            }
        }

        // Always publish diagnostics, even if parsing failed
        self.update_diagnostics(&uri, Some(&text)).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.to_string();

        // Get the full text from changes
        let documents = &mut *self.documents.write().await;
        let previous_doc = documents.get(&uri).cloned();
        let mut text = previous_doc
            .as_ref()
            .map(|d| d.source().to_string())
            .unwrap_or_default();

        // Apply changes
        for change in params.content_changes {
            match change {
                TextDocumentContentChangeEvent {
                    range: Some(range),
                    range_length: _,
                    text: change_text,
                } => {
                    // Convert LSP range to byte offsets
                    let start_offset = lsp_range_to_offset(&text, &range.start);
                    let end_offset = lsp_range_to_offset(&text, &range.end);

                    if start_offset <= text.len() && end_offset <= text.len() {
                        text.replace_range(start_offset..end_offset, &change_text);
                    }
                }
                TextDocumentContentChangeEvent {
                    range: None,
                    range_length: _,
                    text: change_text,
                } => {
                    text = change_text;
                }
            }
        }

        // Re-parse (with fallback) and ALWAYS store latest source.
        if let Some(doc) = self
            .parse_document_with_fallback(&uri, &text, previous_doc.as_ref())
            .await
        {
            documents.insert(uri.clone(), doc);
        }

        // Always publish diagnostics, even if parsing failed
        self.update_diagnostics(&uri, Some(&text)).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri.to_string();
        let mut documents = self.documents.write().await;
        documents.remove(&uri);
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri.to_string();
        let position = params.text_document_position.position;

        let documents = self.documents.read().await;
        if let Some(doc) = documents.get(&uri) {
            // Wrap completion in catch_unwind to prevent panics from crashing the server
            let doc_clone = doc.clone();
            let items = tokio::task::spawn_blocking(move || {
                std::panic::catch_unwind(|| get_completions(&doc_clone, position)).unwrap_or_else(
                    |_| {
                        // If completion panics, return empty list instead of crashing
                        Vec::new()
                    },
                )
            })
            .await
            .unwrap_or_else(|_| Vec::new());

            return Ok(Some(CompletionResponse::Array(items)));
        }

        Ok(None)
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .to_string();
        let position = params.text_document_position_params.position;

        let documents = self.documents.read().await;
        if let Some(doc) = documents.get(&uri) {
            return Ok(get_hover(doc, position));
        }

        Ok(None)
    }
}

fn lsp_range_to_offset(text: &str, position: &Position) -> usize {
    let mut offset = 0;
    let lines: Vec<&str> = text.lines().collect();

    for (line_num, line) in lines.iter().enumerate() {
        if line_num == position.line as usize {
            // Add the character offset on this line
            let char_offset = position.character as usize;
            let line_bytes = line.as_bytes();
            let mut byte_offset = 0;
            let mut chars = 0;

            for (i, _) in line.char_indices() {
                if chars >= char_offset {
                    byte_offset = i;
                    break;
                }
                chars += 1;
            }

            if chars < char_offset {
                byte_offset = line_bytes.len();
            }

            return offset + byte_offset;
        }
        offset += line.len() + 1; // +1 for newline
    }

    offset
}
