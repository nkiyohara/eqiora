//! Private Model-driven admission of exact common Mesh resources and numerical policy.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;
use std::sync::Arc;

use crate::canonical::{
    lower_scalar_elliptic_cartesian_with_resources, recognize_scalar_elliptic_geometry_mathematics,
};
use crate::canonical_elasticity::{
    IsotropicElasticityCartesianModel2d, finalize_isotropic_elasticity_cartesian_q1_on_mesh,
    lower_isotropic_elasticity_geometry_2d, recognize_isotropic_elasticity_geometry_mathematics,
};
use crate::canonical_fsi::{
    FixedReferenceFsiCartesianModel2d, FixedReferenceFsiScaleProfile2d,
    finalize_resolved_fixed_reference_fsi_step_2d, fixed_reference_fsi_plan_2d,
    fixed_reference_fsi_requirements_2d, lower_fixed_reference_fsi_geometry_2d,
};
use crate::canonical_stokes::{
    CellCenteredNavierStokesInitialState2d, IncompressibleScalingReceipt2d,
    IncompressibleScalingRequest2d, ResolvedCellCenteredNavierStokesState2d,
    ResolvedIncompressibleScaling2d, ResolvedTransientNavierStokesState2d,
    TransientIncompressibleNavierStokesCartesianModel2d, TransientNavierStokesInitialState2d,
    TransientNavierStokesRun2d, advance_resolved_transient_navier_stokes_cell_centered_2d,
    advance_resolved_transient_navier_stokes_mini_2d,
    lower_transient_incompressible_navier_stokes_cartesian_2d,
    recognize_steady_incompressible_stokes_geometry_mathematics,
    resolve_complete_manual_incompressible_scaling_2d, resolve_fixed_reference_fsi_scaling_2d,
    solve_resolved_steady_stokes_geometry_mini_2d, transient_navier_stokes_cell_centered_plan_2d,
    transient_navier_stokes_cell_centered_requirements_2d,
    transient_navier_stokes_fieldwise_requirements_2d, transient_navier_stokes_mini_plan_2d,
};
use crate::cartesian_elasticity::CartesianLinearElasticity2dSolution;
use crate::cartesian_elliptic::{
    finalize_scalar_elliptic_cartesian_fem, finalize_scalar_elliptic_cartesian_fvm,
    linearize_scalar_elliptic_cartesian_fem, linearize_scalar_elliptic_cartesian_fem_output,
    linearize_scalar_elliptic_cartesian_fvm, linearize_scalar_elliptic_cartesian_fvm_output,
};
use crate::cartesian_elliptic::{
    solve_scalar_elliptic_cartesian_fem, solve_scalar_elliptic_cartesian_fvm,
};
use crate::common::{AssembledLinearizedRelation, SpatialDesignCoordinate};
use crate::common_ode::{CommonOdePlan, CommonTsitouras45};
use crate::finalized_spatial::FinalizedScalarEllipticCartesianProblem;
use crate::fluid::{
    CellCenteredPressureField2d, CellCenteredVelocityField2d, IncompressibleFlowScaleProfile2d,
    SimplicialMiniVelocityField2d, SteadyStokesGeometryBinding2d, SteadyStokesPressureReference2d,
};
use crate::fsi::{
    FixedReferenceFsiPartition2d, FixedReferenceFsiState2d, ResolvedFixedReferenceFsiSolution2d,
};
use crate::scalar::{
    CartesianScalarFieldLinearization, ResolvedScalarEllipticCartesianSolution,
    ScalarEllipticCartesianModel,
};
use crate::simplicial_elliptic::SimplicialP1Field;
use crate::step_count::NonZeroStepCount;
use eqiora_artifact::{
    CanonicalModelArtifact, CartesianMeshEnvelopeV1, GeometryMeshCorrespondenceEnvelopeV1,
    LayoutArtifacts, MeshProductionLineageEnvelopeV1, ModelEnvelope, RealizationEnvelopeV2,
    RealizationEnvelopeV3, SimplicialMeshEnvelopeV1,
};
use eqiora_assembly::REFERENCE_ASSEMBLY_BACKEND;
use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, DimExponents, DynQuantity};
use eqiora_execution::{
    AdmittedExecution, DeploymentBinding, ExecutionReceipt, HostExecutorDescriptor,
};
use eqiora_geometry::CanonicalGeometryV1;
use eqiora_graph::{GraphStore, InMemoryGraphStore};
use eqiora_io_gmsh::{Msh41Policy, import_msh41};
use eqiora_meshing::{CellId, FacetId, MeshEntity, MeshTopology, QuadratureRule, SimplicialMesh};
use eqiora_realization::{
    CoupledFieldwiseRealizationRequest, Discretization, DiscretizationMethod, ExecutionSchedule,
    FieldwiseRealizationRequest, MeshArtifactReference, MeshKind, MeshPolicy, NonlinearSolvePlan,
    PortableRealizationGraph, QuadraturePolicy, RealizationCapabilities, RealizationPlan,
    RealizationRequest, RealizationRequirements, RealizationRevision,
    ResolvedCoupledFieldwiseRealization, ResolvedFieldwiseRealization,
    ResolvedTransientCellCenteredIncompressibleFlowRealization,
    ResolvedTransientFieldwiseRealization, SemanticRevision, SingleFieldOperatorClaim, Space,
    SpaceFamily, SpatialDimensionSupport, Target, TargetCapabilities,
    TransientCellCenteredIncompressibleFlowCapabilities,
    TransientCellCenteredIncompressibleFlowRealizationRequest,
    TransientFieldwiseRealizationRequest, VectorLayoutKind, resolve, resolve_coupled_fieldwise,
    resolve_fieldwise, resolve_transient_cell_centered_incompressible_flow,
    resolve_transient_fieldwise,
};
use eqiora_sem::KernelProgram;
use eqiora_solver::{
    ExecutionProvider, LinearOperatorProperties, LinearSolveRequest, LinearSolver,
    LinearSolverBackend, PreconditionerPolicy, REFERENCE_LINEAR_SOLVER, ReductionPolicy,
    SERIAL_EXECUTION_PROVIDER, ScalarType, SolveReport, SolverCapabilities, SolverCapability,
    SolverPlan, SolverProvider,
};
use eqiora_time::TimeBackendIdentity;
use sha2::{Digest, Sha256};

const APPLICATION_REALIZATION_REVISION: u64 = 134;
const COMMON_SCALAR_REALIZATION_REVISION: u64 = 170;
const TRANSIENT_REALIZATION_REVISION: u64 = 166;
const COMMON_TRANSIENT_RESOLVER_EPOCH: u64 = 1;
const TIME: DimExponents = DimExponents {
    time: 1,
    ..DimExponents::DIMENSIONLESS
};
const POLICY_DOMAIN: &[u8] = b"eqiora.private-native-numerical-admission/v1\0";
/// Closed spatial choice requested from the Model-first common resolver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonSpatialPolicy {
    Q1,
    P1,
    CellCenteredTpfa,
    MiniP1,
    CellCentered,
}

/// One exact Domain-scoped spatial policy binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonScopedSpatialPolicy {
    model: eqiora_artifact::ArtifactDigest,
    domain: eqiora_core::Id<eqiora_core::entity::kinds::Domain>,
    policy: CommonSpatialPolicy,
}

impl CommonScopedSpatialPolicy {
    #[must_use]
    pub fn new(
        model: eqiora_artifact::ArtifactDigest,
        domain: eqiora_core::Id<eqiora_core::entity::kinds::Domain>,
        policy: CommonSpatialPolicy,
    ) -> Self {
        Self {
            model,
            domain,
            policy,
        }
    }
    #[must_use]
    pub const fn model(&self) -> &eqiora_artifact::ArtifactDigest {
        &self.model
    }
    #[must_use]
    pub const fn domain(&self) -> eqiora_core::Id<eqiora_core::entity::kinds::Domain> {
        self.domain
    }
    #[must_use]
    pub const fn policy(&self) -> CommonSpatialPolicy {
        self.policy
    }
}

/// Closed spatial request consumed by the one common Model-first resolver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommonSpatialRequest {
    Uniform(CommonSpatialPolicy),
    Scoped(Vec<CommonScopedSpatialPolicy>),
}

/// Immutable coherent-SI values for one supported exact Field association.
#[derive(Debug, Clone, PartialEq)]
pub enum CommonInitialValues {
    Scalar(Box<[f64]>),
    Vector2(Box<[[f64; 2]]>),
}

/// One exact Model/Field-bound initial assignment with bounded associations.
#[derive(Debug, Clone, PartialEq)]
pub struct CommonInitialField {
    model: eqiora_artifact::ArtifactDigest,
    field: eqiora_core::Id<eqiora_core::entity::kinds::Field>,
    vertex: Option<CommonInitialValues>,
    cell: Option<CommonInitialValues>,
}

impl CommonInitialField {
    pub fn new(
        model: eqiora_artifact::ArtifactDigest,
        field: eqiora_core::Id<eqiora_core::entity::kinds::Field>,
        vertex: Option<CommonInitialValues>,
        cell: Option<CommonInitialValues>,
    ) -> Result<Self, Diagnostic> {
        if vertex.is_none() && cell.is_none() {
            return Err(invalid(
                "InitialField requires vertex_values or cell_values",
            ));
        }
        let finite = |values: &CommonInitialValues| match values {
            CommonInitialValues::Scalar(values) => values.iter().all(|value| value.is_finite()),
            CommonInitialValues::Vector2(values) => {
                values.iter().flatten().all(|value| value.is_finite())
            }
        };
        if vertex.as_ref().is_some_and(|values| !finite(values))
            || cell.as_ref().is_some_and(|values| !finite(values))
        {
            return Err(invalid(
                "InitialField values must be finite coherent-SI numbers",
            ));
        }
        Ok(Self {
            model,
            field,
            vertex,
            cell,
        })
    }
    #[must_use]
    pub const fn model(&self) -> &eqiora_artifact::ArtifactDigest {
        &self.model
    }
    #[must_use]
    pub const fn field(&self) -> eqiora_core::Id<eqiora_core::entity::kinds::Field> {
        self.field
    }
    #[must_use]
    pub const fn vertex(&self) -> Option<&CommonInitialValues> {
        self.vertex.as_ref()
    }
    #[must_use]
    pub const fn cell(&self) -> Option<&CommonInitialValues> {
        self.cell.as_ref()
    }
}

impl From<CommonSpatialPolicy> for CommonSpatialRequest {
    fn from(value: CommonSpatialPolicy) -> Self {
        Self::Uniform(value)
    }
}

/// Closed time integration policy requested from the common resolver.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CommonBackwardEuler {
    step: DynQuantity,
}

impl CommonBackwardEuler {
    /// Construct one positive coherent-SI operator step.
    pub fn from_seconds(step_s: f64) -> Result<Self, Diagnostic> {
        if !step_s.is_finite() || step_s <= 0.0 || step_s.to_bits() == (-0.0_f64).to_bits() {
            return Err(invalid(
                "BackwardEuler step_s must be finite and strictly positive",
            ));
        }
        Ok(Self {
            step: DynQuantity::new(step_s, TIME),
        })
    }

    /// Exact physical duration of one operator construction.
    #[must_use]
    pub const fn step(self) -> DynQuantity {
        self.step
    }
}

/// Closed linear or Newton/linear hierarchy requested from the common resolver.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CommonSolvePolicy {
    /// One linear solve for a steady linearized problem.
    Linear(SolverPlan),
    /// One bounded Newton policy owning its nested linear controls.
    Newton {
        /// Nonlinear convergence/globalization policy.
        nonlinear: NonlinearSolvePlan,
        /// Nested linear controls.
        linear: SolverPlan,
    },
}

impl CommonSolvePolicy {
    /// Construct one admitted bounded Newton policy around exact linear controls.
    #[must_use]
    pub const fn newton(linear: SolverPlan, nonlinear: NonlinearSolvePlan) -> Self {
        Self::Newton { nonlinear, linear }
    }
}

/// Pressure representative retained by an admitted transient Plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonPressureGauge2d {
    /// Pressure is represented by one zero-integral constraint.
    ZeroIntegral,
    /// Natural traction determines the absolute pressure representative.
    BoundaryTraction,
}

/// Opaque result of Model-first common numerical resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedCommonPlan {
    kind: ResolvedCommonPlanKind,
}

#[derive(Debug, Clone, PartialEq)]
enum ResolvedCommonPlanKind {
    Ode(Box<CommonOdePlan>),
    Scalar(Box<CommonScalarPlan>),
    Elasticity(Box<CommonElasticityPlan>),
    SteadyStokes(Box<CommonSteadyStokesPlan>),
    TransientFlow(Box<CommonTransientFlowPlan>),
    Fsi(Box<CommonFsiPlan>),
}

impl ResolvedCommonPlan {
    /// Project one already-resolved Plan without reopening capability selection.
    pub fn project<T>(
        self,
        ode: impl FnOnce(CommonOdePlan) -> T,
        scalar: impl FnOnce(CommonScalarPlan) -> T,
        elasticity: impl FnOnce(CommonElasticityPlan) -> T,
        steady_stokes: impl FnOnce(CommonSteadyStokesPlan) -> T,
        transient_flow: impl FnOnce(CommonTransientFlowPlan) -> T,
        fsi: impl FnOnce(CommonFsiPlan) -> T,
    ) -> T {
        match self.kind {
            ResolvedCommonPlanKind::Ode(plan) => ode(*plan),
            ResolvedCommonPlanKind::Scalar(plan) => scalar(*plan),
            ResolvedCommonPlanKind::Elasticity(plan) => elasticity(*plan),
            ResolvedCommonPlanKind::SteadyStokes(plan) => steady_stokes(*plan),
            ResolvedCommonPlanKind::TransientFlow(plan) => transient_flow(*plan),
            ResolvedCommonPlanKind::Fsi(plan) => fsi(*plan),
        }
    }
}

/// Opaque native fixed-reference FSI Plan owning exact common resources.
#[derive(Debug, Clone, PartialEq)]
pub struct CommonFsiPlan {
    model: ModelEnvelope,
    canonical: FixedReferenceFsiCartesianModel2d,
    resources: NativeMeshResources,
    partition: FixedReferenceFsiPartition2d,
    resolved: ResolvedCoupledFieldwiseRealization,
    realization: RealizationEnvelopeV3,
    scaling: FixedReferenceFsiScaleProfile2d,
    scaling_receipt: IncompressibleScalingReceipt2d,
    temporal: CommonBackwardEuler,
    linear: SolverPlan,
    solver_provider: SolverProvider,
    solver_capabilities: SolverCapabilities,
    execution_provider: ExecutionProvider,
    workers: NonZeroUsize,
    identity: String,
    model_id: String,
    model_revision: u64,
    model_digest: String,
    geometry_digest: String,
    mesh_digest: String,
    correspondence_digest: String,
    production_digest: String,
    realization_digest: String,
    field_ids: [String; 4],
    domain_ids: [String; 2],
}

/// Resolve one canonical no-Mesh explicit ODE through the common native Plan sum.
pub fn resolve_common_ode_plan(
    model: &ModelEnvelope,
    kernel: &KernelProgram,
    temporal: CommonTsitouras45,
    backend: TimeBackendIdentity,
) -> Result<ResolvedCommonPlan, Diagnostic> {
    CommonOdePlan::resolve(model, kernel, temporal, backend).map(|plan| ResolvedCommonPlan {
        kind: ResolvedCommonPlanKind::Ode(Box::new(plan)),
    })
}

#[derive(Debug, Clone, PartialEq)]
enum CommonTransientResolvedSpatial {
    MiniP1(ResolvedTransientFieldwiseRealization),
    CellCentered(ResolvedTransientCellCenteredIncompressibleFlowRealization),
}

/// Opaque transient-flow Plan owning exact caller resources and numerical policy.
#[derive(Debug, Clone, PartialEq)]
pub struct CommonTransientFlowPlan {
    admission: NativeNumericalAdmission,
    resolved: CommonTransientResolvedSpatial,
    scaling: ResolvedIncompressibleScaling2d,
    temporal: CommonBackwardEuler,
    nonlinear: NonlinearSolvePlan,
    identity: String,
    model_id: String,
    model_revision: u64,
    geometry_digest: String,
    mesh_digest: String,
    correspondence_digest: String,
    production_digest: String,
    velocity_field_id: String,
    pressure_field_id: String,
    velocity_space: Space,
    pressure_space: Space,
    gauge: CommonPressureGauge2d,
}

#[derive(Debug, Clone, PartialEq)]
enum CommonStateKind {
    MiniP1(Box<TransientNavierStokesInitialState2d>),
    CellCentered(Box<CellCenteredNavierStokesInitialState2d>),
    Fsi {
        state: Box<FixedReferenceFsiState2d>,
        pressure: Box<[f64]>,
        accepted: Option<Box<ResolvedFixedReferenceFsiSolution2d>>,
    },
}

/// Opaque coherent-SI state for one exact common transient state space.
#[derive(Debug, Clone, PartialEq)]
pub struct CommonState {
    state_space_identity: String,
    identity: String,
    time_s: f64,
    model: Arc<ModelEnvelope>,
    resources: Arc<NativeMeshResources>,
    kind: CommonStateKind,
}

/// Canonical private execution request for one exact transient Plan and State.
#[derive(Debug, Clone, PartialEq)]
pub struct CommonTransientRunRequest {
    plan: CommonTransientFlowPlan,
    state: CommonState,
    accepted_steps: NonZeroUsize,
    output_steps: Vec<usize>,
    identity: String,
}

/// Canonical common-worker request for one exact FSI Plan and State.
#[derive(Debug, Clone, PartialEq)]
pub struct CommonFsiRunRequest {
    plan: CommonFsiPlan,
    state: CommonState,
    accepted_steps: NonZeroUsize,
    output_steps: Vec<usize>,
    identity: String,
}

/// Opaque common scalar Plan owning authenticated Model, Mesh, and policy state.
#[derive(Debug, Clone, PartialEq)]
pub struct CommonScalarPlan {
    admission: NativeNumericalAdmission,
    portable: PortableRealizationGraph,
    identity: String,
    model_id: String,
    model_revision: u64,
    geometry_digest: String,
    mesh_digest: String,
    correspondence_digest: String,
    production_digest: String,
    field: eqiora_core::Id<eqiora_core::entity::kinds::Field>,
    field_id: String,
    cells: [usize; 2],
}

/// One accepted scalar Parameter point produced through an exact common Plan.
#[derive(Debug, Clone, PartialEq)]
pub struct CommonScalarDifferentiationPoint {
    relation: AssembledLinearizedRelation,
    output: CartesianScalarFieldLinearization,
    receipt: ExecutionReceipt,
}

impl CommonScalarDifferentiationPoint {
    /// Consume the point into its relation, complete Field projection, and solve receipt.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        AssembledLinearizedRelation,
        CartesianScalarFieldLinearization,
        ExecutionReceipt,
    ) {
        (self.relation, self.output, self.receipt)
    }
}

/// Opaque linear-elasticity Plan owning exact Model, Mesh, and policy state.
#[derive(Debug, Clone, PartialEq)]
pub struct CommonElasticityPlan {
    admission: NativeNumericalAdmission,
    identity: String,
    model_id: String,
    model_revision: u64,
    geometry_digest: String,
    mesh_digest: String,
    correspondence_digest: String,
    production_digest: String,
    displacement_field_id: String,
    cells: [usize; 2],
}

/// Opaque steady-Stokes Plan owning one authenticated exact-cylinder occurrence.
#[derive(Debug, Clone, PartialEq)]
pub struct CommonSteadyStokesPlan {
    admission: NativeNumericalAdmission,
    binding: SteadyStokesGeometryBinding2d,
    resolved: ResolvedFieldwiseRealization,
    realization: RealizationEnvelopeV2,
    scaling: ResolvedIncompressibleScaling2d,
    realization_digest: String,
    identity: String,
    model_id: String,
    model_revision: u64,
    geometry_digest: String,
    mesh_digest: String,
    correspondence_digest: String,
    production_digest: String,
    velocity_field_id: String,
    pressure_field_id: String,
    velocity_space: Space,
    pressure_space: Space,
}

/// Plan-authenticated scientific observations for one common steady-Stokes solve.
#[derive(Debug, Clone, PartialEq)]
pub struct CommonSteadyStokesObservation {
    pressure_minimum: f64,
    pressure_maximum: f64,
    exact_bounds: [[f64; 2]; 2],
    cylinder_force_on_fluid: [f64; 2],
    inlet_flux: f64,
    outlet_flux: f64,
    net_flux: f64,
    constrained_reaction: [f64; 2],
    integrated_body_force: [f64; 2],
    integrated_boundary_traction: [f64; 2],
    momentum_closure: [f64; 2],
    solve: SolveReport,
    continuity_residual_norm: f64,
}

/// Exact paired output produced by one common steady-Stokes Plan execution.
#[derive(Debug, Clone, PartialEq)]
pub struct CommonSteadyStokesRunOutput {
    plan_identity: String,
    solution: crate::fluid::SteadyStokesMiniSolution2d,
    observation: CommonSteadyStokesObservation,
}

impl CommonSteadyStokesRunOutput {
    #[must_use]
    pub fn plan_identity(&self) -> &str {
        &self.plan_identity
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        crate::fluid::SteadyStokesMiniSolution2d,
        CommonSteadyStokesObservation,
    ) {
        (self.solution, self.observation)
    }
}

impl CommonSteadyStokesObservation {
    #[must_use]
    pub const fn pressure_minimum(&self) -> f64 {
        self.pressure_minimum
    }
    #[must_use]
    pub const fn pressure_maximum(&self) -> f64 {
        self.pressure_maximum
    }
    #[must_use]
    pub const fn exact_bounds(&self) -> [[f64; 2]; 2] {
        self.exact_bounds
    }
    #[must_use]
    pub const fn cylinder_force_on_fluid(&self) -> [f64; 2] {
        self.cylinder_force_on_fluid
    }
    #[must_use]
    pub const fn inlet_flux(&self) -> f64 {
        self.inlet_flux
    }
    #[must_use]
    pub const fn outlet_flux(&self) -> f64 {
        self.outlet_flux
    }
    #[must_use]
    pub const fn net_flux(&self) -> f64 {
        self.net_flux
    }
    #[must_use]
    pub const fn constrained_reaction(&self) -> [f64; 2] {
        self.constrained_reaction
    }
    #[must_use]
    pub const fn integrated_body_force(&self) -> [f64; 2] {
        self.integrated_body_force
    }
    #[must_use]
    pub const fn integrated_boundary_traction(&self) -> [f64; 2] {
        self.integrated_boundary_traction
    }
    #[must_use]
    pub const fn momentum_closure(&self) -> [f64; 2] {
        self.momentum_closure
    }
    #[must_use]
    pub const fn solve(&self) -> &SolveReport {
        &self.solve
    }
    #[must_use]
    pub const fn continuity_residual_norm(&self) -> f64 {
        self.continuity_residual_norm
    }
}

/// Plan-authenticated scientific observations for one common elasticity solve.
#[derive(Debug, Clone, PartialEq)]
pub struct CommonElasticityObservation {
    constrained_reaction: [f64; 2],
    integrated_body_force: [f64; 2],
    assembly_packets: usize,
    assembly_targets: usize,
    solve: SolveReport,
    exact_bounds: [[f64; 2]; 2],
}

/// Exact paired output produced by one common elasticity Plan execution.
#[derive(Debug, Clone, PartialEq)]
pub struct CommonElasticityRunOutput {
    plan_identity: String,
    solution: CartesianLinearElasticity2dSolution,
    observation: CommonElasticityObservation,
}

impl CommonElasticityRunOutput {
    #[must_use]
    pub fn plan_identity(&self) -> &str {
        &self.plan_identity
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        CartesianLinearElasticity2dSolution,
        CommonElasticityObservation,
    ) {
        (self.solution, self.observation)
    }
}

impl CommonElasticityObservation {
    #[must_use]
    pub const fn constrained_reaction(&self) -> [f64; 2] {
        self.constrained_reaction
    }
    #[must_use]
    pub const fn integrated_body_force(&self) -> [f64; 2] {
        self.integrated_body_force
    }
    #[must_use]
    pub const fn assembly_packets(&self) -> usize {
        self.assembly_packets
    }
    #[must_use]
    pub const fn assembly_targets(&self) -> usize {
        self.assembly_targets
    }
    #[must_use]
    pub const fn solve(&self) -> &SolveReport {
        &self.solve
    }
    #[must_use]
    pub const fn exact_bounds(&self) -> [[f64; 2]; 2] {
        self.exact_bounds
    }
}

/// Resolve Model mathematics first, then admit the requested numerical policies.
mod elasticity;
mod fsi;
mod native;
mod resolve;
mod scalar;
mod state;
mod steady_stokes;
mod transient;

pub use native::AuthenticatedCommonMesh;
pub use resolve::resolve_common_plan;

use native::*;

#[cfg(test)]
mod tests;
