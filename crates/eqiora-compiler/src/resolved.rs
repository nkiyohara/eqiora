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
use eqiora_lang::{Document, SourceAstFactory, TextRange, VisibilitySyntax};

use crate::CompiledModel;
use crate::diagnostics::{source_error, stable_sort};
use crate::hierarchy::HierarchyLimits;
use crate::source_identity::{LocalSourceIdentity, ResolvedAliasTarget};

mod declaration;
mod graph;
mod source;
pub use declaration::{
    CanonicalDeclarationIdentity, CanonicalDeclarationKind, CanonicalDeclarationVisibility,
};

const MAX_NAMESPACE_SEGMENTS: usize = 31;
const MAX_NAMESPACE_SEGMENT_BYTES: usize = 4_096;
const MAX_MODULE_SEGMENTS: usize = 31;
const MAX_MODULE_SEGMENT_BYTES: usize = 512;
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

/// Logical source module name below one compilation/package owner.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ModuleName(Box<[String]>);

impl ModuleName {
    /// Construct one nonempty dotted logical module name.
    ///
    /// # Errors
    /// Rejects invalid Eqiora identifiers and bounded-name violations.
    pub fn new<I, S>(segments: I) -> Result<Self, Diagnostic>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut result = Vec::new();
        for segment in segments {
            if result.len() >= MAX_MODULE_SEGMENTS {
                return Err(resolved_error(format!(
                    "logical module name exceeds the {MAX_MODULE_SEGMENTS} segment limit"
                )));
            }
            let segment = segment.into();
            if segment.len() > MAX_MODULE_SEGMENT_BYTES || !is_identifier(&segment) {
                return Err(resolved_error(format!(
                    "logical module segment `{segment}` must be an Eqiora identifier within {MAX_MODULE_SEGMENT_BYTES} bytes"
                )));
            }
            result.push(segment);
        }
        if result.is_empty() {
            return Err(resolved_error("logical module name must be nonempty"));
        }
        Ok(Self(result.into_boxed_slice()))
    }

    fn main() -> Self {
        Self(vec!["main".to_owned()].into_boxed_slice())
    }

    pub(crate) fn is_main(&self) -> bool {
        self == &Self::main()
    }

    /// Identifier segments in canonical order.
    #[must_use]
    pub fn segments(&self) -> &[String] {
        &self.0
    }
}

impl fmt::Display for ModuleName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, segment) in self.0.iter().enumerate() {
            if index != 0 {
                formatter.write_str(".")?;
            }
            formatter.write_str(segment)?;
        }
        Ok(())
    }
}

/// Exact logical module inside one resolved compilation/package owner.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct CompilationModuleId {
    owner: CompilationNamespaceId,
    name: ModuleName,
}

impl CompilationModuleId {
    /// Bind one logical module to its exact compilation owner.
    #[must_use]
    pub const fn new(owner: CompilationNamespaceId, name: ModuleName) -> Self {
        Self { owner, name }
    }

    fn main(owner: CompilationNamespaceId) -> Self {
        Self::new(owner, ModuleName::main())
    }

    /// Exact package/source compilation owner.
    #[must_use]
    pub const fn owner(&self) -> &CompilationNamespaceId {
        &self.owner
    }

    /// Logical module name.
    #[must_use]
    pub const fn name(&self) -> &ModuleName {
        &self.name
    }
}

impl fmt::Display for CompilationModuleId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.owner.fmt(formatter)?;
        if !self.name.is_main() {
            write!(formatter, "::{}", self.name)?;
        }
        Ok(())
    }
}

/// One exact UTF-8 source unit owned by a resolved package namespace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedSourceUnit {
    module: CompilationModuleId,
    module_from_host: bool,
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
            module: CompilationModuleId::main(namespace),
            module_from_host: false,
            file: file.into(),
            source: source.into(),
        }
    }

    /// Construct one source unit with an explicit logical module identity.
    pub fn in_module<I, S>(
        owner: CompilationNamespaceId,
        module: I,
        file: impl Into<String>,
        source: impl Into<String>,
    ) -> Result<Self, Diagnostic>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Ok(Self {
            module: CompilationModuleId::new(owner, ModuleName::new(module)?),
            module_from_host: true,
            file: file.into(),
            source: source.into(),
        })
    }

    /// Owning package namespace.
    #[must_use]
    pub const fn namespace(&self) -> &CompilationNamespaceId {
        self.module.owner()
    }

    /// Exact logical module owning this source unit.
    #[must_use]
    pub(crate) const fn module(&self) -> &CompilationModuleId {
        &self.module
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
    declaring: CompilationModuleId,
    alias: String,
    target: CompilationModuleId,
    source_import: bool,
    source_span: Option<(String, TextRange)>,
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
            declaring: CompilationModuleId::main(declaring),
            alias: alias.into(),
            target: CompilationModuleId::main(target),
            source_import: false,
            source_span: None,
        }
    }

    fn authored_import(
        declaring: CompilationModuleId,
        alias: impl Into<String>,
        target: CompilationModuleId,
        file: impl Into<String>,
        range: TextRange,
    ) -> Self {
        Self {
            declaring,
            alias: alias.into(),
            target,
            source_import: true,
            source_span: Some((file.into(), range)),
        }
    }

    pub(crate) const fn is_source_import(&self) -> bool {
        self.source_import
    }

    pub(crate) fn source_span(&self) -> Option<(&str, TextRange)> {
        self.source_span
            .as_ref()
            .map(|(file, range)| (file.as_str(), *range))
    }

    /// Package in whose source the alias may be used.
    #[must_use]
    pub const fn declaring(&self) -> &CompilationNamespaceId {
        self.declaring.owner()
    }

    /// Logical module containing the import.
    #[must_use]
    pub(crate) const fn declaring_module(&self) -> &CompilationModuleId {
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
        self.target.owner()
    }

    /// Exact logical module selected by the import.
    #[must_use]
    pub(crate) const fn target_module(&self) -> &CompilationModuleId {
        &self.target
    }
}

/// Closed input for one exact multi-package hierarchy analysis.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedHierarchyInput {
    root: CompilationModuleId,
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
            root: CompilationModuleId::main(root),
            units,
            aliases,
        }
    }

    /// Construct an input graph with an explicit root logical module.
    pub fn with_root_module<I, S>(
        owner: CompilationNamespaceId,
        module: I,
        units: Vec<ResolvedSourceUnit>,
        aliases: Vec<ResolvedAlias>,
    ) -> Result<Self, Diagnostic>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Ok(Self {
            root: CompilationModuleId::new(owner, ModuleName::new(module)?),
            units,
            aliases,
        })
    }

    /// Exact namespace containing the selected executable model.
    #[must_use]
    pub const fn root(&self) -> &CompilationNamespaceId {
        self.root.owner()
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

#[derive(Clone, Debug)]
pub(crate) struct AnalyzedSourceUnit {
    pub(crate) module: CompilationModuleId,
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
    pub(crate) root: CompilationModuleId,
    pub(crate) units: Vec<AnalyzedSourceUnit>,
    pub(crate) aliases: Vec<ResolvedAlias>,
    canonical_declarations: Box<[CanonicalDeclarationIdentity]>,
    property_bindings: Box<[crate::property::ResolvedPropertyBinding]>,
}

impl AnalyzedResolvedHierarchy {
    /// Exact namespace containing the selected executable Model.
    #[must_use]
    pub const fn root(&self) -> &CompilationNamespaceId {
        self.root.owner()
    }

    /// Read-only nominal property bindings retained through elaboration.
    #[must_use]
    pub fn property_bindings(
        &self,
    ) -> impl ExactSizeIterator<Item = (&str, &str, &str, &str, f64, &str, &str, &str)> {
        self.property_bindings.iter().map(|value| {
            (
                value.contract(),
                value.release(),
                value.component(),
                value.requirement(),
                value.normalized_value(),
                value.validity(),
                value.citation(),
                value.license(),
            )
        })
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

    /// Elaborate one root-local or directly imported public Model.
    ///
    /// An imported entry uses exactly `alias.Model`, where `alias` is declared
    /// by the root module or exact root package. Transitive qualification and
    /// private Models fail closed.
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
        match source::analyze_source_unit(unit, limits.provenance.max_source_path_bytes) {
            Ok(unit) => units.push(unit),
            Err(mut errors) => diagnostics.append(&mut errors),
        }
    }

    let authored_imports = units.iter().try_fold(0_usize, |count, unit| {
        count.checked_add(unit.document.imports().len())
    });
    let alias_count = authored_imports.and_then(|count| input.aliases.len().checked_add(count));
    if alias_count.is_none_or(|count| count > MAX_ALIASES) {
        diagnostics.push(resolved_error(format!(
            "resolved hierarchy exceeds the {MAX_ALIASES} direct-alias limit"
        )));
        stable_sort(&mut diagnostics);
        return Err(diagnostics);
    }

    let mut aliases = input.aliases;
    aliases
        .try_reserve(authored_imports.expect("checked authored import count"))
        .map_err(|_| vec![resolved_error("cannot reserve authored module imports")])?;
    for unit in &units {
        for (import_module, import_alias, import_range) in unit.document.imports() {
            let module = match ModuleName::new(import_module.segments()) {
                Ok(module) => module,
                Err(error) => {
                    diagnostics.push(source_error(
                        error.code(),
                        &unit.file,
                        import_range,
                        error.message(),
                    ));
                    continue;
                }
            };
            aliases.push(ResolvedAlias::authored_import(
                unit.module.clone(),
                import_alias,
                CompilationModuleId::new(unit.module.owner().clone(), module),
                &unit.file,
                import_range,
            ));
        }
    }

    graph::validate_graph_shape(&input.root, &units, &aliases, &mut diagnostics);
    if !diagnostics.is_empty() {
        stable_sort(&mut diagnostics);
        return Err(diagnostics);
    }
    let mut analysis = AnalyzedResolvedHierarchy {
        root: input.root,
        units,
        aliases,
        canonical_declarations: Box::new([]),
        property_bindings: Box::new([]),
    };
    let canonical_units = analysis.units.clone();
    for unit in &mut analysis.units {
        if let Err(mut errors) =
            crate::dimensions::elaborate_dimension_aliases_in_place(&unit.file, &mut unit.document)
        {
            diagnostics.append(&mut errors);
        }
    }
    if !diagnostics.is_empty() {
        stable_sort(&mut diagnostics);
        return Err(diagnostics);
    }
    analysis.property_bindings =
        crate::property::validate_and_elaborate(&mut analysis.units, &analysis.aliases)?;
    crate::hierarchy::validate_resolved_hierarchy(&analysis, limits)?;
    analysis.canonical_declarations =
        collect_canonical_declarations(&canonical_units, &analysis.aliases, &mut diagnostics)
            .into_boxed_slice();
    if diagnostics.is_empty() {
        Ok(analysis)
    } else {
        stable_sort(&mut diagnostics);
        Err(diagnostics)
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
            .filter(|alias| alias.declaring_module() == &unit.module)
            .map(|alias| (alias.alias().to_owned(), canonical_alias_target(alias)))
            .collect::<BTreeMap<_, _>>();
        for (name, visibility, is_contract, document) in
            unit.document.isolated_property_declarations()
        {
            push_canonical(
                &mut result,
                &mut paths,
                unit.module.owner(),
                &canonical_declaration_path(&unit.module, &name),
                if is_contract {
                    CanonicalDeclarationKind::PropertyContract
                } else {
                    CanonicalDeclarationKind::PropertyRelease
                },
                visibility,
                &document,
                &resolved_aliases,
                diagnostics,
            );
        }
        for connector in unit.document.connectors() {
            let document =
                SourceAstFactory::document(vec![connector.clone()], Vec::new(), Vec::new())
                    .expect("one parsed Connector is a valid document");
            push_canonical(
                &mut result,
                &mut paths,
                unit.module.owner(),
                &canonical_declaration_path(&unit.module, connector.name()),
                CanonicalDeclarationKind::Connector,
                connector.visibility(),
                &document,
                &resolved_aliases,
                diagnostics,
            );
        }
        for operator in unit.document.pure_operators() {
            let path = canonical_declaration_path(&unit.module, operator.name());
            if !paths.insert((unit.module.owner().clone(), path.clone())) {
                diagnostics.push(resolved_error(format!(
                    "duplicate top-level declaration `{}` in module `{}`",
                    operator.name(),
                    unit.module
                )));
                continue;
            }
            match crate::pure_operator::compile_definition(&unit.file, operator) {
                Ok(definition) => result.push(CanonicalDeclarationIdentity {
                    namespace: unit.module.owner().clone(),
                    path,
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
                unit.module.owner(),
                &canonical_declaration_path(&unit.module, component.name()),
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
                unit.module.owner(),
                &canonical_declaration_path(&unit.module, model.name()),
                CanonicalDeclarationKind::Model,
                model.visibility(),
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

fn canonical_alias_target(alias: &ResolvedAlias) -> ResolvedAliasTarget {
    let target = alias.target_module();
    if alias.is_source_import() {
        ResolvedAliasTarget::local_module(target.name().segments())
    } else {
        ResolvedAliasTarget::external_module(target.owner().segments(), target.name().segments())
    }
}

fn canonical_declaration_path(module: &CompilationModuleId, declaration: &str) -> String {
    if module.name() == &ModuleName::main() {
        declaration.to_owned()
    } else {
        format!("{}.{}", module.name(), declaration)
    }
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
    resolved_aliases: &BTreeMap<String, ResolvedAliasTarget>,
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

fn resolved_source_label(module: &CompilationModuleId, file: &str) -> String {
    use core::fmt::Write;

    let mut label = String::new();
    write!(
        label,
        "eqiora-package-v1:{}:",
        module.owner().segments().len()
    )
    .expect("String writes cannot fail");
    for segment in module.owner().segments() {
        write!(label, "{}:{segment}:", segment.len()).expect("String writes cannot fail");
    }
    if !module.name().is_main() {
        write!(label, "module:{}:", module.name().segments().len())
            .expect("String writes cannot fail");
        for segment in module.name().segments() {
            write!(label, "{}:{segment}:", segment.len()).expect("String writes cannot fail");
        }
    }
    label.push_str(file);
    label
}

fn resolved_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::LANGUAGE_LOWERING_ERROR, message)
}

#[cfg(test)]
mod tests;
