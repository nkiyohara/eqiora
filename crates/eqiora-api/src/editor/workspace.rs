//! Resolved multi-source editor projection.

use std::collections::BTreeMap;

use eqiora_compiler::{
    AnalyzedResolvedHierarchy, CanonicalDeclarationKind, ResolvedHierarchyInput,
    preflight_resolved_hierarchy,
};
use eqiora_core::Diagnostic;
use eqiora_lang::{TextRange, Token, TokenKind, lex};

use super::{EditorPosition, EditorSnapshot, EditorSymbolKind, stale_version};

/// One compiler-resolved top-level declaration and its definition location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorDefinition {
    namespace: Box<[String]>,
    path: String,
    kind: EditorSymbolKind,
    file: String,
    range: TextRange,
    name_range: Option<TextRange>,
}

/// One compiler-resolved source reference and its canonical definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorReference {
    file: String,
    range: TextRange,
    definition: EditorDefinition,
}

impl EditorReference {
    /// Package-qualified source file containing the reference.
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }

    /// Exact UTF-8 byte range of the referenced name.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }

    /// Canonical target and its definition location.
    #[must_use]
    pub const fn definition(&self) -> &EditorDefinition {
        &self.definition
    }
}

impl EditorDefinition {
    /// Exact compilation namespace segments, including locked package identity.
    #[must_use]
    pub fn namespace(&self) -> &[String] {
        &self.namespace
    }

    /// Compiler-canonical declaration path within the namespace.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Declaration category.
    #[must_use]
    pub const fn kind(&self) -> EditorSymbolKind {
        self.kind
    }

    /// Source file supplied by the resolved graph.
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }

    /// Complete UTF-8 byte range of the declaration.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }

    /// Exact UTF-8 byte range of the declared name when it was recovered from
    /// the compiler-accepted source.
    #[must_use]
    pub const fn name_range(&self) -> Option<TextRange> {
        self.name_range
    }
}

/// Immutable editor analysis of one resolved source graph.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorWorkspaceSnapshot {
    version: u64,
    documents: Vec<(String, EditorSnapshot)>,
    diagnostics: Vec<Diagnostic>,
    definitions: Vec<EditorDefinition>,
    references: Vec<EditorReference>,
}

/// Version owner for immutable resolved-workspace analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorWorkspaceService {
    requested_version: u64,
    current: EditorWorkspaceSnapshot,
}

impl EditorWorkspaceService {
    /// Begin publishing from one completely constructed workspace snapshot.
    #[must_use]
    pub const fn new(current: EditorWorkspaceSnapshot) -> Self {
        Self {
            requested_version: current.version,
            current,
        }
    }

    /// Mark a strictly newer workspace version as current before its analysis
    /// starts.
    ///
    /// # Errors
    /// Returns a precondition diagnostic when `version` is stale or repeats
    /// the current request version.
    pub fn begin(&mut self, version: u64) -> Result<(), Diagnostic> {
        if version <= self.requested_version {
            return Err(stale_version(version, self.requested_version));
        }
        self.requested_version = version;
        Ok(())
    }

    /// Atomically publish the completed snapshot for the current request.
    ///
    /// # Errors
    /// Returns a precondition diagnostic without changing the current snapshot
    /// when `next` belongs to a stale or unregistered future request.
    pub fn replace(
        &mut self,
        next: EditorWorkspaceSnapshot,
    ) -> Result<&EditorWorkspaceSnapshot, Diagnostic> {
        if next.version != self.requested_version || self.current.version == self.requested_version
        {
            return Err(stale_version(next.version, self.requested_version));
        }
        self.current = next;
        Ok(&self.current)
    }

    /// Return the current snapshot only when it still matches the request
    /// version.
    ///
    /// # Errors
    /// Returns a precondition diagnostic when the request is stale or refers
    /// to an unknown future version, or while its analysis is still pending.
    pub fn snapshot(&self, version: u64) -> Result<&EditorWorkspaceSnapshot, Diagnostic> {
        if version == self.requested_version && self.current.version == self.requested_version {
            Ok(&self.current)
        } else {
            Err(stale_version(version, self.requested_version))
        }
    }

    /// Current resolved-workspace analysis, or `None` while it is pending.
    #[must_use]
    pub const fn current(&self) -> Option<&EditorWorkspaceSnapshot> {
        if self.current.version == self.requested_version {
            Some(&self.current)
        } else {
            None
        }
    }
}

impl EditorWorkspaceSnapshot {
    /// Analyze one standalone source through the compiler-owned resolved graph.
    ///
    /// This is the single-document adapter path for clients that do not yet
    /// have an `eqiora.toml` project. Imports still require a resolved module or
    /// locked-package workspace.
    #[must_use]
    pub fn analyze_standalone(version: u64, source: impl Into<String>) -> Self {
        let owner = eqiora_compiler::CompilationNamespaceId::new(["editor-standalone"])
            .expect("fixed standalone editor namespace is valid");
        Self::analyze_modules(
            version,
            ResolvedHierarchyInput::new(
                owner.clone(),
                vec![eqiora_compiler::ResolvedSourceUnit::new(
                    owner,
                    "document.eqi",
                    source,
                )],
                vec![],
            ),
        )
    }

    /// Analyze a closed local or package-shaped source graph with the compiler's
    /// existing module and name resolver.
    #[must_use]
    pub fn analyze_modules(version: u64, input: ResolvedHierarchyInput) -> Self {
        Self::analyze_modules_with_cancellation(version, input, || false)
            .expect("non-cancellable analysis produces a snapshot")
    }

    /// Analyze a resolved source graph with cooperative cancellation.
    ///
    /// `None` publishes no partial workspace when `is_cancelled` returns true
    /// at a compiler or snapshot-construction boundary. Invalid source returns
    /// a snapshot with recovered documents and ordered diagnostics.
    pub fn analyze_modules_with_cancellation(
        version: u64,
        input: ResolvedHierarchyInput,
        mut is_cancelled: impl FnMut() -> bool,
    ) -> Option<Self> {
        if is_cancelled() {
            return None;
        }
        if let Err(diagnostic) = preflight_resolved_hierarchy(
            input.units().iter().map(|unit| unit.source().len()),
            input.aliases().len(),
        ) {
            if is_cancelled() {
                return None;
            }
            return Some(Self::from_recovered(version, Vec::new(), vec![diagnostic]));
        }
        let mut sources = Vec::with_capacity(input.units().len());
        for unit in input.units() {
            if is_cancelled() {
                return None;
            }
            sources.push((unit.diagnostic_file(), unit.source().to_owned()));
        }
        let analyzed = match input.analyze_with_cancellation(&mut is_cancelled) {
            Ok(Some(analyzed)) => analyzed,
            Ok(None) => return None,
            Err(diagnostics) => {
                if is_cancelled() {
                    return None;
                }
                return Some(Self::from_recovered(version, sources, diagnostics));
            }
        };
        if let Err(diagnostics) = analyzed.clone().validate_definitions() {
            if is_cancelled() {
                return None;
            }
            return Some(Self::from_recovered(version, sources, diagnostics));
        }
        if is_cancelled() {
            return None;
        }
        Some(Self::from_analyzed(version, sources, &analyzed))
    }

    /// Replay one exact locked package graph through the ordinary package and
    /// compiler owners, then expose its editor projection.
    ///
    /// # Errors
    /// Returns the package resolver, source, or semantic-content error without
    /// publishing a partial workspace.
    pub fn analyze_locked(
        version: u64,
        store: &impl eqiora_package::PackageStore,
        resolution: &eqiora_package::ResolutionRecordV1,
    ) -> Result<Self, crate::package::PackageCompilationError> {
        crate::package::analyze_editor_workspace(version, store, resolution)
    }

    /// Replay and analyze one exact locked package graph with cooperative
    /// cancellation between resolver, compiler, validation, and projection.
    ///
    /// # Errors
    /// Returns package resolution, source, or semantic-content failures
    /// observed before cancellation.
    pub fn analyze_locked_with_cancellation(
        version: u64,
        store: &impl eqiora_package::PackageStore,
        resolution: &eqiora_package::ResolutionRecordV1,
        is_cancelled: impl FnMut() -> bool,
    ) -> Result<Option<Self>, crate::package::PackageCompilationError> {
        crate::package::analyze_editor_workspace_with_cancellation(
            version,
            store,
            resolution,
            is_cancelled,
        )
    }

    pub(crate) fn from_analyzed(
        version: u64,
        sources: Vec<(String, String)>,
        analyzed: &AnalyzedResolvedHierarchy,
    ) -> Self {
        let resolved_sources = analyzed.resolved_source_files().collect::<Vec<_>>();
        debug_assert_eq!(sources.len(), resolved_sources.len());
        let tokens_by_file = sources
            .iter()
            .zip(&resolved_sources)
            .map(|((_file, source), resolved_file)| {
                (
                    (*resolved_file).to_owned(),
                    lex(*resolved_file, source).tokens().to_vec(),
                )
            })
            .collect::<BTreeMap<_, _>>();

        let definitions = analyzed
            .resolved_declarations()
            .filter_map(|(identity, resolved_file, range)| {
                Some(EditorDefinition {
                    namespace: identity.namespace().segments().to_vec().into_boxed_slice(),
                    path: identity.path().to_owned(),
                    kind: canonical_symbol_kind(identity.kind())?,
                    file: resolved_file.to_owned(),
                    range,
                    name_range: declaration_name_range(
                        &tokens_by_file,
                        resolved_file,
                        range,
                        identity.path(),
                    ),
                })
            })
            .collect::<Vec<_>>();
        let references = analyzed
            .resolved_references()
            .filter_map(|(target, file, range, definition_file, definition_range)| {
                Some(EditorReference {
                    file: file.to_owned(),
                    range,
                    definition: EditorDefinition {
                        namespace: target.namespace().segments().to_vec().into_boxed_slice(),
                        path: target.path().to_owned(),
                        kind: canonical_symbol_kind(target.kind())?,
                        file: definition_file.to_owned(),
                        range: definition_range,
                        name_range: declaration_name_range(
                            &tokens_by_file,
                            definition_file,
                            definition_range,
                            target.path(),
                        ),
                    },
                })
            })
            .collect();
        let documents = sources
            .into_iter()
            .zip(resolved_sources)
            .map(|((_file, source), resolved_file)| {
                let snapshot = EditorSnapshot::from_resolved_source(version, resolved_file, source);
                (resolved_file.to_owned(), snapshot)
            })
            .collect();
        Self {
            version,
            documents,
            diagnostics: Vec::new(),
            definitions,
            references,
        }
    }

    fn from_recovered(
        version: u64,
        sources: Vec<(String, String)>,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        let documents = sources
            .into_iter()
            .map(|(file, source)| {
                let document_diagnostics = diagnostics
                    .iter()
                    .filter(|diagnostic| {
                        diagnostic
                            .source_span()
                            .is_some_and(|span| span.file == file)
                    })
                    .cloned()
                    .collect();
                let snapshot = EditorSnapshot::from_recovered_source(
                    version,
                    &file,
                    source,
                    document_diagnostics,
                );
                (file, snapshot)
            })
            .collect();
        Self {
            version,
            documents,
            diagnostics,
            definitions: Vec::new(),
            references: Vec::new(),
        }
    }

    /// Exact workspace version supplied by the client.
    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    /// Ordered compiler diagnostics for the complete resolved graph.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Package-qualified source labels in resolved-graph order.
    #[must_use]
    pub fn files(&self) -> impl ExactSizeIterator<Item = &str> {
        self.documents.iter().map(|(file, _)| file.as_str())
    }

    /// Analyze one source file by its resolver-supplied path.
    #[must_use]
    pub fn document(&self, file: &str) -> Option<&EditorSnapshot> {
        self.documents
            .iter()
            .find_map(|(candidate, snapshot)| (candidate == file).then_some(snapshot))
    }

    /// Compiler-resolved declarations in canonical identity order.
    #[must_use]
    pub fn definitions(&self) -> &[EditorDefinition] {
        &self.definitions
    }

    /// Compiler-resolved declaration references in source order.
    #[must_use]
    pub fn references(&self) -> &[EditorReference] {
        &self.references
    }

    /// Resolve the reference covering one exact UTF-8 byte offset.
    #[must_use]
    pub fn definition_for_reference(
        &self,
        file: &str,
        byte_offset: u32,
    ) -> Option<&EditorDefinition> {
        self.references.iter().find_map(|reference| {
            (reference.file == file
                && reference.range.start() <= byte_offset
                && byte_offset < reference.range.end())
            .then_some(&reference.definition)
        })
    }

    /// Resolve the reference covering one editor-native UTF-16 position.
    #[must_use]
    pub fn definition_for_reference_at_position(
        &self,
        file: &str,
        position: EditorPosition,
    ) -> Option<&EditorDefinition> {
        let byte_offset = self.document(file)?.byte_offset(position)?;
        self.definition_for_reference(file, byte_offset)
    }

    /// Return compiler-owned declaration detail for a definition or reference
    /// covering one exact UTF-8 byte offset.
    #[must_use]
    pub fn hover(&self, file: &str, byte_offset: u32) -> Option<(&EditorDefinition, &str)> {
        let definition = self
            .definition_for_reference(file, byte_offset)
            .or_else(|| {
                self.definitions.iter().find(|definition| {
                    definition.file == file
                        && definition.name_range.is_some_and(|range| {
                            range.start() <= byte_offset && byte_offset < range.end()
                        })
                })
            })?;
        let document = self.document(definition.file())?;
        let text = document.source.get(
            usize::try_from(definition.range().start()).ok()?
                ..usize::try_from(definition.range().end()).ok()?,
        )?;
        Some((definition, text))
    }

    /// Return compiler-owned declaration detail at one editor-native UTF-16
    /// position.
    #[must_use]
    pub fn hover_at_position(
        &self,
        file: &str,
        position: EditorPosition,
    ) -> Option<(&EditorDefinition, &str)> {
        let byte_offset = self.document(file)?.byte_offset(position)?;
        self.hover(file, byte_offset)
    }
}

fn declaration_name_range(
    tokens: &BTreeMap<String, Vec<Token>>,
    file: &str,
    declaration: TextRange,
    path: &str,
) -> Option<TextRange> {
    let name = path.rsplit('.').next()?;
    tokens.get(file)?.iter().find_map(|token| {
        (token.kind() == TokenKind::Identifier
            && token.text() == name
            && declaration.start() <= token.range().start()
            && token.range().end() <= declaration.end())
        .then_some(token.range())
    })
}

const fn canonical_symbol_kind(kind: CanonicalDeclarationKind) -> Option<EditorSymbolKind> {
    Some(match kind {
        CanonicalDeclarationKind::PropertyContract | CanonicalDeclarationKind::PropertyRelease => {
            EditorSymbolKind::Property
        }
        CanonicalDeclarationKind::MaterialComposition => EditorSymbolKind::Material,
        CanonicalDeclarationKind::PureOperator => EditorSymbolKind::Operator,
        CanonicalDeclarationKind::Connector => EditorSymbolKind::Connector,
        CanonicalDeclarationKind::Component => EditorSymbolKind::Component,
        CanonicalDeclarationKind::Model => EditorSymbolKind::Model,
        _ => return None,
    })
}
