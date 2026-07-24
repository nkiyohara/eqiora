//! Equation-aware admission of the fixed-topology ALE FSI realization.
//!
//! This bridge is deliberately narrower than the generic Realization contract.
//! It accepts one resolved serial-host plan in either admitted spatial
//! dimension, replays every canonical role and numerical choice, seals
//! harmonic mesh motion against the authenticated reference topology, and only
//! then constructs the initial moving state. Coordinates and mesh velocity are
//! never accepted as inputs.

use std::collections::BTreeSet;
use std::num::{NonZeroU16, NonZeroUsize};

use eqiora_assembly::{AssemblyBackend, REFERENCE_ASSEMBLY_BACKEND};
use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, DimExponents, Id, OntologyId};
use eqiora_meshing::{
    CellId, MeshEntity, MeshTopology, QuadratureRule, SimplicialMesh, simplex_duffy_gauss_legendre,
    triangle_duffy_gauss_legendre,
};
use eqiora_realization::{
    AleFsiRemeshTransferPlan2d, AlgebraicBlock, BackwardEulerStatePair, ConformingTraceQuotient,
    CoupledFieldwiseRealizationRequirements, DiscretizationMethod, DomainFieldInventory,
    ExecutionSchedule, FixedTopologyAleCoupledRealizationPlan,
    FixedTopologyAleCoupledRealizationRequirements, MeshArtifactReference, MeshPolicy,
    PortableRealizationGraph, QuadraturePolicy, RealizationRequirements, RealizationRevision,
    ResolvedFixedTopologyAleCoupledRealization, SemanticRevision, SolveRoot, Space, Target,
    TraceFieldEndpoint, VectorLayoutKind,
};
use eqiora_schema::Model;
use eqiora_schema::kernel::BoundarySide;
use eqiora_solver::{
    LinearOperatorProperties, LinearSolveRequest, LinearSolverBackend, ScalarType,
};

use super::{AleFsiCartesianModel, FsiInterfaceSide};
use crate::canonical_boundary::PhysicalBoundaryDisposition;
use crate::simplicial_ale_fsi::{
    AleFsiBoundary, AleFsiState, AleFsiStepPlan, AleFsiTrajectory, P1HarmonicMeshMotionAction,
    advance_simplicial_ale_fsi_2d_with_assembly, advance_simplicial_ale_fsi_3d_with_assembly,
};
use crate::simplicial_ale_remesh::{
    AcceptedAleFsiRemeshProjection2d, project_simplicial_ale_fsi_remesh_2d,
};
use crate::simplicial_fsi::{
    FixedReferenceFsiLoad, FixedReferenceFsiMaterial, FixedReferenceFsiPartition,
    FixedReferenceFsiScale,
};
use crate::step_count::NonZeroStepCount;

const LEGACY_TRIANGLE_DUFFY_POINTS_PER_AXIS: usize = 5;
const TETRAHEDRON_DUFFY_POINTS_PER_AXIS: usize = 7;

const LENGTH: DimExponents = DimExponents {
    length: 1,
    ..DimExponents::DIMENSIONLESS
};
const VELOCITY: DimExponents = DimExponents {
    length: 1,
    time: -1,
    ..DimExponents::DIMENSIONLESS
};
const PRESSURE: DimExponents = DimExponents {
    mass: 1,
    length: -1,
    time: -2,
    ..DimExponents::DIMENSIONLESS
};
/// Initial physical coefficients before the sole harmonic geometry action.
///
/// This value contains no coordinates, mesh displacement outside the solid,
/// mesh velocity, or GCL coefficient.  Finalization validates its exact mesh
/// support and derives an [`AleFsiState`] through the resolved motion policy.
#[derive(Debug, Clone, PartialEq)]
pub struct AleFsiInitialPhysicalState<const D: usize> {
    time: f64,
    vertex_velocity: Vec<[f64; D]>,
    fluid_cell_bubble_velocity: Vec<[f64; D]>,
    fluid_pressure: Vec<f64>,
    solid_displacement: Vec<[f64; D]>,
}

/// Established two-dimensional initial-state API.
pub type AleFsiInitialPhysicalState2d = AleFsiInitialPhysicalState<2>;

/// Three-dimensional initial physical coefficients before harmonic motion.
pub type AleFsiInitialPhysicalState3d = AleFsiInitialPhysicalState<3>;

impl<const D: usize> AleFsiInitialPhysicalState<D> {
    /// Admit finite physical coefficients without inventing geometry.
    ///
    /// Shape, support, and boundary closure are checked against the exact
    /// authenticated topology during finalization.
    ///
    /// # Errors
    /// Returns `EQ0807` for negative/non-finite time or a non-finite value.
    pub fn new(
        time: f64,
        vertex_velocity: Vec<[f64; D]>,
        fluid_cell_bubble_velocity: Vec<[f64; D]>,
        fluid_pressure: Vec<f64>,
        solid_displacement: Vec<[f64; D]>,
    ) -> Result<Self, Diagnostic> {
        require_supported_dimension::<D>()?;
        if !time.is_finite()
            || time < 0.0
            || vertex_velocity
                .iter()
                .chain(&fluid_cell_bubble_velocity)
                .chain(&solid_displacement)
                .flatten()
                .chain(fluid_pressure.iter())
                .any(|value| !value.is_finite())
        {
            return Err(invalid_realization(
                "ALE FSI initial physical state must contain finite coefficients at finite non-negative time",
            ));
        }
        Ok(Self {
            time,
            vertex_velocity,
            fluid_cell_bubble_velocity,
            fluid_pressure,
            solid_displacement,
        })
    }

    fn into_state(
        self,
        mesh: &SimplicialMesh,
        partition: &FixedReferenceFsiPartition<D>,
        motion: &P1HarmonicMeshMotionAction<D>,
    ) -> Result<AleFsiState<D>, Diagnostic> {
        AleFsiState::<D>::new(
            self.time,
            mesh,
            partition,
            motion,
            self.vertex_velocity,
            self.fluid_cell_bubble_velocity,
            self.fluid_pressure,
            self.solid_displacement,
        )
    }
}

/// Exact Semantic Field identities retained by the finalized ALE trajectory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AleFsiFieldIdentities<const D: usize> {
    fluid_velocity: Id<kinds::Field>,
    fluid_pressure: Id<kinds::Field>,
    solid_velocity: Id<kinds::Field>,
    solid_displacement: Id<kinds::Field>,
}

/// Established two-dimensional Field-identity API.
pub type AleFsiFieldIdentities2d = AleFsiFieldIdentities<2>;

/// Three-dimensional canonical Field identities.
pub type AleFsiFieldIdentities3d = AleFsiFieldIdentities<3>;

impl<const D: usize> AleFsiFieldIdentities<D> {
    /// Conservative fluid velocity represented by MINI coefficients.
    #[must_use]
    pub const fn fluid_velocity(self) -> Id<kinds::Field> {
        self.fluid_velocity
    }

    /// Incompressibility multiplier represented by fluid-vertex P1 coefficients.
    #[must_use]
    pub const fn fluid_pressure(self) -> Id<kinds::Field> {
        self.fluid_pressure
    }

    /// Dynamic-solid velocity sharing the conforming interface trace.
    #[must_use]
    pub const fn solid_velocity(self) -> Id<kinds::Field> {
        self.solid_velocity
    }

    /// Absolute solid displacement which is the sole geometry driver.
    #[must_use]
    pub const fn solid_displacement(self) -> Id<kinds::Field> {
        self.solid_displacement
    }
}

/// Fully replayed canonical/Realization/numerical ALE trajectory input.
///
/// The value owns one immutable reference mesh, exact material partition,
/// physical boundary closure, sealed harmonic map, derived initial geometry,
/// dimension-appropriate simplex quadrature, and the unchanged common
/// nonlinear/linear plans.
#[derive(Debug, Clone, PartialEq)]
pub struct FinalizedResolvedFixedTopologyAleFsi<const D: usize> {
    model: OntologyId<Model>,
    semantic_revision: SemanticRevision,
    realization_revision: RealizationRevision,
    mesh_artifact: MeshArtifactReference,
    fields: AleFsiFieldIdentities<D>,
    plan: FixedTopologyAleCoupledRealizationPlan,
    realization_graph: PortableRealizationGraph,
    reference: SimplicialMesh,
    partition: FixedReferenceFsiPartition<D>,
    boundary: AleFsiBoundary<D>,
    motion: P1HarmonicMeshMotionAction<D>,
    initial: AleFsiState<D>,
    step_plan: AleFsiStepPlan<D>,
    quadrature: QuadratureRule,
}

/// Established two-dimensional finalized-operator API.
pub type FinalizedResolvedFixedTopologyAleFsi2d = FinalizedResolvedFixedTopologyAleFsi<2>;

/// Three-dimensional finalized fixed-topology ALE FSI operator.
pub type FinalizedResolvedFixedTopologyAleFsi3d = FinalizedResolvedFixedTopologyAleFsi<3>;

/// Replayed canonical and Realization facts awaiting one physical state.
///
/// This private boundary deliberately separates immutable target-mesh
/// admission from state admission.  Ordinary starts and conservative remesh
/// restarts therefore enter the same finalized operator through exactly one
/// state validator.
#[derive(Debug, Clone, PartialEq)]
struct ReplayedResolvedFixedTopologyAleFsi<const D: usize> {
    model: OntologyId<Model>,
    semantic_revision: SemanticRevision,
    realization_revision: RealizationRevision,
    mesh_artifact: MeshArtifactReference,
    fields: AleFsiFieldIdentities<D>,
    plan: FixedTopologyAleCoupledRealizationPlan,
    realization_graph: PortableRealizationGraph,
    reference: SimplicialMesh,
    partition: FixedReferenceFsiPartition<D>,
    boundary: AleFsiBoundary<D>,
    motion: P1HarmonicMeshMotionAction<D>,
    step_plan: AleFsiStepPlan<D>,
    quadrature: QuadratureRule,
}

impl<const D: usize> ReplayedResolvedFixedTopologyAleFsi<D> {
    fn accept_initial(
        self,
        initial: AleFsiInitialPhysicalState<D>,
    ) -> Result<FinalizedResolvedFixedTopologyAleFsi<D>, Diagnostic> {
        let initial = initial.into_state(&self.reference, &self.partition, &self.motion)?;
        if self
            .boundary
            .fixed_zero_velocity_vertices()
            .iter()
            .any(|vertex| initial.vertex_velocity()[vertex.index()] != [0.0; D])
        {
            return Err(invalid_realization(
                "fixed-topology ALE FSI initial velocity violates the complete homogeneous exterior closure",
            ));
        }
        Ok(FinalizedResolvedFixedTopologyAleFsi::<D> {
            model: self.model,
            semantic_revision: self.semantic_revision,
            realization_revision: self.realization_revision,
            mesh_artifact: self.mesh_artifact,
            fields: self.fields,
            plan: self.plan,
            realization_graph: self.realization_graph,
            reference: self.reference,
            partition: self.partition,
            boundary: self.boundary,
            motion: self.motion,
            initial,
            step_plan: self.step_plan,
            quadrature: self.quadrature,
        })
    }
}

impl<const D: usize> FinalizedResolvedFixedTopologyAleFsi<D> {
    /// Exact Semantic Model identity admitted by the resolved plan.
    #[must_use]
    pub const fn model(&self) -> OntologyId<Model> {
        self.model
    }

    /// Exact admitted Semantic Model revision.
    #[must_use]
    pub const fn semantic_revision(&self) -> SemanticRevision {
        self.semantic_revision
    }

    /// Independently selected Realization revision.
    #[must_use]
    pub const fn realization_revision(&self) -> RealizationRevision {
        self.realization_revision
    }

    /// Exact authenticated immutable-mesh content reference.
    #[must_use]
    pub const fn mesh_artifact(&self) -> MeshArtifactReference {
        self.mesh_artifact
    }

    /// Canonical Field roles represented by the numerical state.
    #[must_use]
    pub const fn fields(&self) -> AleFsiFieldIdentities<D> {
        self.fields
    }

    /// Portable nonlinear graph from the exact resolved Realization.
    #[must_use]
    pub const fn realization_graph(&self) -> &PortableRealizationGraph {
        &self.realization_graph
    }

    /// Sole sealed harmonic motion action, including its solve evidence.
    #[must_use]
    pub const fn motion(&self) -> &P1HarmonicMeshMotionAction<D> {
        &self.motion
    }

    /// Initial physical state with geometry derived by [`Self::motion`].
    #[must_use]
    pub const fn initial_state(&self) -> &AleFsiState<D> {
        &self.initial
    }

    /// Common nonlinear, linear, material, scale, and time-step policy.
    #[must_use]
    pub const fn step_plan(&self) -> AleFsiStepPlan<D> {
        self.step_plan
    }
}

type AdvanceAleFsiWithAssembly<const D: usize> = fn(
    &SimplicialMesh,
    &FixedReferenceFsiPartition<D>,
    &AleFsiBoundary<D>,
    &P1HarmonicMeshMotionAction<D>,
    AleFsiState<D>,
    NonZeroStepCount,
    AleFsiStepPlan<D>,
    &QuadratureRule,
    &dyn AssemblyBackend,
    &dyn LinearSolverBackend,
) -> Result<AleFsiTrajectory<D>, Diagnostic>;

fn solve_finalized_with_assembly<const D: usize>(
    finalized: FinalizedResolvedFixedTopologyAleFsi<D>,
    step_count: NonZeroStepCount,
    assembly: &dyn AssemblyBackend,
    backend: &dyn LinearSolverBackend,
    advance: AdvanceAleFsiWithAssembly<D>,
) -> Result<AleFsiTrajectory<D>, Diagnostic> {
    let FinalizedResolvedFixedTopologyAleFsi {
        reference,
        partition,
        boundary,
        motion,
        initial,
        step_plan,
        quadrature,
        ..
    } = finalized;
    advance(
        &reference,
        &partition,
        &boundary,
        &motion,
        initial,
        step_count,
        step_plan,
        &quadrature,
        assembly,
        backend,
    )
}

macro_rules! impl_finalized_solve {
    ($dimension:literal, $advance:path) => {
        impl FinalizedResolvedFixedTopologyAleFsi<$dimension> {
            /// Execute the exact finalized trajectory with reference assembly.
            ///
            /// Step count remains a Run choice and is intentionally absent from
            /// the Realization.
            ///
            /// # Errors
            /// Preserves solver, nonlinear, geometry, assembly, and acceptance diagnostics.
            pub fn solve(
                self,
                step_count: NonZeroStepCount,
                backend: &dyn LinearSolverBackend,
            ) -> Result<AleFsiTrajectory<$dimension>, Diagnostic> {
                self.solve_with_assembly(step_count, &REFERENCE_ASSEMBLY_BACKEND, backend)
            }

            /// Execute through an explicit assembly conformance adapter.
            ///
            /// # Errors
            /// Preserves every reference admission rule and adapter diagnostic.
            #[doc(hidden)]
            pub fn solve_with_assembly(
                self,
                step_count: NonZeroStepCount,
                assembly: &dyn AssemblyBackend,
                backend: &dyn LinearSolverBackend,
            ) -> Result<AleFsiTrajectory<$dimension>, Diagnostic> {
                solve_finalized_with_assembly(self, step_count, assembly, backend, $advance)
            }
        }
    };
}

impl_finalized_solve!(2, advance_simplicial_ale_fsi_2d_with_assembly);
impl_finalized_solve!(3, advance_simplicial_ale_fsi_3d_with_assembly);

/// Accepted zero-time transfer paired with its ordinary target finalizer.
///
/// The target is not a remesh-specific executor.  It is the same finalized ALE
/// operator returned for an ordinary initial state, reached only after the
/// transferred physical state passed the common admission boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct AcceptedResolvedAleFsiRemesh2d {
    projection: AcceptedAleFsiRemeshProjection2d,
    target: FinalizedResolvedFixedTopologyAleFsi2d,
}

impl AcceptedResolvedAleFsiRemesh2d {
    /// Accepted conservative projection and its independently replayed evidence.
    #[must_use]
    pub const fn projection(&self) -> &AcceptedAleFsiRemeshProjection2d {
        &self.projection
    }

    /// Ordinary target-mesh ALE finalizer at the unchanged remesh time.
    #[must_use]
    pub const fn target(&self) -> &FinalizedResolvedFixedTopologyAleFsi2d {
        &self.target
    }

    /// Separate durable transfer evidence from the consumable target executor.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        AcceptedAleFsiRemeshProjection2d,
        FinalizedResolvedFixedTopologyAleFsi2d,
    ) {
        (self.projection, self.target)
    }
}

/// Exact lowerer facts for the bounded fixed-topology ALE FSI path.
#[must_use]
fn fixed_topology_ale_fsi_requirements<const D: usize>(
    model: &AleFsiCartesianModel<D>,
) -> FixedTopologyAleCoupledRealizationRequirements {
    let fields = field_identities(model);
    let coupled = CoupledFieldwiseRealizationRequirements::new(
        [
            DomainFieldInventory::new(
                fluid_domain(model),
                [fields.fluid_velocity, fields.fluid_pressure],
            )
            .expect("lowered ALE fluid owns distinct velocity and pressure Fields"),
            DomainFieldInventory::new(
                solid_domain(model),
                [fields.solid_velocity, fields.solid_displacement],
            )
            .expect("lowered ALE solid owns distinct velocity and displacement Fields"),
        ],
        trace_quotient(model),
        state_pair(model),
        RealizationRequirements::new(
            NonZeroUsize::new(D).expect("supported ALE FSI dimension is non-zero"),
            ScalarType::F64,
            VectorLayoutKind::Replicated,
        ),
    )
    .expect("lowered ALE FSI roles form one exact coupled inventory");
    FixedTopologyAleCoupledRealizationRequirements::new(
        coupled,
        fluid_domain(model),
        solid_domain(model),
        fluid_relation(model),
        solid_kinematic_relation(model),
        fields.fluid_velocity,
        fields.solid_displacement,
    )
    .expect("lowered ALE FSI roles form one exact fixed-topology requirement")
}

/// Exact two-dimensional lowerer facts retained for compatibility.
#[must_use]
pub fn fixed_topology_ale_fsi_requirements_2d(
    model: &AleFsiCartesianModel<2>,
) -> FixedTopologyAleCoupledRealizationRequirements {
    fixed_topology_ale_fsi_requirements(model)
}

/// Exact three-dimensional lowerer facts for tetrahedral ALE FSI.
#[must_use]
pub fn fixed_topology_ale_fsi_requirements_3d(
    model: &AleFsiCartesianModel<3>,
) -> FixedTopologyAleCoupledRealizationRequirements {
    fixed_topology_ale_fsi_requirements(model)
}

/// Finalize one exact resolved fixed-topology ALE trajectory input.
///
/// The artifact layer authenticates `mesh` as `mesh_artifact` before this
/// boundary.  This function independently proves that the resolved plan names
/// that digest, the exact canonical revision and roles, the supported spaces,
/// scale, configuration, quality policy, and serial execution tuple.  It then
/// executes the resolved harmonic solver once to seal the common motion map.
///
/// # Errors
/// Rejects any stale Model/revision/mesh/role, unsupported Realization choice,
/// semantic/mesh partition mismatch, physical-boundary drift, harmonic-solver
/// failure, or invalid initial physical state before returning a finalizer.
#[allow(clippy::too_many_arguments)]
fn finalize_resolved_fixed_topology_ale_fsi<const D: usize>(
    model: &AleFsiCartesianModel<D>,
    resolved: &ResolvedFixedTopologyAleCoupledRealization,
    mesh_artifact: MeshArtifactReference,
    mesh: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition<D>,
    boundary: &AleFsiBoundary<D>,
    initial: AleFsiInitialPhysicalState<D>,
    harmonic_backend: &dyn LinearSolverBackend,
) -> Result<FinalizedResolvedFixedTopologyAleFsi<D>, Diagnostic> {
    replay_resolved_fixed_topology_ale_fsi(
        model,
        resolved,
        mesh_artifact,
        mesh,
        partition,
        boundary,
        harmonic_backend,
    )?
    .accept_initial(initial)
}

/// Finalize the established two-dimensional fixed-topology ALE path.
#[allow(clippy::too_many_arguments)]
pub fn finalize_resolved_fixed_topology_ale_fsi_2d(
    model: &AleFsiCartesianModel<2>,
    resolved: &ResolvedFixedTopologyAleCoupledRealization,
    mesh_artifact: MeshArtifactReference,
    mesh: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition<2>,
    boundary: &AleFsiBoundary<2>,
    initial: AleFsiInitialPhysicalState<2>,
    harmonic_backend: &dyn LinearSolverBackend,
) -> Result<FinalizedResolvedFixedTopologyAleFsi<2>, Diagnostic> {
    finalize_resolved_fixed_topology_ale_fsi(
        model,
        resolved,
        mesh_artifact,
        mesh,
        partition,
        boundary,
        initial,
        harmonic_backend,
    )
}

/// Finalize one exact resolved tetrahedral fixed-topology ALE trajectory input.
#[allow(clippy::too_many_arguments)]
pub fn finalize_resolved_fixed_topology_ale_fsi_3d(
    model: &AleFsiCartesianModel<3>,
    resolved: &ResolvedFixedTopologyAleCoupledRealization,
    mesh_artifact: MeshArtifactReference,
    mesh: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition<3>,
    boundary: &AleFsiBoundary<3>,
    initial: AleFsiInitialPhysicalState<3>,
    harmonic_backend: &dyn LinearSolverBackend,
) -> Result<FinalizedResolvedFixedTopologyAleFsi<3>, Diagnostic> {
    finalize_resolved_fixed_topology_ale_fsi(
        model,
        resolved,
        mesh_artifact,
        mesh,
        partition,
        boundary,
        initial,
        harmonic_backend,
    )
}

/// Conservatively transfer one accepted ALE FSI state to a new mesh revision.
///
/// Source and target must retain the exact canonical meaning and every
/// numerical policy except their authenticated imported-mesh references.  The
/// transition does not advance model time.  After field-aware projection, the
/// target state enters the ordinary finalizer through the same physical-state
/// validator used by a non-remesh start.
///
/// # Errors
/// Rejects stale source state, unchanged or policy-incompatible Realizations,
/// unsupported transfer policy/backend, incomplete common refinement,
/// failed constraints or conservation, bad target geometry, or target-state
/// admission failure before publishing a target executor.
#[allow(clippy::too_many_arguments)]
pub fn remesh_resolved_fixed_topology_ale_fsi_2d(
    model: &AleFsiCartesianModel<2>,
    source: &FinalizedResolvedFixedTopologyAleFsi2d,
    source_state: &AleFsiState<2>,
    target_resolved: &ResolvedFixedTopologyAleCoupledRealization,
    target_mesh_artifact: MeshArtifactReference,
    target_mesh: &SimplicialMesh,
    target_partition: &FixedReferenceFsiPartition<2>,
    target_boundary: &AleFsiBoundary<2>,
    transfer_plan: AleFsiRemeshTransferPlan2d,
    harmonic_backend: &dyn LinearSolverBackend,
    transfer_backend: &dyn LinearSolverBackend,
) -> Result<AcceptedResolvedAleFsiRemesh2d, Diagnostic> {
    source_state.validate_against(&source.reference, &source.partition, &source.motion)?;
    let target = replay_resolved_fixed_topology_ale_fsi(
        model,
        target_resolved,
        target_mesh_artifact,
        target_mesh,
        target_partition,
        target_boundary,
        harmonic_backend,
    )?;
    require_remesh_compatible(source, &target, transfer_plan)?;
    let transfer_scale = numeric_remesh_scale(transfer_plan)?;

    let projection = project_simplicial_ale_fsi_remesh_2d(
        &source.reference,
        &source.partition,
        &source.motion,
        source_state,
        &target.reference,
        &target.partition,
        &target.motion,
        source.step_plan.material(),
        transfer_scale,
        &target.quadrature,
        LinearSolveRequest::new(transfer_backend, transfer_plan.solver()),
    )?;
    let target = target.accept_initial(projection.initial_physical_state()?)?;
    if target.initial.time() != source_state.time()
        || target.initial.geometry() != projection.evidence().target_geometry()
    {
        return Err(invalid_realization(
            "accepted ALE FSI remesh target changed time or harmonic geometry during ordinary state admission",
        ));
    }
    Ok(AcceptedResolvedAleFsiRemesh2d { projection, target })
}

#[allow(clippy::too_many_arguments)]
fn replay_resolved_fixed_topology_ale_fsi<const D: usize>(
    model: &AleFsiCartesianModel<D>,
    resolved: &ResolvedFixedTopologyAleCoupledRealization,
    mesh_artifact: MeshArtifactReference,
    mesh: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition<D>,
    boundary: &AleFsiBoundary<D>,
    harmonic_backend: &dyn LinearSolverBackend,
) -> Result<ReplayedResolvedFixedTopologyAleFsi<D>, Diagnostic> {
    require_supported_dimension::<D>()?;
    require_zero_load(model)?;
    require_boundary_meaning(model)?;
    require_mesh_partition(model, mesh, partition)?;
    if boundary != &AleFsiBoundary::<D>::homogeneous_exterior(mesh)? {
        return Err(invalid_realization(
            "fixed-topology ALE FSI numerical boundary differs from the complete homogeneous canonical exterior closure",
        ));
    }

    let (fields, scale, quadrature) = require_exact_plan(model, resolved, mesh_artifact, mesh)?;
    let plan = resolved.plan();
    let motion_policy = plan.mesh_motion();
    let motion = P1HarmonicMeshMotionAction::<D>::new(
        mesh,
        partition,
        LinearSolveRequest::new(harmonic_backend, motion_policy.solver()),
    )?;
    let material = FixedReferenceFsiMaterial::<D>::new(
        model.fluid().mass_density(),
        model.fluid().dynamic_viscosity(),
        model.solid().mass_density(),
        model.solid().shear_modulus(),
        model.solid().first_lame_parameter(),
    )?;
    let step_plan = AleFsiStepPlan::<D>::new(
        plan.fluid_time_step().duration().value(),
        material,
        scale,
        FixedReferenceFsiLoad::Zero,
        plan.nonlinear(),
        plan.coupled().solver(),
        plan.coupled().target(),
    )?;
    let realization_graph = resolved.portable_graph()?;
    if !matches!(realization_graph.root(), SolveRoot::Nonlinear(_)) {
        return Err(invalid_realization(
            "fixed-topology ALE FSI portable graph must retain one nonlinear solve root",
        ));
    }
    Ok(ReplayedResolvedFixedTopologyAleFsi::<D> {
        model: resolved.model(),
        semantic_revision: resolved.semantic_revision(),
        realization_revision: resolved.realization_revision(),
        mesh_artifact,
        fields,
        plan: plan.clone(),
        realization_graph,
        reference: mesh.clone(),
        partition: partition.clone(),
        boundary: boundary.clone(),
        motion,
        step_plan,
        quadrature,
    })
}

fn require_remesh_compatible(
    source: &FinalizedResolvedFixedTopologyAleFsi2d,
    target: &ReplayedResolvedFixedTopologyAleFsi<2>,
    transfer: AleFsiRemeshTransferPlan2d,
) -> Result<(), Diagnostic> {
    if source.model != target.model
        || source.semantic_revision != target.semantic_revision
        || source.fields != target.fields
        || source.realization_revision == target.realization_revision
        || source.mesh_artifact == target.mesh_artifact
        || !same_ale_policy_except_imported_mesh(&source.plan, &target.plan)
        || numeric_remesh_scale(transfer)? != source.step_plan.scale()
        || transfer.quadrature()
            != target
                .plan
                .coupled()
                .spatial()
                .discretization()
                .quadrature()
    {
        return Err(invalid_realization(
            "ALE FSI remesh requires distinct mesh-bound Realizations with identical canonical meaning and numerical policy except the imported mesh artifact",
        ));
    }
    Ok(())
}

fn numeric_remesh_scale(
    transfer: AleFsiRemeshTransferPlan2d,
) -> Result<FixedReferenceFsiScale<2>, Diagnostic> {
    let scales = transfer.scales();
    FixedReferenceFsiScale::<2>::new(
        scales.length().value(),
        scales.velocity().value(),
        scales.pressure().value(),
    )
}

fn same_ale_policy_except_imported_mesh(
    source: &FixedTopologyAleCoupledRealizationPlan,
    target: &FixedTopologyAleCoupledRealizationPlan,
) -> bool {
    let source_coupled = source.coupled();
    let target_coupled = target.coupled();
    let source_spatial = source_coupled.spatial();
    let target_spatial = target_coupled.spatial();
    let source_discretization = source_spatial.discretization();
    let target_discretization = target_spatial.discretization();

    source.fluid_time_step() == target.fluid_time_step()
        && source.solid_kinematic_relation() == target.solid_kinematic_relation()
        && source.mesh_motion() == target.mesh_motion()
        && source.pullback() == target.pullback()
        && source.nonlinear() == target.nonlinear()
        && source_coupled.time_step() == target_coupled.time_step()
        && source_coupled.scaling() == target_coupled.scaling()
        && source_coupled.operator_properties() == target_coupled.operator_properties()
        && source_coupled.solver() == target_coupled.solver()
        && source_coupled.target() == target_coupled.target()
        && source_coupled.schedule() == target_coupled.schedule()
        && source_spatial.coordinate_length_scale() == target_spatial.coordinate_length_scale()
        && source_spatial.domains() == target_spatial.domains()
        && source_spatial.trace_quotient() == target_spatial.trace_quotient()
        && source_discretization.method() == target_discretization.method()
        && source_discretization.quadrature() == target_discretization.quadrature()
        && matches!(
            (source_discretization.mesh(), target_discretization.mesh()),
            (
                MeshPolicy::ImportedSimplicial { .. },
                MeshPolicy::ImportedSimplicial { .. }
            )
        )
}

/// Finalize and execute through the reference assembly backend.
///
/// # Errors
/// Preserves finalization and numerical execution diagnostics without fallback.
#[allow(clippy::too_many_arguments)]
pub fn solve_resolved_fixed_topology_ale_fsi_2d(
    model: &AleFsiCartesianModel<2>,
    resolved: &ResolvedFixedTopologyAleCoupledRealization,
    mesh_artifact: MeshArtifactReference,
    mesh: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition<2>,
    boundary: &AleFsiBoundary<2>,
    initial: AleFsiInitialPhysicalState<2>,
    step_count: NonZeroStepCount,
    harmonic_backend: &dyn LinearSolverBackend,
    nonlinear_backend: &dyn LinearSolverBackend,
) -> Result<AleFsiTrajectory<2>, Diagnostic> {
    finalize_resolved_fixed_topology_ale_fsi_2d(
        model,
        resolved,
        mesh_artifact,
        mesh,
        partition,
        boundary,
        initial,
        harmonic_backend,
    )?
    .solve(step_count, nonlinear_backend)
}

/// Finalize and execute one tetrahedral trajectory through reference assembly.
///
/// # Errors
/// Preserves finalization and numerical execution diagnostics without fallback.
#[allow(clippy::too_many_arguments)]
pub fn solve_resolved_fixed_topology_ale_fsi_3d(
    model: &AleFsiCartesianModel<3>,
    resolved: &ResolvedFixedTopologyAleCoupledRealization,
    mesh_artifact: MeshArtifactReference,
    mesh: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition<3>,
    boundary: &AleFsiBoundary<3>,
    initial: AleFsiInitialPhysicalState<3>,
    step_count: NonZeroStepCount,
    harmonic_backend: &dyn LinearSolverBackend,
    nonlinear_backend: &dyn LinearSolverBackend,
) -> Result<AleFsiTrajectory<3>, Diagnostic> {
    finalize_resolved_fixed_topology_ale_fsi_3d(
        model,
        resolved,
        mesh_artifact,
        mesh,
        partition,
        boundary,
        initial,
        harmonic_backend,
    )?
    .solve(step_count, nonlinear_backend)
}

/// Finalize and execute through an explicit assembly conformance adapter.
///
/// # Errors
/// Preserves finalization, adapter, and numerical execution diagnostics.
#[doc(hidden)]
#[allow(clippy::too_many_arguments)]
pub fn solve_resolved_fixed_topology_ale_fsi_2d_with_assembly(
    model: &AleFsiCartesianModel<2>,
    resolved: &ResolvedFixedTopologyAleCoupledRealization,
    mesh_artifact: MeshArtifactReference,
    mesh: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition<2>,
    boundary: &AleFsiBoundary<2>,
    initial: AleFsiInitialPhysicalState<2>,
    step_count: NonZeroStepCount,
    harmonic_backend: &dyn LinearSolverBackend,
    assembly: &dyn AssemblyBackend,
    nonlinear_backend: &dyn LinearSolverBackend,
) -> Result<AleFsiTrajectory<2>, Diagnostic> {
    finalize_resolved_fixed_topology_ale_fsi_2d(
        model,
        resolved,
        mesh_artifact,
        mesh,
        partition,
        boundary,
        initial,
        harmonic_backend,
    )?
    .solve_with_assembly(step_count, assembly, nonlinear_backend)
}

/// Finalize and execute one tetrahedral trajectory through an explicit assembly adapter.
///
/// # Errors
/// Preserves finalization, adapter, and numerical execution diagnostics.
#[doc(hidden)]
#[allow(clippy::too_many_arguments)]
pub fn solve_resolved_fixed_topology_ale_fsi_3d_with_assembly(
    model: &AleFsiCartesianModel<3>,
    resolved: &ResolvedFixedTopologyAleCoupledRealization,
    mesh_artifact: MeshArtifactReference,
    mesh: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition<3>,
    boundary: &AleFsiBoundary<3>,
    initial: AleFsiInitialPhysicalState<3>,
    step_count: NonZeroStepCount,
    harmonic_backend: &dyn LinearSolverBackend,
    assembly: &dyn AssemblyBackend,
    nonlinear_backend: &dyn LinearSolverBackend,
) -> Result<AleFsiTrajectory<3>, Diagnostic> {
    finalize_resolved_fixed_topology_ale_fsi_3d(
        model,
        resolved,
        mesh_artifact,
        mesh,
        partition,
        boundary,
        initial,
        harmonic_backend,
    )?
    .solve_with_assembly(step_count, assembly, nonlinear_backend)
}

fn require_supported_dimension<const D: usize>() -> Result<(), Diagnostic> {
    if matches!(D, 2 | 3) {
        Ok(())
    } else {
        Err(invalid_realization(
            "fixed-topology ALE FSI finalization supports dimensions two and three",
        ))
    }
}

fn required_quadrature_policy<const D: usize>() -> Result<QuadraturePolicy, Diagnostic> {
    match D {
        2 => Ok(QuadraturePolicy::TriangleDuffyGaussLegendre {
            points_per_axis: NonZeroUsize::new(LEGACY_TRIANGLE_DUFFY_POINTS_PER_AXIS)
                .expect("five is non-zero"),
        }),
        3 => Ok(QuadraturePolicy::SimplexDuffyGaussLegendre {
            spatial_dimension: NonZeroUsize::new(3).expect("three is non-zero"),
            points_per_axis: NonZeroUsize::new(TETRAHEDRON_DUFFY_POINTS_PER_AXIS)
                .expect("seven is non-zero"),
        }),
        _ => Err(invalid_realization(
            "fixed-topology ALE FSI has no quadrature policy for this dimension",
        )),
    }
}

fn required_quadrature_rule<const D: usize>() -> Result<QuadratureRule, Diagnostic> {
    match D {
        2 => triangle_duffy_gauss_legendre(LEGACY_TRIANGLE_DUFFY_POINTS_PER_AXIS),
        3 => simplex_duffy_gauss_legendre(3, TETRAHEDRON_DUFFY_POINTS_PER_AXIS),
        _ => Err(invalid_realization(
            "fixed-topology ALE FSI has no simplex quadrature rule for this dimension",
        )),
    }
}

fn weak_functional_power<const D: usize>(
    pressure: eqiora_core::DynQuantity,
    velocity: eqiora_core::DynQuantity,
    length: eqiora_core::DynQuantity,
) -> Result<eqiora_core::DynQuantity, Diagnostic> {
    require_supported_dimension::<D>()?;
    let mut power = pressure * velocity;
    for _ in 1..D {
        power = power * length;
    }
    Ok(power)
}

fn require_exact_plan<const D: usize>(
    model: &AleFsiCartesianModel<D>,
    resolved: &ResolvedFixedTopologyAleCoupledRealization,
    mesh_artifact: MeshArtifactReference,
    mesh: &SimplicialMesh,
) -> Result<
    (
        AleFsiFieldIdentities<D>,
        FixedReferenceFsiScale<D>,
        QuadratureRule,
    ),
    Diagnostic,
> {
    if resolved.model() != model.model()
        || resolved.semantic_revision().get() != model.semantic_revision()
    {
        return Err(invalid_realization(
            "resolved fixed-topology ALE Realization does not reference the exact lowered Semantic Model revision",
        ));
    }
    require_supported_dimension::<D>()?;
    let expected_requirements = fixed_topology_ale_fsi_requirements(model);
    if resolved.requirements() != &expected_requirements {
        return Err(invalid_realization(
            "resolved fixed-topology ALE requirements differ from the exact canonical Domain, Field, Relation, Connection, state, or execution roles",
        ));
    }

    let ale = resolved.plan();
    let plan = ale.coupled();
    let spatial = plan.spatial();
    if plan.target()
        != (Target::HostCpu {
            threads: NonZeroUsize::MIN,
        })
        || plan.schedule() != ExecutionSchedule::Offline
        || plan.operator_properties() != LinearOperatorProperties::General
    {
        return Err(invalid_realization(
            "fixed-topology ALE FSI v1 requires one offline serial-host general nonlinear operator",
        ));
    }
    let discretization = spatial.discretization();
    if discretization.method() != DiscretizationMethod::ContinuousGalerkin
        || discretization.mesh()
            != (MeshPolicy::ImportedSimplicial {
                artifact: mesh_artifact,
            })
        || discretization.quadrature() != required_quadrature_policy::<D>()?
    {
        return Err(invalid_realization(
            "fixed-topology ALE FSI requires the exact authenticated mesh digest, continuous Galerkin method, and dimension-appropriate Duffy simplex quadrature",
        ));
    }
    let fields = field_identities(model);
    require_spaces(model, resolved)?;
    require_dimension(
        spatial.coordinate_length_scale().quantity().dim(),
        LENGTH,
        "ALE FSI coordinate scale",
    )?;
    let length = spatial.coordinate_length_scale().quantity();
    let velocity = scale_for(plan, AlgebraicBlock::Field(fields.fluid_velocity))?;
    let pressure = scale_for(plan, AlgebraicBlock::Field(fields.fluid_pressure))?;
    let solid_velocity_scale = scale_for(plan, AlgebraicBlock::Field(fields.solid_velocity))?;
    require_dimension(velocity.dim(), VELOCITY, "ALE FSI velocity scale")?;
    require_dimension(pressure.dim(), PRESSURE, "ALE FSI pressure scale")?;
    if solid_velocity_scale != velocity
        || plan.time_step().eliminated_state().state_scale().quantity() != length
        || plan.scaling().weak_functional_scale().quantity()
            != weak_functional_power::<D>(pressure, velocity, length)?
    {
        return Err(invalid_realization(
            "fixed-topology ALE FSI requires one shared velocity scale, displacement scale L, and derived weak-functional scale P U L^(D - 1)",
        ));
    }
    let quality = ale.mesh_motion().quality_gate().minimum_mean_ratio();
    if mesh.quality_gate().minimum_mean_ratio() != quality {
        return Err(invalid_realization(
            "authenticated ALE reference mesh must carry the exact resolved geometry-quality gate",
        ));
    }
    Ok((
        fields,
        FixedReferenceFsiScale::<D>::new(length.value(), velocity.value(), pressure.value())?,
        required_quadrature_rule::<D>()?,
    ))
}

fn require_spaces<const D: usize>(
    model: &AleFsiCartesianModel<D>,
    resolved: &ResolvedFixedTopologyAleCoupledRealization,
) -> Result<(), Diagnostic> {
    let p1 = Space::continuous_lagrange(NonZeroU16::MIN);
    let expected = [
        (
            fluid_domain(model),
            vec![
                (fluid_velocity(model), Space::simplex_p1_bubble()),
                (fluid_pressure(model), p1),
            ],
        ),
        (solid_domain(model), vec![(solid_velocity(model), p1)]),
    ];
    let expected_domain_count = expected.len();
    let spatial = resolved.plan().coupled().spatial();
    for (domain, mut bindings) in expected {
        bindings.sort_by_key(|(field, _)| field.ulid());
        let Some(actual) = spatial
            .domains()
            .iter()
            .find(|selection| selection.domain() == domain)
        else {
            return Err(invalid_realization(
                "fixed-topology ALE FSI spatial plan omits an exact canonical Domain",
            ));
        };
        let actual_bindings = actual
            .field_spaces()
            .iter()
            .map(|binding| (binding.field(), binding.space()))
            .collect::<Vec<_>>();
        if actual_bindings != bindings || !actual.constraints().is_empty() {
            return Err(invalid_realization(
                "fixed-topology ALE FSI requires exact MINI/P1/P1 spaces without an independent pressure-gauge block",
            ));
        }
    }
    if spatial.domains().len() != expected_domain_count {
        return Err(invalid_realization(
            "fixed-topology ALE FSI spatial plan contains a foreign Domain",
        ));
    }
    Ok(())
}

fn scale_for(
    plan: &eqiora_realization::CoupledFieldwiseRealizationPlan,
    block: AlgebraicBlock,
) -> Result<eqiora_core::DynQuantity, Diagnostic> {
    plan.scaling()
        .block_scales()
        .iter()
        .find(|entry| entry.block() == block)
        .map(|entry| entry.scale().quantity())
        .ok_or_else(|| invalid_realization("fixed-topology ALE FSI omits an exact block scale"))
}

fn require_zero_load<const D: usize>(model: &AleFsiCartesianModel<D>) -> Result<(), Diagnostic> {
    if model.fluid().force_potential_expression().constant_value() != Some(0.0)
        || model.solid().load_potential_expression().constant_value() != Some(0.0)
    {
        return Err(invalid_realization(
            "fixed-topology ALE FSI v1 requires exact zero canonical fluid and solid load potentials",
        ));
    }
    Ok(())
}

fn require_boundary_meaning<const D: usize>(
    model: &AleFsiCartesianModel<D>,
) -> Result<(), Diagnostic> {
    let interface = model.interface();
    require_physics_boundary(
        model.fluid().boundary_inventory(),
        interface.axis(),
        interface.fluid(),
        interface.connection(),
        "fluid",
    )?;
    require_physics_boundary(
        model.solid().boundary_inventory(),
        interface.axis(),
        interface.solid(),
        interface.connection(),
        "solid",
    )
}

fn require_physics_boundary<const D: usize>(
    inventory: &crate::canonical_boundary::CartesianBoundaryInventory<D>,
    interface_axis: usize,
    interface_side: FsiInterfaceSide,
    connection: eqiora_core::RawId,
    physics: &str,
) -> Result<(), Diagnostic> {
    for axis in 0..D {
        for side in [BoundarySide::Lower, BoundarySide::Upper] {
            let entry = inventory.boundary(axis, side).ok_or_else(|| {
                invalid_realization(format!(
                    "fixed-topology ALE FSI {physics} boundary inventory omits axis {axis} {side:?}"
                ))
            })?;
            if axis == interface_axis && side == interface_side.side() {
                if entry.boundary() != interface_side.boundary()
                    || entry.disposition()
                        != (PhysicalBoundaryDisposition::PortBinding {
                            connection,
                            port: interface_side.port(),
                        })
                {
                    return Err(invalid_realization(format!(
                        "fixed-topology ALE FSI {physics} interface boundary identity or live Port binding drifted"
                    )));
                }
            } else if entry.disposition() != PhysicalBoundaryDisposition::TraceZero {
                return Err(invalid_realization(format!(
                    "fixed-topology ALE FSI v1 requires TraceZero on every exterior {physics} side"
                )));
            }
        }
    }
    Ok(())
}

fn require_mesh_partition<const D: usize>(
    model: &AleFsiCartesianModel<D>,
    mesh: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition<D>,
) -> Result<(), Diagnostic> {
    require_supported_dimension::<D>()?;
    if mesh.topological_dimension() != D || mesh.vertices().iter().any(|point| point.len() != D) {
        return Err(invalid_realization(format!(
            "fixed-topology ALE FSI canonical bridge requires one intrinsic {D}D mesh"
        )));
    }
    let replayed = FixedReferenceFsiPartition::<D>::new(
        mesh,
        partition.fluid_cells().to_vec(),
        partition.solid_cells().to_vec(),
        partition.interface_facets().to_vec(),
    )?;
    if &replayed != partition {
        return Err(invalid_realization(
            "fixed-topology ALE FSI partition cache differs from exact mesh replay",
        ));
    }
    require_cells_in_bounds(
        mesh,
        partition.fluid_cells(),
        model.fluid().bounds(),
        "fluid",
    )?;
    require_cells_in_bounds(
        mesh,
        partition.solid_cells(),
        model.solid().bounds(),
        "solid",
    )?;

    let interface = model.interface();
    let interface_coordinate = match interface.fluid().side() {
        BoundarySide::Lower => model.fluid().bounds()[interface.axis()][0],
        BoundarySide::Upper => model.fluid().bounds()[interface.axis()][1],
    };
    for facet in partition.interface_facets() {
        let vertices = mesh
            .entity_vertices(MeshEntity::new(D - 1, facet.index()))
            .ok_or_else(|| invalid_realization("ALE FSI interface facet is outside the mesh"))?;
        if vertices
            .iter()
            .any(|vertex| mesh.vertices()[vertex.index()][interface.axis()] != interface_coordinate)
        {
            return Err(invalid_realization(
                "fixed-topology ALE FSI partition interface does not lie on the exact semantic interface",
            ));
        }
    }

    let fluid_cells = partition
        .fluid_cells()
        .iter()
        .map(|cell| cell.index())
        .collect::<BTreeSet<_>>();
    let mut fluid_coverage = [[false; 2]; D];
    let mut solid_coverage = [[false; 2]; D];
    let facet_count = mesh
        .entity_count(D - 1)
        .ok_or_else(|| invalid_realization("ALE FSI mesh omits its facet stratum"))?;
    for facet_index in 0..facet_count {
        let facet = MeshEntity::new(D - 1, facet_index);
        if !mesh
            .is_boundary_entity(facet)
            .ok_or_else(|| invalid_realization("ALE FSI facet is outside the mesh"))?
        {
            continue;
        }
        let adjacent = mesh
            .incidence(facet, D)
            .ok_or_else(|| invalid_realization("ALE FSI exterior facet has no cell incidence"))?;
        let [cell] = adjacent.as_slice() else {
            return Err(invalid_realization(
                "ALE FSI exterior facet must own exactly one adjacent cell",
            ));
        };
        let (bounds, coverage, interface_side) = if fluid_cells.contains(&cell.entity.index()) {
            (
                model.fluid().bounds(),
                &mut fluid_coverage,
                interface.fluid().side(),
            )
        } else {
            (
                model.solid().bounds(),
                &mut solid_coverage,
                interface.solid().side(),
            )
        };
        let vertices = mesh
            .entity_vertices(facet)
            .ok_or_else(|| invalid_realization("ALE FSI exterior facet has no vertex closure"))?;
        let mut matched = None;
        for (axis, axis_bounds) in bounds.iter().enumerate() {
            for (side_index, bound) in axis_bounds.iter().enumerate() {
                if vertices
                    .iter()
                    .all(|vertex| mesh.vertices()[vertex.index()][axis] == *bound)
                {
                    if matched.is_some() {
                        return Err(invalid_realization(
                            "ALE FSI exterior facet ambiguously belongs to multiple semantic sides",
                        ));
                    }
                    matched = Some((axis, side_index));
                }
            }
        }
        let Some((axis, side_index)) = matched else {
            return Err(invalid_realization(
                "ALE FSI mesh exterior does not lie on an exact semantic side",
            ));
        };
        let side = if side_index == 0 {
            BoundarySide::Lower
        } else {
            BoundarySide::Upper
        };
        if axis == interface.axis() && side == interface_side {
            return Err(invalid_realization(
                "ALE FSI semantic interface appeared on the mesh exterior",
            ));
        }
        coverage[axis][side_index] = true;
    }
    require_exterior_coverage(
        fluid_coverage,
        interface.axis(),
        interface.fluid().side(),
        "fluid",
    )?;
    require_exterior_coverage(
        solid_coverage,
        interface.axis(),
        interface.solid().side(),
        "solid",
    )
}

fn require_cells_in_bounds<const D: usize>(
    mesh: &SimplicialMesh,
    cells: &[CellId],
    bounds: &[[f64; 2]; D],
    physics: &str,
) -> Result<(), Diagnostic> {
    for cell in cells {
        let vertices = mesh
            .entity_vertices(MeshEntity::new(D, cell.index()))
            .ok_or_else(|| {
                invalid_realization(format!(
                    "fixed-topology ALE FSI {physics} cell is outside the mesh"
                ))
            })?;
        if vertices.iter().any(|vertex| {
            mesh.vertices()[vertex.index()]
                .iter()
                .enumerate()
                .any(|(axis, value)| *value < bounds[axis][0] || *value > bounds[axis][1])
        }) {
            return Err(invalid_realization(format!(
                "fixed-topology ALE FSI {physics} cell lies outside its exact semantic Domain"
            )));
        }
    }
    Ok(())
}

fn require_exterior_coverage<const D: usize>(
    coverage: [[bool; 2]; D],
    interface_axis: usize,
    interface_side: BoundarySide,
    physics: &str,
) -> Result<(), Diagnostic> {
    for (axis, sides) in coverage.iter().enumerate() {
        for (side_index, covered) in sides.iter().enumerate() {
            let side = if side_index == 0 {
                BoundarySide::Lower
            } else {
                BoundarySide::Upper
            };
            if !(*covered || axis == interface_axis && side == interface_side) {
                return Err(invalid_realization(format!(
                    "ALE FSI mesh does not cover the exact exterior {physics} side on axis {axis} {side:?}"
                )));
            }
        }
    }
    Ok(())
}

fn require_dimension(
    actual: DimExponents,
    expected: DimExponents,
    label: &str,
) -> Result<(), Diagnostic> {
    if actual != expected {
        return Err(invalid_realization(format!(
            "{label} has incompatible physical dimension {actual:?}"
        )));
    }
    Ok(())
}

fn field_identities<const D: usize>(model: &AleFsiCartesianModel<D>) -> AleFsiFieldIdentities<D> {
    AleFsiFieldIdentities::<D> {
        fluid_velocity: fluid_velocity(model),
        fluid_pressure: fluid_pressure(model),
        solid_velocity: solid_velocity(model),
        solid_displacement: solid_displacement(model),
    }
}

fn trace_quotient<const D: usize>(model: &AleFsiCartesianModel<D>) -> ConformingTraceQuotient {
    ConformingTraceQuotient::new(
        connection(model),
        TraceFieldEndpoint::new(fluid_domain(model), fluid_velocity(model)),
        TraceFieldEndpoint::new(solid_domain(model), solid_velocity(model)),
    )
    .expect("lowered ALE FSI interface joins distinct Domains")
}

fn state_pair<const D: usize>(model: &AleFsiCartesianModel<D>) -> BackwardEulerStatePair {
    BackwardEulerStatePair::new(solid_displacement(model), solid_velocity(model))
        .expect("lowered ALE FSI solid state and rate are distinct")
}

fn fluid_domain<const D: usize>(model: &AleFsiCartesianModel<D>) -> Id<kinds::Domain> {
    model
        .fluid()
        .domain()
        .downcast()
        .expect("lowered ALE fluid Domain retains its kind")
}

fn solid_domain<const D: usize>(model: &AleFsiCartesianModel<D>) -> Id<kinds::Domain> {
    model
        .solid()
        .domain()
        .downcast()
        .expect("lowered ALE solid Domain retains its kind")
}

fn fluid_velocity<const D: usize>(model: &AleFsiCartesianModel<D>) -> Id<kinds::Field> {
    model
        .fluid()
        .velocity()
        .downcast()
        .expect("lowered ALE fluid velocity retains its kind")
}

fn fluid_pressure<const D: usize>(model: &AleFsiCartesianModel<D>) -> Id<kinds::Field> {
    model
        .fluid()
        .pressure()
        .downcast()
        .expect("lowered ALE pressure retains its kind")
}

fn solid_velocity<const D: usize>(model: &AleFsiCartesianModel<D>) -> Id<kinds::Field> {
    model
        .solid()
        .velocity()
        .downcast()
        .expect("lowered ALE solid velocity retains its kind")
}

fn solid_displacement<const D: usize>(model: &AleFsiCartesianModel<D>) -> Id<kinds::Field> {
    model
        .solid()
        .displacement()
        .downcast()
        .expect("lowered ALE solid displacement retains its kind")
}

fn fluid_relation<const D: usize>(model: &AleFsiCartesianModel<D>) -> Id<kinds::Relation> {
    model
        .fluid()
        .momentum_relation()
        .downcast()
        .expect("lowered ALE fluid momentum retains its kind")
}

fn solid_kinematic_relation<const D: usize>(
    model: &AleFsiCartesianModel<D>,
) -> Id<kinds::Relation> {
    model
        .solid()
        .kinematic_relation()
        .downcast()
        .expect("lowered ALE solid kinematics retains its kind")
}

fn connection<const D: usize>(model: &AleFsiCartesianModel<D>) -> Id<kinds::Connection> {
    model
        .interface()
        .connection()
        .downcast()
        .expect("lowered ALE FSI Connection retains its kind")
}

fn invalid_realization(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}

#[cfg(test)]
mod tests {
    use std::num::{NonZeroU16, NonZeroUsize};

    use eqiora_compiler::compile;
    use eqiora_core::{Diagnostic, DimExponents, DynQuantity};
    use eqiora_graph::{GraphStore, InMemoryGraphStore};
    use eqiora_meshing::{FacetId, MeshQualityGate};
    use eqiora_realization::{
        AleGeometryQualityGate, AlgebraicBlockScale, BackwardEulerRelationStep,
        BackwardEulerStateBinding, BackwardEulerStep, CoupledFieldwiseRealizationPlan,
        CoupledFieldwiseSpatialDiscretization, Discretization, DomainFieldDiscretization,
        FieldSpaceBinding, FixedTopologyAleCoupledRealizationPlan,
        FixedTopologyAleCoupledRealizationRequest, GclCompatibleAlePullback, MeshKind,
        NonlinearSolvePlan, P1HarmonicMeshMotionPolicy, PositivePhysicalScale,
        RealizationCapabilities, RealizationRevision, SemanticRevision, Space,
        SpatialDimensionSupport, SymmetricCongruenceScaling, TargetCapabilities,
        resolve_fixed_topology_ale_coupled,
    };
    use eqiora_sem::KernelProgram;
    use eqiora_solver::{
        BackendId, LinearProblem, LinearSolution, LinearSolver, PreconditionerPolicy,
        REFERENCE_LINEAR_SOLVER, ReductionPolicy, ReplicatedLinearExecution, SolverCapabilities,
        SolverCapability, SolverPlan, SolverProvider,
    };

    use super::*;
    use crate::{AleFsiBoundary2d, AleFsiCartesianModel2d, FixedReferenceFsiPartition2d};

    const BASE_SOURCE: &str =
        include_str!("../../../../verify/fsi/fixed-reference-monolithic-step-2d/models/direct.eqi");
    const TIME: DimExponents = DimExponents {
        time: 1,
        ..DimExponents::DIMENSIONLESS
    };
    #[test]
    fn exact_resolved_plan_finalizes_and_accepts_a_trajectory() {
        let fixture = Fixture::new();
        let finalized = fixture.finalize(fixture.initial()).unwrap();
        assert_eq!(finalized.model(), fixture.model.model());
        assert_eq!(finalized.mesh_artifact(), mesh_reference());
        assert_eq!(finalized.fields(), field_identities(&fixture.model));
        assert_eq!(
            finalized.initial_state().geometry().coordinates(),
            fixture.mesh.vertices()
        );
        assert!(!finalized.motion().influence_solve_reports().is_empty());

        let trajectory = finalized
            .solve(
                NonZeroStepCount::new(NonZeroUsize::new(2).unwrap()),
                &NoSolveGeneralBackend,
            )
            .expect("zero equilibrium accepts without invoking a Newton linear solve");
        assert_eq!(trajectory.states().len(), 3);
        assert_eq!(trajectory.steps().len(), 2);
        assert!(
            trajectory
                .steps()
                .iter()
                .all(|step| step.nonlinear_iterations() == 0)
        );
    }

    #[test]
    fn direct_tetrahedral_model_reaches_the_same_finalized_newton_boundary() {
        let program = compile_program(&ale_source_3d());
        let model = super::super::lower_ale_fsi_cartesian_3d(&program).unwrap();
        let mesh = mesh_3d(MeshQualityGate::new(0.1).unwrap());
        let (fluid, solid, interface) = inventories_3d(&mesh);
        let partition = FixedReferenceFsiPartition::<3>::new(
            &mesh,
            fluid.clone(),
            solid.clone(),
            interface.clone(),
        )
        .unwrap();
        let boundary = AleFsiBoundary::<3>::homogeneous_exterior(&mesh).unwrap();
        assert!(AleFsiBoundary::<2>::homogeneous_exterior(&mesh).is_err());
        assert!(
            FixedReferenceFsiPartition::<3>::new(
                &mesh,
                fluid,
                solid,
                interface[..interface.len() - 1].to_vec(),
            )
            .is_err()
        );
        let requirements = fixed_topology_ale_fsi_requirements_3d(&model);
        assert_eq!(
            requirements.coupled().execution().spatial_dimension().get(),
            3
        );
        let plan = build_plan(&model, fluid_pressure(&model), 0.1);
        assert_eq!(
            plan.coupled().spatial().discretization().quadrature(),
            required_quadrature_policy::<3>().unwrap()
        );
        let resolved = resolve(
            &program,
            plan,
            requirements,
            SemanticRevision::new(model.semantic_revision()),
            3,
        );

        let mut fixed_velocity = vec![[0.0; 3]; mesh.vertices().len()];
        let fixed = boundary.fixed_zero_velocity_vertices()[0];
        fixed_velocity[fixed.index()][2] = 1.0;
        let invalid = AleFsiInitialPhysicalState3d::new(
            0.0,
            fixed_velocity,
            vec![[0.0; 3]; partition.fluid_cells().len()],
            vec![0.0; partition.fluid_vertices().len()],
            vec![[0.0; 3]; mesh.vertices().len()],
        )
        .unwrap();
        assert!(
            finalize_resolved_fixed_topology_ale_fsi_3d(
                &model,
                &resolved,
                mesh_reference(),
                &mesh,
                &partition,
                &boundary,
                invalid,
                &REFERENCE_LINEAR_SOLVER,
            )
            .unwrap_err()
            .message()
            .contains("homogeneous exterior closure")
        );

        let short_mesh = mesh_3d_with_upper_z(0.9, MeshQualityGate::new(0.1).unwrap());
        let (fluid, solid, interface) = inventories_3d(&short_mesh);
        let short_partition =
            FixedReferenceFsiPartition::<3>::new(&short_mesh, fluid, solid, interface).unwrap();
        let short_boundary = AleFsiBoundary::<3>::homogeneous_exterior(&short_mesh).unwrap();
        assert!(
            finalize_resolved_fixed_topology_ale_fsi_3d(
                &model,
                &resolved,
                mesh_reference(),
                &short_mesh,
                &short_partition,
                &short_boundary,
                initial_for_3d(&short_mesh, &short_partition),
                &REFERENCE_LINEAR_SOLVER,
            )
            .unwrap_err()
            .message()
            .contains("mesh exterior does not lie on an exact semantic side")
        );

        let finalized: FinalizedResolvedFixedTopologyAleFsi3d =
            finalize_resolved_fixed_topology_ale_fsi_3d(
                &model,
                &resolved,
                mesh_reference(),
                &mesh,
                &partition,
                &boundary,
                initial_for_3d(&mesh, &partition),
                &REFERENCE_LINEAR_SOLVER,
            )
            .unwrap();
        let expected_fields: AleFsiFieldIdentities3d = field_identities(&model);
        assert_eq!(finalized.fields(), expected_fields);
        assert_eq!(finalized.step_plan().scale().power(), 4.0);
        assert_eq!(
            finalized.initial_state().geometry().coordinates(),
            mesh.vertices()
        );
        let trajectory = finalized
            .solve(
                NonZeroStepCount::new(NonZeroUsize::MIN),
                &NoSolveGeneralBackend,
            )
            .unwrap();
        assert_eq!(trajectory.states().len(), 2);
        assert_eq!(trajectory.steps().len(), 1);
        assert_eq!(trajectory.steps()[0].nonlinear_iterations(), 0);
    }

    #[test]
    fn finalization_rejects_stale_model_revision_mesh_and_role() {
        let fixture = Fixture::new();
        let foreign_source = ale_source().replace(
            "parameter fluid_density: kg / m ^ 3 = 2;",
            "parameter fluid_density: kg / m ^ 3 = 2.5;",
        );
        let foreign = Fixture::from_source(&foreign_source);
        assert!(
            finalize_resolved_fixed_topology_ale_fsi_2d(
                &fixture.model,
                &foreign.resolved,
                mesh_reference(),
                &fixture.mesh,
                &fixture.partition,
                &fixture.boundary,
                fixture.initial(),
                &REFERENCE_LINEAR_SOLVER,
            )
            .unwrap_err()
            .message()
            .contains("exact lowered Semantic Model revision")
        );

        let stale_revision = fixture.resolve_with(
            fixture.plan(fluid_pressure(&fixture.model)),
            fixed_topology_ale_fsi_requirements_2d(&fixture.model),
            SemanticRevision::new(fixture.model.semantic_revision() + 1),
        );
        assert!(
            fixture
                .finalize_resolved(&stale_revision, mesh_reference(), fixture.initial())
                .unwrap_err()
                .message()
                .contains("exact lowered Semantic Model revision")
        );

        let mut stale = mesh_reference().sha256();
        stale[0] ^= 1;
        assert!(
            fixture
                .finalize_resolved(
                    &fixture.resolved,
                    MeshArtifactReference::from_sha256(stale),
                    fixture.initial(),
                )
                .unwrap_err()
                .message()
                .contains("authenticated mesh digest")
        );

        let foreign_pressure = Id::new();
        let role_plan = fixture.plan(foreign_pressure);
        let role_requirements = requirements_with_pressure(&fixture.model, foreign_pressure);
        let role_resolved = fixture.resolve_with(
            role_plan,
            role_requirements,
            SemanticRevision::new(fixture.model.semantic_revision()),
        );
        assert!(
            fixture
                .finalize_resolved(&role_resolved, mesh_reference(), fixture.initial())
                .unwrap_err()
                .message()
                .contains("exact canonical Domain")
        );
    }

    #[test]
    fn finalization_rejects_quality_and_initial_boundary_drift() {
        let fixture = Fixture::new();
        let lower_quality_mesh = mesh(MeshQualityGate::new(0.2).unwrap());
        let (fluid, solid, interface) = inventories(&lower_quality_mesh);
        let partition =
            FixedReferenceFsiPartition2d::new(&lower_quality_mesh, fluid, solid, interface)
                .unwrap();
        let boundary = AleFsiBoundary2d::homogeneous_exterior(&lower_quality_mesh).unwrap();
        assert!(
            finalize_resolved_fixed_topology_ale_fsi_2d(
                &fixture.model,
                &fixture.resolved,
                mesh_reference(),
                &lower_quality_mesh,
                &partition,
                &boundary,
                initial_for(&lower_quality_mesh, &partition),
                &REFERENCE_LINEAR_SOLVER,
            )
            .unwrap_err()
            .message()
            .contains("exact resolved geometry-quality gate")
        );

        let mut velocity = vec![[0.0; 2]; fixture.mesh.vertices().len()];
        velocity[0] = [1.0, 0.0];
        let initial = AleFsiInitialPhysicalState2d::new(
            0.0,
            velocity,
            vec![[0.0; 2]; fixture.partition.fluid_cells().len()],
            vec![0.0; fixture.partition.fluid_vertices().len()],
            vec![[0.0; 2]; fixture.mesh.vertices().len()],
        )
        .unwrap();
        assert!(
            fixture
                .finalize(initial)
                .unwrap_err()
                .message()
                .contains("homogeneous exterior closure")
        );
    }

    struct Fixture {
        program: KernelProgram,
        model: AleFsiCartesianModel2d,
        mesh: SimplicialMesh,
        partition: FixedReferenceFsiPartition2d,
        boundary: AleFsiBoundary2d,
        resolved: ResolvedFixedTopologyAleCoupledRealization,
    }

    impl Fixture {
        fn new() -> Self {
            Self::from_source(&ale_source())
        }

        fn from_source(source: &str) -> Self {
            let program = compile_program(source);
            let model = super::super::lower_ale_fsi_cartesian_2d(&program).unwrap();
            let mesh = mesh(MeshQualityGate::new(0.3).unwrap());
            let (fluid, solid, interface) = inventories(&mesh);
            let partition =
                FixedReferenceFsiPartition2d::new(&mesh, fluid, solid, interface).unwrap();
            let boundary = AleFsiBoundary2d::homogeneous_exterior(&mesh).unwrap();
            let plan = Self::build_plan(&model, fluid_pressure(&model), 0.3);
            let resolved = resolve(
                &program,
                plan,
                fixed_topology_ale_fsi_requirements_2d(&model),
                SemanticRevision::new(model.semantic_revision()),
                2,
            );
            Self {
                program,
                model,
                mesh,
                partition,
                boundary,
                resolved,
            }
        }

        fn initial(&self) -> AleFsiInitialPhysicalState2d {
            initial_for(&self.mesh, &self.partition)
        }

        fn plan(&self, pressure: Id<kinds::Field>) -> FixedTopologyAleCoupledRealizationPlan {
            Self::build_plan(&self.model, pressure, 0.3)
        }

        fn build_plan(
            model: &AleFsiCartesianModel2d,
            pressure: Id<kinds::Field>,
            minimum_mean_ratio: f64,
        ) -> FixedTopologyAleCoupledRealizationPlan {
            build_plan(model, pressure, minimum_mean_ratio)
        }

        fn resolve_with(
            &self,
            plan: FixedTopologyAleCoupledRealizationPlan,
            requirements: FixedTopologyAleCoupledRealizationRequirements,
            semantic_revision: SemanticRevision,
        ) -> ResolvedFixedTopologyAleCoupledRealization {
            resolve(&self.program, plan, requirements, semantic_revision, 2)
        }

        fn finalize(
            &self,
            initial: AleFsiInitialPhysicalState2d,
        ) -> Result<FinalizedResolvedFixedTopologyAleFsi2d, Diagnostic> {
            self.finalize_resolved(&self.resolved, mesh_reference(), initial)
        }

        fn finalize_resolved(
            &self,
            resolved: &ResolvedFixedTopologyAleCoupledRealization,
            mesh_artifact: MeshArtifactReference,
            initial: AleFsiInitialPhysicalState2d,
        ) -> Result<FinalizedResolvedFixedTopologyAleFsi2d, Diagnostic> {
            finalize_resolved_fixed_topology_ale_fsi_2d(
                &self.model,
                resolved,
                mesh_artifact,
                &self.mesh,
                &self.partition,
                &self.boundary,
                initial,
                &REFERENCE_LINEAR_SOLVER,
            )
        }
    }

    fn build_plan<const D: usize>(
        model: &AleFsiCartesianModel<D>,
        pressure: Id<kinds::Field>,
        minimum_mean_ratio: f64,
    ) -> FixedTopologyAleCoupledRealizationPlan {
        let p1 = Space::continuous_lagrange(NonZeroU16::MIN);
        let length = physical_scale(2.0, LENGTH);
        let velocity = physical_scale(1.0, VELOCITY);
        let pressure_scale = physical_scale(1.0, PRESSURE);
        let coupled = CoupledFieldwiseRealizationPlan::new(
            CoupledFieldwiseSpatialDiscretization::new(
                length,
                [
                    DomainFieldDiscretization::new(
                        fluid_domain(model),
                        [
                            FieldSpaceBinding::new(
                                fluid_velocity(model),
                                Space::simplex_p1_bubble(),
                            ),
                            FieldSpaceBinding::new(pressure, p1),
                        ],
                        [],
                    )
                    .unwrap(),
                    DomainFieldDiscretization::new(
                        solid_domain(model),
                        [FieldSpaceBinding::new(solid_velocity(model), p1)],
                        [],
                    )
                    .unwrap(),
                ],
                trace_quotient(model),
                Discretization::new(
                    DiscretizationMethod::ContinuousGalerkin,
                    MeshPolicy::ImportedSimplicial {
                        artifact: mesh_reference(),
                    },
                    required_quadrature_policy::<D>().unwrap(),
                ),
            )
            .unwrap(),
            BackwardEulerStep::new(
                DynQuantity::new(0.02, TIME),
                BackwardEulerStateBinding::new(state_pair(model), p1, length),
            )
            .unwrap(),
            SymmetricCongruenceScaling::new(
                [
                    AlgebraicBlockScale::new(
                        AlgebraicBlock::Field(fluid_velocity(model)),
                        velocity,
                    ),
                    AlgebraicBlockScale::new(AlgebraicBlock::Field(pressure), pressure_scale),
                    AlgebraicBlockScale::new(
                        AlgebraicBlock::Field(solid_velocity(model)),
                        velocity,
                    ),
                ],
                PositivePhysicalScale::new(
                    weak_functional_power::<D>(
                        pressure_scale.quantity(),
                        velocity.quantity(),
                        length.quantity(),
                    )
                    .unwrap(),
                )
                .unwrap(),
            )
            .unwrap(),
            LinearOperatorProperties::General,
            nonlinear_solver(),
            Target::HostCpu {
                threads: NonZeroUsize::MIN,
            },
            ExecutionSchedule::Offline,
        )
        .unwrap();
        let duration = DynQuantity::new(0.02, TIME);
        FixedTopologyAleCoupledRealizationPlan::new(
            coupled,
            BackwardEulerRelationStep::new(fluid_relation(model), fluid_velocity(model), duration)
                .unwrap(),
            solid_kinematic_relation(model),
            P1HarmonicMeshMotionPolicy::new(
                fluid_domain(model),
                solid_domain(model),
                solid_displacement(model),
                connection(model),
                AleGeometryQualityGate::new(minimum_mean_ratio).unwrap(),
                harmonic_solver(),
            )
            .unwrap(),
            GclCompatibleAlePullback::new(fluid_relation(model), fluid_velocity(model)),
            NonlinearSolvePlan::new(1.0e-7, 1.0e-10, NonZeroUsize::new(20).unwrap(), 16).unwrap(),
        )
        .unwrap()
    }

    fn resolve(
        program: &KernelProgram,
        plan: FixedTopologyAleCoupledRealizationPlan,
        requirements: FixedTopologyAleCoupledRealizationRequirements,
        semantic_revision: SemanticRevision,
        dimension: usize,
    ) -> ResolvedFixedTopologyAleCoupledRealization {
        resolve_fixed_topology_ale_coupled(
            &FixedTopologyAleCoupledRealizationRequest::explicit(
                program.model(),
                semantic_revision,
                RealizationRevision::new(3),
                plan,
            ),
            requirements,
            &capabilities(dimension),
        )
        .unwrap()
    }

    fn requirements_with_pressure(
        model: &AleFsiCartesianModel2d,
        pressure: Id<kinds::Field>,
    ) -> FixedTopologyAleCoupledRealizationRequirements {
        let requirements = fixed_topology_ale_fsi_requirements_2d(model);
        let coupled = CoupledFieldwiseRealizationRequirements::new(
            [
                DomainFieldInventory::new(fluid_domain(model), [fluid_velocity(model), pressure])
                    .unwrap(),
                DomainFieldInventory::new(
                    solid_domain(model),
                    [solid_velocity(model), solid_displacement(model)],
                )
                .unwrap(),
            ],
            requirements.coupled().trace_quotient(),
            requirements.coupled().eliminated_state(),
            requirements.coupled().execution(),
        )
        .unwrap();
        FixedTopologyAleCoupledRealizationRequirements::new(
            coupled,
            fluid_domain(model),
            solid_domain(model),
            fluid_relation(model),
            solid_kinematic_relation(model),
            fluid_velocity(model),
            solid_displacement(model),
        )
        .unwrap()
    }

    fn capabilities(dimension: usize) -> RealizationCapabilities {
        RealizationCapabilities::cartesian_product(
            [DiscretizationMethod::ContinuousGalerkin],
            [(
                MeshKind::ImportedAffineSimplicial,
                SpatialDimensionSupport::exact(NonZeroUsize::new(dimension).unwrap()),
            )],
            [VectorLayoutKind::Replicated],
            SolverCapabilities::exact([
                SolverCapability {
                    algorithm: LinearSolver::BiConjugateGradientStabilized,
                    operator_properties: LinearOperatorProperties::General,
                    preconditioner: PreconditionerPolicy::Identity,
                    reduction: ReductionPolicy::Reproducible,
                    scalar_type: ScalarType::F64,
                },
                SolverCapability {
                    algorithm: LinearSolver::ConjugateGradient,
                    operator_properties: LinearOperatorProperties::SymmetricPositiveDefinite,
                    preconditioner: PreconditionerPolicy::Identity,
                    reduction: ReductionPolicy::Reproducible,
                    scalar_type: ScalarType::F64,
                },
            ])
            .unwrap(),
            TargetCapabilities::none().with_host_cpu(NonZeroUsize::MIN),
        )
        .unwrap()
    }

    fn initial_for(
        mesh: &SimplicialMesh,
        partition: &FixedReferenceFsiPartition2d,
    ) -> AleFsiInitialPhysicalState2d {
        AleFsiInitialPhysicalState2d::new(
            0.0,
            vec![[0.0; 2]; mesh.vertices().len()],
            vec![[0.0; 2]; partition.fluid_cells().len()],
            vec![0.0; partition.fluid_vertices().len()],
            vec![[0.0; 2]; mesh.vertices().len()],
        )
        .unwrap()
    }

    fn initial_for_3d(
        mesh: &SimplicialMesh,
        partition: &FixedReferenceFsiPartition<3>,
    ) -> AleFsiInitialPhysicalState3d {
        AleFsiInitialPhysicalState3d::new(
            0.0,
            vec![[0.0; 3]; mesh.vertices().len()],
            vec![[0.0; 3]; partition.fluid_cells().len()],
            vec![0.0; partition.fluid_vertices().len()],
            vec![[0.0; 3]; mesh.vertices().len()],
        )
        .unwrap()
    }

    fn mesh(quality: MeshQualityGate) -> SimplicialMesh {
        let xs = [0.0, 0.5, 1.0, 1.5, 2.0];
        let mut vertices = Vec::new();
        for y in [0.0, 0.5, 1.0] {
            for x in xs {
                vertices.push(vec![x, y]);
            }
        }
        let width = xs.len();
        let mut cells = Vec::new();
        for row in 0..2 {
            for column in 0..width - 1 {
                let lower_left = row * width + column;
                let lower_right = lower_left + 1;
                let upper_left = lower_left + width;
                let upper_right = upper_left + 1;
                cells.push(vec![lower_left, lower_right, upper_right]);
                cells.push(vec![lower_left, upper_right, upper_left]);
            }
        }
        SimplicialMesh::new(2, vertices, cells, quality).unwrap()
    }

    fn inventories(mesh: &SimplicialMesh) -> (Vec<CellId>, Vec<CellId>, Vec<FacetId>) {
        let mut fluid = Vec::new();
        let mut solid = Vec::new();
        for (index, cell) in mesh.cells().iter().enumerate() {
            let centroid_x = cell
                .iter()
                .map(|vertex| mesh.vertices()[*vertex][0])
                .sum::<f64>()
                / 3.0;
            if centroid_x < 1.0 {
                fluid.push(CellId::new(index));
            } else {
                solid.push(CellId::new(index));
            }
        }
        let interface = (0..mesh.entity_count(1).unwrap())
            .filter(|&facet| {
                mesh.entity_vertices(MeshEntity::new(1, facet))
                    .unwrap()
                    .iter()
                    .all(|vertex| mesh.vertices()[vertex.index()][0] == 1.0)
            })
            .map(FacetId::new)
            .collect();
        (fluid, solid, interface)
    }

    fn mesh_3d(quality: MeshQualityGate) -> SimplicialMesh {
        mesh_3d_with_upper_z(1.0, quality)
    }

    fn mesh_3d_with_upper_z(upper_z: f64, quality: MeshQualityGate) -> SimplicialMesh {
        let vertices = vec![
            vec![0.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 1.0, upper_z],
            vec![0.0, 0.0, upper_z],
            vec![1.0, 0.0, 0.0],
            vec![1.0, 1.0, 0.0],
            vec![1.0, 1.0, upper_z],
            vec![1.0, 0.0, upper_z],
            vec![2.0, 0.0, 0.0],
            vec![2.0, 1.0, 0.0],
            vec![2.0, 1.0, upper_z],
            vec![2.0, 0.0, upper_z],
            vec![0.5, 0.5, upper_z / 2.0],
            vec![1.0, 0.5, upper_z / 2.0],
            vec![1.5, 0.5, upper_z / 2.0],
        ];
        let interface = [[4, 5, 13], [5, 6, 13], [6, 7, 13], [7, 4, 13]];
        let fluid_surface = [
            [0, 3, 2],
            [0, 2, 1],
            [0, 4, 7],
            [0, 7, 3],
            [1, 2, 6],
            [1, 6, 5],
            [0, 1, 5],
            [0, 5, 4],
            [3, 7, 6],
            [3, 6, 2],
        ];
        let solid_surface = [
            [8, 9, 10],
            [8, 10, 11],
            [4, 8, 11],
            [4, 11, 7],
            [5, 6, 10],
            [5, 10, 9],
            [4, 5, 9],
            [4, 9, 8],
            [7, 11, 10],
            [7, 10, 6],
        ];
        let mut cells = fluid_surface
            .into_iter()
            .chain(interface)
            .map(|face| vec![12, face[0], face[1], face[2]])
            .chain(
                solid_surface
                    .into_iter()
                    .chain(interface)
                    .map(|face| vec![14, face[0], face[1], face[2]]),
            )
            .collect::<Vec<_>>();
        for cell in &mut cells {
            if signed_tetrahedron_measure(&vertices, cell) < 0.0 {
                cell.swap(1, 2);
            }
        }
        SimplicialMesh::new(3, vertices, cells, quality).unwrap()
    }

    fn inventories_3d(mesh: &SimplicialMesh) -> (Vec<CellId>, Vec<CellId>, Vec<FacetId>) {
        let fluid = (0..14).map(CellId::new).collect();
        let solid = (14..28).map(CellId::new).collect();
        let interface = (0..mesh.entity_count(2).unwrap())
            .filter(|&facet| {
                mesh.entity_vertices(MeshEntity::new(2, facet))
                    .unwrap()
                    .iter()
                    .all(|vertex| mesh.vertices()[vertex.index()][0] == 1.0)
            })
            .map(FacetId::new)
            .collect();
        (fluid, solid, interface)
    }

    fn signed_tetrahedron_measure(vertices: &[Vec<f64>], cell: &[usize]) -> f64 {
        let origin = &vertices[cell[0]];
        let column = |vertex: usize, axis: usize| vertices[cell[vertex]][axis] - origin[axis];
        column(1, 0) * (column(2, 1) * column(3, 2) - column(3, 1) * column(2, 2))
            - column(2, 0) * (column(1, 1) * column(3, 2) - column(3, 1) * column(1, 2))
            + column(3, 0) * (column(1, 1) * column(2, 2) - column(2, 1) * column(1, 2))
    }

    fn compile_program(source: &str) -> KernelProgram {
        let mut compiled = compile("ale-fsi.eqi", source).unwrap();
        let (transaction, model, _) = compiled.remove(0).into_parts();
        let mut store = InMemoryGraphStore::new();
        store.commit(transaction).unwrap();
        KernelProgram::from_snapshot(&store.snapshot(), model).unwrap()
    }

    fn ale_source() -> String {
        format!(
            "public pure operator outer_product(left: spatial[1], right: spatial[1]) -> spatial[2]\n  = component(left, 0) * component(right, 1);\n{}",
            BASE_SOURCE.replace(
                "fluid_density * derivative(fluid_velocity)\n      - div(",
                "fluid_density * derivative(fluid_velocity)\n      + div(fluid_density * outer_product(fluid_velocity, fluid_velocity))\n      - div(",
            )
        )
    }

    fn ale_source_3d() -> String {
        let mut source = ale_source();
        replace_exactly(
            &mut source,
            "ambient_dimension = 2",
            "ambient_dimension = 3",
            3,
        );
        for (from, to) in [
            (
                "domain fluid = box(0, 1, 0, 1);",
                "domain fluid = box(0, 1, 0, 1, 0, 1);",
            ),
            (
                "domain solid = box(1, 2, 0, 1);",
                "domain solid = box(1, 2, 0, 1, 0, 1);",
            ),
            (
                "  domain fluid_y_upper = boundary(fluid, axis = 1, side = upper);",
                "  domain fluid_y_upper = boundary(fluid, axis = 1, side = upper);\n  domain fluid_z_lower = boundary(fluid, axis = 2, side = lower);\n  domain fluid_z_upper = boundary(fluid, axis = 2, side = upper);",
            ),
            (
                "  domain solid_y_upper = boundary(solid, axis = 1, side = upper);",
                "  domain solid_y_upper = boundary(solid, axis = 1, side = upper);\n  domain solid_z_lower = boundary(solid, axis = 2, side = lower);\n  domain solid_z_upper = boundary(solid, axis = 2, side = upper);",
            ),
            (
                "      fluid_x_lower, fluid_x_upper, fluid_y_lower, fluid_y_upper\n",
                "      fluid_x_lower, fluid_x_upper, fluid_y_lower, fluid_y_upper,\n      fluid_z_lower, fluid_z_upper\n",
            ),
            (
                "      solid_x_lower, solid_x_upper, solid_y_lower, solid_y_upper\n",
                "      solid_x_lower, solid_x_upper, solid_y_lower, solid_y_upper,\n      solid_z_lower, solid_z_upper\n",
            ),
            (
                "  instance fluid_y_upper_zero: ZeroVelocity2d(\n    support body = fluid, support face = fluid_y_upper\n  );",
                "  instance fluid_y_upper_zero: ZeroVelocity2d(\n    support body = fluid, support face = fluid_y_upper\n  );\n  instance fluid_z_lower_zero: ZeroVelocity2d(\n    support body = fluid, support face = fluid_z_lower\n  );\n  instance fluid_z_upper_zero: ZeroVelocity2d(\n    support body = fluid, support face = fluid_z_upper\n  );",
            ),
            (
                "  instance solid_y_upper_zero: ZeroVelocity2d(\n    support body = solid, support face = solid_y_upper\n  );",
                "  instance solid_y_upper_zero: ZeroVelocity2d(\n    support body = solid, support face = solid_y_upper\n  );\n  instance solid_z_lower_zero: ZeroVelocity2d(\n    support body = solid, support face = solid_z_lower\n  );\n  instance solid_z_upper_zero: ZeroVelocity2d(\n    support body = solid, support face = solid_z_upper\n  );",
            ),
            (
                "  connect conserving\n    fluid_boundary.mechanical[boundary = fluid_y_upper],\n    fluid_y_upper_zero.mechanical;",
                "  connect conserving\n    fluid_boundary.mechanical[boundary = fluid_y_upper],\n    fluid_y_upper_zero.mechanical;\n  connect conserving\n    fluid_boundary.mechanical[boundary = fluid_z_lower],\n    fluid_z_lower_zero.mechanical;\n  connect conserving\n    fluid_boundary.mechanical[boundary = fluid_z_upper],\n    fluid_z_upper_zero.mechanical;",
            ),
            (
                "  connect conserving\n    solid_boundary.mechanical[boundary = solid_y_upper],\n    solid_y_upper_zero.mechanical;",
                "  connect conserving\n    solid_boundary.mechanical[boundary = solid_y_upper],\n    solid_y_upper_zero.mechanical;\n  connect conserving\n    solid_boundary.mechanical[boundary = solid_z_lower],\n    solid_z_lower_zero.mechanical;\n  connect conserving\n    solid_boundary.mechanical[boundary = solid_z_upper],\n    solid_z_upper_zero.mechanical;",
            ),
        ] {
            replace_exactly(&mut source, from, to, 1);
        }
        for (from, to, expected) in [
            ("ZeroVelocity2d", "ZeroVelocity3d", 11),
            (
                "NewtonianMechanicalInterface2d",
                "NewtonianMechanicalInterface3d",
                2,
            ),
            (
                "ElastodynamicMechanicalInterface2d",
                "ElastodynamicMechanicalInterface3d",
                2,
            ),
        ] {
            replace_exactly(&mut source, from, to, expected);
        }
        source
    }

    fn replace_exactly(source: &mut String, from: &str, to: &str, expected: usize) {
        assert_eq!(
            source.match_indices(from).count(),
            expected,
            "source lift drifted"
        );
        *source = source.replace(from, to);
    }

    fn mesh_reference() -> MeshArtifactReference {
        MeshArtifactReference::from_sha256([154; 32])
    }

    fn physical_scale(value: f64, dimension: DimExponents) -> PositivePhysicalScale {
        PositivePhysicalScale::new(DynQuantity::new(value, dimension)).unwrap()
    }

    fn harmonic_solver() -> eqiora_solver::SolverPlan {
        SolverPlan::new(
            LinearSolver::ConjugateGradient,
            1.0e-12,
            1.0e-14,
            NonZeroUsize::new(500).unwrap(),
        )
        .unwrap()
        .with_preconditioner(PreconditionerPolicy::Identity)
        .with_reduction(ReductionPolicy::Reproducible)
    }

    fn nonlinear_solver() -> eqiora_solver::SolverPlan {
        SolverPlan::new(
            LinearSolver::BiConjugateGradientStabilized,
            1.0e-9,
            1.0e-11,
            NonZeroUsize::new(500).unwrap(),
        )
        .unwrap()
        .with_preconditioner(PreconditionerPolicy::Identity)
        .with_reduction(ReductionPolicy::Reproducible)
    }

    #[derive(Debug)]
    struct NoSolveGeneralBackend;

    impl LinearSolverBackend for NoSolveGeneralBackend {
        fn provider(&self) -> SolverProvider {
            SolverProvider::new(
                BackendId::new("eqiora.test.no-solve-general"),
                env!("CARGO_PKG_VERSION"),
                &[],
            )
        }

        fn capabilities(&self) -> SolverCapabilities {
            SolverCapabilities::exact([SolverCapability {
                algorithm: LinearSolver::BiConjugateGradientStabilized,
                operator_properties: LinearOperatorProperties::General,
                preconditioner: PreconditionerPolicy::Identity,
                reduction: ReductionPolicy::Reproducible,
                scalar_type: ScalarType::F64,
            }])
            .unwrap()
        }

        fn solve_with_execution(
            &self,
            _problem: &LinearProblem<'_>,
            _plan: SolverPlan,
            _execution: &dyn ReplicatedLinearExecution,
        ) -> Result<LinearSolution, Diagnostic> {
            Err(Diagnostic::error(
                codes::NUMERICAL_SOLVE_FAILED,
                "zero-equilibrium bridge test must not invoke its linear backend",
            ))
        }
    }
}
