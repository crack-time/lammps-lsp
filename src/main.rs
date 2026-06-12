mod completion;
mod db;
mod diagnostics;
mod hover;

use std::collections::HashMap;
use std::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tower_lsp::lsp_types::Url;

#[derive(Debug)]
struct Backend {
    client: Client,
    db: db::CommandDb,
    docs: Mutex<HashMap<Url, String>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "lammps-lsp".to_string(),
                version: Some("0.5.8".to_string()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![" ".to_string()]),
                    ..Default::default()
                }),
                ..Default::default()
            },
        })
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = &params.text_document_position_params.position;

        let line = self.docs.lock().unwrap().get(uri).and_then(|text| {
            text.lines().nth(pos.line as usize).map(|s| s.to_string())
        }).unwrap_or_default();

        Ok(hover::get_hover(&self.db, &line, pos.line))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;
        let pos = &params.text_document_position.position;

        let line = self.docs.lock().unwrap().get(uri).and_then(|text| {
            text.lines().nth(pos.line as usize).map(|s| s.to_string())
        }).unwrap_or_default();

        let list = completion::get_completions(&self.db, pos, &line);
        Ok(Some(CompletionResponse::List(list)))
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let text = params.text_document.text.clone();
        self.validate(&uri, &text).await;
        self.docs.lock().unwrap().insert(uri, text);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        if let Some(change) = params.content_changes.into_iter().next() {
            self.validate(&uri, &change.text).await;
            self.docs.lock().unwrap().insert(uri, change.text);
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let text = self.docs.lock().unwrap().get(&params.text_document.uri).cloned();
        if let Some(content) = text {
            self.validate(&params.text_document.uri, &content).await;
        }
    }
}

impl Backend {
    async fn validate(&self, uri: &Url, text: &str) {
        let lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();

        let mut diags = Vec::new();

        let group_diags = diagnostics::check_group_count(&lines);
        diags.extend(group_diags);

        let vars = diagnostics::variables_map(&lines);
        let path_diags = diagnostics::check_file_paths(&lines, &vars);
        diags.extend(path_diags);

        for (i, line) in lines.iter().enumerate() {
            if !line.trim().starts_with('#') {
                if let Some(d) = diagnostics::check_line_brackets(line, i as u32) {
                    diags.push(d);
                }
            }
        }

        self.client
            .publish_diagnostics(uri.clone(), diags, None)
            .await;
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let db = db::CommandDb::from_embedded();

    let (service, socket) = LspService::build(|client| Backend {
        client,
        db,
        docs: Mutex::new(HashMap::new()),
    })
    .finish();

    Server::new(stdin, stdout, socket).serve(service).await;
}
