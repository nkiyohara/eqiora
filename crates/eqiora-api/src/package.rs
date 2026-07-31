//! Locked model-package compilation at the application boundary.
//!
//! Package storage and exact resolution remain typed L3 contracts. This
//! module is the single L4 composition point that feeds a verified graph into
//! the compiler, compares compiler-owned canonical declarations with every
//! release before elaboration, and admits the resulting transaction through
//! the ordinary [`ModelDocument`](crate::ModelDocument) artifact boundary.

mod model_document;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use eqiora_compiler::{
    AnalyzedResolvedHierarchy, CanonicalDeclarationKind, CanonicalDeclarationVisibility,
    CompilationNamespaceId, ResolvedAlias, ResolvedHierarchyInput, ResolvedSourceUnit,
    analyze_resolved_hierarchy, preflight_resolved_hierarchy,
};
use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_package::{
    AuthorPackageSourcesV1, BundleRoleV1, CanonicalDeclaration, CanonicalModelDigest,
    ContractError, DeclarationKindV1, ExactResolver, ModelPackageIdentityV1, PackageReleaseV1,
    QualifiedName, ResolutionError, ResolutionRecordV1, SemanticContentV1, SemanticDeclarationV1,
    VisibilityV1,
};

pub use model_document::PackagedModelDocument;

const AUTHORING_NAMESPACE_DOMAIN_V1: &str = "eqiora.package-authoring.v1";

/// Failure while deriving one package release from admitted author sources.
#[derive(Debug)]
pub enum PackagePreparationError {
    /// Exact dependency resolution or in-memory replay failed.
    Resolution(ResolutionError),
    /// A closed package contract could not be constructed.
    Contract(ContractError),
    /// Parsing or hierarchy analysis failed without producing a release.
    Diagnostics(Vec<Diagnostic>),
    /// More than one supplied release has the same exact package identity.
    DuplicateDependency(Box<ModelPackageIdentityV1>),
    /// One dependency target required by an author manifest was not supplied.
    MissingDependency {
        /// Package name whose manifest declares the missing target.
        declaring: QualifiedName,
        /// Exact dependency identity which must be supplied.
        target: Box<ModelPackageIdentityV1>,
    },
    /// Compiler-derived declarations differ from one candidate release claim.
    SemanticContentMismatch {
        /// Exact package whose source failed revalidation.
        package: Box<ModelPackageIdentityV1>,
        /// Semantic content carried by the candidate release.
        release: Box<SemanticContentV1>,
        /// Semantic content reconstructed from its exact source.
        compiler: Box<SemanticContentV1>,
    },
}

impl std::fmt::Display for PackagePreparationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resolution(error) => {
                write!(
                    formatter,
                    "exact package preparation replay failed: {error}"
                )
            }
            Self::Contract(error) => write!(formatter, "package source contract failed: {error}"),
            Self::Diagnostics(diagnostics) => write!(
                formatter,
                "package source analysis produced {} diagnostic(s)",
                diagnostics.len()
            ),
            Self::DuplicateDependency(identity) => write!(
                formatter,
                "duplicate exact dependency release `{}@{}`",
                identity.name, identity.version
            ),
            Self::MissingDependency { declaring, target } => write!(
                formatter,
                "package `{declaring}` requires missing exact dependency `{}@{}`",
                target.name, target.version
            ),
            Self::SemanticContentMismatch { package, .. } => write!(
                formatter,
                "compiler semantic content does not match candidate package `{}@{}`",
                package.name, package.version
            ),
        }
    }
}

impl std::error::Error for PackagePreparationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resolution(error) => Some(error),
            Self::Contract(error) => Some(error),
            Self::Diagnostics(_)
            | Self::DuplicateDependency(_)
            | Self::MissingDependency { .. }
            | Self::SemanticContentMismatch { .. } => None,
        }
    }
}

impl From<ContractError> for PackagePreparationError {
    fn from(error: ContractError) -> Self {
        Self::Contract(error)
    }
}

impl From<ResolutionError> for PackagePreparationError {
    fn from(error: ResolutionError) -> Self {
        Self::Resolution(error)
    }
}

impl From<Vec<Diagnostic>> for PackagePreparationError {
    fn from(diagnostics: Vec<Diagnostic>) -> Self {
        Self::Diagnostics(diagnostics)
    }
}

impl From<Diagnostic> for PackagePreparationError {
    fn from(diagnostic: Diagnostic) -> Self {
        Self::Diagnostics(vec![diagnostic])
    }
}

#[derive(Debug)]
enum SemanticContentDerivationError {
    Contract(ContractError),
    Diagnostics(Vec<Diagnostic>),
}

impl From<ContractError> for SemanticContentDerivationError {
    fn from(error: ContractError) -> Self {
        Self::Contract(error)
    }
}

impl From<SemanticContentDerivationError> for PackagePreparationError {
    fn from(error: SemanticContentDerivationError) -> Self {
        match error {
            SemanticContentDerivationError::Contract(error) => Self::Contract(error),
            SemanticContentDerivationError::Diagnostics(diagnostics) => {
                Self::Diagnostics(diagnostics)
            }
        }
    }
}

impl From<SemanticContentDerivationError> for PackageCompilationError {
    fn from(error: SemanticContentDerivationError) -> Self {
        match error {
            SemanticContentDerivationError::Contract(error) => Self::Contract(error),
            SemanticContentDerivationError::Diagnostics(diagnostics) => {
                Self::Diagnostics(diagnostics)
            }
        }
    }
}

/// Derive one exact package release through the compiler-owned semantic path.
///
/// `sources` has already crossed the package crate's bounded inventory and
/// UTF-8 boundary. `dependencies` must contain exactly one release for every
/// exact identity reachable from the author manifest. Their claimed semantic
/// content is reconstructed from source, and no candidate root release is
/// returned until every claim passes exact replay.
///
/// The returned value is the ordinary [`PackageReleaseV1`] contract accepted
/// by package stores and exact resolution. No author-supplied semantic payload
/// enters this operation.
///
/// # Errors
///
/// Returns a typed source-contract, dependency-closure, compiler-diagnostic,
/// or semantic-mismatch error. No partial release is returned.
pub fn prepare_package_release_v1(
    sources: AuthorPackageSourcesV1,
    dependencies: &[PackageReleaseV1],
) -> Result<PackageReleaseV1, PackagePreparationError> {
    ResolutionRecordV1::preflight_exact_release_closure(sources.manifest(), dependencies)?;
    preflight_preparation_hierarchy(&sources, dependencies)?;
    let dependency_inputs = PreparationDependencies::new(dependencies)?;
    let root_namespace = authoring_namespace(sources.manifest())?;
    let namespaces = dependency_inputs.namespaces()?;
    let input =
        preparation_compiler_input(&sources, &root_namespace, &dependency_inputs, &namespaces)?;
    let analyzed = analyze_resolved_hierarchy(input)?;
    let semantic = semantic_content_for_namespace(&analyzed, &root_namespace)?;
    let _validated = analyzed.validate_definitions()?;
    let (manifest, files) = sources.into_parts();
    let release = PackageReleaseV1::new(manifest, semantic, files)?;
    let resolution = ResolutionRecordV1::from_exact_releases(&release, dependencies)?;

    let resolved = ExactResolver.resolve_releases(&resolution, &release, dependencies)?;
    let exact_namespaces =
        compilation_namespaces(&resolved).map_err(map_compilation_preparation)?;
    let exact_input =
        compiler_input(&resolved, &exact_namespaces).map_err(map_compilation_preparation)?;
    let exact_analyzed = analyze_resolved_hierarchy(exact_input)?;
    verify_semantic_content(&resolved, &exact_namespaces, &exact_analyzed)
        .map_err(map_compilation_preparation)?;
    let _validated = exact_analyzed
        .validate_definitions()
        .map_err(PackagePreparationError::Diagnostics)?;
    Ok(release)
}

fn preflight_preparation_hierarchy(
    root: &AuthorPackageSourcesV1,
    dependencies: &[PackageReleaseV1],
) -> Result<(), PackagePreparationError> {
    let alias_count =
        dependencies
            .iter()
            .try_fold(root.manifest().dependencies().len(), |count, release| {
                count
                    .checked_add(release.manifest().dependencies().len())
                    .ok_or_else(|| {
                        PackagePreparationError::Diagnostics(vec![Diagnostic::error(
                            codes::LANGUAGE_LOWERING_ERROR,
                            "package preparation alias count overflow",
                        )])
                    })
            })?;
    let source_lengths = root
        .files()
        .iter()
        .chain(
            dependencies
                .iter()
                .flat_map(|release| release.source().files()),
        )
        .filter(|file| file.role() == BundleRoleV1::ModelSource)
        .map(|file| file.bytes().len());
    preflight_resolved_hierarchy(source_lengths, alias_count)?;
    Ok(())
}

fn map_compilation_preparation(error: PackageCompilationError) -> PackagePreparationError {
    match error {
        PackageCompilationError::Resolution(error) => PackagePreparationError::Resolution(error),
        PackageCompilationError::Contract(error) => PackagePreparationError::Contract(error),
        PackageCompilationError::Diagnostics(diagnostics) => {
            PackagePreparationError::Diagnostics(diagnostics)
        }
        PackageCompilationError::SemanticContentMismatch {
            package,
            release,
            compiler,
        } => PackagePreparationError::SemanticContentMismatch {
            package,
            release,
            compiler,
        },
    }
}

/// Failure from one exact, offline package compilation.
///
/// Resolver, package-contract, and compiler diagnostics remain available in
/// their original typed forms. A semantic mismatch retains both canonical
/// declaration sets rather than reducing the failure to text.
#[derive(Debug)]
pub enum PackageCompilationError {
    /// Exact resolution or store verification failed.
    Resolution(ResolutionError),
    /// A closed package or provenance contract could not be constructed.
    Contract(ContractError),
    /// Parsing, hierarchy analysis, elaboration, or model admission failed.
    Diagnostics(Vec<Diagnostic>),
    /// Compiler-owned declarations differ from the release's claimed meaning.
    SemanticContentMismatch {
        /// Exact package whose release failed verification.
        package: Box<ModelPackageIdentityV1>,
        /// Semantic content carried by the verified release.
        release: Box<SemanticContentV1>,
        /// Semantic content reconstructed from the exact source bundle.
        compiler: Box<SemanticContentV1>,
    },
}

impl std::fmt::Display for PackageCompilationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resolution(error) => {
                write!(formatter, "exact package resolution failed: {error}")
            }
            Self::Contract(error) => write!(formatter, "package contract failed: {error}"),
            Self::Diagnostics(diagnostics) => write!(
                formatter,
                "package compilation produced {} diagnostic(s)",
                diagnostics.len()
            ),
            Self::SemanticContentMismatch { package, .. } => write!(
                formatter,
                "compiler semantic content does not match package `{}@{}`",
                package.name, package.version
            ),
        }
    }
}

impl std::error::Error for PackageCompilationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resolution(error) => Some(error),
            Self::Contract(error) => Some(error),
            Self::Diagnostics(_) | Self::SemanticContentMismatch { .. } => None,
        }
    }
}

impl From<ResolutionError> for PackageCompilationError {
    fn from(error: ResolutionError) -> Self {
        Self::Resolution(error)
    }
}

impl From<ContractError> for PackageCompilationError {
    fn from(error: ContractError) -> Self {
        Self::Contract(error)
    }
}

impl From<Vec<Diagnostic>> for PackageCompilationError {
    fn from(diagnostics: Vec<Diagnostic>) -> Self {
        Self::Diagnostics(diagnostics)
    }
}

impl From<Diagnostic> for PackageCompilationError {
    fn from(diagnostic: Diagnostic) -> Self {
        Self::Diagnostics(vec![diagnostic])
    }
}

/// Failure while constructing or independently replaying package-to-run
/// lineage.
#[derive(Debug)]
pub enum PackageRunBindingError {
    /// A canonical Model or Run artifact identity could not be reconstructed.
    Artifact(Diagnostic),
    /// The closed package lineage or exact resolution contract failed.
    Contract(ContractError),
    /// The admitted Model and its package-compilation sidecar disagree.
    CompilationModelMismatch {
        /// Canonical Model admitted by the application boundary.
        model: CanonicalModelDigest,
        /// Model identity recorded by the package compilation.
        compilation: CanonicalModelDigest,
    },
    /// The Run references a different Model than the package compilation.
    RunModelMismatch {
        /// Model identity recorded by the package compilation.
        compilation: CanonicalModelDigest,
        /// Model identity referenced by the Run manifest.
        run: CanonicalModelDigest,
    },
    /// The Run's semantic revision disagrees with the admitted Model.
    RunRevisionMismatch {
        /// Revision of the admitted canonical Model.
        model: u64,
        /// Revision recorded by the Run manifest.
        run: u64,
    },
}

impl std::fmt::Display for PackageRunBindingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Artifact(error) => write!(formatter, "artifact identity failed: {error}"),
            Self::Contract(error) => write!(formatter, "package run binding failed: {error}"),
            Self::CompilationModelMismatch { model, compilation } => write!(
                formatter,
                "admitted Model `{model}` does not match package compilation `{compilation}`"
            ),
            Self::RunModelMismatch { compilation, run } => write!(
                formatter,
                "Run Model `{run}` does not match package compilation `{compilation}`"
            ),
            Self::RunRevisionMismatch { model, run } => write!(
                formatter,
                "Run semantic revision `{run}` does not match admitted Model revision `{model}`"
            ),
        }
    }
}

impl std::error::Error for PackageRunBindingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Artifact(error) => Some(error),
            Self::Contract(error) => Some(error),
            Self::CompilationModelMismatch { .. }
            | Self::RunModelMismatch { .. }
            | Self::RunRevisionMismatch { .. } => None,
        }
    }
}

impl From<Diagnostic> for PackageRunBindingError {
    fn from(error: Diagnostic) -> Self {
        Self::Artifact(error)
    }
}

impl From<ContractError> for PackageRunBindingError {
    fn from(error: ContractError) -> Self {
        Self::Contract(error)
    }
}

/// Failure while constructing or independently replaying exact package
/// lineage through a typed Realization and Run v2.
#[derive(Debug)]
pub enum PackageExecutionBindingError {
    /// A canonical Model, Realization, or Run artifact could not be validated.
    Artifact(Diagnostic),
    /// The closed package lineage or exact resolution contract failed.
    Contract(ContractError),
    /// The admitted Model and its package-compilation sidecar disagree.
    CompilationModelMismatch {
        /// Canonical Model admitted by the application boundary.
        model: CanonicalModelDigest,
        /// Model identity recorded by the package compilation.
        compilation: CanonicalModelDigest,
    },
    /// The typed Realization references a different Model artifact.
    RealizationModelMismatch {
        /// Model identity recorded by the package compilation.
        compilation: CanonicalModelDigest,
        /// Model identity referenced by the typed Realization.
        realization: CanonicalModelDigest,
    },
    /// The typed Realization references a different semantic revision.
    RealizationRevisionMismatch {
        /// Revision of the admitted canonical Model.
        model: u64,
        /// Revision recorded by the typed Realization.
        realization: u64,
    },
    /// The typed Realization names a different canonical Model ontology.
    RealizationOntologyMismatch,
}

impl std::fmt::Display for PackageExecutionBindingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Artifact(error) => write!(formatter, "artifact identity failed: {error}"),
            Self::Contract(error) => {
                write!(formatter, "package execution binding failed: {error}")
            }
            Self::CompilationModelMismatch { model, compilation } => write!(
                formatter,
                "admitted Model `{model}` does not match package compilation `{compilation}`"
            ),
            Self::RealizationModelMismatch {
                compilation,
                realization,
            } => write!(
                formatter,
                "Realization Model `{realization}` does not match package compilation `{compilation}`"
            ),
            Self::RealizationRevisionMismatch { model, realization } => write!(
                formatter,
                "Realization semantic revision `{realization}` does not match admitted Model revision `{model}`"
            ),
            Self::RealizationOntologyMismatch => formatter
                .write_str("Realization Model ontology does not match the admitted packaged Model"),
        }
    }
}

impl std::error::Error for PackageExecutionBindingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Artifact(error) => Some(error),
            Self::Contract(error) => Some(error),
            Self::CompilationModelMismatch { .. }
            | Self::RealizationModelMismatch { .. }
            | Self::RealizationRevisionMismatch { .. }
            | Self::RealizationOntologyMismatch => None,
        }
    }
}

impl From<Diagnostic> for PackageExecutionBindingError {
    fn from(error: Diagnostic) -> Self {
        Self::Artifact(error)
    }
}

impl From<ContractError> for PackageExecutionBindingError {
    fn from(error: ContractError) -> Self {
        Self::Contract(error)
    }
}

struct PreparationDependencies<'a> {
    releases: BTreeMap<ModelPackageIdentityV1, &'a PackageReleaseV1>,
}

impl<'a> PreparationDependencies<'a> {
    fn new(dependencies: &'a [PackageReleaseV1]) -> Result<Self, PackagePreparationError> {
        let mut releases = BTreeMap::new();
        for release in dependencies {
            let identity = release.package_identity()?;
            if releases.insert(identity.clone(), release).is_some() {
                return Err(PackagePreparationError::DuplicateDependency(Box::new(
                    identity,
                )));
            }
        }
        Ok(Self { releases })
    }

    fn namespaces(
        &self,
    ) -> Result<BTreeMap<ModelPackageIdentityV1, CompilationNamespaceId>, PackagePreparationError>
    {
        self.releases
            .keys()
            .map(|identity| {
                exact_namespace(identity)
                    .map(|namespace| (identity.clone(), namespace))
                    .map_err(Into::into)
            })
            .collect()
    }
}

fn authoring_namespace(
    manifest: &eqiora_package::AuthorManifestV1,
) -> Result<CompilationNamespaceId, Diagnostic> {
    CompilationNamespaceId::new([
        manifest.name().as_str(),
        manifest.version().as_str(),
        AUTHORING_NAMESPACE_DOMAIN_V1,
    ])
}

fn exact_namespace(
    identity: &ModelPackageIdentityV1,
) -> Result<CompilationNamespaceId, Diagnostic> {
    CompilationNamespaceId::new([
        identity.name.as_str(),
        identity.version.as_str(),
        &identity.semantic_digest.to_hex(),
    ])
}

fn preparation_compiler_input(
    root: &AuthorPackageSourcesV1,
    root_namespace: &CompilationNamespaceId,
    dependencies: &PreparationDependencies<'_>,
    namespaces: &BTreeMap<ModelPackageIdentityV1, CompilationNamespaceId>,
) -> Result<ResolvedHierarchyInput, PackagePreparationError> {
    let mut units = Vec::new();
    append_preparation_sources(&mut units, root_namespace, root.files())?;
    for (identity, release) in &dependencies.releases {
        append_preparation_sources(
            &mut units,
            preparation_dependency_namespace(namespaces, &identity.name, identity)?,
            release.source().files(),
        )?;
    }

    let mut aliases = Vec::new();
    for requirement in root.manifest().dependencies() {
        aliases.push(ResolvedAlias::new(
            root_namespace.clone(),
            requirement.alias().as_str(),
            preparation_dependency_namespace(
                namespaces,
                root.manifest().name(),
                requirement.target(),
            )?
            .clone(),
        ));
    }
    for (identity, release) in &dependencies.releases {
        let declaring = preparation_dependency_namespace(namespaces, &identity.name, identity)?;
        for requirement in release.manifest().dependencies() {
            aliases.push(ResolvedAlias::new(
                declaring.clone(),
                requirement.alias().as_str(),
                preparation_dependency_namespace(namespaces, &identity.name, requirement.target())?
                    .clone(),
            ));
        }
    }
    Ok(ResolvedHierarchyInput::new(
        root_namespace.clone(),
        units,
        aliases,
    ))
}

fn append_preparation_sources(
    units: &mut Vec<ResolvedSourceUnit>,
    namespace: &CompilationNamespaceId,
    files: &[eqiora_package::SourceFileV1],
) -> Result<(), PackagePreparationError> {
    for file in files
        .iter()
        .filter(|file| file.role() == BundleRoleV1::ModelSource)
    {
        let source = std::str::from_utf8(file.bytes()).map_err(|error| {
            PackagePreparationError::Diagnostics(vec![Diagnostic::error(
                codes::LANGUAGE_LOWERING_ERROR,
                format!(
                    "admitted model source `{}` is not UTF-8: {error}",
                    file.path()
                ),
            )])
        })?;
        units.push(ResolvedSourceUnit::new(
            namespace.clone(),
            file.path().as_str(),
            source,
        ));
    }
    Ok(())
}

fn preparation_dependency_namespace<'a>(
    namespaces: &'a BTreeMap<ModelPackageIdentityV1, CompilationNamespaceId>,
    declaring: &QualifiedName,
    identity: &ModelPackageIdentityV1,
) -> Result<&'a CompilationNamespaceId, PackagePreparationError> {
    namespaces
        .get(identity)
        .ok_or_else(|| PackagePreparationError::MissingDependency {
            declaring: declaring.clone(),
            target: Box::new(identity.clone()),
        })
}

fn compilation_namespaces(
    resolved: &eqiora_package::ResolvedPackageGraph,
) -> Result<BTreeMap<ModelPackageIdentityV1, CompilationNamespaceId>, PackageCompilationError> {
    resolved
        .packages()
        .map(|(identity, _)| {
            exact_namespace(identity)
                .map(|namespace| (identity.clone(), namespace))
                .map_err(|diagnostic| PackageCompilationError::Diagnostics(vec![diagnostic]))
        })
        .collect()
}

fn compiler_input(
    resolved: &eqiora_package::ResolvedPackageGraph,
    namespaces: &BTreeMap<ModelPackageIdentityV1, CompilationNamespaceId>,
) -> Result<ResolvedHierarchyInput, PackageCompilationError> {
    let mut units = Vec::new();
    for (identity, release) in resolved.packages() {
        let namespace = namespace(namespaces, identity)?;
        for file in release
            .source()
            .files()
            .iter()
            .filter(|file| file.role() == BundleRoleV1::ModelSource)
        {
            let source = std::str::from_utf8(file.bytes()).map_err(|error| {
                PackageCompilationError::Diagnostics(vec![Diagnostic::error(
                    codes::LANGUAGE_LOWERING_ERROR,
                    format!(
                        "verified model source `{}` is not UTF-8: {error}",
                        file.path()
                    ),
                )])
            })?;
            units.push(ResolvedSourceUnit::new(
                namespace.clone(),
                file.path().as_str(),
                source,
            ));
        }
    }

    let aliases = resolved
        .edges()
        .iter()
        .map(|edge| {
            Ok(ResolvedAlias::new(
                namespace(namespaces, edge.declaring())?.clone(),
                edge.alias().as_str(),
                namespace(namespaces, edge.target())?.clone(),
            ))
        })
        .collect::<Result<Vec<_>, PackageCompilationError>>()?;

    Ok(ResolvedHierarchyInput::new(
        namespace(namespaces, resolved.root())?.clone(),
        units,
        aliases,
    ))
}

fn verify_semantic_content(
    resolved: &eqiora_package::ResolvedPackageGraph,
    namespaces: &BTreeMap<ModelPackageIdentityV1, CompilationNamespaceId>,
    analyzed: &AnalyzedResolvedHierarchy,
) -> Result<(), PackageCompilationError> {
    for (identity, release) in resolved.packages() {
        let namespace = namespace(namespaces, identity)?;
        let compiler = semantic_content_for_namespace(analyzed, namespace)?;
        if &compiler != release.semantic() {
            return Err(PackageCompilationError::SemanticContentMismatch {
                package: Box::new(identity.clone()),
                release: Box::new(release.semantic().clone()),
                compiler: Box::new(compiler),
            });
        }
    }
    Ok(())
}

fn semantic_content_for_namespace(
    analyzed: &AnalyzedResolvedHierarchy,
    namespace: &CompilationNamespaceId,
) -> Result<SemanticContentV1, SemanticContentDerivationError> {
    let declarations = analyzed
        .canonical_declarations()
        .iter()
        .filter(|declaration| declaration.namespace() == namespace)
        .map(|declaration| {
            let kind = match declaration.kind() {
                CanonicalDeclarationKind::PureOperator => DeclarationKindV1::PureOperator,
                CanonicalDeclarationKind::Connector => DeclarationKindV1::Connector,
                CanonicalDeclarationKind::Component => DeclarationKindV1::Component,
                CanonicalDeclarationKind::Model => DeclarationKindV1::Model,
                _ => {
                    return Err(SemanticContentDerivationError::Diagnostics(vec![
                        Diagnostic::error(
                            codes::LANGUAGE_LOWERING_ERROR,
                            format!(
                                "compiler declaration `{}` has no package semantic v1 representation",
                                declaration.path()
                            ),
                        ),
                    ]));
                }
            };
            Ok(SemanticDeclarationV1::new(
                QualifiedName::parse(declaration.path())?,
                kind,
                match declaration.visibility() {
                    CanonicalDeclarationVisibility::Private => VisibilityV1::Private,
                    CanonicalDeclarationVisibility::Public => VisibilityV1::Public,
                },
                CanonicalDeclaration::new(declaration.canonical_form())?,
            ))
        })
        .collect::<Result<Vec<_>, SemanticContentDerivationError>>()?;
    SemanticContentV1::new(declarations).map_err(Into::into)
}

fn namespace<'a>(
    namespaces: &'a BTreeMap<ModelPackageIdentityV1, CompilationNamespaceId>,
    identity: &ModelPackageIdentityV1,
) -> Result<&'a CompilationNamespaceId, PackageCompilationError> {
    namespaces.get(identity).ok_or_else(|| {
        PackageCompilationError::Diagnostics(vec![Diagnostic::error(
            codes::LANGUAGE_LOWERING_ERROR,
            format!(
                "resolved package `{}@{}` has no compiler namespace",
                identity.name, identity.version
            ),
        )])
    })
}
