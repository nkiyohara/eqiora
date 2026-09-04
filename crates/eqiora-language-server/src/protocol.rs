use std::{collections::BTreeMap, error::Error};

use eqiora::api::{
    EditorPosition, EditorService, EditorSnapshot, EditorSymbol, EditorSymbolKind,
    EditorWorkspaceSnapshot,
};
use eqiora::{Diagnostic as EqioraDiagnostic, Severity};
use lsp_server::{Connection, ErrorCode, Message, Notification, Request, Response};
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DocumentFormattingParams, DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse,
    FoldingRange, FoldingRangeParams, FoldingRangeProviderCapability, GotoDefinitionParams,
    GotoDefinitionResponse, Hover, HoverContents, HoverParams, HoverProviderCapability, Location,
    MarkupContent, MarkupKind, NumberOrString, OneOf, Position, PositionEncodingKind,
    PublishDiagnosticsParams, Range, ServerCapabilities, SymbolKind, TextDocumentSyncCapability,
    TextDocumentSyncKind, TextDocumentSyncOptions, TextEdit, Uri,
};
use serde::de::DeserializeOwned;

type ServerResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

struct OpenDocument {
    uri: Uri,
    source: String,
    version: i32,
    analysis: EditorService,
    resolved: Option<(EditorWorkspaceSnapshot, String)>,
}

impl OpenDocument {
    fn new(uri: Uri, version: i32, source: String) -> Self {
        let analysis_version = analysis_version(version);
        let analysis = EditorService::new(uri.as_str(), analysis_version, source.clone());
        let resolved = (source.len() <= EditorSnapshot::MAX_SOURCE_BYTES)
            .then(|| resolved_snapshot(analysis_version, &source))
            .flatten();
        Self {
            uri,
            source,
            version,
            analysis,
            resolved,
        }
    }

    fn replace(&mut self, version: i32, source: String) -> bool {
        let analysis_version = analysis_version(version);
        if version <= self.version
            || self
                .analysis
                .replace(analysis_version, source.clone())
                .is_err()
        {
            return false;
        }
        self.source = source;
        self.version = version;
        self.resolved = (self.source.len() <= EditorSnapshot::MAX_SOURCE_BYTES)
            .then(|| resolved_snapshot(analysis_version, &self.source))
            .flatten();
        true
    }

    fn snapshot(&self) -> &EditorSnapshot {
        self.analysis.current()
    }
}

fn analysis_version(version: i32) -> u64 {
    u64::try_from(i64::from(version) - i64::from(i32::MIN))
        .expect("every LSP document version maps to u64")
}

fn resolved_snapshot(version: u64, source: &str) -> Option<(EditorWorkspaceSnapshot, String)> {
    let snapshot = EditorWorkspaceSnapshot::analyze_standalone(version, source);
    let file = snapshot.files().next()?.to_owned();
    Some((snapshot, file))
}

pub fn run(connection: Connection, version: &str) -> ServerResult<()> {
    let capabilities = ServerCapabilities {
        position_encoding: Some(PositionEncodingKind::UTF16),
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
                ..TextDocumentSyncOptions::default()
            },
        )),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        definition_provider: Some(OneOf::Left(true)),
        document_symbol_provider: Some(OneOf::Left(true)),
        document_formatting_provider: Some(OneOf::Left(true)),
        folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
        ..ServerCapabilities::default()
    };
    let (initialize_id, _) = connection.initialize_start()?;
    connection.initialize_finish(
        initialize_id,
        serde_json::json!({
            "capabilities": capabilities,
            "serverInfo": { "name": "eqiora-language-server", "version": version },
        }),
    )?;

    let mut documents = BTreeMap::<String, OpenDocument>::new();
    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request)? {
                    break;
                }
                handle_request(&connection, request, &documents)?;
            }
            Message::Notification(notification) => {
                if notification.method == "exit" {
                    break;
                }
                handle_notification(&connection, notification, &mut documents)?;
            }
            Message::Response(_) => {}
        }
    }
    Ok(())
}

fn handle_notification(
    connection: &Connection,
    notification: Notification,
    documents: &mut BTreeMap<String, OpenDocument>,
) -> ServerResult<()> {
    match notification.method.as_str() {
        "textDocument/didOpen" => {
            let Some(params) = decode_notification(notification.params) else {
                return Ok(());
            };
            let params: DidOpenTextDocumentParams = params;
            let item = params.text_document;
            let document = OpenDocument::new(item.uri.clone(), item.version, item.text);
            publish_diagnostics(connection, &document)?;
            documents.insert(item.uri.as_str().to_owned(), document);
        }
        "textDocument/didChange" => {
            let Some(params) = decode_notification(notification.params) else {
                return Ok(());
            };
            let params: DidChangeTextDocumentParams = params;
            let identifier = params.text_document;
            if params.content_changes.len() != 1 {
                return Ok(());
            }
            let Some(change) = params.content_changes.into_iter().next() else {
                return Ok(());
            };
            if change.range.is_some() {
                return Ok(());
            }
            if let Some(document) = documents.get_mut(identifier.uri.as_str())
                && document.replace(identifier.version, change.text)
            {
                publish_diagnostics(connection, document)?;
            }
        }
        "textDocument/didClose" => {
            let Some(params) = decode_notification(notification.params) else {
                return Ok(());
            };
            let params: DidCloseTextDocumentParams = params;
            if documents
                .remove(params.text_document.uri.as_str())
                .is_some()
            {
                let notification = Notification::new(
                    "textDocument/publishDiagnostics".to_owned(),
                    PublishDiagnosticsParams::new(params.text_document.uri, Vec::new(), None),
                );
                connection.sender.send(notification.into())?;
            }
        }
        "initialized" | "$/cancelRequest" => {}
        _ => {}
    }
    Ok(())
}

fn decode_notification<T: DeserializeOwned>(params: serde_json::Value) -> Option<T> {
    serde_json::from_value(params).ok()
}

fn handle_request(
    connection: &Connection,
    request: Request,
    documents: &BTreeMap<String, OpenDocument>,
) -> ServerResult<()> {
    let id = request.id.clone();
    let response = match request.method.as_str() {
        "textDocument/formatting" => response_from(
            id,
            decode(request.params).and_then(|params| formatting(params, documents)),
        ),
        "textDocument/documentSymbol" => response_from(
            id,
            decode(request.params).and_then(|params| document_symbols(params, documents)),
        ),
        "textDocument/foldingRange" => response_from(
            id,
            decode(request.params).and_then(|params| folding_ranges(params, documents)),
        ),
        "textDocument/hover" => response_from(
            id,
            decode(request.params).and_then(|params| hover(params, documents)),
        ),
        "textDocument/definition" => response_from(
            id,
            decode(request.params).and_then(|params| definition(params, documents)),
        ),
        _ => Response::new_err(
            id,
            ErrorCode::MethodNotFound as i32,
            format!("unsupported request method `{}`", request.method),
        ),
    };
    connection.sender.send(response.into())?;
    Ok(())
}

fn decode<T: DeserializeOwned>(params: serde_json::Value) -> Result<T, String> {
    serde_json::from_value(params).map_err(|error| format!("invalid request parameters: {error}"))
}

fn response_from<T: serde::Serialize>(
    id: lsp_server::RequestId,
    result: Result<T, String>,
) -> Response {
    match result {
        Ok(value) => Response::new_ok(id, value),
        Err(message) => Response::new_err(id, ErrorCode::InvalidParams as i32, message),
    }
}

fn document<'a>(
    documents: &'a BTreeMap<String, OpenDocument>,
    uri: &Uri,
) -> Result<&'a OpenDocument, String> {
    documents
        .get(uri.as_str())
        .ok_or_else(|| format!("document `{}` is not open", uri.as_str()))
}

fn formatting(
    params: DocumentFormattingParams,
    documents: &BTreeMap<String, OpenDocument>,
) -> Result<Vec<TextEdit>, String> {
    let document = document(documents, &params.text_document.uri)?;
    let Some(formatted) = document.snapshot().formatted() else {
        return Ok(Vec::new());
    };
    if formatted == document.source {
        return Ok(Vec::new());
    }
    Ok(vec![TextEdit {
        range: source_range(document.snapshot(), 0, document.source.len())?,
        new_text: formatted.to_owned(),
    }])
}

fn document_symbols(
    params: DocumentSymbolParams,
    documents: &BTreeMap<String, OpenDocument>,
) -> Result<DocumentSymbolResponse, String> {
    let document = document(documents, &params.text_document.uri)?;
    let symbols = document
        .snapshot()
        .symbols()
        .iter()
        .map(|symbol| lsp_symbol(document.snapshot(), symbol))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DocumentSymbolResponse::Nested(symbols))
}

#[allow(deprecated)]
fn lsp_symbol(snapshot: &EditorSnapshot, symbol: &EditorSymbol) -> Result<DocumentSymbol, String> {
    let range = symbol_range(snapshot, symbol)?;
    let children = symbol
        .children()
        .iter()
        .map(|child| lsp_symbol(snapshot, child))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DocumentSymbol {
        name: symbol.name().to_owned(),
        detail: Some(symbol_label(symbol.kind()).to_owned()),
        kind: symbol_kind(symbol.kind()),
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: (!children.is_empty()).then_some(children),
    })
}

fn folding_ranges(
    params: FoldingRangeParams,
    documents: &BTreeMap<String, OpenDocument>,
) -> Result<Vec<FoldingRange>, String> {
    let document = document(documents, &params.text_document.uri)?;
    let mut ranges = Vec::new();
    collect_folding_ranges(
        document.snapshot(),
        document.snapshot().symbols(),
        &mut ranges,
    )?;
    Ok(ranges)
}

fn collect_folding_ranges(
    snapshot: &EditorSnapshot,
    symbols: &[EditorSymbol],
    output: &mut Vec<FoldingRange>,
) -> Result<(), String> {
    for symbol in symbols {
        let range = symbol_range(snapshot, symbol)?;
        if range.start.line < range.end.line {
            output.push(FoldingRange {
                start_line: range.start.line,
                start_character: Some(range.start.character),
                end_line: range.end.line,
                end_character: Some(range.end.character),
                kind: None,
                collapsed_text: Some(symbol.name().to_owned()),
            });
        }
        collect_folding_ranges(snapshot, symbol.children(), output)?;
    }
    Ok(())
}

fn hover(
    params: HoverParams,
    documents: &BTreeMap<String, OpenDocument>,
) -> Result<Option<Hover>, String> {
    let uri = &params.text_document_position_params.text_document.uri;
    let document = document(documents, uri)?;
    let Some((workspace, file)) = &document.resolved else {
        return Ok(None);
    };
    let position = editor_position(params.text_document_position_params.position);
    let Some((definition, source)) = workspace.hover_at_position(file, position) else {
        return Ok(None);
    };
    Ok(Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: markdown_hover(definition.kind(), definition.path(), source),
        }),
        range: None,
    }))
}

fn markdown_hover(kind: EditorSymbolKind, path: &str, source: &str) -> String {
    let longest_run = source
        .split(|character| character != '`')
        .map(str::len)
        .max()
        .unwrap_or_default();
    let fence = "`".repeat(longest_run.saturating_add(1).max(3));
    format!(
        "**{}** `{path}`\n\n{fence}eqiora\n{source}\n{fence}",
        symbol_label(kind)
    )
}

fn definition(
    params: GotoDefinitionParams,
    documents: &BTreeMap<String, OpenDocument>,
) -> Result<Option<GotoDefinitionResponse>, String> {
    let uri = &params.text_document_position_params.text_document.uri;
    let document = document(documents, uri)?;
    let Some((workspace, file)) = &document.resolved else {
        return Ok(None);
    };
    let position = editor_position(params.text_document_position_params.position);
    let Some(definition) = workspace.definition_for_reference_at_position(file, position) else {
        return Ok(None);
    };
    if definition.file() != file {
        return Ok(None);
    }
    let target = workspace
        .document(file)
        .ok_or_else(|| "resolved definition document is unavailable".to_owned())?;
    let definition_range = definition.name_range().unwrap_or(definition.range());
    let range = source_range(
        target,
        usize::try_from(definition_range.start()).map_err(|_| "source offset exceeds usize")?,
        usize::try_from(definition_range.end()).map_err(|_| "source offset exceeds usize")?,
    )?;
    Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
        document.uri.clone(),
        range,
    ))))
}

fn publish_diagnostics(connection: &Connection, document: &OpenDocument) -> ServerResult<()> {
    let diagnostics = document
        .snapshot()
        .diagnostics()
        .iter()
        .map(|diagnostic| lsp_diagnostic(document.snapshot(), diagnostic))
        .collect();
    let params =
        PublishDiagnosticsParams::new(document.uri.clone(), diagnostics, Some(document.version));
    connection
        .sender
        .send(Notification::new("textDocument/publishDiagnostics".to_owned(), params).into())?;
    Ok(())
}

fn lsp_diagnostic(
    snapshot: &EditorSnapshot,
    diagnostic: &EqioraDiagnostic,
) -> lsp_types::Diagnostic {
    let range = diagnostic
        .source_span()
        .and_then(|span| {
            source_range(
                snapshot,
                usize::try_from(span.start).ok()?,
                usize::try_from(span.end).ok()?,
            )
            .ok()
        })
        .unwrap_or_default();
    lsp_types::Diagnostic {
        range,
        severity: Some(match diagnostic.severity() {
            Severity::Error => lsp_types::DiagnosticSeverity::ERROR,
            Severity::Warning => lsp_types::DiagnosticSeverity::WARNING,
            Severity::Note => lsp_types::DiagnosticSeverity::INFORMATION,
        }),
        code: Some(NumberOrString::String(diagnostic.code().to_string())),
        source: Some("eqiora".to_owned()),
        message: diagnostic.message().to_owned(),
        ..lsp_types::Diagnostic::default()
    }
}

fn source_range(snapshot: &EditorSnapshot, start: usize, end: usize) -> Result<Range, String> {
    let start = u32::try_from(start).map_err(|_| "source offset exceeds u32".to_owned())?;
    let end = u32::try_from(end).map_err(|_| "source offset exceeds u32".to_owned())?;
    let start = snapshot
        .position(start)
        .ok_or_else(|| "source start is not an exact UTF-8 boundary".to_owned())?;
    let end = snapshot
        .position(end)
        .ok_or_else(|| "source end is not an exact UTF-8 boundary".to_owned())?;
    Ok(Range::new(lsp_position(start), lsp_position(end)))
}

fn symbol_range(snapshot: &EditorSnapshot, symbol: &EditorSymbol) -> Result<Range, String> {
    source_range(
        snapshot,
        usize::try_from(symbol.range().start()).map_err(|_| "source offset exceeds usize")?,
        usize::try_from(symbol.range().end()).map_err(|_| "source offset exceeds usize")?,
    )
}

fn lsp_position(position: EditorPosition) -> Position {
    Position::new(position.line(), position.character())
}

const fn editor_position(position: Position) -> EditorPosition {
    EditorPosition::new(position.line, position.character)
}

const fn symbol_kind(kind: EditorSymbolKind) -> SymbolKind {
    match kind {
        EditorSymbolKind::Module => SymbolKind::MODULE,
        EditorSymbolKind::Import => SymbolKind::NAMESPACE,
        EditorSymbolKind::Dimension => SymbolKind::TYPE_PARAMETER,
        EditorSymbolKind::Property | EditorSymbolKind::Parameter => SymbolKind::PROPERTY,
        EditorSymbolKind::Material => SymbolKind::OBJECT,
        EditorSymbolKind::Connector => SymbolKind::INTERFACE,
        EditorSymbolKind::Component | EditorSymbolKind::Model => SymbolKind::CLASS,
        EditorSymbolKind::Operator => SymbolKind::FUNCTION,
        EditorSymbolKind::Domain | EditorSymbolKind::Support => SymbolKind::NAMESPACE,
        EditorSymbolKind::Let | EditorSymbolKind::Formal => SymbolKind::CONSTANT,
        EditorSymbolKind::Field => SymbolKind::FIELD,
        EditorSymbolKind::Representation => SymbolKind::TYPE_PARAMETER,
        EditorSymbolKind::Port => SymbolKind::INTERFACE,
        EditorSymbolKind::Clock => SymbolKind::EVENT,
        EditorSymbolKind::Relation => SymbolKind::OPERATOR,
        EditorSymbolKind::Instance => SymbolKind::OBJECT,
        _ => SymbolKind::OBJECT,
    }
}

const fn symbol_label(kind: EditorSymbolKind) -> &'static str {
    match kind {
        EditorSymbolKind::Module => "Module",
        EditorSymbolKind::Import => "Import",
        EditorSymbolKind::Dimension => "Dimension",
        EditorSymbolKind::Property => "Property",
        EditorSymbolKind::Material => "Material",
        EditorSymbolKind::Connector => "Connector",
        EditorSymbolKind::Component => "Component",
        EditorSymbolKind::Operator => "Operator",
        EditorSymbolKind::Model => "Model",
        EditorSymbolKind::Domain => "Domain",
        EditorSymbolKind::Parameter => "Parameter",
        EditorSymbolKind::Let => "Let",
        EditorSymbolKind::Formal => "Formal",
        EditorSymbolKind::Support => "Support",
        EditorSymbolKind::Field => "Field",
        EditorSymbolKind::Representation => "Representation",
        EditorSymbolKind::Port => "Port",
        EditorSymbolKind::Clock => "Clock",
        EditorSymbolKind::Relation => "Relation",
        EditorSymbolKind::Instance => "Instance",
        _ => "Declaration",
    }
}
