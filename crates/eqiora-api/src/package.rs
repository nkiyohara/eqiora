//! Locked model-package compilation at the application boundary.
//!
//! Package storage and exact resolution remain typed L3 contracts. This
//! module is the single L4 composition point that feeds a verified graph into
//! the compiler, compares compiler-owned canonical declarations with every
//! release before elaboration, and admits the resulting transaction through
//! the ordinary [`ModelDocument`] artifact boundary.

use std::collections::BTreeMap;

use eqiora_artifact::{
    ArtifactDigest, PhysicalExposureCatalogEnvelopeV1, PhysicalExposureObservationBindingV1,
    PhysicalExposureProjectionV1, PhysicalExposureQuantityV1, PhysicalExposureSourceOriginV1,
    PhysicalExposureSourceSpanV1, RealizationEnvelopeV1, RunManifestV1, RunManifestV2,
};
use eqiora_compiler::projection::{PhysicalExposureContract, PhysicalExposureProjectionMap};
use eqiora_compiler::provenance::ProvenanceMap;
use eqiora_compiler::{
    AnalyzedResolvedHierarchy, CanonicalDeclarationKind, CanonicalDeclarationVisibility,
    CompilationNamespaceId, ResolvedAlias, ResolvedHierarchyInput, ResolvedSourceUnit,
    analyze_resolved_hierarchy, preflight_resolved_hierarchy,
};
use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_package::{
    AuthorPackageSourcesV1, BoundRunManifestSchemaV1, BundleRoleV1, CanonicalDeclaration,
    CanonicalModelDigest, CanonicalRealizationDigest, CanonicalRunDigest, CompilationToolchainV1,
    ContractError, DeclarationKindV1, ExactResolver, ExactVersion, ModelPackageIdentityV1,
    PackageCompilationRecordV1, PackageExecutionBindingV1, PackageReleaseV1, PackageRunBindingV1,
    PackageStore, QualifiedName, ResolutionError, ResolutionRecordV1, SemanticContentV1,
    SemanticDeclarationV1, VisibilityV1,
};

use crate::{ExactModelCodec, ModelDocument};

const COMPILER_IDENTITY: &str = "Eqiora.Compiler";
const CONTRACT_VERSION_V1: u32 = 1;
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

/// One admitted canonical model plus exact package compilation provenance.
#[derive(Debug, Clone, PartialEq)]
pub struct PackagedModelDocument {
    model: ModelDocument,
    compilation: PackageCompilationRecordV1,
    provenance: ProvenanceMap,
    physical_exposures: PhysicalExposureProjectionMap,
    physical_exposure_catalog: Option<PhysicalExposureCatalogEnvelopeV1>,
}

impl PackagedModelDocument {
    /// Resolve a locked package graph offline, verify source semantics, and
    /// compile one package-local Model from the exact root.
    ///
    /// Resolution has no discovery, version selection, network, environment,
    /// or fallback path. Every source unit is analyzed and compared with its
    /// release's canonical semantic content before the selected root is
    /// elaborated or any graph transaction is committed.
    ///
    /// # Errors
    /// Returns the original resolver error, package contract error, complete
    /// compiler diagnostic set, or a typed semantic-content mismatch.
    pub fn compile_locked(
        store: &impl PackageStore,
        resolution: &ResolutionRecordV1,
        entry_model: &str,
        exact_codec: ExactModelCodec,
    ) -> Result<Self, PackageCompilationError> {
        let resolved = ExactResolver.resolve(resolution, store)?;
        let namespaces = compilation_namespaces(&resolved)?;
        let input = compiler_input(&resolved, &namespaces)?;
        let analyzed = analyze_resolved_hierarchy(input)?;
        verify_semantic_content(&resolved, &namespaces, &analyzed)?;
        let validated = analyzed.validate_definitions()?;

        let compiled = validated.compile_root(entry_model)?;
        let provenance = compiled.provenance().cloned().ok_or_else(|| {
            PackageCompilationError::Diagnostics(vec![Diagnostic::error(
                codes::LANGUAGE_LOWERING_ERROR,
                "locked package compilation did not produce hierarchy provenance",
            )])
        })?;
        let physical_exposures = compiled.physical_exposures().clone();
        let model = ModelDocument::accept_compiled(compiled, exact_codec)?;
        let model_digest = CanonicalModelDigest::parse(&model.digest()?)?;
        let toolchain = CompilationToolchainV1::new(
            QualifiedName::parse(COMPILER_IDENTITY)?,
            ExactVersion::parse(env!("CARGO_PKG_VERSION"))?,
            CONTRACT_VERSION_V1,
            CONTRACT_VERSION_V1,
            CONTRACT_VERSION_V1,
        );
        let compilation = PackageCompilationRecordV1::new(model_digest, &resolved, toolchain)?;
        let physical_exposure_catalog = (!physical_exposures.is_empty())
            .then(|| {
                seal_physical_exposure_catalog(
                    &model,
                    &compilation,
                    &provenance,
                    &physical_exposures,
                )
            })
            .transpose()?;

        Ok(Self {
            model,
            compilation,
            provenance,
            physical_exposures,
            physical_exposure_catalog,
        })
    }

    /// Canonical model admitted through the ordinary application boundary.
    #[must_use]
    pub const fn model(&self) -> &ModelDocument {
        &self.model
    }

    /// Exact resolution, package inventory, toolchain, and model digest.
    #[must_use]
    pub const fn compilation(&self) -> &PackageCompilationRecordV1 {
        &self.compilation
    }

    /// Package-qualified definition, instance, and binding source locations.
    #[must_use]
    pub const fn provenance(&self) -> &ProvenanceMap {
        &self.provenance
    }

    /// Versioned, content-addressed observation cuts for every ownerless
    /// public physical Port eliminated during exact package compilation.
    #[must_use]
    pub const fn physical_exposure_catalog(&self) -> Option<&PhysicalExposureCatalogEnvelopeV1> {
        self.physical_exposure_catalog.as_ref()
    }

    /// Independently replay a decoded physical exposure catalog against the
    /// exact package compilation and compiler-derived occurrence cuts.
    ///
    /// A flat Kernel graph alone cannot reconstruct which proper subset came
    /// from an eliminated hierarchy occurrence. Replay therefore checks the
    /// resolution and recompilation-owned sidecars, not only graph membership.
    ///
    /// # Errors
    /// Returns a typed resolution/package failure or `EQ0901` diagnostic for
    /// any stale Model, source lineage, occurrence cut, contract, or source
    /// provenance.
    pub fn validate_physical_exposure_catalog(
        &self,
        catalog: &PhysicalExposureCatalogEnvelopeV1,
        resolution: &ResolutionRecordV1,
    ) -> Result<(), PackageCompilationError> {
        self.compilation.validate_against(resolution)?;
        if self.physical_exposures.is_empty() {
            return Err(Diagnostic::error(
                codes::INVALID_ARTIFACT,
                "package compilation has no eliminated physical exposure catalog",
            )
            .into());
        }
        let expected = seal_physical_exposure_catalog(
            &self.model,
            &self.compilation,
            &self.provenance,
            &self.physical_exposures,
        )?;
        catalog.validate_against(
            expected.model_artifact(),
            self.model.program(),
            expected.package_compilation(),
        )?;
        if catalog != &expected {
            return Err(Diagnostic::error(
                codes::INVALID_ARTIFACT,
                "physical exposure catalog differs from exact compiler projection or source provenance",
            )
            .into());
        }
        Ok(())
    }

    /// Bind one cataloged physical exposure quantity to an output of an exact
    /// package-linked Run v1.
    ///
    /// This is a value-free lineage operation. The output artifact must
    /// already be listed by the Run; numerical acceptance and payload typing
    /// remain the responsibility of the producing result adapter.
    ///
    /// # Errors
    /// Returns a typed package/Run linkage or artifact failure for a missing
    /// catalog, projection, exact Model/revision mismatch, or absent output.
    pub fn bind_physical_observation_v1(
        &self,
        projection: ArtifactDigest,
        quantity: PhysicalExposureQuantityV1,
        run: &RunManifestV1,
        result: ArtifactDigest,
    ) -> Result<PhysicalExposureObservationBindingV1, PackageRunBindingError> {
        self.validate_run_v1_model(run)?;
        let catalog = self.physical_exposure_catalog.as_ref().ok_or_else(|| {
            Diagnostic::error(
                codes::INVALID_ARTIFACT,
                "package compilation has no eliminated physical exposure catalog",
            )
        })?;
        PhysicalExposureObservationBindingV1::new_v1(catalog, projection, quantity, run, result)
            .map_err(Into::into)
    }

    /// Independently replay one physical observation binding against the
    /// exact package resolution, sealed catalog, Run, and registered output.
    ///
    /// # Errors
    /// Returns a typed package/Run linkage or artifact failure for any stale
    /// source lineage, catalog, projection, Run, or output identity.
    pub fn validate_physical_observation_v1(
        &self,
        binding: &PhysicalExposureObservationBindingV1,
        run: &RunManifestV1,
        resolution: &ResolutionRecordV1,
    ) -> Result<(), PackageRunBindingError> {
        self.compilation.validate_against(resolution)?;
        self.validate_run_v1_model(run)?;
        let catalog = self.physical_exposure_catalog.as_ref().ok_or_else(|| {
            Diagnostic::error(
                codes::INVALID_ARTIFACT,
                "package compilation has no eliminated physical exposure catalog",
            )
        })?;
        binding
            .validate_against_v1(catalog, run)
            .map_err(Into::into)
    }

    /// Bind one caller-designated v1 Run manifest to this exact package
    /// compilation identity.
    ///
    /// The Run must reference the admitted Model exactly. This constructs a
    /// content-addressed lineage edge; it does not prove that an execution
    /// occurred or that the caller accepted it. Evidence producers should
    /// invoke it only after their independent acceptance checks succeed.
    ///
    /// # Errors
    /// Returns a typed artifact, package-contract, or Model-linkage failure.
    pub fn bind_run_v1(
        &self,
        run: &RunManifestV1,
    ) -> Result<PackageRunBindingV1, PackageRunBindingError> {
        let run_digest = self.validate_run_v1_model(run)?;
        PackageRunBindingV1::new(
            &self.compilation,
            BoundRunManifestSchemaV1::RunManifestV1,
            run_digest,
        )
        .map_err(Into::into)
    }

    /// Independently replay an exact v1 Run lineage edge.
    ///
    /// This revalidates the complete resolution record against the package
    /// compilation, the admitted Model against the compilation, and the Run's
    /// Model and canonical digest against the binding.
    ///
    /// # Errors
    /// Returns a typed artifact, package-contract, or Model-linkage failure.
    pub fn validate_run_v1_binding(
        &self,
        binding: &PackageRunBindingV1,
        run: &RunManifestV1,
        resolution: &ResolutionRecordV1,
    ) -> Result<(), PackageRunBindingError> {
        self.compilation.validate_against(resolution)?;
        let run_digest = self.validate_run_v1_model(run)?;
        binding.validate_against(
            &self.compilation,
            BoundRunManifestSchemaV1::RunManifestV1,
            run_digest,
        )?;
        Ok(())
    }

    /// Bind one typed Realization and its validated Run v2 to this exact
    /// package compilation.
    ///
    /// This method validates the complete Model/revision/Realization/Run chain
    /// before constructing a separate content-addressed lineage edge. The edge
    /// changes none of the linked artifacts and does not independently prove
    /// execution or numerical acceptance.
    ///
    /// # Errors
    /// Returns a typed artifact, package-contract, or exact-linkage failure.
    pub fn bind_execution_v2(
        &self,
        realization: &RealizationEnvelopeV1,
        run: &RunManifestV2,
    ) -> Result<PackageExecutionBindingV1, PackageExecutionBindingError> {
        let identities = self.validate_execution_v2_artifacts(realization, run)?;
        PackageExecutionBindingV1::new(
            &self.compilation,
            identities.semantic_revision,
            identities.realization,
            identities.run,
        )
        .map_err(Into::into)
    }

    /// Independently replay exact package execution lineage against all
    /// concrete artifacts.
    ///
    /// Resolution and compilation inventory are replayed first. The admitted
    /// Model, typed Realization, and Run v2 are then revalidated before their
    /// exact identities are compared with the lineage edge.
    ///
    /// # Errors
    /// Returns a typed artifact, package-contract, or exact-linkage failure.
    pub fn validate_execution_v2_binding(
        &self,
        binding: &PackageExecutionBindingV1,
        realization: &RealizationEnvelopeV1,
        run: &RunManifestV2,
        resolution: &ResolutionRecordV1,
    ) -> Result<(), PackageExecutionBindingError> {
        self.compilation.validate_against(resolution)?;
        let identities = self.validate_execution_v2_artifacts(realization, run)?;
        binding.validate_against(
            &self.compilation,
            identities.semantic_revision,
            identities.realization,
            identities.run,
        )?;
        Ok(())
    }

    /// Transfer the canonical model while deliberately discarding its package
    /// compilation and source-provenance sidecars.
    #[must_use]
    pub fn into_model(self) -> ModelDocument {
        self.model
    }

    fn validate_run_v1_model(
        &self,
        run: &RunManifestV1,
    ) -> Result<CanonicalRunDigest, PackageRunBindingError> {
        let model = CanonicalModelDigest::parse(&self.model.digest()?)?;
        let compilation = self.compilation.model_digest();
        if model != compilation {
            return Err(PackageRunBindingError::CompilationModelMismatch { model, compilation });
        }
        let run_model = CanonicalModelDigest::parse(run.model().as_str())?;
        if run_model != compilation {
            return Err(PackageRunBindingError::RunModelMismatch {
                compilation,
                run: run_model,
            });
        }
        let model_revision = self.model.program().revision().0;
        if run.semantic_revision() != model_revision {
            return Err(PackageRunBindingError::RunRevisionMismatch {
                model: model_revision,
                run: run.semantic_revision(),
            });
        }
        CanonicalRunDigest::parse(run.digest()?.as_str()).map_err(Into::into)
    }

    fn validate_execution_v2_artifacts(
        &self,
        realization: &RealizationEnvelopeV1,
        run: &RunManifestV2,
    ) -> Result<TypedExecutionIdentities, PackageExecutionBindingError> {
        let model = CanonicalModelDigest::parse(&self.model.digest()?)?;
        let compilation = self.compilation.model_digest();
        if model != compilation {
            return Err(PackageExecutionBindingError::CompilationModelMismatch {
                model,
                compilation,
            });
        }

        let realization_model =
            CanonicalModelDigest::parse(&realization.model_artifact().to_string())?;
        if realization_model != compilation {
            return Err(PackageExecutionBindingError::RealizationModelMismatch {
                compilation,
                realization: realization_model,
            });
        }

        let model_revision = self.model.program().revision().0;
        let realization_revision = realization.semantic_revision().get();
        if realization_revision != model_revision {
            return Err(PackageExecutionBindingError::RealizationRevisionMismatch {
                model: model_revision,
                realization: realization_revision,
            });
        }
        if realization.model()? != self.model.program().model() {
            return Err(PackageExecutionBindingError::RealizationOntologyMismatch);
        }

        run.validate_against(realization)?;
        Ok(TypedExecutionIdentities {
            semantic_revision: model_revision,
            realization: CanonicalRealizationDigest::parse(&realization.digest()?.to_string())?,
            run: CanonicalRunDigest::parse(&run.digest()?.to_string())?,
        })
    }
}

fn seal_physical_exposure_catalog(
    model: &ModelDocument,
    compilation: &PackageCompilationRecordV1,
    provenance: &ProvenanceMap,
    projections: &PhysicalExposureProjectionMap,
) -> Result<PhysicalExposureCatalogEnvelopeV1, PackageCompilationError> {
    let entries = projections
        .iter()
        .map(|projection| {
            let origins = provenance.get(projection.exposure()).ok_or_else(|| {
                Diagnostic::error(
                    codes::LANGUAGE_LOWERING_ERROR,
                    "physical exposure projection has no exact source provenance",
                )
            })?;
            let origins = origins
                .origins()
                .iter()
                .map(|origin| {
                    let definition = source_span(origin.definition_span())?;
                    let instance = source_span(origin.instance_span())?;
                    let bindings = origin
                        .binding_spans()
                        .iter()
                        .map(source_span)
                        .collect::<Result<Vec<_>, Diagnostic>>()?;
                    PhysicalExposureSourceOriginV1::new(definition, instance, bindings)
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            let exposure = *projection.exposure().as_bytes();
            let connection = *projection.connection().full_identity().as_bytes();
            let interior = projection
                .interior()
                .iter()
                .map(|port| *port.full_identity().as_bytes())
                .collect();
            match projection.contract() {
                PhysicalExposureContract::ScalarPhysical { connector } => {
                    PhysicalExposureProjectionV1::scalar(
                        projection.selector(),
                        exposure,
                        connection,
                        interior,
                        *connector.full_identity().as_bytes(),
                        origins,
                    )
                }
                PhysicalExposureContract::FieldBoundary {
                    connector,
                    boundary,
                } => PhysicalExposureProjectionV1::field_boundary(
                    projection.selector(),
                    exposure,
                    connection,
                    interior,
                    *connector.full_identity().as_bytes(),
                    *boundary.full_identity().as_bytes(),
                    origins,
                ),
            }
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let model_artifact = ArtifactDigest::from_hex(model.digest()?)?;
    let package_compilation = ArtifactDigest::from_hex(compilation.digest()?.to_hex())
        .map_err(PackageCompilationError::from)?;
    PhysicalExposureCatalogEnvelopeV1::new(
        model_artifact,
        model.program(),
        package_compilation,
        entries,
    )
    .map_err(Into::into)
}

fn source_span(span: &eqiora_core::Span) -> Result<PhysicalExposureSourceSpanV1, Diagnostic> {
    PhysicalExposureSourceSpanV1::new(span.file.clone(), span.start, span.end)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TypedExecutionIdentities {
    semantic_revision: u64,
    realization: CanonicalRealizationDigest,
    run: CanonicalRunDigest,
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

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_artifact::ArtifactDigest;
    use eqiora_package::{
        AuthorManifestV1, BundleEntryV1, DependencyRequirementV1, InMemoryPackageStore,
        NormalizedRelativePath, PackageReleaseV1, SourceFileV1,
    };

    const VERSION: &str = "1.0.0";
    const SOURCE_PATH: &str = "src/package.eqi";

    fn author_sources(
        name: &str,
        source: &str,
        dependencies: &[(&str, &PackageReleaseV1)],
    ) -> AuthorPackageSourcesV1 {
        let mut requirements = Vec::new();
        for (alias, dependency) in dependencies {
            let target = dependency.package_identity().expect("dependency identity");
            requirements.push(
                DependencyRequirementV1::new(QualifiedName::parse(*alias).expect("alias"), target)
                    .expect("dependency requirement"),
            );
        }
        let path = NormalizedRelativePath::parse(SOURCE_PATH).expect("source path");
        let manifest = AuthorManifestV1::new(
            QualifiedName::parse(name).expect("package name"),
            ExactVersion::parse(VERSION).expect("version"),
            requirements,
            vec![BundleEntryV1::new(path.clone(), BundleRoleV1::ModelSource)],
        )
        .expect("manifest");
        AuthorPackageSourcesV1::new(
            manifest,
            vec![SourceFileV1::new(
                path,
                BundleRoleV1::ModelSource,
                source.as_bytes().to_vec(),
            )],
        )
        .expect("admitted author sources")
    }

    fn release(
        name: &str,
        source: &str,
        dependencies: &[(&str, &PackageReleaseV1)],
    ) -> PackageReleaseV1 {
        let sources = author_sources(name, source, dependencies);
        let dependency_releases = dependencies
            .iter()
            .map(|(_, release)| (*release).clone())
            .collect::<Vec<_>>();
        prepare_package_release_v1(sources, &dependency_releases).expect("compiler-derived release")
    }

    #[test]
    fn locked_compilation_binds_exact_graph_model_and_package_provenance() {
        const LIBRARY_SOURCE: &str = r#"
public component Resistor {
  public parameter resistance: 1 = 2;
  relation law continuous { resistance - 2 = 0; }
}
"#;
        const ROOT_SOURCE: &str = r#"
model Main {
  instance load: electrical.Resistor(resistance = 3);
}
"#;

        let library = release("Eqiora.Electrical.Basic", LIBRARY_SOURCE, &[]);
        let root = release(
            "org.example.ParallelDc",
            ROOT_SOURCE,
            &[("electrical", &library)],
        );
        let mut store = InMemoryPackageStore::default();
        let library_source = store.insert(&library).expect("store library");
        let root_source = store.insert(&root).expect("store root");
        let resolution =
            ResolutionRecordV1::from_exact_releases(&root, std::slice::from_ref(&library))
                .expect("derived resolution");
        assert!(
            resolution
                .nodes()
                .iter()
                .any(|node| node.source_digest() == library_source)
        );
        assert!(
            resolution
                .nodes()
                .iter()
                .any(|node| node.source_digest() == root_source)
        );

        let packaged =
            PackagedModelDocument::compile_locked(&store, &resolution, "Main", ExactModelCodec::V1)
                .expect("locked compilation");

        packaged
            .compilation()
            .validate_against(&resolution)
            .expect("compilation binds the resolution");
        assert_eq!(
            packaged.compilation().model_digest(),
            CanonicalModelDigest::parse(&packaged.model().digest().expect("model digest"))
                .expect("canonical digest")
        );
        assert_eq!(
            packaged.model().aliases().get("load.resistance"),
            None,
            "a literal Component binding does not fabricate a Parameter alias"
        );
        let law = packaged.model().aliases()["load.law"];
        let provenance = packaged
            .provenance()
            .get_by_graph_id(law)
            .expect("imported Relation provenance");
        assert!(provenance.definition_span().file.ends_with(SOURCE_PATH));
        assert!(provenance.instance_span().file.ends_with(SOURCE_PATH));
        assert_ne!(
            provenance.definition_span().file,
            provenance.instance_span().file,
            "package-qualified labels disambiguate equal bundle paths"
        );
        assert!(
            provenance
                .definition_span()
                .file
                .contains("Eqiora.Electrical.Basic")
        );
        assert!(
            provenance
                .instance_span()
                .file
                .contains("org.example.ParallelDc")
        );

        let make_run = |executor: &str, topology: &str, reduction: &str| {
            RunManifestV1::new(
                ArtifactDigest::from_hex(packaged.model().digest().expect("model digest"))
                    .expect("artifact digest"),
                packaged.model().program().revision().0,
                executor,
                env!("CARGO_PKG_VERSION"),
            )
            .expect("run manifest")
            .with_numerical_setting("execution.topology", topology)
            .expect("execution topology")
            .with_numerical_setting("solver.method", "reference")
            .expect("solver method")
            .with_numerical_setting("solver.reduction", reduction)
            .expect("solver reduction")
        };
        let run = make_run(
            "eqiora-reference",
            "one-host-process-one-worker",
            "reproducible",
        );
        let binding = packaged.bind_run_v1(&run).expect("package run binding");
        packaged
            .validate_run_v1_binding(&binding, &run, &resolution)
            .expect("exact package run replay");

        let changed_run = make_run("eqiora-reference", "one-host-process-one-worker", "fast");
        assert!(
            packaged
                .validate_run_v1_binding(&binding, &changed_run, &resolution)
                .is_err()
        );

        let changed_backend = make_run(
            "eqiora-other-backend",
            "one-host-process-one-worker",
            "reproducible",
        );
        assert!(
            packaged
                .validate_run_v1_binding(&binding, &changed_backend, &resolution)
                .is_err()
        );

        let changed_topology = make_run(
            "eqiora-reference",
            "one-host-process-two-workers",
            "reproducible",
        );
        assert!(
            packaged
                .validate_run_v1_binding(&binding, &changed_topology, &resolution)
                .is_err()
        );

        let changed_output = run
            .clone()
            .with_output(ArtifactDigest::from_hex("cd".repeat(32)).expect("output digest"));
        assert!(
            packaged
                .validate_run_v1_binding(&binding, &changed_output, &resolution)
                .is_err()
        );

        let wrong_run = RunManifestV1::new(
            ArtifactDigest::from_hex("ab".repeat(32)).expect("different model digest"),
            packaged.model().program().revision().0,
            "eqiora-reference",
            env!("CARGO_PKG_VERSION"),
        )
        .expect("wrong-model run");
        assert!(matches!(
            packaged.bind_run_v1(&wrong_run),
            Err(PackageRunBindingError::RunModelMismatch { .. })
        ));

        let wrong_revision_run = RunManifestV1::new(
            ArtifactDigest::from_hex(packaged.model().digest().expect("model digest"))
                .expect("artifact digest"),
            packaged.model().program().revision().0 + 1,
            "eqiora-reference",
            env!("CARGO_PKG_VERSION"),
        )
        .expect("wrong-revision run");
        assert!(matches!(
            packaged.bind_run_v1(&wrong_revision_run),
            Err(PackageRunBindingError::RunRevisionMismatch { .. })
        ));
    }

    #[test]
    fn preparation_is_order_independent_over_one_transitive_exact_closure() {
        const LEAF: &str = r#"
public component Resistor {
  public parameter resistance: 1 = 2;
  relation law continuous { resistance - 2 = 0; }
}
"#;
        const MIDDLE: &str = r#"
public component Branch {
  instance load: leaf.Resistor(resistance = 3);
}
"#;
        const ROOT: &str = r#"
model Main {
  instance branch: middle.Branch;
}
"#;
        let leaf = release("org.example.Leaf", LEAF, &[]);
        let middle = release("org.example.Middle", MIDDLE, &[("leaf", &leaf)]);
        let sources = author_sources("org.example.Root", ROOT, &[("middle", &middle)]);

        let first = prepare_package_release_v1(sources.clone(), &[leaf.clone(), middle.clone()])
            .expect("forward dependency order");
        let second = prepare_package_release_v1(sources, &[middle.clone(), leaf.clone()])
            .expect("reverse dependency order");
        assert_eq!(first, second);
        assert_eq!(first.canonical_json(), second.canonical_json());
        assert_eq!(first.package_identity(), second.package_identity());

        let resolution =
            ResolutionRecordV1::from_exact_releases(&first, &[middle, leaf]).expect("exact lock");
        assert_eq!(resolution.nodes().len(), 3);
        assert_eq!(resolution.edges().len(), 2);
    }

    #[test]
    fn preparation_rejects_incomplete_duplicate_and_unreachable_inputs() {
        const LIBRARY: &str = r#"
public component Resistor {
  public parameter resistance: 1 = 2;
  relation law continuous { resistance - 2 = 0; }
}
"#;
        const ROOT: &str = r#"
model Main {
  instance load: electrical.Resistor(resistance = 3);
}
"#;
        let library = release("org.example.Library", LIBRARY, &[]);
        let sources = author_sources("org.example.Root", ROOT, &[("electrical", &library)]);
        assert!(matches!(
            prepare_package_release_v1(sources.clone(), &[]),
            Err(PackagePreparationError::MissingDependency { .. })
        ));
        assert!(matches!(
            prepare_package_release_v1(sources, &[library.clone(), library.clone()]),
            Err(PackagePreparationError::DuplicateDependency(_))
        ));

        let independent = author_sources("org.example.Independent", "model Main {}\n", &[]);
        assert!(matches!(
            prepare_package_release_v1(independent, &[library]),
            Err(PackagePreparationError::Contract(_))
        ));
    }

    #[test]
    fn dishonest_dependency_source_fails_before_root_release_is_returned() {
        const LIBRARY_SOURCE: &str = r#"
public component Resistor {
  public parameter resistance: 1 = 2;
  relation law continuous { resistance - 2 = 0; }
}
"#;
        const ROOT_SOURCE: &str = r#"
model Main {
  instance load: electrical.Resistor(resistance = 3);
}
"#;
        let path = NormalizedRelativePath::parse(SOURCE_PATH).expect("source path");
        let manifest = AuthorManifestV1::new(
            QualifiedName::parse("org.example.Dishonest").expect("name"),
            ExactVersion::parse(VERSION).expect("version"),
            vec![],
            vec![BundleEntryV1::new(path.clone(), BundleRoleV1::ModelSource)],
        )
        .expect("manifest");
        let claimed = SemanticContentV1::new(vec![SemanticDeclarationV1::new(
            QualifiedName::parse("Resistor").expect("declaration"),
            DeclarationKindV1::Component,
            VisibilityV1::Public,
            CanonicalDeclaration::new("eqiora.source-declaration.v1:sha256:deadbeef")
                .expect("false canonical claim"),
        )])
        .expect("semantic claim");
        let dishonest = PackageReleaseV1::new(
            manifest,
            claimed.clone(),
            vec![SourceFileV1::new(
                path,
                BundleRoleV1::ModelSource,
                LIBRARY_SOURCE.as_bytes().to_vec(),
            )],
        )
        .expect("locally valid dishonest release");
        let identity = dishonest.package_identity().expect("dishonest identity");
        let sources = author_sources(
            "org.example.Root",
            ROOT_SOURCE,
            &[("electrical", &dishonest)],
        );

        let error = prepare_package_release_v1(sources, &[dishonest])
            .expect_err("dishonest dependency must not produce a root release");
        match error {
            PackagePreparationError::SemanticContentMismatch {
                package,
                release,
                compiler,
            } => {
                assert_eq!(*package, identity);
                assert_eq!(*release, claimed);
                assert_ne!(compiler, release);
            }
            other => panic!("unexpected preparation error: {other:?}"),
        }
    }

    #[test]
    fn semantic_mismatch_fails_before_model_admission() {
        let path = NormalizedRelativePath::parse(SOURCE_PATH).expect("source path");
        let manifest = AuthorManifestV1::new(
            QualifiedName::parse("org.example.FalseClaim").expect("name"),
            ExactVersion::parse(VERSION).expect("version"),
            vec![],
            vec![BundleEntryV1::new(path.clone(), BundleRoleV1::ModelSource)],
        )
        .expect("manifest");
        let claimed = SemanticContentV1::new(vec![SemanticDeclarationV1::new(
            QualifiedName::parse("Main").expect("declaration"),
            DeclarationKindV1::Model,
            VisibilityV1::Private,
            CanonicalDeclaration::new("eqiora.source-declaration.v1:sha256:deadbeef")
                .expect("false canonical claim"),
        )])
        .expect("semantic claim");
        let release = PackageReleaseV1::new(
            manifest,
            claimed.clone(),
            vec![SourceFileV1::new(
                path,
                BundleRoleV1::ModelSource,
                b"model Main {}\n".to_vec(),
            )],
        )
        .expect("release");
        let identity = release.package_identity().expect("identity");
        let mut store = InMemoryPackageStore::default();
        store.insert(&release).expect("store release");
        let resolution =
            ResolutionRecordV1::from_exact_releases(&release, &[]).expect("resolution");

        let error =
            PackagedModelDocument::compile_locked(&store, &resolution, "Main", ExactModelCodec::V1)
                .expect_err("false semantic claim must fail");
        match error {
            PackageCompilationError::SemanticContentMismatch {
                package,
                release,
                compiler,
            } => {
                assert_eq!(*package, identity);
                assert_eq!(*release, claimed);
                assert_ne!(compiler, release);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
