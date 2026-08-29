//! Compiler-owned input boundary for exact, already-resolved package graphs.
//!
//! Package storage, version selection, and digest verification remain outside
//! the compiler. This module accepts only an explicit set of source units and
//! direct dependency aliases. Analysis is a separate phase so callers can
//! compare canonical declarations with a signed or locked package record
//! before hierarchy elaboration creates a graph transaction.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_lang::{Document, SourceAstFactory, VisibilitySyntax, parse};

use crate::CompiledModel;
use crate::diagnostics::stable_sort;
use crate::hierarchy::HierarchyLimits;
use crate::source_identity::LocalSourceIdentity;

const MAX_NAMESPACE_SEGMENTS: usize = 31;
const MAX_NAMESPACE_SEGMENT_BYTES: usize = 4_096;
const MAX_SOURCE_UNITS: usize = 1_000_000;
const MAX_ALIASES: usize = 1_000_000;
const MAX_TOTAL_SOURCE_BYTES: usize = 256 * 1_024 * 1_024;

#[derive(Clone, Copy)]
struct ResolvedHierarchyResourceLimits {
    source_units: usize,
    aliases: usize,
    source_unit_bytes: usize,
    total_source_bytes: usize,
}

fn resolved_hierarchy_limits() -> ResolvedHierarchyResourceLimits {
    ResolvedHierarchyResourceLimits {
        source_units: MAX_SOURCE_UNITS,
        aliases: MAX_ALIASES,
        source_unit_bytes: HierarchyLimits::default().max_source_bytes,
        total_source_bytes: MAX_TOTAL_SOURCE_BYTES,
    }
}

/// Opaque, deterministic identity of one package compilation namespace.
///
/// The compiler deliberately does not interpret package names, versions, or
/// digests. L4 supplies their already-verified canonical identity as bounded
/// segments; the compiler adds its own domain separator before using them for
/// elaboration identities.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilationNamespaceId(Box<[String]>);

impl CompilationNamespaceId {
    /// Construct one nonempty bounded namespace identity.
    ///
    /// # Errors
    /// Returns a diagnostic for an empty namespace, empty segment, embedded
    /// NUL, or a v1 resource-limit violation.
    pub fn new<I, S>(segments: I) -> Result<Self, Diagnostic>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut result = Vec::new();
        for segment in segments {
            if result.len() >= MAX_NAMESPACE_SEGMENTS {
                return Err(resolved_error(format!(
                    "compilation namespace exceeds the {MAX_NAMESPACE_SEGMENTS} segment limit"
                )));
            }
            let segment = segment.into();
            if segment.is_empty()
                || segment.len() > MAX_NAMESPACE_SEGMENT_BYTES
                || segment.contains('\0')
            {
                return Err(resolved_error(format!(
                    "compilation namespace segments must be nonempty NUL-free UTF-8 within {MAX_NAMESPACE_SEGMENT_BYTES} bytes"
                )));
            }
            result
                .try_reserve(1)
                .map_err(|_| resolved_error("cannot reserve compilation namespace"))?;
            result.push(segment);
        }
        if result.is_empty() {
            return Err(resolved_error("compilation namespace must be nonempty"));
        }
        Ok(Self(result.into_boxed_slice()))
    }

    /// Opaque identity segments in canonical order.
    #[must_use]
    pub fn segments(&self) -> &[String] {
        &self.0
    }
}

impl fmt::Display for CompilationNamespaceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, segment) in self.0.iter().enumerate() {
            if index != 0 {
                formatter.write_str("/")?;
            }
            formatter.write_str(segment)?;
        }
        Ok(())
    }
}

/// One exact UTF-8 source unit owned by a resolved package namespace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedSourceUnit {
    namespace: CompilationNamespaceId,
    file: String,
    source: String,
}

impl ResolvedSourceUnit {
    /// Construct a source unit without parsing it prematurely.
    ///
    /// Parsing every unit together is part of [`analyze_resolved_hierarchy`],
    /// which accumulates diagnostics before any root can be elaborated.
    #[must_use]
    pub fn new(
        namespace: CompilationNamespaceId,
        file: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            namespace,
            file: file.into(),
            source: source.into(),
        }
    }

    /// Owning package namespace.
    #[must_use]
    pub const fn namespace(&self) -> &CompilationNamespaceId {
        &self.namespace
    }

    /// Provenance path supplied by the exact source bundle.
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }

    /// Exact decoded UTF-8 source.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }
}

/// One direct source alias from a declaring package to an exact target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedAlias {
    declaring: CompilationNamespaceId,
    alias: String,
    target: CompilationNamespaceId,
}

impl ResolvedAlias {
    /// Construct one direct alias. Alias spelling is validated during joint
    /// analysis so all input errors are reported together.
    #[must_use]
    pub fn new(
        declaring: CompilationNamespaceId,
        alias: impl Into<String>,
        target: CompilationNamespaceId,
    ) -> Self {
        Self {
            declaring,
            alias: alias.into(),
            target,
        }
    }

    /// Package in whose source the alias may be used.
    #[must_use]
    pub const fn declaring(&self) -> &CompilationNamespaceId {
        &self.declaring
    }

    /// Exact source identifier used as the first path segment.
    #[must_use]
    pub fn alias(&self) -> &str {
        &self.alias
    }

    /// Exact target package namespace.
    #[must_use]
    pub const fn target(&self) -> &CompilationNamespaceId {
        &self.target
    }
}

/// Closed input for one exact multi-package hierarchy analysis.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedHierarchyInput {
    root: CompilationNamespaceId,
    units: Vec<ResolvedSourceUnit>,
    aliases: Vec<ResolvedAlias>,
}

impl ResolvedHierarchyInput {
    /// Construct an input graph. Structural validation intentionally belongs
    /// to the joint analysis phase.
    #[must_use]
    pub fn new(
        root: CompilationNamespaceId,
        units: Vec<ResolvedSourceUnit>,
        aliases: Vec<ResolvedAlias>,
    ) -> Self {
        Self {
            root,
            units,
            aliases,
        }
    }

    /// Exact namespace containing the selected executable model.
    #[must_use]
    pub const fn root(&self) -> &CompilationNamespaceId {
        &self.root
    }

    /// Source units in resolver-provided order. Meaning does not depend on
    /// this order.
    #[must_use]
    pub fn units(&self) -> &[ResolvedSourceUnit] {
        &self.units
    }

    /// Direct aliases in resolver-provided order.
    #[must_use]
    pub fn aliases(&self) -> &[ResolvedAlias] {
        &self.aliases
    }
}

/// Validate the allocation footprint of one resolved hierarchy before source
/// strings or alias records are copied into compiler-owned inputs.
///
/// This is the compiler-owned resource authority used by composition layers
/// that borrow source bundles. Passing this preflight does not replace the
/// structural and semantic checks in [`analyze_resolved_hierarchy`].
///
/// # Errors
///
/// Returns a diagnostic when the source-unit count, alias count, one source
/// length, aggregate source length, or a checked counter exceeds the compiler
/// v1 hierarchy bounds.
pub fn preflight_resolved_hierarchy<I>(
    source_byte_lengths: I,
    alias_count: usize,
) -> Result<(), Diagnostic>
where
    I: IntoIterator<Item = usize>,
{
    preflight_resolved_hierarchy_with_limits(
        source_byte_lengths,
        alias_count,
        resolved_hierarchy_limits(),
    )
}

fn preflight_resolved_hierarchy_with_limits<I>(
    source_byte_lengths: I,
    alias_count: usize,
    limits: ResolvedHierarchyResourceLimits,
) -> Result<(), Diagnostic>
where
    I: IntoIterator<Item = usize>,
{
    if alias_count > limits.aliases {
        return Err(resolved_error(format!(
            "resolved hierarchy exceeds the {} direct-alias limit",
            limits.aliases
        )));
    }
    let mut source_units = 0_usize;
    let mut total_source_bytes = 0_usize;
    for source_bytes in source_byte_lengths {
        source_units = source_units
            .checked_add(1)
            .ok_or_else(|| resolved_error("resolved hierarchy source-unit count overflow"))?;
        if source_units > limits.source_units {
            return Err(resolved_error(format!(
                "resolved hierarchy exceeds the {} source-unit limit",
                limits.source_units
            )));
        }
        if source_bytes > limits.source_unit_bytes {
            return Err(resolved_error(format!(
                "source requires {source_bytes} bytes, exceeding the {} byte hierarchy limit",
                limits.source_unit_bytes
            )));
        }
        total_source_bytes = total_source_bytes
            .checked_add(source_bytes)
            .ok_or_else(|| resolved_error("resolved hierarchy source-byte count overflow"))?;
        if total_source_bytes > limits.total_source_bytes {
            return Err(resolved_error(format!(
                "resolved hierarchy exceeds the {} total source-byte limit",
                limits.total_source_bytes
            )));
        }
    }
    Ok(())
}

/// Top-level declaration families currently understood by package lowering.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum CanonicalDeclarationKind {
    /// Bounded content-addressed pure operator.
    PureOperator,
    /// Nominal physical connector family.
    Connector,
    /// Reusable component definition.
    Component,
    /// Package-local executable entry model.
    Model,
}

/// Package visibility after parsing private-by-default source syntax.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CanonicalDeclarationVisibility {
    /// Visible only inside the exact owning package.
    Private,
    /// Available through a declared direct dependency alias.
    Public,
}

impl From<VisibilitySyntax> for CanonicalDeclarationVisibility {
    fn from(value: VisibilitySyntax) -> Self {
        match value {
            VisibilitySyntax::Private => Self::Private,
            VisibilitySyntax::Public => Self::Public,
        }
    }
}

/// File-layout-independent canonical declaration emitted by the compiler.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalDeclarationIdentity {
    namespace: CompilationNamespaceId,
    path: String,
    kind: CanonicalDeclarationKind,
    visibility: CanonicalDeclarationVisibility,
    canonical_form: String,
}

impl CanonicalDeclarationIdentity {
    /// Owning package namespace.
    #[must_use]
    pub const fn namespace(&self) -> &CompilationNamespaceId {
        &self.namespace
    }

    /// Package-relative top-level declaration path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Closed declaration family.
    #[must_use]
    pub const fn kind(&self) -> CanonicalDeclarationKind {
        self.kind
    }

    /// Exact package visibility recognized by the compiler.
    #[must_use]
    pub const fn visibility(&self) -> CanonicalDeclarationVisibility {
        self.visibility
    }

    /// Domain-separated canonical source-declaration identity string.
    #[must_use]
    pub fn canonical_form(&self) -> &str {
        &self.canonical_form
    }
}

#[derive(Clone, Debug)]
pub(crate) struct AnalyzedSourceUnit {
    pub(crate) namespace: CompilationNamespaceId,
    pub(crate) file: String,
    pub(crate) source_bytes: usize,
    pub(crate) document: Document,
}

/// Completely parsed and globally indexed package hierarchy.
///
/// This value is the comparison barrier: callers inspect
/// [`Self::canonical_declarations`], compare package semantic content, and
/// cross [`Self::validate_definitions`] before
/// [`ValidatedResolvedHierarchy::compile_root`] becomes available.
#[derive(Clone, Debug)]
pub struct AnalyzedResolvedHierarchy {
    pub(crate) root: CompilationNamespaceId,
    pub(crate) units: Vec<AnalyzedSourceUnit>,
    pub(crate) aliases: Vec<ResolvedAlias>,
    canonical_declarations: Box<[CanonicalDeclarationIdentity]>,
}

impl AnalyzedResolvedHierarchy {
    /// Exact namespace containing the selected executable Model.
    #[must_use]
    pub const fn root(&self) -> &CompilationNamespaceId {
        &self.root
    }

    /// Compiler-canonical declarations in `(namespace, path, kind)` order.
    #[must_use]
    pub fn canonical_declarations(&self) -> &[CanonicalDeclarationIdentity] {
        &self.canonical_declarations
    }

    /// Validate every distributable definition before selected-root
    /// elaboration.
    ///
    /// # Errors
    /// Returns accumulated definition diagnostics. No Model, graph
    /// Transaction, occurrence identity, or provenance entry is created.
    pub fn validate_definitions(self) -> Result<ValidatedResolvedHierarchy, Vec<Diagnostic>> {
        let checked =
            crate::hierarchy::validate_resolved_definitions(&self, HierarchyLimits::default())?;
        Ok(ValidatedResolvedHierarchy {
            analysis: self,
            checked,
        })
    }
}

/// A resolved hierarchy whose complete distributable definition graph has
/// crossed the compiler-owned static validation barrier.
#[derive(Clone, Debug)]
pub struct ValidatedResolvedHierarchy {
    pub(crate) analysis: AnalyzedResolvedHierarchy,
    pub(crate) checked: crate::hierarchy::CheckedDefinitionGraph,
}

impl ValidatedResolvedHierarchy {
    /// Exact namespace containing the selected executable Model.
    #[must_use]
    pub const fn root(&self) -> &CompilationNamespaceId {
        self.analysis.root()
    }

    /// Compiler-canonical declarations in `(namespace, path, kind)` order.
    #[must_use]
    pub fn canonical_declarations(&self) -> &[CanonicalDeclarationIdentity] {
        self.analysis.canonical_declarations()
    }

    /// Elaborate one package-local Model from the exact root namespace.
    ///
    /// # Errors
    /// Returns accumulated occurrence diagnostics. No partial transaction is
    /// returned.
    pub fn compile_root(&self, model: &str) -> Result<CompiledModel, Vec<Diagnostic>> {
        crate::hierarchy::compile_resolved_hierarchy(
            &self.analysis,
            &self.checked,
            model,
            HierarchyLimits::default(),
        )
    }
}

/// Parse and globally analyze every source unit of an exact resolved graph.
///
/// # Errors
/// Returns all parser and global namespace diagnostics together. Analysis
/// creates no graph transaction and performs no package I/O.
pub fn analyze_resolved_hierarchy(
    input: ResolvedHierarchyInput,
) -> Result<AnalyzedResolvedHierarchy, Vec<Diagnostic>> {
    preflight_resolved_hierarchy(
        input.units.iter().map(|unit| unit.source.len()),
        input.aliases.len(),
    )
    .map_err(|diagnostic| vec![diagnostic])?;

    let limits = HierarchyLimits::default();
    let mut diagnostics = Vec::new();
    let mut units = Vec::new();
    for unit in input.units {
        if unit.file.is_empty() || unit.file.contains('\0') {
            diagnostics.push(resolved_error(
                "resolved source paths must be nonempty and NUL-free",
            ));
            continue;
        }
        let provenance_file = resolved_source_label(&unit.namespace, &unit.file);
        if provenance_file.len() > limits.provenance.max_source_path_bytes {
            diagnostics.push(resolved_error(format!(
                "package-qualified source path requires {} bytes, exceeding the {} byte provenance-path limit",
                provenance_file.len(),
                limits.provenance.max_source_path_bytes
            )));
            continue;
        }
        match parse(&provenance_file, &unit.source).into_document() {
            Ok(document) => units.push(AnalyzedSourceUnit {
                namespace: unit.namespace,
                file: provenance_file,
                source_bytes: unit.source.len(),
                document,
            }),
            Err(mut errors) => diagnostics.append(&mut errors),
        }
    }

    validate_graph_shape(&input.root, &units, &input.aliases, &mut diagnostics);
    if !diagnostics.is_empty() {
        stable_sort(&mut diagnostics);
        return Err(diagnostics);
    }
    let mut analysis = AnalyzedResolvedHierarchy {
        root: input.root,
        units,
        aliases: input.aliases,
        canonical_declarations: Box::new([]),
    };
    crate::hierarchy::validate_resolved_hierarchy(&analysis, limits)?;
    analysis.canonical_declarations =
        collect_canonical_declarations(&analysis.units, &analysis.aliases, &mut diagnostics)
            .into_boxed_slice();
    if diagnostics.is_empty() {
        Ok(analysis)
    } else {
        stable_sort(&mut diagnostics);
        Err(diagnostics)
    }
}

fn validate_graph_shape(
    root: &CompilationNamespaceId,
    units: &[AnalyzedSourceUnit],
    aliases: &[ResolvedAlias],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let namespaces = units
        .iter()
        .map(|unit| unit.namespace.clone())
        .collect::<BTreeSet<_>>();
    if !namespaces.contains(root) {
        diagnostics.push(resolved_error(format!(
            "root compilation namespace `{root}` has no source unit"
        )));
    }

    let mut files = BTreeSet::new();
    for unit in units {
        if !files.insert((unit.namespace.clone(), unit.file.clone())) {
            diagnostics.push(resolved_error(format!(
                "duplicate source unit `{}` in namespace `{}`",
                unit.file, unit.namespace
            )));
        }
    }

    let mut alias_index = BTreeMap::new();
    for alias in aliases {
        if !is_identifier(alias.alias()) {
            diagnostics.push(resolved_error(format!(
                "direct alias `{}` is not an Eqiora identifier",
                alias.alias()
            )));
        }
        if !namespaces.contains(alias.declaring()) {
            diagnostics.push(resolved_error(format!(
                "direct alias `{}` has unknown declaring namespace `{}`",
                alias.alias(),
                alias.declaring()
            )));
        }
        if !namespaces.contains(alias.target()) {
            diagnostics.push(resolved_error(format!(
                "direct alias `{}` has unknown target namespace `{}`",
                alias.alias(),
                alias.target()
            )));
        }
        if alias.declaring() == alias.target() {
            diagnostics.push(resolved_error(format!(
                "direct alias `{}` cannot target its declaring namespace",
                alias.alias()
            )));
        }
        let key = (alias.declaring().clone(), alias.alias().to_owned());
        if alias_index.insert(key, alias.target()).is_some() {
            diagnostics.push(resolved_error(format!(
                "duplicate direct alias `{}` in namespace `{}`",
                alias.alias(),
                alias.declaring()
            )));
        }
    }
}

fn collect_canonical_declarations(
    units: &[AnalyzedSourceUnit],
    aliases: &[ResolvedAlias],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<CanonicalDeclarationIdentity> {
    let mut result = Vec::new();
    let mut paths = BTreeSet::new();
    for unit in units {
        let resolved_aliases = aliases
            .iter()
            .filter(|alias| alias.declaring() == &unit.namespace)
            .map(|alias| {
                (
                    alias.alias().to_owned(),
                    alias.target().segments().to_vec().into_boxed_slice(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        for connector in unit.document.connectors() {
            let document =
                SourceAstFactory::document(vec![connector.clone()], Vec::new(), Vec::new())
                    .expect("one parsed Connector is a valid document");
            push_canonical(
                &mut result,
                &mut paths,
                &unit.namespace,
                connector.name(),
                CanonicalDeclarationKind::Connector,
                connector.visibility(),
                &document,
                &resolved_aliases,
                diagnostics,
            );
        }
        for operator in unit.document.pure_operators() {
            if !paths.insert((unit.namespace.clone(), operator.name().to_owned())) {
                diagnostics.push(resolved_error(format!(
                    "duplicate top-level declaration `{}` in namespace `{}`",
                    operator.name(),
                    unit.namespace
                )));
                continue;
            }
            match crate::pure_operator::compile_definition(&unit.file, operator) {
                Ok(definition) => result.push(CanonicalDeclarationIdentity {
                    namespace: unit.namespace.clone(),
                    path: operator.name().to_owned(),
                    kind: CanonicalDeclarationKind::PureOperator,
                    visibility: operator.visibility().into(),
                    canonical_form: pure_operator_identity_form(definition.digest().bytes()),
                }),
                Err(error) => diagnostics.push(error),
            }
        }
        for component in unit.document.components() {
            let document =
                SourceAstFactory::document(Vec::new(), vec![component.clone()], Vec::new())
                    .expect("one parsed component is a valid document");
            push_canonical(
                &mut result,
                &mut paths,
                &unit.namespace,
                component.name(),
                CanonicalDeclarationKind::Component,
                component.visibility(),
                &document,
                &resolved_aliases,
                diagnostics,
            );
        }
        for model in unit.document.models() {
            let document = SourceAstFactory::document(Vec::new(), Vec::new(), vec![model.clone()])
                .expect("one parsed Model is a valid document");
            push_canonical(
                &mut result,
                &mut paths,
                &unit.namespace,
                model.name(),
                CanonicalDeclarationKind::Model,
                VisibilitySyntax::Private,
                &document,
                &resolved_aliases,
                diagnostics,
            );
        }
    }
    result.sort_by(|left, right| {
        (
            &left.namespace,
            &left.path,
            left.kind,
            visibility_rank(left.visibility),
            &left.canonical_form,
        )
            .cmp(&(
                &right.namespace,
                &right.path,
                right.kind,
                visibility_rank(right.visibility),
                &right.canonical_form,
            ))
    });
    result
}

#[allow(clippy::too_many_arguments)]
fn push_canonical(
    result: &mut Vec<CanonicalDeclarationIdentity>,
    paths: &mut BTreeSet<(CompilationNamespaceId, String)>,
    namespace: &CompilationNamespaceId,
    path: &str,
    kind: CanonicalDeclarationKind,
    visibility: VisibilitySyntax,
    document: &Document,
    resolved_aliases: &BTreeMap<String, Box<[String]>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !paths.insert((namespace.clone(), path.to_owned())) {
        diagnostics.push(resolved_error(format!(
            "duplicate top-level declaration `{path}` in namespace `{namespace}`"
        )));
        return;
    }
    let identity = match LocalSourceIdentity::from_document_with_resolved_aliases(
        document,
        resolved_aliases,
    ) {
        Ok(identity) => identity,
        Err(error) => {
            diagnostics.push(error);
            return;
        }
    };
    result.push(CanonicalDeclarationIdentity {
        namespace: namespace.clone(),
        path: path.to_owned(),
        kind,
        visibility: visibility.into(),
        canonical_form: declaration_identity_form(identity.digest()),
    });
}

fn is_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

const fn visibility_rank(visibility: CanonicalDeclarationVisibility) -> u8 {
    match visibility {
        CanonicalDeclarationVisibility::Private => 0,
        CanonicalDeclarationVisibility::Public => 1,
    }
}

fn declaration_identity_form(digest: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(105);
    result.push_str("eqiora.source-declaration.v1:sha256:");
    for byte in digest {
        result.push(char::from(HEX[usize::from(byte >> 4)]));
        result.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    result
}

fn pure_operator_identity_form(digest: [u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(111);
    result.push_str("eqiora.pure-operator-definition.v1:sha256:");
    for byte in digest {
        result.push(char::from(HEX[usize::from(byte >> 4)]));
        result.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    result
}

fn resolved_source_label(namespace: &CompilationNamespaceId, file: &str) -> String {
    use core::fmt::Write;

    let mut label = String::new();
    write!(label, "eqiora-package-v1:{}:", namespace.segments().len())
        .expect("String writes cannot fail");
    for segment in namespace.segments() {
        write!(label, "{}:{segment}:", segment.len()).expect("String writes cannot fail");
    }
    label.push_str(file);
    label
}

fn resolved_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::LANGUAGE_LOWERING_ERROR, message)
}

#[cfg(test)]
mod tests {
    use eqiora_graph::{GraphStore, InMemoryGraphStore};

    use super::*;

    fn namespace(name: &str) -> CompilationNamespaceId {
        CompilationNamespaceId::new([name, "1.0.0", "semantic-digest"]).expect("namespace")
    }

    fn unit(namespace: &CompilationNamespaceId, file: &str, source: &str) -> ResolvedSourceUnit {
        ResolvedSourceUnit::new(namespace.clone(), file, source)
    }

    fn alias(
        declaring: &CompilationNamespaceId,
        name: &str,
        target: &CompilationNamespaceId,
    ) -> ResolvedAlias {
        ResolvedAlias::new(declaring.clone(), name, target.clone())
    }

    const LIBRARY: &str = r#"
public component Resistor {
  public parameter resistance: 1;
  relation law continuous { resistance - 2 = 0; }
}
"#;

    #[test]
    fn hierarchy_footprint_fails_before_source_input_allocation() {
        let limits = ResolvedHierarchyResourceLimits {
            source_units: 2,
            aliases: 1,
            source_unit_bytes: 8,
            total_source_bytes: 10,
        };
        assert!(
            preflight_resolved_hierarchy_with_limits([1], 2, limits)
                .expect_err("alias overflow")
                .message()
                .contains("direct-alias limit")
        );
        assert!(
            preflight_resolved_hierarchy_with_limits([1, 1, 1], 0, limits)
                .expect_err("source-unit overflow")
                .message()
                .contains("source-unit limit")
        );
        assert!(
            preflight_resolved_hierarchy_with_limits([9], 0, limits)
                .expect_err("per-source overflow")
                .message()
                .contains("byte hierarchy limit")
        );
        assert!(
            preflight_resolved_hierarchy_with_limits([6, 5], 0, limits)
                .expect_err("aggregate overflow")
                .message()
                .contains("total source-byte limit")
        );
        preflight_resolved_hierarchy_with_limits([4, 6], 1, limits).expect("exact footprint limit");
    }

    #[test]
    fn parser_diagnostics_are_independent_of_source_unit_input_order() {
        let root = namespace("root");
        let units = vec![
            unit(&root, "z.eqi", "model Z { relation broken"),
            unit(&root, "a.eqi", "model A { parameter p:"),
        ];
        let forward = analyze_resolved_hierarchy(ResolvedHierarchyInput::new(
            root.clone(),
            units.clone(),
            vec![],
        ))
        .expect_err("both source units are invalid");
        let reverse = analyze_resolved_hierarchy(ResolvedHierarchyInput::new(
            root,
            units.into_iter().rev().collect(),
            vec![],
        ))
        .expect_err("input permutation remains invalid");

        assert_eq!(forward, reverse);
    }

    #[test]
    fn exact_direct_alias_elaborates_with_cross_file_provenance() {
        let root = namespace("org.example.root");
        let electrical = namespace("org.eqiora.electrical");
        let input = ResolvedHierarchyInput::new(
            root.clone(),
            vec![
                unit(
                    &root,
                    "root/main.eqi",
                    "model Main { instance load: electrical.Resistor(resistance = 2); }",
                ),
                unit(&electrical, "electrical/resistor.eqi", LIBRARY),
            ],
            vec![alias(&root, "electrical", &electrical)],
        );

        let analysis = analyze_resolved_hierarchy(input).expect("resolved graph analyzes");
        assert_eq!(analysis.canonical_declarations().len(), 2);
        let compiled = analysis
            .validate_definitions()
            .expect("definitions validate")
            .compile_root("Main")
            .expect("root elaborates");
        assert!(
            compiled.symbols().get("load.resistance").is_none(),
            "literal component arguments do not fabricate Kernel Parameters"
        );
        let law = compiled
            .symbols()
            .get("load.law")
            .expect("imported relation symbol");
        let provenance = compiled.provenance().expect("hierarchy provenance");
        let source = provenance
            .get_by_graph_id(law)
            .expect("relation provenance");
        assert!(
            source
                .definition_span()
                .file
                .ends_with("electrical/resistor.eqi")
        );
        assert!(source.instance_span().file.ends_with("root/main.eqi"));
        assert!(source.binding_spans()[0].file.ends_with("root/main.eqi"));
        assert_ne!(
            source.definition_span().file,
            source.instance_span().file,
            "package-qualified source labels remain unambiguous"
        );

        let (transaction, _, _) = compiled.into_parts();
        InMemoryGraphStore::new()
            .commit(transaction)
            .expect("complete transaction commits");
    }

    #[test]
    fn canonical_declarations_ignore_files_formatting_and_input_order() {
        let package = namespace("org.example.library");
        let split = ResolvedHierarchyInput::new(
            package.clone(),
            vec![
                unit(
                    &package,
                    "z/component.eqi",
                    "public component C { public parameter p: 1 = 2; public parameter q: 1 = 3; }",
                ),
                unit(
                    &package,
                    "a/connector.eqi",
                    "public connector Pin = scalar_physical(across = 1, through = 1);",
                ),
            ],
            vec![],
        );
        let moved = ResolvedHierarchyInput::new(
            package,
            vec![
                unit(
                    &namespace("org.example.library"),
                    "elsewhere/pin.eqi",
                    "// relocated\npublic connector Pin=scalar_physical(across=1,through=1);",
                ),
                unit(
                    &namespace("org.example.library"),
                    "elsewhere/c.eqi",
                    "public component C {\n public parameter q: 1=3;\n public parameter p: 1=2;\n}",
                ),
            ],
            vec![],
        );

        let first = analyze_resolved_hierarchy(split).expect("first analysis");
        let second = analyze_resolved_hierarchy(moved).expect("second analysis");
        assert_eq!(
            first.canonical_declarations(),
            second.canonical_declarations()
        );
        assert_eq!(
            first.canonical_declarations()[0].path(),
            "C",
            "canonical declarations sort by path"
        );
        assert!(
            first.canonical_declarations()[0]
                .canonical_form()
                .starts_with("eqiora.source-declaration.v1:sha256:")
        );
    }

    #[test]
    fn canonical_declarations_normalize_aliases_to_exact_targets() {
        let root = namespace("root");
        let target = namespace("target");
        let renamed = |alias_name: &str| {
            ResolvedHierarchyInput::new(
                root.clone(),
                vec![
                    unit(
                        &root,
                        "root.eqi",
                        &format!("model Main {{ instance c: {alias_name}.Resistor; }}"),
                    ),
                    unit(&target, "target.eqi", LIBRARY),
                ],
                vec![alias(&root, alias_name, &target)],
            )
        };
        let first = analyze_resolved_hierarchy(renamed("electrical")).expect("first alias");
        let second = analyze_resolved_hierarchy(renamed("components")).expect("renamed alias");
        assert_eq!(
            first.canonical_declarations(),
            second.canonical_declarations(),
            "resolution aliases are not package semantics"
        );

        let other_target = namespace("other-target");
        let changed = analyze_resolved_hierarchy(ResolvedHierarchyInput::new(
            root.clone(),
            vec![
                unit(
                    &root,
                    "root.eqi",
                    "model Main { instance c: electrical.Resistor; }",
                ),
                unit(&other_target, "target.eqi", LIBRARY),
            ],
            vec![alias(&root, "electrical", &other_target)],
        ))
        .expect("changed exact target");
        let root_form = |analysis: &AnalyzedResolvedHierarchy| {
            analysis
                .canonical_declarations()
                .iter()
                .find(|declaration| {
                    declaration.namespace() == &root && declaration.path() == "Main"
                })
                .expect("root declaration")
                .canonical_form()
                .to_owned()
        };
        assert_ne!(root_form(&first), root_form(&changed));
    }

    #[test]
    fn pure_operator_declarations_and_calls_are_file_and_alias_invariant() {
        let root = namespace("root");
        let operators = namespace("operators");
        let dependency = r#"
public pure operator outer(left: spatial[1], right: spatial[1]) -> spatial[2]
  = component(left, 0) * component(right, 1);
"#;
        let analyzed = |alias_name: &str, operator_file: &str| {
            analyze_resolved_hierarchy(ResolvedHierarchyInput::new(
                root.clone(),
                vec![
                    unit(
                        &root,
                        "root.eqi",
                        &format!(
                            "model Main {{ domain d = box(0,1,0,1); representation s = continuum; field a on d as s: 1 shape spatial_vector; field b on d as s: 1 shape spatial_vector; relation r continuous on d {{ div(div({alias_name}.outer(a,b))) = 0; }} }}"
                        ),
                    ),
                    unit(&operators, operator_file, dependency),
                ],
                vec![alias(&root, alias_name, &operators)],
            ))
            .expect("resolved pure operator")
        };

        let first = analyzed("ops", "a/operator.eqi");
        let renamed = analyzed("math", "relocated/definition.eqi");
        assert_eq!(
            first.canonical_declarations(),
            renamed.canonical_declarations()
        );
        let operator = first
            .canonical_declarations()
            .iter()
            .find(|declaration| declaration.kind() == CanonicalDeclarationKind::PureOperator)
            .expect("pure declaration");
        assert!(
            operator
                .canonical_form()
                .starts_with("eqiora.pure-operator-definition.v1:sha256:")
        );
    }

    #[test]
    fn private_pure_operator_cannot_cross_an_exact_package_boundary() {
        let root = namespace("root");
        let dependency = namespace("operators");
        let input = ResolvedHierarchyInput::new(
            root.clone(),
            vec![
                unit(
                    &root,
                    "root.eqi",
                    "model Main { domain d = box(0,1); representation s = continuum; field a on d as s: 1 shape spatial_vector; field b on d as s: 1 shape spatial_vector; relation r continuous on d { div(ops.outer(a,b)) = 0; } }",
                ),
                unit(
                    &dependency,
                    "operator.eqi",
                    "private pure operator outer(a: spatial[1], b: spatial[1]) -> spatial[2] = component(a,0) * component(b,1);",
                ),
            ],
            vec![alias(&root, "ops", &dependency)],
        );
        let diagnostics = analyze_resolved_hierarchy(input)
            .expect("global package shape")
            .validate_definitions()
            .expect_err("private exact definitions are not importable");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("private pure operator `ops.outer` cannot be imported")
        }));
    }

    #[test]
    fn private_unknown_and_transitive_imports_fail_during_analysis() {
        let root = namespace("root");
        let dependency = namespace("dependency");
        let cases = [
            (
                "model Main { instance c: dep.Private; }",
                "component Private {}",
                "private component `dep.Private` cannot be imported",
            ),
            (
                "model Main { instance c: missing.C; }",
                "public component C {}",
                "unknown direct package alias `missing`",
            ),
            (
                "model Main { instance c: dep.nested.C; }",
                "public component C {}",
                "uses transitive or member qualification",
            ),
        ];
        for (root_source, dependency_source, expected) in cases {
            let input = ResolvedHierarchyInput::new(
                root.clone(),
                vec![
                    unit(&root, "root.eqi", root_source),
                    unit(&dependency, "dependency.eqi", dependency_source),
                ],
                vec![alias(&root, "dep", &dependency)],
            );
            let diagnostics = analyze_resolved_hierarchy(input).unwrap_err();
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message().contains(expected)),
                "expected `{expected}`, got {diagnostics:#?}"
            );
        }
    }

    #[test]
    fn package_local_names_do_not_collide_but_duplicates_and_aliases_do() {
        let root = namespace("root");
        let first = namespace("first");
        let second = namespace("second");
        let valid = ResolvedHierarchyInput::new(
            root.clone(),
            vec![
                unit(
                    &root,
                    "root.eqi",
                    "model Main { instance a: one.C; instance b: two.C; }",
                ),
                unit(
                    &first,
                    "first.eqi",
                    "public component C { parameter p: 1 = 1; relation law continuous { p - 1 = 0; } }",
                ),
                unit(
                    &second,
                    "second.eqi",
                    "public component C { parameter p: 1 = 2; relation law continuous { p - 2 = 0; } }",
                ),
            ],
            vec![alias(&root, "one", &first), alias(&root, "two", &second)],
        );
        let analysis = analyze_resolved_hierarchy(valid).expect("names are package-local");
        let compiled = analysis
            .validate_definitions()
            .expect("definitions validate")
            .compile_root("Main")
            .expect("both definitions resolve");
        let (transaction, _, _) = compiled.into_parts();
        InMemoryGraphStore::new()
            .commit(transaction)
            .expect("both package-local definitions elaborate atomically");

        let duplicate = ResolvedHierarchyInput::new(
            first.clone(),
            vec![
                unit(&first, "a.eqi", "public component C {}"),
                unit(&first, "b.eqi", "public component C {}"),
            ],
            vec![],
        );
        let diagnostics = analyze_resolved_hierarchy(duplicate).unwrap_err();
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("duplicate component declaration `C`")
        }));

        let duplicate_alias = ResolvedHierarchyInput::new(
            root.clone(),
            vec![
                unit(&root, "root.eqi", "model Main {}"),
                unit(&first, "first.eqi", "public component C {}"),
                unit(&second, "second.eqi", "public component D {}"),
            ],
            vec![alias(&root, "lib", &first), alias(&root, "lib", &second)],
        );
        let diagnostics = analyze_resolved_hierarchy(duplicate_alias).unwrap_err();
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("duplicate direct alias `lib`")
        }));
    }

    #[test]
    fn cross_package_recursion_fails_before_a_transaction_exists() {
        let root = namespace("root");
        let dependency = namespace("dependency");
        let input = ResolvedHierarchyInput::new(
            root.clone(),
            vec![
                unit(
                    &root,
                    "root.eqi",
                    "public component A { instance b: dep.B; } model Main {}",
                ),
                unit(
                    &dependency,
                    "dependency.eqi",
                    "public component B { instance a: app.A; }",
                ),
            ],
            vec![
                alias(&root, "dep", &dependency),
                alias(&dependency, "app", &root),
            ],
        );
        let analysis = analyze_resolved_hierarchy(input).expect("all names resolve");
        let diagnostics = analysis.validate_definitions().unwrap_err();
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("recursive component definition graph")
        }));
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.graph_path().is_none())
        );
    }

    #[test]
    fn unused_connector_contract_fails_definition_validation() {
        let root = namespace("root");
        let input = ResolvedHierarchyInput::new(
            root.clone(),
            vec![unit(
                &root,
                "root.eqi",
                "public connector Broken = scalar_physical(across = mystery, through = A); model Main {}",
            )],
            vec![],
        );
        let analysis = analyze_resolved_hierarchy(input).expect("declarations index");
        let diagnostics = analysis.validate_definitions().unwrap_err();
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("unknown SI base-dimension symbol `mystery`")
        }));
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.graph_path().is_none())
        );
    }

    #[test]
    fn symbolic_component_interfaces_validate_without_occurrence_values() {
        let root = namespace("root");
        let input = ResolvedHierarchyInput::new(
            root.clone(),
            vec![unit(
                &root,
                "root.eqi",
                r#"
public component Leaf {
  public parameter period: s;
  public parameter offset: s = period;
  relation invariant continuous { offset - period = 0; }
}
public component Wrapper {
  public parameter period: s;
  instance leaf: Leaf(period = period);
}
model Empty {}
"#,
            )],
            vec![],
        );
        analyze_resolved_hierarchy(input)
            .expect("declarations analyze")
            .validate_definitions()
            .expect("required public Parameters remain typed free variables");
    }

    #[test]
    fn unused_nested_parameter_contracts_fail_before_root_selection() {
        let root = namespace("root");
        let input = ResolvedHierarchyInput::new(
            root.clone(),
            vec![unit(
                &root,
                "root.eqi",
                r#"
public component Leaf { public parameter period: s; }
public component Missing { instance leaf: Leaf; }
public component WrongDimension {
  public parameter length: m;
  instance leaf: Leaf(period = length);
}
public component InvalidPrivate { parameter hidden: s; }
model Empty {}
"#,
            )],
            vec![],
        );
        let diagnostics = analyze_resolved_hierarchy(input)
            .expect("declarations analyze")
            .validate_definitions()
            .unwrap_err();
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("required Parameter `period` has no instance binding")
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("Parameter binding has dimension")
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("required private Parameter `hidden` has no default")
        }));
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.graph_path().is_none())
        );
    }
}
