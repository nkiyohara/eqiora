use std::{
    collections::BTreeMap,
    error::Error,
    path::{Path, PathBuf},
    str::FromStr,
};

use eqiora::api::{
    EditorPosition, EditorService, EditorSnapshot, EditorSymbol, EditorSymbolKind,
    EditorWorkspaceSnapshot,
};
use eqiora::compiler::{CompilationNamespaceId, ResolvedHierarchyInput, ResolvedSourceUnit};
use eqiora::{Diagnostic as EqioraDiagnostic, Severity};
use lsp_server::{Connection, ErrorCode, Message, Notification, Request, Response};
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DocumentFormattingParams, DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse,
    FoldingRange, FoldingRangeParams, FoldingRangeProviderCapability, GotoDefinitionParams,
    GotoDefinitionResponse, Hover, HoverContents, HoverParams, HoverProviderCapability,
    InitializeParams, Location, MarkupContent, MarkupKind, NumberOrString, OneOf, Position,
    PositionEncodingKind, PublishDiagnosticsParams, Range, ServerCapabilities, SymbolKind,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions, TextEdit, Uri,
    WorkspaceFoldersServerCapabilities, WorkspaceServerCapabilities,
};
use serde::de::DeserializeOwned;

type ServerResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

struct OpenDocument {
    uri: Uri,
    source: String,
    version: i32,
    analysis: EditorService,
}

impl OpenDocument {
    fn new(uri: Uri, version: i32, source: String) -> Self {
        let analysis_version = analysis_version(version);
        let analysis = EditorService::new(uri.as_str(), analysis_version, source.clone());
        Self {
            uri,
            source,
            version,
            analysis,
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
        true
    }

    fn snapshot(&self) -> &EditorSnapshot {
        self.analysis.current()
    }
}

struct WorkspaceAnalysis {
    snapshot: EditorWorkspaceSnapshot,
    file_by_uri: BTreeMap<String, String>,
    uri_by_file: BTreeMap<String, Uri>,
}

struct PackageProject {
    root_path: PathBuf,
    relative_by_uri: BTreeMap<String, PathBuf>,
}

struct ServerState {
    documents: BTreeMap<String, OpenDocument>,
    roots: Vec<String>,
    projects: BTreeMap<String, PackageProject>,
    workspaces: BTreeMap<String, WorkspaceAnalysis>,
    next_analysis_version: u64,
}

impl ServerState {
    fn new(mut roots: Vec<String>) -> Self {
        roots.sort();
        roots.dedup();
        roots.sort_by_key(|root| std::cmp::Reverse(root.len()));
        let mut state = Self {
            documents: BTreeMap::new(),
            roots,
            projects: BTreeMap::new(),
            workspaces: BTreeMap::new(),
            next_analysis_version: 0,
        };
        for root in state.roots.clone() {
            let Ok(uri) = Uri::from_str(&root) else {
                continue;
            };
            let Some(root_path) = file_uri_path(&uri) else {
                continue;
            };
            if !root_path.join("eqiora.toml").is_file() {
                continue;
            }
            state.next_analysis_version = state.next_analysis_version.saturating_add(1);
            let Ok((snapshot, paths)) = EditorWorkspaceSnapshot::analyze_local_package_project_v1(
                state.next_analysis_version,
                &root_path,
                &BTreeMap::new(),
            ) else {
                continue;
            };
            let Some((workspace, relative_by_uri)) = package_workspace(&root, snapshot, paths)
            else {
                continue;
            };
            state.projects.insert(
                root.clone(),
                PackageProject {
                    root_path,
                    relative_by_uri,
                },
            );
            state.workspaces.insert(root, workspace);
        }
        state
    }

    fn group_for_uri(&self, uri: &str) -> String {
        self.roots
            .iter()
            .find(|root| uri.starts_with(root.as_str()))
            .cloned()
            .unwrap_or_else(|| uri.to_owned())
    }

    fn rebuild_group(&mut self, group: &str) {
        if let Some(project) = self.projects.get(group) {
            let root_path = project.root_path.clone();
            let overrides = self
                .documents
                .iter()
                .filter_map(|(uri, document)| {
                    project
                        .relative_by_uri
                        .get(uri)
                        .cloned()
                        .map(|path| (path, document.source.clone()))
                })
                .collect::<BTreeMap<_, _>>();
            self.next_analysis_version = self.next_analysis_version.saturating_add(1);
            if let Ok((snapshot, paths)) = EditorWorkspaceSnapshot::analyze_local_package_project_v1(
                self.next_analysis_version,
                &root_path,
                &overrides,
            ) && let Some((workspace, relative_by_uri)) =
                package_workspace(group, snapshot, paths)
            {
                self.projects
                    .get_mut(group)
                    .expect("package project remains indexed")
                    .relative_by_uri = relative_by_uri;
                self.workspaces.insert(group.to_owned(), workspace);
                return;
            }
        }
        let owner = CompilationNamespaceId::new(["editor-workspace"])
            .expect("fixed editor workspace namespace is valid");
        let mut units = Vec::new();
        let mut file_by_uri = BTreeMap::new();
        let mut uri_by_file = BTreeMap::new();
        for (uri, document) in &self.documents {
            if self.group_for_uri(uri) != group
                || document.source.len() > EditorSnapshot::MAX_SOURCE_BYTES
            {
                continue;
            }
            let unit = ResolvedSourceUnit::new(owner.clone(), uri, document.source.as_str());
            let file = unit.diagnostic_file();
            file_by_uri.insert(uri.clone(), file.clone());
            uri_by_file.insert(file, document.uri.clone());
            units.push(unit);
        }
        if units.is_empty() {
            self.workspaces.remove(group);
            return;
        }
        self.next_analysis_version = self.next_analysis_version.saturating_add(1);
        let snapshot = EditorWorkspaceSnapshot::analyze_modules(
            self.next_analysis_version,
            ResolvedHierarchyInput::new(owner, units, vec![]),
        );
        self.workspaces.insert(
            group.to_owned(),
            WorkspaceAnalysis {
                snapshot,
                file_by_uri,
                uri_by_file,
            },
        );
    }

    fn resolved(&self, uri: &Uri) -> Option<(&EditorWorkspaceSnapshot, &str)> {
        let group = self.group_for_uri(uri.as_str());
        let workspace = self.workspaces.get(&group)?;
        let file = workspace.file_by_uri.get(uri.as_str())?;
        Some((&workspace.snapshot, file))
    }

    fn snapshot(&self, uri: &Uri) -> Result<&EditorSnapshot, String> {
        let document = self
            .documents
            .get(uri.as_str())
            .ok_or_else(|| format!("document `{}` is not open", uri.as_str()))?;
        Ok(self
            .resolved(uri)
            .and_then(|(workspace, file)| workspace.document(file))
            .unwrap_or_else(|| document.snapshot()))
    }

    fn uri_for_file(&self, source_uri: &Uri, file: &str) -> Option<Uri> {
        let group = self.group_for_uri(source_uri.as_str());
        self.workspaces.get(&group)?.uri_by_file.get(file).cloned()
    }
}

fn package_workspace(
    root: &str,
    snapshot: EditorWorkspaceSnapshot,
    paths: BTreeMap<String, PathBuf>,
) -> Option<(WorkspaceAnalysis, BTreeMap<String, PathBuf>)> {
    let mut file_by_uri = BTreeMap::new();
    let mut uri_by_file = BTreeMap::new();
    let mut relative_by_uri = BTreeMap::new();
    for (file, path) in paths {
        let uri = project_file_uri(root, &path)?;
        file_by_uri.insert(uri.as_str().to_owned(), file.clone());
        relative_by_uri.insert(uri.as_str().to_owned(), path);
        uri_by_file.insert(file, uri);
    }
    Some((
        WorkspaceAnalysis {
            snapshot,
            file_by_uri,
            uri_by_file,
        },
        relative_by_uri,
    ))
}

fn file_uri_path(uri: &Uri) -> Option<PathBuf> {
    if !uri.scheme()?.as_str().eq_ignore_ascii_case("file")
        || uri.authority().is_some_and(|authority| {
            !authority.as_str().is_empty() && !authority.as_str().eq_ignore_ascii_case("localhost")
        })
    {
        return None;
    }
    let path = uri
        .path()
        .as_estr()
        .decode()
        .into_string()
        .ok()?
        .into_owned();
    #[cfg(windows)]
    let path = if path.starts_with('/') && path.as_bytes().get(2) == Some(&b':') {
        path[1..].to_owned()
    } else {
        path
    };
    Some(PathBuf::from(path))
}

fn project_file_uri(root: &str, relative: &Path) -> Option<Uri> {
    let path = relative.to_str()?.replace(std::path::MAIN_SEPARATOR, "/");
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            write!(&mut encoded, "%{byte:02X}").expect("writing to String cannot fail");
        }
    }
    Uri::from_str(&format!("{root}{encoded}")).ok()
}

fn analysis_version(version: i32) -> u64 {
    u64::try_from(i64::from(version) - i64::from(i32::MIN))
        .expect("every LSP document version maps to u64")
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
        workspace: Some(WorkspaceServerCapabilities {
            workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                supported: Some(true),
                change_notifications: Some(OneOf::Left(false)),
            }),
            file_operations: None,
        }),
        ..ServerCapabilities::default()
    };
    let (initialize_id, initialize_params) = connection.initialize_start()?;
    let initialize_params: InitializeParams = serde_json::from_value(initialize_params)?;
    let roots = workspace_roots(&initialize_params);
    connection.initialize_finish(
        initialize_id,
        serde_json::json!({
            "capabilities": capabilities,
            "serverInfo": { "name": "eqiora-language-server", "version": version },
        }),
    )?;

    let mut state = ServerState::new(roots);
    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request)? {
                    break;
                }
                handle_request(&connection, request, &state)?;
            }
            Message::Notification(notification) => {
                if notification.method == "exit" {
                    break;
                }
                handle_notification(&connection, notification, &mut state)?;
            }
            Message::Response(_) => {}
        }
    }
    Ok(())
}

#[allow(deprecated)]
fn workspace_roots(params: &InitializeParams) -> Vec<String> {
    let roots = params.workspace_folders.as_ref().map_or_else(
        || params.root_uri.iter().collect::<Vec<_>>(),
        |folders| folders.iter().map(|folder| &folder.uri).collect(),
    );
    roots
        .into_iter()
        .map(|uri| {
            let uri = uri.as_str();
            if uri.ends_with('/') {
                uri.to_owned()
            } else {
                format!("{uri}/")
            }
        })
        .collect()
}

fn handle_notification(
    connection: &Connection,
    notification: Notification,
    state: &mut ServerState,
) -> ServerResult<()> {
    match notification.method.as_str() {
        "textDocument/didOpen" => {
            let Some(params) = decode_notification(notification.params) else {
                return Ok(());
            };
            let params: DidOpenTextDocumentParams = params;
            let item = params.text_document;
            let document = OpenDocument::new(item.uri.clone(), item.version, item.text);
            let group = state.group_for_uri(item.uri.as_str());
            state
                .documents
                .insert(item.uri.as_str().to_owned(), document);
            rebuild_and_publish(connection, state, &group)?;
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
            let group = state.group_for_uri(identifier.uri.as_str());
            let accepted = state
                .documents
                .get_mut(identifier.uri.as_str())
                .is_some_and(|document| document.replace(identifier.version, change.text));
            if accepted {
                rebuild_and_publish(connection, state, &group)?;
            }
        }
        "textDocument/didClose" => {
            let Some(params) = decode_notification(notification.params) else {
                return Ok(());
            };
            let params: DidCloseTextDocumentParams = params;
            let group = state.group_for_uri(params.text_document.uri.as_str());
            if state
                .documents
                .remove(params.text_document.uri.as_str())
                .is_some()
            {
                rebuild_and_publish(connection, state, &group)?;
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

fn rebuild_and_publish(
    connection: &Connection,
    state: &mut ServerState,
    group: &str,
) -> ServerResult<()> {
    state.rebuild_group(group);
    let uris = state
        .documents
        .values()
        .filter(|document| state.group_for_uri(document.uri.as_str()) == group)
        .map(|document| document.uri.clone())
        .collect::<Vec<_>>();
    for uri in uris {
        publish_diagnostics(connection, state, &uri)?;
    }
    Ok(())
}

fn decode_notification<T: DeserializeOwned>(params: serde_json::Value) -> Option<T> {
    serde_json::from_value(params).ok()
}

fn handle_request(
    connection: &Connection,
    request: Request,
    state: &ServerState,
) -> ServerResult<()> {
    let id = request.id.clone();
    let response = match request.method.as_str() {
        "textDocument/formatting" => response_from(
            id,
            decode(request.params).and_then(|params| formatting(params, state)),
        ),
        "textDocument/documentSymbol" => response_from(
            id,
            decode(request.params).and_then(|params| document_symbols(params, state)),
        ),
        "textDocument/foldingRange" => response_from(
            id,
            decode(request.params).and_then(|params| folding_ranges(params, state)),
        ),
        "textDocument/hover" => response_from(
            id,
            decode(request.params).and_then(|params| hover(params, state)),
        ),
        "textDocument/definition" => response_from(
            id,
            decode(request.params).and_then(|params| definition(params, state)),
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

fn document<'a>(state: &'a ServerState, uri: &Uri) -> Result<&'a OpenDocument, String> {
    state
        .documents
        .get(uri.as_str())
        .ok_or_else(|| format!("document `{}` is not open", uri.as_str()))
}

fn formatting(
    params: DocumentFormattingParams,
    state: &ServerState,
) -> Result<Vec<TextEdit>, String> {
    let document = document(state, &params.text_document.uri)?;
    let snapshot = state.snapshot(&params.text_document.uri)?;
    let Some(formatted) = snapshot.formatted() else {
        return Ok(Vec::new());
    };
    if formatted == document.source {
        return Ok(Vec::new());
    }
    Ok(vec![TextEdit {
        range: source_range(snapshot, 0, document.source.len())?,
        new_text: formatted.to_owned(),
    }])
}

fn document_symbols(
    params: DocumentSymbolParams,
    state: &ServerState,
) -> Result<DocumentSymbolResponse, String> {
    let snapshot = state.snapshot(&params.text_document.uri)?;
    let symbols = snapshot
        .symbols()
        .iter()
        .map(|symbol| lsp_symbol(snapshot, symbol))
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
    state: &ServerState,
) -> Result<Vec<FoldingRange>, String> {
    let snapshot = state.snapshot(&params.text_document.uri)?;
    let mut ranges = Vec::new();
    collect_folding_ranges(snapshot, snapshot.symbols(), &mut ranges)?;
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

fn hover(params: HoverParams, state: &ServerState) -> Result<Option<Hover>, String> {
    let uri = &params.text_document_position_params.text_document.uri;
    document(state, uri)?;
    let Some((workspace, file)) = state.resolved(uri) else {
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
    state: &ServerState,
) -> Result<Option<GotoDefinitionResponse>, String> {
    let uri = &params.text_document_position_params.text_document.uri;
    document(state, uri)?;
    let Some((workspace, file)) = state.resolved(uri) else {
        return Ok(None);
    };
    let position = editor_position(params.text_document_position_params.position);
    let Some(definition) = workspace.definition_for_reference_at_position(file, position) else {
        return Ok(None);
    };
    let target_uri = state
        .uri_for_file(uri, definition.file())
        .ok_or_else(|| "resolved definition URI is unavailable".to_owned())?;
    let target = workspace
        .document(definition.file())
        .ok_or_else(|| "resolved definition document is unavailable".to_owned())?;
    let definition_range = definition.name_range().unwrap_or(definition.range());
    let range = source_range(
        target,
        usize::try_from(definition_range.start()).map_err(|_| "source offset exceeds usize")?,
        usize::try_from(definition_range.end()).map_err(|_| "source offset exceeds usize")?,
    )?;
    Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
        target_uri, range,
    ))))
}

fn publish_diagnostics(
    connection: &Connection,
    state: &ServerState,
    uri: &Uri,
) -> ServerResult<()> {
    let document = document(state, uri)?;
    let snapshot = state.snapshot(uri)?;
    let diagnostics = snapshot
        .diagnostics()
        .iter()
        .map(|diagnostic| lsp_diagnostic(snapshot, diagnostic))
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
