//! What a scalar-elliptic run is asked to do, and how it is steered.
//!
//! Everything here is settled before any allocation: the intent a client
//! proposes, the environment it may run in, the accepted plan that results, and
//! the observer handles through which a caller watches or cancels the run. None
//! of it touches a mesh, an assembler, or a solver.

use super::diagnostic::{capability_error, run_manifest_error, single};
use super::execution::{host_executor, scalar_elliptic_execution_provenance};
use super::field::{CartesianScalarFieldProjection, ScalarEllipticRunResult};
use eqiora_artifact::{
    CartesianMeshEnvelopeV1, JsonDecoderLimits, RealizationEnvelopeV1, RunManifestV2,
};
use eqiora_core::Diagnostic;
use eqiora_meshing::CartesianMesh;
use eqiora_realization::{
    DiscretizationMethod, PortableRealizationGraph, QuadraturePolicy, RealizationPlan,
    RealizationRequirements, RealizationRevision, ResolvedRealization, Space,
};
use std::num::{NonZeroU16, NonZeroUsize};
use std::time::Duration;

/// Maximum topological cells or reported scalar values admitted before any
/// mesh, assembly, or solver allocation.
pub const MAX_SCALAR_ELLIPTIC_ENTITY_COUNT: usize = 250_000;

/// Exact effective generated Cartesian Mesh owned by a resolved Plan.
///
/// This value is deliberately narrower than a universal mesh request: it is
/// the already executable uniform Cartesian mesh family used by the scalar
/// Q1 and TPFA consumers.
#[derive(Debug, Clone, PartialEq)]
pub struct ScalarEllipticMesh {
    pub(super) cells_per_axis: NonZeroUsize,
    pub(super) mesh: CartesianMeshEnvelopeV1,
}

impl ScalarEllipticMesh {
    /// Materialize one exact uniform Cartesian Mesh from resolved bounds.
    ///
    /// # Errors
    /// Rejects dimensions or resource shapes outside the current scalar FEM/FVM
    /// envelope before allocating Mesh storage.
    pub(super) fn uniform(
        bounds: &[[f64; 2]],
        cells_per_axis: NonZeroUsize,
    ) -> Result<Self, Vec<Diagnostic>> {
        let dimension = NonZeroUsize::new(bounds.len()).ok_or_else(|| {
            single(capability_error(
                "a scalar-elliptic Cartesian Mesh requires at least one dimension",
            ))
        })?;
        if dimension.get() > 3 {
            return Err(single(capability_error(
                "scalar-elliptic Cartesian Meshes admit one through three dimensions",
            )));
        }
        resource_shape(
            ScalarEllipticIntent::new(
                RealizationRevision::new(1),
                ScalarEllipticMethod::FiniteElement,
                cells_per_axis,
                NonZeroUsize::MIN,
            ),
            dimension,
        )?;
        let extents = vec![cells_per_axis.get(); dimension.get()];
        let mesh = CartesianMesh::uniform(bounds, &extents).map_err(single)?;
        let mesh = CartesianMeshEnvelopeV1::from_mesh(&mesh).map_err(single)?;
        Ok(Self {
            cells_per_axis,
            mesh,
        })
    }

    /// Uniform cell count on every topological axis.
    #[must_use]
    pub const fn cells_per_axis(&self) -> NonZeroUsize {
        self.cells_per_axis
    }

    /// Runtime topological dimension.
    #[must_use]
    pub fn dimension(&self) -> usize {
        self.mesh.dimension()
    }

    /// Exact generated Mesh artifact.
    #[must_use]
    pub const fn artifact(&self) -> &CartesianMeshEnvelopeV1 {
        &self.mesh
    }
}

/// Numerical family selected by one scalar-elliptic Realization intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarEllipticMethod {
    /// Continuous Q1 Galerkin finite elements on a generated Cartesian mesh.
    FiniteElement,
    /// Cell-centred finite volumes on a generated Cartesian mesh.
    FiniteVolume,
}

impl ScalarEllipticMethod {
    pub(super) const fn discretization(self) -> DiscretizationMethod {
        match self {
            Self::FiniteElement => DiscretizationMethod::ContinuousGalerkin,
            Self::FiniteVolume => DiscretizationMethod::CellCenteredFiniteVolume,
        }
    }

    pub(super) const fn space(self) -> Space {
        match self {
            Self::FiniteElement => Space::continuous_lagrange(NonZeroU16::MIN),
            Self::FiniteVolume => Space::cell_constant(),
        }
    }

    pub(super) const fn quadrature(self) -> QuadraturePolicy {
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
    pub(super) realization_revision: RealizationRevision,
    pub(super) method: ScalarEllipticMethod,
    pub(super) cells_per_axis: NonZeroUsize,
    pub(super) workers: NonZeroUsize,
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
    pub(super) maximum_workers: NonZeroUsize,
    pub(super) threaded: bool,
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

    pub(super) fn supports(self, workers: NonZeroUsize) -> bool {
        workers <= self.maximum_workers && (workers == NonZeroUsize::MIN || self.threaded)
    }
}

/// Exact content-addressed plan admitted before numerical allocation.
#[derive(Debug, Clone, PartialEq)]
pub struct ScalarEllipticRunPlan {
    pub(super) model_digest: String,
    pub(super) intent: ScalarEllipticIntent,
    pub(super) environment: ScalarEllipticExecutionEnvironment,
    pub(super) resolved: ResolvedRealization,
    pub(super) portable: PortableRealizationGraph,
    pub(super) artifact: RealizationEnvelopeV1,
    pub(super) key: String,
    pub(super) cell_count: usize,
    pub(super) field_value_count: usize,
    pub(super) field_projection: CartesianScalarFieldProjection,
    pub(super) mesh: Option<ScalarEllipticMesh>,
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

    /// Exact effective Mesh authenticated alongside the Model by this Plan.
    #[must_use]
    pub const fn mesh(&self) -> Option<&ScalarEllipticMesh> {
        self.mesh.as_ref()
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

    /// Solver backend selected by this exact accepted host plan.
    #[must_use]
    pub fn solver_backend(&self) -> &'static str {
        host_executor(self.environment, self.intent.workers)
            .solver_provider()
            .id()
            .as_str()
    }

    /// Implementation version of the selected solver backend.
    #[must_use]
    pub fn solver_backend_version(&self) -> &'static str {
        host_executor(self.environment, self.intent.workers)
            .solver_provider()
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
    pub(super) plan: ScalarEllipticRunPlan,
    pub(super) elapsed: Duration,
    pub(super) progress: ScalarEllipticRunProgress,
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
pub(super) struct UninterruptedScalarEllipticRun;

impl ScalarEllipticRunObserver for UninterruptedScalarEllipticRun {
    fn observe(&mut self, _progress: ScalarEllipticRunProgress) -> ScalarEllipticRunDirective {
        ScalarEllipticRunDirective::Continue
    }
}

pub(super) fn resource_shape(
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

pub(super) fn checked_power(
    base: usize,
    exponent: usize,
    label: &str,
) -> Result<usize, Vec<Diagnostic>> {
    (0..exponent).try_fold(1usize, |value, _| {
        value
            .checked_mul(base)
            .ok_or_else(|| single(capability_error(format!("{label} overflowed"))))
    })
}
