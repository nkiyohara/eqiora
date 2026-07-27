//! Capability-resolved scalar-elliptic application workflow.
//!
//! The values in this module are application contracts, not Studio DTOs. A
//! client proposes one independently revisioned Realization, receives a
//! content-addressed artifact after complete capability validation, and may
//! execute only that exact accepted artifact.
mod error_metric;
use std::num::{NonZeroU16, NonZeroUsize};
use std::time::{Duration, Instant};

use eqiora_artifact::{
    ExecutionProvenanceV1, ExecutionTopologyV1, JsonDecoderLimits, LayoutArtifacts,
    RealizationEnvelopeV1, RunManifestV2,
};
use eqiora_assembly::AssemblyReport;
#[cfg(feature = "rayon")]
use eqiora_backend_rayon::{CpuThreadPool, RAYON_EXECUTION_PROVIDER};
use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, DimExponents, Id};
use eqiora_execution::{
    AdmittedExecution, DeploymentBinding, ExecutionReceipt, HostExecutorDescriptor,
};
use eqiora_meshing::MeshTopology;
#[cfg(feature = "rayon")]
use eqiora_numerics::scalar::finalize_resolved_scalar_elliptic_cartesian_with_assembly;
use eqiora_numerics::{
    common::CartesianMesh, scalar::AcceptedScalarEllipticParameterPoint,
    scalar::FinalizedScalarEllipticCartesianProblem, scalar::FinalizedScalarEllipticParameterPoint,
    scalar::ResolvedScalarEllipticCartesianSolution, scalar::ScalarEllipticCartesianModel,
    scalar::finalize_resolved_scalar_elliptic_cartesian,
    scalar::finalize_scalar_elliptic_parameter_point, scalar::lower_scalar_elliptic_cartesian,
};
use eqiora_realization::{
    Discretization, DiscretizationMethod, ExecutionSchedule, MeshKind, MeshPolicy,
    PortableRealizationGraph, QuadraturePolicy, RealizationCapabilities, RealizationPlan,
    RealizationRequest, RealizationRequirements, RealizationRevision, ResolvedRealization,
    SemanticRevision, SingleFieldOperatorClaim, Space, SpatialDimensionSupport, Target,
    TargetCapabilities, VectorLayoutKind, resolve,
};
use eqiora_schema::kernel::KernelNode;
#[cfg(all(test, feature = "rayon"))]
use eqiora_solver::ExecutionReport;
#[cfg(test)]
use eqiora_solver::ProviderLibrary;
use eqiora_solver::{
    CanonicalCsrSystemView, ExecutionProvider, ExecutionTopology as SolverExecutionTopology,
    LinearOperatorProperties, LinearProblem, LinearSolution, LinearSolver, LinearSolverBackend,
    PreconditionerPolicy, REFERENCE_LINEAR_SOLVER, REFERENCE_SOLVER_PROVIDER, ReductionPolicy,
    SERIAL_EXECUTION_PROVIDER, ScalarType, SolveReport, SolverCapabilities, SolverCapability,
    SolverPlan, SolverProvider,
};

use crate::ModelDocument;

/// Maximum topological cells or reported scalar values admitted before any
/// mesh, assembly, or solver allocation.
pub const MAX_SCALAR_ELLIPTIC_ENTITY_COUNT: usize = 250_000;

/// Numerical family selected by one scalar-elliptic Realization intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarEllipticMethod {
    /// Continuous Q1 Galerkin finite elements on a generated Cartesian mesh.
    FiniteElement,
    /// Cell-centred finite volumes on a generated Cartesian mesh.
    FiniteVolume,
}

impl ScalarEllipticMethod {
    const fn discretization(self) -> DiscretizationMethod {
        match self {
            Self::FiniteElement => DiscretizationMethod::ContinuousGalerkin,
            Self::FiniteVolume => DiscretizationMethod::CellCenteredFiniteVolume,
        }
    }

    const fn space(self) -> Space {
        match self {
            Self::FiniteElement => Space::continuous_lagrange(NonZeroU16::MIN),
            Self::FiniteVolume => Space::cell_constant(),
        }
    }

    const fn quadrature(self) -> QuadraturePolicy {
        match self {
            Self::FiniteElement => QuadraturePolicy::GaussLegendre {
                points_per_axis: NonZeroUsize::new(2).expect("two is non-zero"),
            },
            Self::FiniteVolume => QuadraturePolicy::CellCentroid,
        }
    }
}

/// One proposed explicit Realization revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarEllipticIntent {
    realization_revision: RealizationRevision,
    method: ScalarEllipticMethod,
    cells_per_axis: NonZeroUsize,
    workers: NonZeroUsize,
}

impl ScalarEllipticIntent {
    /// Construct a complete generated-Cartesian scalar-elliptic intent.
    #[must_use]
    pub const fn new(
        realization_revision: RealizationRevision,
        method: ScalarEllipticMethod,
        cells_per_axis: NonZeroUsize,
        workers: NonZeroUsize,
    ) -> Self {
        Self {
            realization_revision,
            method,
            cells_per_axis,
            workers,
        }
    }

    /// Independent Realization revision proposed by the control plane.
    #[must_use]
    pub const fn realization_revision(self) -> RealizationRevision {
        self.realization_revision
    }

    /// Numerical method family.
    #[must_use]
    pub const fn method(self) -> ScalarEllipticMethod {
        self.method
    }

    /// Uniform cell count on every topological axis.
    #[must_use]
    pub const fn cells_per_axis(self) -> NonZeroUsize {
        self.cells_per_axis
    }

    /// Run-owned host worker count.
    #[must_use]
    pub const fn workers(self) -> NonZeroUsize {
        self.workers
    }
}

/// Concrete adapters and a caller-owned host worker budget.
///
/// The budget is deployment policy, not a claim about physical core count or
/// current load. Callers may use `std::thread::available_parallelism()` as a
/// recommendation, but must label that estimate accordingly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarEllipticExecutionEnvironment {
    maximum_workers: NonZeroUsize,
    threaded: bool,
}

impl ScalarEllipticExecutionEnvironment {
    /// The dependency-minimal one-worker reference environment.
    #[must_use]
    pub const fn host_serial() -> Self {
        Self {
            maximum_workers: NonZeroUsize::MIN,
            threaded: false,
        }
    }

    /// A run-owned Rayon environment bounded by explicit caller policy.
    #[cfg(feature = "rayon")]
    #[must_use]
    pub const fn host_threaded(maximum_workers: NonZeroUsize) -> Self {
        Self {
            maximum_workers,
            threaded: true,
        }
    }

    /// Largest worker count admitted by this application environment.
    #[must_use]
    pub const fn maximum_workers(self) -> NonZeroUsize {
        self.maximum_workers
    }

    /// Whether requests above one worker have a concrete execution adapter.
    #[must_use]
    pub const fn threaded(self) -> bool {
        self.threaded
    }

    fn supports(self, workers: NonZeroUsize) -> bool {
        workers <= self.maximum_workers && (workers == NonZeroUsize::MIN || self.threaded)
    }
}

/// Exact content-addressed plan admitted before numerical allocation.
#[derive(Debug, Clone, PartialEq)]
pub struct ScalarEllipticRunPlan {
    model_digest: String,
    intent: ScalarEllipticIntent,
    environment: ScalarEllipticExecutionEnvironment,
    resolved: ResolvedRealization,
    portable: PortableRealizationGraph,
    artifact: RealizationEnvelopeV1,
    key: String,
    cell_count: usize,
    field_value_count: usize,
    field_projection: CartesianScalarFieldProjection,
}

impl ScalarEllipticRunPlan {
    /// Exact Realization artifact digest used as preview-to-run key.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Semantic model artifact digest resolved by this plan.
    #[must_use]
    pub fn model_digest(&self) -> &str {
        &self.model_digest
    }

    /// Original complete control-plane intent.
    #[must_use]
    pub const fn intent(&self) -> ScalarEllipticIntent {
        self.intent
    }

    /// Environment against which the capability decision was made.
    #[must_use]
    pub const fn environment(&self) -> ScalarEllipticExecutionEnvironment {
        self.environment
    }

    /// Model/lowering facts admitted by the Realization resolver.
    #[must_use]
    pub const fn requirements(&self) -> RealizationRequirements {
        self.resolved.requirements()
    }

    /// Validated typed Realization payload.
    #[must_use]
    pub const fn realization(&self) -> &RealizationPlan {
        self.resolved.plan()
    }

    /// Exact backend-neutral execution graph retained from preview.
    #[must_use]
    pub const fn portable_realization(&self) -> &PortableRealizationGraph {
        &self.portable
    }

    /// Versioned content-addressed Realization artifact.
    #[must_use]
    pub const fn artifact(&self) -> &RealizationEnvelopeV1 {
        &self.artifact
    }

    /// Total generated topological cells.
    #[must_use]
    pub const fn cell_count(&self) -> usize {
        self.cell_count
    }

    /// Number of scalar values in the primary admitted result field.
    #[must_use]
    pub const fn field_value_count(&self) -> usize {
        self.field_value_count
    }

    /// Semantic Field, Domain, and Cartesian layout fixed during preview.
    #[must_use]
    pub const fn field_projection(&self) -> &CartesianScalarFieldProjection {
        &self.field_projection
    }

    /// Execution adapter selected by this exact accepted host plan.
    #[must_use]
    pub fn adapter(&self) -> &'static str {
        host_executor(self.environment, self.intent.workers)
            .adapter()
            .as_str()
    }

    /// Implementation version of the selected execution adapter.
    #[must_use]
    pub fn adapter_version(&self) -> &'static str {
        host_executor(self.environment, self.intent.workers)
            .execution_provider()
            .implementation_version()
    }

    /// Decode one persisted Run and validate the complete bounded execution profile.
    ///
    /// Linkage and typed target policy are necessary but not sufficient here: this
    /// application path additionally owns the exact adapter/backend identities and
    /// currently produces no durable output artifacts.
    ///
    /// # Errors
    /// Returns `EQ0901` for malformed bytes, foreign linkage, forged execution
    /// provenance, or output artifacts unsupported by this bounded profile.
    pub fn replay_run_manifest(
        &self,
        bytes: &[u8],
        limits: JsonDecoderLimits,
    ) -> Result<RunManifestV2, Diagnostic> {
        let manifest = RunManifestV2::from_json(bytes, limits)?;
        self.validate_run_manifest(&manifest)?;
        Ok(manifest)
    }

    /// Validate one decoded Run against this exact bounded execution profile.
    ///
    /// # Errors
    /// Returns `EQ0901` for foreign linkage, forged execution provenance, or
    /// output artifacts unsupported by this profile.
    pub fn validate_run_manifest(&self, manifest: &RunManifestV2) -> Result<(), Diagnostic> {
        manifest.validate_against(&self.artifact)?;
        let expected = scalar_elliptic_execution_provenance(self)?;
        if manifest.execution() != expected {
            return Err(run_manifest_error(
                "run execution provenance does not match the accepted scalar-elliptic profile",
            ));
        }
        if !manifest.outputs().is_empty() {
            return Err(run_manifest_error(
                "the bounded scalar-elliptic profile does not produce durable output artifacts",
            ));
        }
        Ok(())
    }
}

/// Fully accepted spatial-application phase exposed to bounded observers.
///
/// These variants are exact facts, not inferred completion percentages. The
/// linear solve remains atomic between [`Self::SystemFinalized`] and
/// [`Self::SolutionAccepted`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarEllipticRunProgress {
    /// Exact plan replay and deployment binding succeeded before numerical
    /// mesh, matrix, pool, or result allocation.
    PlanReplayed,
    /// Method-native assembly produced the finalized canonical system; the
    /// atomic linear solve has not started.
    SystemFinalized,
    /// Solver output, execution receipt, and method-native solution passed
    /// independent acceptance; no public Result or Run manifest exists yet.
    SolutionAccepted,
}

/// Decision returned by a scalar-elliptic observer at an accepted phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarEllipticRunDirective {
    /// Continue execution.
    Continue,
    /// Cancel without publishing a partial or successful Result.
    Cancel,
}

/// Bounded application observer for scalar-elliptic progress and cancellation.
pub trait ScalarEllipticRunObserver {
    /// Inspect one accepted application phase and decide whether execution
    /// continues.
    fn observe(&mut self, progress: ScalarEllipticRunProgress) -> ScalarEllipticRunDirective;
}

/// Evidence that cancellation was observed at one accepted application phase.
#[derive(Debug, Clone, PartialEq)]
pub struct ScalarEllipticRunCancellation {
    plan: ScalarEllipticRunPlan,
    elapsed: Duration,
    progress: ScalarEllipticRunProgress,
}

impl ScalarEllipticRunCancellation {
    /// Exact replayed plan that was cancelled.
    #[must_use]
    pub const fn plan(&self) -> &ScalarEllipticRunPlan {
        &self.plan
    }

    /// Wall time from controlled-call entry through cancellation observation.
    #[must_use]
    pub const fn elapsed(&self) -> Duration {
        self.elapsed
    }

    /// Last fully accepted application phase.
    #[must_use]
    pub const fn progress(&self) -> ScalarEllipticRunProgress {
        self.progress
    }
}

/// Terminal result of one controlled scalar-elliptic application run.
#[derive(Debug, PartialEq)]
pub enum ScalarEllipticRunOutcome {
    /// Complete accepted result and evidence.
    Completed(Box<ScalarEllipticRunResult>),
    /// Accepted cancellation phase; no partial Result is published.
    Cancelled(Box<ScalarEllipticRunCancellation>),
}

#[derive(Debug, Default)]
struct UninterruptedScalarEllipticRun;

impl ScalarEllipticRunObserver for UninterruptedScalarEllipticRun {
    fn observe(&mut self, _progress: ScalarEllipticRunProgress) -> ScalarEllipticRunDirective {
        ScalarEllipticRunDirective::Continue
    }
}

/// Location semantics of values summarized by an accepted spatial result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarFieldLocation {
    /// Continuous finite-element values at canonical mesh vertices.
    Vertex,
    /// Finite-volume algebraic values at canonical cell centres.
    CellCenter,
}

/// Canonical value order of one generated Cartesian Field projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CartesianFieldOrder {
    /// The final logical axis varies fastest in the accepted value array.
    LastAxisFastest,
}

/// Method-neutral semantic layout of the primary scalar Field in one plan.
///
/// This application projection is derived during preview, before mesh or
/// result allocation. It contains no run identity, transport encoding, cache
/// policy, renderer state, or durable result artifact.
#[derive(Debug, Clone, PartialEq)]
pub struct CartesianScalarFieldProjection {
    field: Id<kinds::Field>,
    preferred_alias: Option<String>,
    value_dimension: DimExponents,
    domain: Id<kinds::Domain>,
    spatial_dimension: usize,
    bounds: [[f64; 2]; 3],
    location: ScalarFieldLocation,
    logical_shape: [usize; 3],
    value_count: usize,
}

impl CartesianScalarFieldProjection {
    /// Canonical scalar Field identity.
    #[must_use]
    pub const fn field(&self) -> Id<kinds::Field> {
        self.field
    }

    /// Deterministic non-semantic source alias, when the document retained one.
    #[must_use]
    pub fn preferred_alias(&self) -> Option<&str> {
        self.preferred_alias.as_deref()
    }

    /// Physical dimension of each Field value.
    #[must_use]
    pub const fn value_dimension(&self) -> DimExponents {
        self.value_dimension
    }

    /// Canonical Cartesian volume Domain identity.
    #[must_use]
    pub const fn domain(&self) -> Id<kinds::Domain> {
        self.domain
    }

    /// Number of coherent-SI Cartesian coordinate axes.
    #[must_use]
    pub const fn spatial_dimension(&self) -> usize {
        self.spatial_dimension
    }

    /// Coherent-SI coordinate bounds in canonical axis order.
    #[must_use]
    pub fn bounds(&self) -> &[[f64; 2]] {
        &self.bounds[..self.spatial_dimension]
    }

    /// Vertex or cell-centre association selected by the Realization.
    #[must_use]
    pub const fn location(&self) -> ScalarFieldLocation {
        self.location
    }

    /// Per-axis value extents in canonical Cartesian axis order.
    #[must_use]
    pub fn logical_shape(&self) -> &[usize] {
        &self.logical_shape[..self.spatial_dimension]
    }

    /// Number of values admitted before allocation.
    #[must_use]
    pub const fn value_count(&self) -> usize {
        self.value_count
    }

    /// Canonical flattening order of complete accepted values.
    #[must_use]
    pub const fn order(&self) -> CartesianFieldOrder {
        CartesianFieldOrder::LastAxisFastest
    }

    fn matches_summary(&self, summary: ScalarFieldSummary) -> bool {
        self.location == summary.location()
            && self.spatial_dimension == summary.spatial_dimension()
            && self.logical_shape() == summary.logical_shape()
            && self.value_count == summary.value_count()
    }
}

/// Bounded scalar result summary; complete arrays stay on the data plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScalarFieldSummary {
    location: ScalarFieldLocation,
    spatial_dimension: usize,
    logical_shape: [usize; 3],
    value_count: usize,
    minimum: f64,
    maximum: f64,
}

impl ScalarFieldSummary {
    /// Vertex or cell-centre meaning of the summarized values.
    #[must_use]
    pub const fn location(self) -> ScalarFieldLocation {
        self.location
    }

    /// Number of physical Cartesian axes in this Field layout.
    #[must_use]
    pub const fn spatial_dimension(self) -> usize {
        self.spatial_dimension
    }

    /// Per-axis Field extents in canonical Cartesian axis order.
    #[must_use]
    pub fn logical_shape(&self) -> &[usize] {
        &self.logical_shape[..self.spatial_dimension]
    }

    /// Number of finite scalar values summarized.
    #[must_use]
    pub const fn value_count(self) -> usize {
        self.value_count
    }

    /// Minimum accepted field value.
    #[must_use]
    pub const fn minimum(self) -> f64 {
        self.minimum
    }

    /// Maximum accepted field value.
    #[must_use]
    pub const fn maximum(self) -> f64 {
        self.maximum
    }
}

/// Continuous conservation evidence independent from the linear residual.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScalarEllipticBalanceEvidence {
    boundary_total: f64,
    integrated_source: f64,
    relative_imbalance: f64,
}

impl ScalarEllipticBalanceEvidence {
    /// Recovered outward reaction or flux total.
    #[must_use]
    pub const fn boundary_total(self) -> f64 {
        self.boundary_total
    }

    /// Source integral represented by the discrete load/balance equations.
    #[must_use]
    pub const fn integrated_source(self) -> f64 {
        self.integrated_source
    }

    /// `|boundary + source| / (|boundary| + |source|)`.
    #[must_use]
    pub const fn relative_imbalance(self) -> f64 {
        self.relative_imbalance
    }
}

/// Successful result and exact producer/verifier evidence.
#[derive(Debug, PartialEq)]
pub struct ScalarEllipticRunResult {
    plan: ScalarEllipticRunPlan,
    elapsed: Duration,
    field: ScalarFieldSummary,
    field_values: Vec<f64>,
    balance: ScalarEllipticBalanceEvidence,
    assembly: AssemblyReport,
    run_manifest: RunManifestV2,
    receipt: ExecutionReceipt,
}

impl ScalarEllipticRunResult {
    /// Exact plan replayed immediately before allocation.
    #[must_use]
    pub const fn plan(&self) -> &ScalarEllipticRunPlan {
        &self.plan
    }

    /// Wall duration measured by this local application operation.
    #[must_use]
    pub const fn elapsed(&self) -> Duration {
        self.elapsed
    }

    /// Bounded primary field summary.
    #[must_use]
    pub const fn field(&self) -> ScalarFieldSummary {
        self.field
    }

    /// Complete accepted primary Field values in canonical location order.
    #[must_use]
    pub fn field_values(&self) -> &[f64] {
        &self.field_values
    }

    /// Consume this result into its complete primary Field values.
    #[must_use]
    pub fn into_field_values(self) -> Vec<f64> {
        self.field_values
    }

    /// Recovered continuous conservation evidence.
    #[must_use]
    pub const fn balance(&self) -> ScalarEllipticBalanceEvidence {
        self.balance
    }

    /// Accepted local assembly placement and shape evidence.
    #[must_use]
    pub const fn assembly(&self) -> AssemblyReport {
        self.assembly
    }

    /// Versioned Model, Realization, and actual execution provenance.
    #[must_use]
    pub const fn run_manifest(&self) -> &RunManifestV2 {
        &self.run_manifest
    }

    /// Independently accepted linear solve report.
    #[must_use]
    pub const fn solve(&self) -> &SolveReport {
        self.receipt.report()
    }

    /// Immutable deployment, operator, plan, and execution-DAG evidence.
    #[must_use]
    pub const fn receipt(&self) -> &ExecutionReceipt {
        &self.receipt
    }
}

impl ModelDocument {
    /// Resolve one explicit scalar-elliptic Realization without allocating its
    /// mesh, matrix, worker pool, or result buffers.
    ///
    /// # Errors
    /// Returns one structured lowering, resource, artifact, or capability
    /// diagnostic. Unsupported plans never fall back to a default.
    pub fn preview_scalar_elliptic_run(
        &self,
        intent: ScalarEllipticIntent,
        environment: ScalarEllipticExecutionEnvironment,
    ) -> Result<ScalarEllipticRunPlan, Vec<Diagnostic>> {
        let model_reference = self.artifact_reference().map_err(single)?;
        if !environment.supports(intent.workers) {
            return Err(single(capability_error(format!(
                "host execution admits at most {} worker(s){}; {} were requested",
                environment.maximum_workers,
                if environment.threaded {
                    " through a run-owned threaded adapter"
                } else {
                    " through the serial adapter"
                },
                intent.workers,
            ))));
        }

        let model = lower_scalar_elliptic_cartesian(self.program()).map_err(single)?;
        let dimension = NonZeroUsize::new(model.dimension()).ok_or_else(|| {
            single(capability_error(
                "scalar-elliptic lowering produced a zero spatial dimension",
            ))
        })?;
        let (cell_count, field_value_count) = resource_shape(intent, dimension)?;
        let field_projection =
            scalar_field_projection(self, &model, intent, field_value_count).map_err(single)?;
        let solver = SolverPlan::new(
            LinearSolver::ConjugateGradient,
            1.0e-10,
            1.0e-12,
            NonZeroUsize::new(10_000).expect("10,000 is non-zero"),
        )
        .map_err(single)?;
        let plan = RealizationPlan::new(
            intent.method.space(),
            Discretization::new(
                intent.method.discretization(),
                MeshPolicy::GeneratedUniform {
                    cells_per_axis: intent.cells_per_axis,
                },
                intent.method.quadrature(),
            ),
            solver,
            Target::HostCpu {
                threads: intent.workers,
            },
            ExecutionSchedule::Offline,
        )
        .map_err(single)?;
        let requirements =
            RealizationRequirements::new(dimension, ScalarType::F64, VectorLayoutKind::Replicated);
        let capabilities = scalar_elliptic_capabilities(environment)?;
        let resolved = resolve(
            &RealizationRequest::explicit(
                self.program().model(),
                SemanticRevision::new(self.program().revision().0),
                intent.realization_revision,
                plan,
            ),
            requirements,
            &capabilities,
        )
        .map_err(single)?;
        let portable = resolved
            .portable_graph(SingleFieldOperatorClaim::new(
                model.domain_id(),
                model.field_id(),
                LinearOperatorProperties::SymmetricPositiveDefinite,
            ))
            .map_err(single)?;
        let artifact = RealizationEnvelopeV1::from_resolved(
            &model_reference,
            &resolved,
            LayoutArtifacts::Replicated,
        )
        .map_err(single)?;
        let key = artifact.digest().map_err(single)?.to_string();
        Ok(ScalarEllipticRunPlan {
            model_digest: self.digest().map_err(single)?,
            intent,
            environment,
            resolved,
            portable,
            artifact,
            key,
            cell_count,
            field_value_count,
            field_projection,
        })
    }

    /// Replay and execute one exact capability-admitted scalar-elliptic plan.
    ///
    /// Capability and resource checks run again before the first numerical
    /// allocation. Complete primary Field values cross the data plane only
    /// after numerical and continuous acceptance.
    ///
    /// # Errors
    /// Returns a structured diagnostic when replay, allocation, assembly,
    /// solve, or independent acceptance fails.
    pub fn run_scalar_elliptic_plan(
        &self,
        accepted: ScalarEllipticRunPlan,
        environment: ScalarEllipticExecutionEnvironment,
    ) -> Result<ScalarEllipticRunResult, Vec<Diagnostic>> {
        let mut observer = UninterruptedScalarEllipticRun;
        match self.run_scalar_elliptic_plan_controlled(accepted, environment, &mut observer)? {
            ScalarEllipticRunOutcome::Completed(result) => Ok(*result),
            ScalarEllipticRunOutcome::Cancelled(_) => {
                unreachable!("the uninterrupted observer cannot request cancellation")
            }
        }
    }

    /// Execute one exact plan while observing only fully accepted application
    /// phases.
    ///
    /// Cancellation is a typed terminal outcome. The linear solve is one
    /// atomic interval between `SystemFinalized` and `SolutionAccepted`; a
    /// request made during that interval is observed only after the solution
    /// has passed independent acceptance. No Python or client callback runs
    /// inside assembly or the solver.
    ///
    /// # Errors
    /// Returns the same structured diagnostics as
    /// [`Self::run_scalar_elliptic_plan`].
    pub fn run_scalar_elliptic_plan_controlled(
        &self,
        accepted: ScalarEllipticRunPlan,
        environment: ScalarEllipticExecutionEnvironment,
        observer: &mut impl ScalarEllipticRunObserver,
    ) -> Result<ScalarEllipticRunOutcome, Vec<Diagnostic>> {
        let controlled_started = Instant::now();
        let accepted = match self.execute_scalar_elliptic_plan_controlled(
            accepted,
            environment,
            controlled_started,
            observer,
        )? {
            ControlledScalarEllipticExecution::Accepted(accepted) => *accepted,
            ControlledScalarEllipticExecution::Cancelled(cancellation) => {
                return Ok(ScalarEllipticRunOutcome::Cancelled(cancellation));
            }
        };
        let (field, balance, assembly, solve) = summarize(&accepted.solution)?;
        if !accepted.plan.field_projection.matches_summary(field) {
            return Err(single(capability_error(
                "accepted scalar Field summary differs from its previewed semantic layout",
            )));
        }
        debug_assert_eq!(&solve, accepted.receipt.report());
        let run_manifest = scalar_elliptic_run_manifest(&accepted.plan, &accepted.receipt)?;
        let field_values = accepted.solution.into_primary_field_values();
        debug_assert_eq!(field_values.len(), field.value_count());
        Ok(ScalarEllipticRunOutcome::Completed(Box::new(
            ScalarEllipticRunResult {
                plan: accepted.plan,
                elapsed: accepted.elapsed,
                field,
                field_values,
                balance,
                assembly,
                run_manifest,
                receipt: accepted.receipt,
            },
        )))
    }

    fn execute_scalar_elliptic_plan_controlled(
        &self,
        accepted: ScalarEllipticRunPlan,
        environment: ScalarEllipticExecutionEnvironment,
        controlled_started: Instant,
        observer: &mut impl ScalarEllipticRunObserver,
    ) -> Result<ControlledScalarEllipticExecution, Vec<Diagnostic>> {
        let replayed = self.preview_scalar_elliptic_run(accepted.intent, environment)?;
        if replayed.key != accepted.key
            || replayed.artifact != accepted.artifact
            || replayed.portable != accepted.portable
        {
            return Err(single(capability_error(
                "scalar-elliptic Realization no longer matches its accepted artifact",
            )));
        }

        let binding = DeploymentBinding::bind_host(
            &replayed.portable,
            host_executor(environment, replayed.intent.workers),
        )
        .map_err(single)?;
        if observer.observe(ScalarEllipticRunProgress::PlanReplayed)
            == ScalarEllipticRunDirective::Cancel
        {
            return Ok(ControlledScalarEllipticExecution::Cancelled(Box::new(
                scalar_elliptic_cancellation(
                    replayed,
                    controlled_started,
                    ScalarEllipticRunProgress::PlanReplayed,
                ),
            )));
        }
        let started = Instant::now();
        let workers = replayed.intent.workers;
        let solved = if workers == NonZeroUsize::MIN {
            let (_, finalized) =
                finalize_resolved_scalar_elliptic_cartesian(self.program(), &replayed.resolved)
                    .map_err(single)?;
            solve_finalized_controlled(binding, finalized, &REFERENCE_LINEAR_SOLVER, observer)?
        } else {
            threaded_solve_controlled(self, &replayed, binding, observer)?
        };
        let Some((solution, receipt)) = solved else {
            return Ok(ControlledScalarEllipticExecution::Cancelled(Box::new(
                scalar_elliptic_cancellation(
                    replayed,
                    controlled_started,
                    ScalarEllipticRunProgress::SystemFinalized,
                ),
            )));
        };
        let elapsed = started.elapsed();
        validate_scalar_elliptic_solution(&replayed, &solution, &receipt)?;
        if observer.observe(ScalarEllipticRunProgress::SolutionAccepted)
            == ScalarEllipticRunDirective::Cancel
        {
            return Ok(ControlledScalarEllipticExecution::Cancelled(Box::new(
                scalar_elliptic_cancellation(
                    replayed,
                    controlled_started,
                    ScalarEllipticRunProgress::SolutionAccepted,
                ),
            )));
        }
        Ok(ControlledScalarEllipticExecution::Accepted(Box::new(
            AcceptedScalarEllipticRun {
                plan: replayed,
                elapsed,
                solution,
                receipt,
            },
        )))
    }
}

pub(crate) fn execute_bound_scalar_elliptic_point(
    plan: &ScalarEllipticRunPlan,
    model: ScalarEllipticCartesianModel,
) -> Result<(AcceptedScalarEllipticParameterPoint, ExecutionReceipt), Vec<Diagnostic>> {
    if plan.environment != ScalarEllipticExecutionEnvironment::host_serial()
        || plan.intent.workers != NonZeroUsize::MIN
    {
        return Err(single(capability_error(
            "differentiable Parameter-point execution admits the host-serial adapter only",
        )));
    }
    let binding = DeploymentBinding::bind_host(
        &plan.portable,
        host_executor(plan.environment, plan.intent.workers),
    )
    .map_err(single)?;
    let finalized =
        finalize_scalar_elliptic_parameter_point(model, &plan.resolved).map_err(single)?;
    let mut observer = UninterruptedScalarEllipticRun;
    let Some((solution, receipt)) = solve_finalized_linear_controlled(
        binding,
        &finalized,
        &REFERENCE_LINEAR_SOLVER,
        &mut observer,
    )?
    else {
        unreachable!("the uninterrupted observer cannot request cancellation")
    };
    let solution = finalized.finish(solution).map_err(single)?;
    validate_scalar_elliptic_solution(plan, solution.solution(), &receipt)?;
    Ok((solution, receipt))
}

fn validate_scalar_elliptic_solution(
    plan: &ScalarEllipticRunPlan,
    solution: &ResolvedScalarEllipticCartesianSolution,
    receipt: &ExecutionReceipt,
) -> Result<(), Vec<Diagnostic>> {
    let (solve, value_count) = match solution {
        ResolvedScalarEllipticCartesianSolution::FiniteElement(solution) => (
            solution.solve_report(),
            solution.field().vertex_values().len(),
        ),
        ResolvedScalarEllipticCartesianSolution::FiniteVolume(solution) => {
            (solution.solve_report(), solution.cell_values().len())
        }
    };
    if solve != receipt.report() {
        return Err(single(capability_error(
            "method-native solution report differs from its accepted execution receipt",
        )));
    }
    if value_count != plan.field_value_count {
        return Err(single(capability_error(format!(
            "accepted result contains {value_count} primary field values, but the plan admits {}",
            plan.field_value_count,
        ))));
    }
    Ok(())
}

#[derive(Debug)]
enum ControlledScalarEllipticExecution {
    Accepted(Box<AcceptedScalarEllipticRun>),
    Cancelled(Box<ScalarEllipticRunCancellation>),
}

#[derive(Debug)]
pub(crate) struct AcceptedScalarEllipticRun {
    pub(crate) plan: ScalarEllipticRunPlan,
    pub(crate) elapsed: Duration,
    pub(crate) solution: ResolvedScalarEllipticCartesianSolution,
    pub(crate) receipt: ExecutionReceipt,
}

fn scalar_elliptic_cancellation(
    plan: ScalarEllipticRunPlan,
    started: Instant,
    progress: ScalarEllipticRunProgress,
) -> ScalarEllipticRunCancellation {
    ScalarEllipticRunCancellation {
        plan,
        elapsed: started.elapsed(),
        progress,
    }
}

fn scalar_elliptic_run_manifest(
    plan: &ScalarEllipticRunPlan,
    receipt: &ExecutionReceipt,
) -> Result<RunManifestV2, Vec<Diagnostic>> {
    let report = receipt.report();
    let execution = report.execution();
    let topology = match execution.topology() {
        SolverExecutionTopology::Host { workers } => ExecutionTopologyV1::Host { workers },
        SolverExecutionTopology::Distributed { .. } | SolverExecutionTopology::Cuda { .. } => {
            return Err(single(capability_error(
                "host scalar-elliptic execution produced a non-host topology",
            )));
        }
    };
    let provenance = provider_execution_provenance(
        report.solver_provider(),
        report.execution_provider(),
        report.verification_provider(),
        topology,
        report.reduction(),
    )
    .map_err(single)?;
    let manifest = RunManifestV2::new(&plan.artifact, provenance).map_err(single)?;
    plan.validate_run_manifest(&manifest).map_err(single)?;
    Ok(manifest)
}

fn scalar_elliptic_execution_provenance(
    plan: &ScalarEllipticRunPlan,
) -> Result<ExecutionProvenanceV1, Diagnostic> {
    let workers = plan.intent.workers;
    let executor = host_executor(plan.environment, workers);
    provider_execution_provenance(
        executor.solver_provider(),
        executor.execution_provider(),
        executor.execution_provider(),
        ExecutionTopologyV1::Host { workers },
        plan.resolved.plan().solver().reduction(),
    )
}

fn provider_execution_provenance(
    solver: SolverProvider,
    execution: ExecutionProvider,
    verification: ExecutionProvider,
    topology: ExecutionTopologyV1,
    reduction: ReductionPolicy,
) -> Result<ExecutionProvenanceV1, Diagnostic> {
    verification.validate()?;
    ExecutionProvenanceV1::from_provider_releases(
        solver,
        execution,
        topology,
        reduction,
        verification
            .libraries()
            .iter()
            .map(|library| (library.name(), library.version())),
    )
}

fn scalar_elliptic_capabilities(
    environment: ScalarEllipticExecutionEnvironment,
) -> Result<RealizationCapabilities, Vec<Diagnostic>> {
    let solver = SolverCapabilities::exact([SolverCapability {
        algorithm: LinearSolver::ConjugateGradient,
        operator_properties: LinearOperatorProperties::SymmetricPositiveDefinite,
        preconditioner: PreconditionerPolicy::Identity,
        reduction: ReductionPolicy::Reproducible,
        scalar_type: ScalarType::F64,
    }])
    .map_err(single)?;
    RealizationCapabilities::cartesian_product(
        [
            DiscretizationMethod::ContinuousGalerkin,
            DiscretizationMethod::CellCenteredFiniteVolume,
        ],
        [(
            MeshKind::GeneratedCartesian,
            SpatialDimensionSupport::inclusive(
                NonZeroUsize::MIN,
                NonZeroUsize::new(3).expect("three is non-zero"),
            )
            .map_err(single)?,
        )],
        [VectorLayoutKind::Replicated],
        solver,
        TargetCapabilities::none().with_host_cpu(environment.maximum_workers),
    )
    .map_err(single)
}

fn scalar_field_projection(
    document: &ModelDocument,
    model: &ScalarEllipticCartesianModel,
    intent: ScalarEllipticIntent,
    value_count: usize,
) -> Result<CartesianScalarFieldProjection, Diagnostic> {
    let spatial_dimension = model.dimension();
    if !(1..=3).contains(&spatial_dimension) {
        return Err(capability_error(format!(
            "scalar Field projection does not admit {spatial_dimension} Cartesian dimensions"
        )));
    }
    let field = model.field_id();
    let Some(KernelNode::Field(field_definition)) = document.program().node(field.erase()) else {
        return Err(capability_error(
            "scalar Field projection names a missing canonical Field",
        ));
    };
    let value_dimension = document
        .program()
        .value(field.erase())
        .map(|value| value.dim())
        .unwrap_or_else(|| field_definition.dimension());
    if value_dimension != field_definition.dimension() {
        return Err(capability_error(
            "scalar Field value dimension differs from its canonical definition",
        ));
    }
    let domain = model.domain_id();
    let preferred_alias = document
        .aliases()
        .iter()
        .find_map(|(name, &id)| (id == field.erase()).then(|| name.clone()));
    let mut bounds = [[0.0; 2]; 3];
    bounds[..spatial_dimension].copy_from_slice(model.bounds());
    let location = match intent.method {
        ScalarEllipticMethod::FiniteElement => ScalarFieldLocation::Vertex,
        ScalarEllipticMethod::FiniteVolume => ScalarFieldLocation::CellCenter,
    };
    let axis_extent = match location {
        ScalarFieldLocation::Vertex => intent.cells_per_axis.get().checked_add(1),
        ScalarFieldLocation::CellCenter => Some(intent.cells_per_axis.get()),
    }
    .ok_or_else(|| capability_error("scalar Field projection shape overflowed"))?;
    let mut logical_shape = [1; 3];
    logical_shape[..spatial_dimension].fill(axis_extent);
    let projected_count = logical_shape[..spatial_dimension]
        .iter()
        .try_fold(1_usize, |count, extent| count.checked_mul(*extent))
        .ok_or_else(|| capability_error("scalar Field projection value count overflowed"))?;
    if projected_count != value_count {
        return Err(capability_error(format!(
            "scalar Field projection describes {projected_count} values, but the plan admits {value_count}"
        )));
    }
    Ok(CartesianScalarFieldProjection {
        field,
        preferred_alias,
        value_dimension,
        domain,
        spatial_dimension,
        bounds,
        location,
        logical_shape,
        value_count,
    })
}

fn resource_shape(
    intent: ScalarEllipticIntent,
    dimension: NonZeroUsize,
) -> Result<(usize, usize), Vec<Diagnostic>> {
    let cell_count = checked_power(intent.cells_per_axis.get(), dimension.get(), "cell count")?;
    let field_axis = match intent.method {
        ScalarEllipticMethod::FiniteElement => intent.cells_per_axis.get().checked_add(1),
        ScalarEllipticMethod::FiniteVolume => Some(intent.cells_per_axis.get()),
    }
    .ok_or_else(|| single(capability_error("result field shape overflowed")))?;
    let field_value_count = checked_power(field_axis, dimension.get(), "result field value count")?;
    if cell_count > MAX_SCALAR_ELLIPTIC_ENTITY_COUNT
        || field_value_count > MAX_SCALAR_ELLIPTIC_ENTITY_COUNT
    {
        return Err(single(capability_error(format!(
            "requested mesh would create {cell_count} cells and {field_value_count} primary field values; each is bounded to {MAX_SCALAR_ELLIPTIC_ENTITY_COUNT} before allocation",
        ))));
    }
    Ok((cell_count, field_value_count))
}

fn checked_power(base: usize, exponent: usize, label: &str) -> Result<usize, Vec<Diagnostic>> {
    (0..exponent).try_fold(1usize, |value, _| {
        value
            .checked_mul(base)
            .ok_or_else(|| single(capability_error(format!("{label} overflowed"))))
    })
}

fn host_executor(
    environment: ScalarEllipticExecutionEnvironment,
    workers: NonZeroUsize,
) -> HostExecutorDescriptor {
    let execution_provider = if workers == NonZeroUsize::MIN {
        SERIAL_EXECUTION_PROVIDER
    } else {
        #[cfg(feature = "rayon")]
        {
            RAYON_EXECUTION_PROVIDER
        }
        #[cfg(not(feature = "rayon"))]
        {
            // Preview rejects this branch before deployment binding when the
            // concrete threaded adapter is absent.
            SERIAL_EXECUTION_PROVIDER
        }
    };
    HostExecutorDescriptor::new(
        REFERENCE_SOLVER_PROVIDER,
        execution_provider,
        environment.maximum_workers,
        REFERENCE_LINEAR_SOLVER.capabilities(),
    )
}

fn solve_finalized_controlled(
    binding: DeploymentBinding,
    finalized: FinalizedScalarEllipticCartesianProblem,
    backend: &dyn LinearSolverBackend,
    observer: &mut impl ScalarEllipticRunObserver,
) -> Result<Option<(ResolvedScalarEllipticCartesianSolution, ExecutionReceipt)>, Vec<Diagnostic>> {
    let solved = solve_admitted_linear_controlled(
        binding,
        finalized.portable_realization(),
        finalized.canonical_csr_system_view(),
        finalized.linear_problem().map_err(single)?,
        finalized.solver_plan(),
        backend,
        observer,
    )?;
    let Some((solution, receipt)) = solved else {
        return Ok(None);
    };
    Ok(Some((finalized.finish(solution).map_err(single)?, receipt)))
}

fn solve_finalized_linear_controlled(
    binding: DeploymentBinding,
    finalized: &FinalizedScalarEllipticParameterPoint,
    backend: &dyn LinearSolverBackend,
    observer: &mut impl ScalarEllipticRunObserver,
) -> Result<Option<(LinearSolution, ExecutionReceipt)>, Vec<Diagnostic>> {
    solve_admitted_linear_controlled(
        binding,
        finalized.portable_realization(),
        finalized.canonical_csr_system_view(),
        finalized.linear_problem().map_err(single)?,
        finalized.solver_plan(),
        backend,
        observer,
    )
}

fn solve_admitted_linear_controlled(
    binding: DeploymentBinding,
    portable: &PortableRealizationGraph,
    system: &CanonicalCsrSystemView,
    problem: LinearProblem<'_>,
    solver_plan: SolverPlan,
    backend: &dyn LinearSolverBackend,
    observer: &mut impl ScalarEllipticRunObserver,
) -> Result<Option<(LinearSolution, ExecutionReceipt)>, Vec<Diagnostic>> {
    if observer.observe(ScalarEllipticRunProgress::SystemFinalized)
        == ScalarEllipticRunDirective::Cancel
    {
        return Ok(None);
    }
    let admitted =
        AdmittedExecution::admit_host_linear(portable, system, binding).map_err(single)?;
    let produced = backend.solve(&problem, solver_plan).map_err(single)?;
    let accepted = admitted.accept(produced).map_err(single)?;
    let (solution, receipt) = accepted.into_parts();
    Ok(Some((solution, receipt)))
}

#[cfg(feature = "rayon")]
fn threaded_solve_controlled(
    document: &ModelDocument,
    plan: &ScalarEllipticRunPlan,
    binding: DeploymentBinding,
    observer: &mut impl ScalarEllipticRunObserver,
) -> Result<Option<(ResolvedScalarEllipticCartesianSolution, ExecutionReceipt)>, Vec<Diagnostic>> {
    let pool = CpuThreadPool::from_deployment(&binding).map_err(single)?;
    let solver = pool
        .solver(plan.resolved.plan().target(), &REFERENCE_LINEAR_SOLVER)
        .map_err(single)?;
    let assembly = pool
        .assembler(plan.resolved.plan().target())
        .map_err(single)?;
    let (_, finalized) = finalize_resolved_scalar_elliptic_cartesian_with_assembly(
        document.program(),
        &plan.resolved,
        &assembly,
    )
    .map_err(single)?;
    solve_finalized_controlled(binding, finalized, &solver, observer)
}

#[cfg(not(feature = "rayon"))]
fn threaded_solve_controlled(
    _document: &ModelDocument,
    _plan: &ScalarEllipticRunPlan,
    _binding: DeploymentBinding,
    _observer: &mut impl ScalarEllipticRunObserver,
) -> Result<Option<(ResolvedScalarEllipticCartesianSolution, ExecutionReceipt)>, Vec<Diagnostic>> {
    Err(single(capability_error(
        "threaded scalar-elliptic execution is unavailable in this build",
    )))
}

fn summarize(
    solution: &ResolvedScalarEllipticCartesianSolution,
) -> Result<
    (
        ScalarFieldSummary,
        ScalarEllipticBalanceEvidence,
        AssemblyReport,
        SolveReport,
    ),
    Vec<Diagnostic>,
> {
    let (field, boundary, source, assembly, solve) = match solution {
        ResolvedScalarEllipticCartesianSolution::FiniteElement(solution) => {
            let field = summarize_field(
                solution.field().vertex_values(),
                solution.field().mesh(),
                ScalarFieldLocation::Vertex,
            )?;
            (
                field,
                solution.boundary_reaction_sum(),
                solution.integrated_source(),
                *solution.assembly_report(),
                solution.solve_report().clone(),
            )
        }
        ResolvedScalarEllipticCartesianSolution::FiniteVolume(solution) => {
            let field = summarize_field(
                solution.cell_values(),
                solution.mesh(),
                ScalarFieldLocation::CellCenter,
            )?;
            (
                field,
                solution.boundary_flux_sum(),
                solution.integrated_source(),
                *solution.assembly_report(),
                solution.solve_report().clone(),
            )
        }
    };
    let relative_imbalance =
        (boundary + source).abs() / (boundary.abs() + source.abs()).max(f64::MIN_POSITIVE);
    if !relative_imbalance.is_finite() {
        return Err(single(capability_error(
            "continuous balance evidence is non-finite",
        )));
    }
    Ok((
        field,
        ScalarEllipticBalanceEvidence {
            boundary_total: boundary,
            integrated_source: source,
            relative_imbalance,
        },
        assembly,
        solve,
    ))
}

fn summarize_field(
    values: &[f64],
    mesh: &CartesianMesh,
    location: ScalarFieldLocation,
) -> Result<ScalarFieldSummary, Vec<Diagnostic>> {
    let spatial_dimension = mesh.topological_dimension();
    if !(1..=3).contains(&spatial_dimension) {
        return Err(single(capability_error(format!(
            "accepted Cartesian Field has unsupported dimension {spatial_dimension}"
        ))));
    }
    let mut logical_shape = [1_usize; 3];
    for (axis, extent) in logical_shape[..spatial_dimension].iter_mut().enumerate() {
        let cells = mesh.axis_cell_count(axis).ok_or_else(|| {
            single(capability_error(
                "accepted Cartesian Field is missing an axis extent",
            ))
        })?;
        *extent = match location {
            ScalarFieldLocation::Vertex => cells.checked_add(1).ok_or_else(|| {
                single(capability_error(
                    "accepted Cartesian vertex Field shape overflowed",
                ))
            })?,
            ScalarFieldLocation::CellCenter => cells,
        };
    }
    let expected_count = logical_shape[..spatial_dimension]
        .iter()
        .try_fold(1_usize, |count, extent| count.checked_mul(*extent))
        .ok_or_else(|| {
            single(capability_error(
                "accepted Cartesian Field shape overflowed",
            ))
        })?;
    if expected_count != values.len() {
        return Err(single(capability_error(format!(
            "accepted Cartesian Field shape describes {expected_count} values, but the solution contains {}",
            values.len()
        ))));
    }
    let (minimum, maximum) = finite_range(values)?;
    Ok(ScalarFieldSummary {
        location,
        spatial_dimension,
        logical_shape,
        value_count: values.len(),
        minimum,
        maximum,
    })
}

fn finite_range(values: &[f64]) -> Result<(f64, f64), Vec<Diagnostic>> {
    let Some((&first, rest)) = values.split_first() else {
        return Err(single(capability_error(
            "accepted scalar field unexpectedly contains no values",
        )));
    };
    if !first.is_finite() {
        return Err(single(capability_error(
            "accepted scalar field contains a non-finite value",
        )));
    }
    rest.iter()
        .try_fold((first, first), |(minimum, maximum), &value| {
            value
                .is_finite()
                .then_some((minimum.min(value), maximum.max(value)))
                .ok_or_else(|| {
                    single(capability_error(
                        "accepted scalar field contains a non-finite value",
                    ))
                })
        })
}

fn capability_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}

fn run_manifest_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}

fn single(diagnostic: Diagnostic) -> Vec<Diagnostic> {
    vec![diagnostic]
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_solver::ExecutionTopology;

    const POISSON_2D: &str =
        include_str!("../../../verify/numerics/cartesian-poisson-fem-fvm/models/poisson.eqi");

    fn document() -> ModelDocument {
        ModelDocument::compile("poisson.eqi", POISSON_2D).unwrap()
    }

    fn intent(method: ScalarEllipticMethod, cells: usize, workers: usize) -> ScalarEllipticIntent {
        ScalarEllipticIntent::new(
            RealizationRevision::new(7),
            method,
            NonZeroUsize::new(cells).unwrap(),
            NonZeroUsize::new(workers).unwrap(),
        )
    }

    #[derive(Debug, Default)]
    struct RecordingScalarEllipticObserver {
        cancel_at: Option<ScalarEllipticRunProgress>,
        observed: Vec<ScalarEllipticRunProgress>,
    }

    impl ScalarEllipticRunObserver for RecordingScalarEllipticObserver {
        fn observe(&mut self, progress: ScalarEllipticRunProgress) -> ScalarEllipticRunDirective {
            self.observed.push(progress);
            if self.cancel_at == Some(progress) {
                ScalarEllipticRunDirective::Cancel
            } else {
                ScalarEllipticRunDirective::Continue
            }
        }
    }

    #[test]
    fn preview_is_a_stable_content_addressed_capability_decision() {
        let document = document();
        let plan = document
            .preview_scalar_elliptic_run(
                intent(ScalarEllipticMethod::FiniteElement, 16, 1),
                ScalarEllipticExecutionEnvironment::host_serial(),
            )
            .unwrap();
        let replay = document
            .preview_scalar_elliptic_run(
                intent(ScalarEllipticMethod::FiniteElement, 16, 1),
                ScalarEllipticExecutionEnvironment::host_serial(),
            )
            .unwrap();

        assert_eq!(plan, replay);
        assert_eq!(plan.key().len(), 64);
        assert_eq!(plan.requirements().spatial_dimension().get(), 2);
        assert_eq!(plan.requirements().scalar_type(), ScalarType::F64);
        assert_eq!(
            plan.requirements().vector_layout(),
            VectorLayoutKind::Replicated
        );
        assert_eq!(plan.cell_count(), 256);
        assert_eq!(plan.field_value_count(), 289);
        assert_eq!(plan.artifact().digest().unwrap().to_string(), plan.key());
        let lowered = lower_scalar_elliptic_cartesian(document.program()).unwrap();
        assert_eq!(
            plan.portable_realization().domains()[0].domain(),
            lowered.domain_id()
        );
        assert_eq!(
            plan.portable_realization().fields()[0].field(),
            lowered.field_id()
        );
        assert_eq!(
            plan.portable_realization().systems()[0].operator_properties(),
            LinearOperatorProperties::SymmetricPositiveDefinite
        );
    }

    #[test]
    fn method_revision_and_mesh_choices_change_realization_identity_not_model_identity() {
        let document = document();
        let fem = document
            .preview_scalar_elliptic_run(
                intent(ScalarEllipticMethod::FiniteElement, 16, 1),
                ScalarEllipticExecutionEnvironment::host_serial(),
            )
            .unwrap();
        let fvm = document
            .preview_scalar_elliptic_run(
                intent(ScalarEllipticMethod::FiniteVolume, 16, 1),
                ScalarEllipticExecutionEnvironment::host_serial(),
            )
            .unwrap();

        assert_eq!(fem.model_digest(), fvm.model_digest());
        assert_ne!(fem.key(), fvm.key());
        assert_eq!(fvm.field_value_count(), 256);
        assert_eq!(
            fvm.realization().discretization().method(),
            DiscretizationMethod::CellCenteredFiniteVolume
        );
    }

    #[test]
    fn unsupported_workers_and_oversized_meshes_fail_before_allocation() {
        let document = document();
        let workers = document
            .preview_scalar_elliptic_run(
                intent(ScalarEllipticMethod::FiniteElement, 16, 2),
                ScalarEllipticExecutionEnvironment::host_serial(),
            )
            .unwrap_err();
        assert_eq!(workers[0].code(), codes::INVALID_REALIZATION);
        assert!(workers[0].message().contains("serial adapter"));

        let oversized = document
            .preview_scalar_elliptic_run(
                intent(ScalarEllipticMethod::FiniteElement, 500, 1),
                ScalarEllipticExecutionEnvironment::host_serial(),
            )
            .unwrap_err();
        assert_eq!(oversized[0].code(), codes::INVALID_REALIZATION);
        assert!(oversized[0].message().contains("before allocation"));
    }

    #[test]
    fn controlled_run_observes_only_the_three_exact_application_phases() {
        let document = document();
        let environment = ScalarEllipticExecutionEnvironment::host_serial();
        for method in [
            ScalarEllipticMethod::FiniteElement,
            ScalarEllipticMethod::FiniteVolume,
        ] {
            let plan = document
                .preview_scalar_elliptic_run(intent(method, 8, 1), environment)
                .unwrap();
            let mut observer = RecordingScalarEllipticObserver::default();
            let outcome = document
                .run_scalar_elliptic_plan_controlled(plan, environment, &mut observer)
                .unwrap();
            let ScalarEllipticRunOutcome::Completed(result) = outcome else {
                panic!("the recording observer cannot cancel the run");
            };

            assert_eq!(
                observer.observed,
                [
                    ScalarEllipticRunProgress::PlanReplayed,
                    ScalarEllipticRunProgress::SystemFinalized,
                    ScalarEllipticRunProgress::SolutionAccepted,
                ]
            );
            assert_eq!(result.plan().intent().method(), method);
            assert_eq!(result.field_values().len(), result.field().value_count());
            result
                .plan()
                .validate_run_manifest(result.run_manifest())
                .unwrap();
        }
    }

    #[test]
    fn cancellation_stops_at_each_exact_phase_without_a_partial_result() {
        let document = document();
        let environment = ScalarEllipticExecutionEnvironment::host_serial();
        let phases = [
            ScalarEllipticRunProgress::PlanReplayed,
            ScalarEllipticRunProgress::SystemFinalized,
            ScalarEllipticRunProgress::SolutionAccepted,
        ];

        for (cancel_index, cancel_at) in phases.into_iter().enumerate() {
            let plan = document
                .preview_scalar_elliptic_run(
                    intent(ScalarEllipticMethod::FiniteElement, 8, 1),
                    environment,
                )
                .unwrap();
            let key = plan.key().to_owned();
            let mut observer = RecordingScalarEllipticObserver {
                cancel_at: Some(cancel_at),
                observed: Vec::new(),
            };
            let outcome = document
                .run_scalar_elliptic_plan_controlled(plan, environment, &mut observer)
                .unwrap();
            let ScalarEllipticRunOutcome::Cancelled(cancellation) = outcome else {
                panic!("the selected accepted phase must cancel the run");
            };

            assert_eq!(observer.observed, phases[..=cancel_index]);
            assert_eq!(cancellation.progress(), cancel_at);
            assert_eq!(cancellation.plan().key(), key);
        }
    }

    #[test]
    fn serial_fem_and_fvm_return_bounded_fields_and_independent_evidence() {
        let document = document();
        for method in [
            ScalarEllipticMethod::FiniteElement,
            ScalarEllipticMethod::FiniteVolume,
        ] {
            let accepted = document
                .preview_scalar_elliptic_run(
                    intent(method, 8, 1),
                    ScalarEllipticExecutionEnvironment::host_serial(),
                )
                .unwrap();
            let result = document
                .run_scalar_elliptic_plan(
                    accepted,
                    ScalarEllipticExecutionEnvironment::host_serial(),
                )
                .unwrap();

            assert!(result.field().minimum().is_finite());
            assert!(result.field().maximum().is_finite());
            assert!(result.field().maximum() >= result.field().minimum());
            let extent = match method {
                ScalarEllipticMethod::FiniteElement => 9,
                ScalarEllipticMethod::FiniteVolume => 8,
            };
            assert_eq!(result.field().spatial_dimension(), 2);
            assert_eq!(result.field().logical_shape(), &[extent, extent]);
            assert_eq!(result.field_values().len(), result.field().value_count());
            assert!(result.field_values().iter().all(|value| value.is_finite()));
            let minimum = result
                .field_values()
                .iter()
                .copied()
                .reduce(f64::min)
                .unwrap();
            let maximum = result
                .field_values()
                .iter()
                .copied()
                .reduce(f64::max)
                .unwrap();
            assert_eq!(minimum, result.field().minimum());
            assert_eq!(maximum, result.field().maximum());
            assert!(result.balance().relative_imbalance() < 1.0e-12);
            assert!(result.solve().true_residual_norm() <= result.solve().residual_target());
            assert_eq!(
                result.solve().execution().topology(),
                ExecutionTopology::Host {
                    workers: NonZeroUsize::MIN
                }
            );
            assert_eq!(result.assembly().execution(), result.solve().execution());
            assert_eq!(result.receipt().report(), result.solve());
            assert_eq!(
                result.receipt().binding().realization(),
                result.plan().portable_realization()
            );
        }
    }

    #[test]
    fn successful_run_manifest_replays_exact_actual_host_provenance_and_linkage() {
        let document = document();
        let environment = ScalarEllipticExecutionEnvironment::host_serial();
        let result = document
            .run_scalar_elliptic_plan(
                document
                    .preview_scalar_elliptic_run(
                        intent(ScalarEllipticMethod::FiniteElement, 8, 1),
                        environment,
                    )
                    .unwrap(),
                environment,
            )
            .unwrap();

        let manifest = result.run_manifest();
        assert_eq!(manifest.model().to_string(), result.plan().model_digest());
        assert_eq!(
            manifest.realization(),
            result.plan().artifact().digest().unwrap()
        );
        assert_eq!(
            manifest.semantic_revision(),
            result.plan().artifact().semantic_revision().get()
        );
        assert!(manifest.outputs().is_empty());

        let actual = result.solve();
        let execution = manifest.execution();
        assert_eq!(execution.adapter(), actual.execution().adapter().as_str());
        assert_eq!(
            execution.adapter_version(),
            SERIAL_EXECUTION_PROVIDER.implementation_version()
        );
        assert_eq!(execution.solver_backend(), actual.backend().as_str());
        assert_eq!(
            execution.solver_backend_version(),
            REFERENCE_SOLVER_PROVIDER.implementation_version()
        );
        assert_eq!(execution.reduction(), actual.reduction());
        assert!(execution.libraries().is_empty());
        assert_eq!(
            execution.topology().unwrap(),
            ExecutionTopologyV1::Host {
                workers: NonZeroUsize::MIN,
            }
        );

        let bytes = manifest.canonical_json().unwrap();
        let replay = RunManifestV2::from_json(&bytes, JsonDecoderLimits::default()).unwrap();
        assert_eq!(replay.canonical_json().unwrap(), bytes);
        assert_eq!(replay.digest().unwrap(), manifest.digest().unwrap());
        replay.validate_against(result.plan().artifact()).unwrap();
        assert_eq!(
            result
                .plan()
                .replay_run_manifest(&bytes, JsonDecoderLimits::default())
                .unwrap(),
            replay
        );

        let forged_execution = ExecutionProvenanceV1::new(
            "example.forged-adapter",
            env!("CARGO_PKG_VERSION"),
            result.solve().backend().as_str(),
            env!("CARGO_PKG_VERSION"),
            ExecutionTopologyV1::Host {
                workers: NonZeroUsize::MIN,
            },
            result.solve().reduction(),
        )
        .unwrap();
        let forged = RunManifestV2::new(result.plan().artifact(), forged_execution).unwrap();
        assert_eq!(
            result
                .plan()
                .validate_run_manifest(&forged)
                .unwrap_err()
                .code(),
            codes::INVALID_ARTIFACT
        );

        let forged_output = manifest
            .clone()
            .with_output(eqiora_artifact::ArtifactDigest::from_hex("00".repeat(32)).unwrap());
        assert_eq!(
            result
                .plan()
                .validate_run_manifest(&forged_output)
                .unwrap_err()
                .code(),
            codes::INVALID_ARTIFACT
        );

        let foreign_realization = document
            .preview_scalar_elliptic_run(
                intent(ScalarEllipticMethod::FiniteVolume, 8, 1),
                environment,
            )
            .unwrap();
        assert!(
            replay
                .validate_against(foreign_realization.artifact())
                .is_err()
        );
    }

    #[test]
    fn run_provenance_rejects_contradictory_provider_library_versions() {
        const SOLVER_LIBRARIES: &[ProviderLibrary] =
            &[ProviderLibrary::new("shared-runtime", "1.0.0")];
        const EXECUTION_LIBRARIES: &[ProviderLibrary] =
            &[ProviderLibrary::new("shared-runtime", "2.0.0")];
        let error = provider_execution_provenance(
            SolverProvider::new(
                eqiora_solver::BackendId::new("eqiora.test.solver"),
                "0.1.0",
                SOLVER_LIBRARIES,
            ),
            ExecutionProvider::new(
                eqiora_solver::ExecutionId::new("eqiora.test.execution"),
                "0.1.0",
                EXECUTION_LIBRARIES,
            ),
            SERIAL_EXECUTION_PROVIDER,
            ExecutionTopologyV1::Host {
                workers: NonZeroUsize::MIN,
            },
            ReductionPolicy::Reproducible,
        )
        .unwrap_err();

        assert_eq!(error.code(), codes::INVALID_ARTIFACT);
        assert!(error.message().contains("contradictory versions"));
    }

    #[cfg(feature = "rayon")]
    #[test]
    fn deployment_capacity_rejection_precedes_pool_factory() {
        use std::cell::Cell;

        let document = document();
        let environment =
            ScalarEllipticExecutionEnvironment::host_threaded(NonZeroUsize::new(2).unwrap());
        let plan = document
            .preview_scalar_elliptic_run(
                intent(ScalarEllipticMethod::FiniteElement, 8, 2),
                environment,
            )
            .unwrap();
        let pool_allocations = Cell::new(0usize);
        let rejected = DeploymentBinding::bind_host(
            plan.portable_realization(),
            HostExecutorDescriptor::new(
                REFERENCE_SOLVER_PROVIDER,
                eqiora_backend_rayon::RAYON_EXECUTION_PROVIDER,
                NonZeroUsize::MIN,
                REFERENCE_LINEAR_SOLVER.capabilities(),
            ),
        )
        .and_then(|binding| {
            pool_allocations.set(pool_allocations.get() + 1);
            CpuThreadPool::from_deployment(&binding)
        })
        .unwrap_err();

        assert_eq!(rejected.code(), codes::INVALID_REALIZATION);
        assert!(rejected.message().contains("executor capacity"));
        assert_eq!(pool_allocations.get(), 0);
    }

    #[cfg(feature = "rayon")]
    #[test]
    fn threaded_execution_replays_the_same_typed_plan_and_records_workers() {
        let document = document();
        let serial = document
            .run_scalar_elliptic_plan(
                document
                    .preview_scalar_elliptic_run(
                        intent(ScalarEllipticMethod::FiniteElement, 8, 1),
                        ScalarEllipticExecutionEnvironment::host_serial(),
                    )
                    .unwrap(),
                ScalarEllipticExecutionEnvironment::host_serial(),
            )
            .unwrap();
        let environment =
            ScalarEllipticExecutionEnvironment::host_threaded(NonZeroUsize::new(2).unwrap());
        let accepted = document
            .preview_scalar_elliptic_run(
                intent(ScalarEllipticMethod::FiniteElement, 8, 2),
                environment,
            )
            .unwrap();
        let result = document
            .run_scalar_elliptic_plan(accepted, environment)
            .unwrap();

        assert_eq!(
            result.solve().execution().topology(),
            ExecutionTopology::Host {
                workers: NonZeroUsize::new(2).unwrap()
            }
        );
        let manifest_execution = result.run_manifest().execution();
        assert_eq!(
            manifest_execution.adapter(),
            result.solve().execution().adapter().as_str()
        );
        assert_eq!(
            manifest_execution.solver_backend(),
            result.solve().backend().as_str()
        );
        assert_eq!(
            manifest_execution.adapter_version(),
            eqiora_backend_rayon::RAYON_ADAPTER_VERSION
        );
        assert_eq!(
            manifest_execution.solver_backend_version(),
            REFERENCE_SOLVER_PROVIDER.implementation_version()
        );
        assert_eq!(
            manifest_execution
                .libraries()
                .get("rayon")
                .map(String::as_str),
            Some(eqiora_backend_rayon::RAYON_VERSION)
        );
        assert_eq!(manifest_execution.libraries().len(), 1);
        assert_eq!(manifest_execution.reduction(), result.solve().reduction());
        assert_eq!(
            manifest_execution.topology().unwrap(),
            ExecutionTopologyV1::Host {
                workers: NonZeroUsize::new(2).unwrap(),
            }
        );
        assert_eq!(result.assembly().execution(), result.solve().execution());
        assert_eq!(
            result.solve().verification(),
            ExecutionReport::host(
                eqiora_backend_rayon::RAYON_EXECUTION,
                NonZeroUsize::new(2).unwrap()
            )
        );
        assert_eq!(
            result.receipt().acceptance_verification(),
            ExecutionReport::host_serial()
        );
        assert!(result.balance().relative_imbalance() < 1.0e-12);
        assert_eq!(result.field(), serial.field());
        assert_eq!(result.field().logical_shape(), &[9, 9]);
        assert_eq!(result.field_values(), serial.field_values());
        assert_eq!(result.balance(), serial.balance());
        assert_eq!(
            result.receipt().binding().realization().lineage(),
            serial.receipt().binding().realization().lineage()
        );
        assert_ne!(result.receipt().binding(), serial.receipt().binding());
        assert_ne!(result.solve().execution(), serial.solve().execution());
        assert_eq!(
            result.receipt().dag().steps(),
            serial.receipt().dag().steps()
        );
        assert_eq!(
            result.receipt().dag().operator(),
            serial.receipt().dag().operator()
        );
        assert_eq!(
            result.receipt().dag().solver_plan(),
            serial.receipt().dag().solver_plan()
        );
        assert_eq!(result.receipt().dimension(), serial.receipt().dimension());
    }
}
