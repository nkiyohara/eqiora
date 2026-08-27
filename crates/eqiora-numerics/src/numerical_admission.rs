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
use crate::canonical_stokes::{
    CellCenteredNavierStokesInitialState2d, IncompressibleScalingReceipt2d,
    IncompressibleScalingRequest2d, ResolvedCellCenteredNavierStokesState2d,
    ResolvedIncompressibleScaling2d, ResolvedTransientNavierStokesState2d,
    TransientIncompressibleNavierStokesCartesianModel2d, TransientNavierStokesInitialState2d,
    TransientNavierStokesRun2d, advance_resolved_transient_navier_stokes_cell_centered_2d,
    advance_resolved_transient_navier_stokes_mini_2d,
    lower_transient_incompressible_navier_stokes_cartesian_2d,
    recognize_steady_incompressible_stokes_geometry_mathematics,
    resolve_complete_manual_incompressible_scaling_2d,
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
use crate::common::{AssembledLinearizedRelation, SpatialDesignCoordinate};
use crate::common_ode::{CommonOdePlan, CommonTsitouras45};
use crate::finalized_spatial::FinalizedScalarEllipticCartesianProblem;
use crate::fluid::{
    CellCenteredPressureField2d, CellCenteredVelocityField2d, IncompressibleFlowScaleProfile2d,
    SimplicialMiniVelocityField2d, SteadyStokesGeometryBinding2d, SteadyStokesPressureReference2d,
};
use crate::scalar::{
    CartesianScalarFieldLinearization, ResolvedScalarEllipticCartesianSolution,
    ScalarEllipticCartesianModel, solve_scalar_elliptic_cartesian_fem,
    solve_scalar_elliptic_cartesian_fvm,
};
use crate::simplicial_elliptic::SimplicialP1Field;
use crate::step_count::NonZeroStepCount;
use eqiora_artifact::{
    CanonicalModelArtifact, CartesianMeshEnvelopeV1, GeometryMeshCorrespondenceEnvelopeV1,
    MeshProductionLineageEnvelopeV1, ModelEnvelope, RealizationEnvelopeV2,
    SimplicialMeshEnvelopeV1,
};
use eqiora_assembly::REFERENCE_ASSEMBLY_BACKEND;
use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, DimExponents, DynQuantity};
use eqiora_execution::{
    AdmittedExecution, DeploymentBinding, ExecutionReceipt, HostExecutorDescriptor,
};
use eqiora_geometry::CanonicalGeometryV1;
use eqiora_graph::{GraphStore, InMemoryGraphStore};
use eqiora_io_gmsh::{GmshImportLimits, GmshSimplexImporter, GmshSimplicialImport};
use eqiora_meshing::{MeshEntity, MeshTopology, QuadratureRule};
use eqiora_realization::{
    Discretization, DiscretizationMethod, ExecutionSchedule, FieldwiseRealizationRequest, MeshKind,
    MeshPolicy, NonlinearSolvePlan, PortableRealizationGraph, QuadraturePolicy,
    RealizationCapabilities, RealizationPlan, RealizationRequest, RealizationRequirements,
    RealizationRevision, ResolvedFieldwiseRealization,
    ResolvedTransientCellCenteredIncompressibleFlowRealization,
    ResolvedTransientFieldwiseRealization, SemanticRevision, SingleFieldOperatorClaim, Space,
    SpaceFamily, SpatialDimensionSupport, Target, TargetCapabilities,
    TransientCellCenteredIncompressibleFlowCapabilities,
    TransientCellCenteredIncompressibleFlowRealizationRequest,
    TransientFieldwiseRealizationRequest, VectorLayoutKind, resolve, resolve_fieldwise,
    resolve_transient_cell_centered_incompressible_flow, resolve_transient_fieldwise,
};
use eqiora_sem::KernelProgram;
use eqiora_solver::{
    ExecutionProvider, LinearOperatorProperties, LinearSolveRequest, LinearSolver,
    LinearSolverBackend, PreconditionerPolicy, REFERENCE_LINEAR_SOLVER, ReductionPolicy,
    SERIAL_EXECUTION_PROVIDER, ScalarType, SolverCapabilities, SolverCapability, SolverPlan,
    SolverProvider,
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
type TaggedMeshAssignments = (BTreeMap<u32, Vec<usize>>, BTreeMap<u32, Vec<usize>>);

/// Closed spatial choice requested from the Model-first common resolver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonSpatialPolicy {
    Q1,
    CellCenteredTpfa,
    MiniP1,
    CellCentered,
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
    ) -> T {
        match self.kind {
            ResolvedCommonPlanKind::Ode(plan) => ode(*plan),
            ResolvedCommonPlanKind::Scalar(plan) => scalar(*plan),
            ResolvedCommonPlanKind::Elasticity(plan) => elasticity(*plan),
            ResolvedCommonPlanKind::SteadyStokes(plan) => steady_stokes(*plan),
            ResolvedCommonPlanKind::TransientFlow(plan) => transient_flow(*plan),
        }
    }
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

impl CommonTransientRunRequest {
    /// Canonicalize a step-count horizon and explicit accepted-step outputs.
    pub fn from_steps(
        plan: CommonTransientFlowPlan,
        state: CommonState,
        steps: usize,
        output_steps: Vec<usize>,
    ) -> Result<Self, Diagnostic> {
        let accepted_steps = NonZeroUsize::new(steps)
            .ok_or_else(|| invalid("transient Run steps must be strictly positive"))?;
        Self::new(plan, state, accepted_steps, output_steps)
    }

    /// Canonicalize an exact Backward-Euler time horizon and output-time grid.
    pub fn from_times(
        plan: CommonTransientFlowPlan,
        state: CommonState,
        until_s: f64,
        output_times_s: Vec<f64>,
    ) -> Result<Self, Diagnostic> {
        if !until_s.is_finite() || until_s <= state.time_s() {
            return Err(invalid(
                "transient Run until_s must be finite and later than State.time_s",
            ));
        }
        if output_times_s.is_empty()
            || output_times_s.iter().any(|value| !value.is_finite())
            || output_times_s.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(invalid(
                "output_times_s must be finite, nonempty, and strictly increasing",
            ));
        }
        let step_s = plan.temporal().step().value();
        let accepted_steps = exact_grid_index(state.time_s(), until_s, step_s, "until_s")?;
        let output_steps = output_times_s
            .into_iter()
            .map(|time| exact_grid_index(state.time_s(), time, step_s, "output_times_s"))
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(
            plan,
            state,
            NonZeroUsize::new(accepted_steps)
                .ok_or_else(|| invalid("transient Run horizon contains no accepted step"))?,
            output_steps,
        )
    }

    fn new(
        plan: CommonTransientFlowPlan,
        state: CommonState,
        accepted_steps: NonZeroUsize,
        output_steps: Vec<usize>,
    ) -> Result<Self, Diagnostic> {
        if state.state_space_identity() != plan.state_space_identity() {
            return Err(invalid(
                "transient Run State belongs to a different exact common state space",
            ));
        }
        if output_steps.is_empty()
            || output_steps.windows(2).any(|pair| pair[0] >= pair[1])
            || output_steps
                .iter()
                .any(|step| *step == 0 || *step > accepted_steps.get())
        {
            return Err(invalid(
                "output_steps must be nonempty, strictly increasing accepted-step indices within the inclusive horizon",
            ));
        }
        let mut bytes = Vec::new();
        push_framed(&mut bytes, plan.identity().as_bytes());
        push_framed(&mut bytes, state.identity().as_bytes());
        let accepted_steps_u64 = u64::try_from(accepted_steps.get())
            .map_err(|_| invalid("transient Run horizon exceeds canonical u64 identity range"))?;
        bytes.extend_from_slice(&accepted_steps_u64.to_be_bytes());
        for step in &output_steps {
            let step = u64::try_from(*step).map_err(|_| {
                invalid("transient Run output index exceeds canonical u64 identity range")
            })?;
            bytes.extend_from_slice(&step.to_be_bytes());
        }
        bytes.extend_from_slice(&1_u64.to_be_bytes());
        let identity = hex_bytes(&Sha256::digest(
            [
                b"eqiora.common-transient-run-request/v1\0".as_slice(),
                bytes.as_slice(),
            ]
            .concat(),
        ));
        Ok(Self {
            plan,
            state,
            accepted_steps,
            output_steps,
            identity,
        })
    }

    #[must_use]
    pub const fn plan(&self) -> &CommonTransientFlowPlan {
        &self.plan
    }
    #[must_use]
    pub const fn state(&self) -> &CommonState {
        &self.state
    }
    #[must_use]
    pub const fn accepted_steps(&self) -> NonZeroUsize {
        self.accepted_steps
    }
    #[must_use]
    pub fn output_steps(&self) -> &[usize] {
        &self.output_steps
    }
    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }
}

fn exact_grid_index(
    start_s: f64,
    target_s: f64,
    step_s: f64,
    label: &str,
) -> Result<usize, Diagnostic> {
    if !target_s.is_finite() || target_s <= start_s {
        return Err(invalid(format!(
            "{label} values must be finite and later than State.time_s"
        )));
    }
    let raw = (target_s - start_s) / step_s;
    if !raw.is_finite() || raw < 1.0 || raw.fract() != 0.0 || raw > usize::MAX as f64 {
        return Err(invalid(format!(
            "{label} values must align exactly to the Plan Backward-Euler grid"
        )));
    }
    let index = raw as usize;
    let reconstructed = start_s + step_s * index as f64;
    if reconstructed.to_bits() != target_s.to_bits() {
        return Err(invalid(format!(
            "{label} values must align exactly to the Plan Backward-Euler grid"
        )));
    }
    Ok(index)
}

impl CommonState {
    fn new(
        state_space_identity: String,
        time_s: f64,
        model: Arc<ModelEnvelope>,
        resources: Arc<NativeMeshResources>,
        kind: CommonStateKind,
    ) -> Result<Self, Diagnostic> {
        if !time_s.is_finite() || time_s < 0.0 || time_s.to_bits() == (-0.0_f64).to_bits() {
            return Err(invalid("State time_s must be finite and non-negative"));
        }
        let mut bytes = Vec::new();
        push_framed(&mut bytes, state_space_identity.as_bytes());
        bytes.extend_from_slice(&time_s.to_bits().to_be_bytes());
        match &kind {
            CommonStateKind::MiniP1(state) => {
                push_framed(&mut bytes, b"mini-p1/backward-euler/no-extra-history/v1");
                for value in state
                    .velocity()
                    .vertex_values()
                    .iter()
                    .flatten()
                    .chain(state.velocity().cell_bubble_values().iter().flatten())
                    .chain(state.pressure().vertex_values())
                {
                    bytes.extend_from_slice(&value.to_bits().to_be_bytes());
                }
                match state.pressure_reference() {
                    SteadyStokesPressureReference2d::ZeroIntegral { multiplier } => {
                        push_framed(&mut bytes, b"zero-integral");
                        bytes.extend_from_slice(&multiplier.to_bits().to_be_bytes());
                    }
                    SteadyStokesPressureReference2d::BoundaryTraction => {
                        push_framed(&mut bytes, b"boundary-traction");
                    }
                }
            }
            CommonStateKind::CellCentered(state) => {
                push_framed(
                    &mut bytes,
                    b"cell-centered/backward-euler/bdf1-previous-accepted-face-volume-flux/v1",
                );
                for value in state
                    .velocity()
                    .values()
                    .iter()
                    .flatten()
                    .chain(state.pressure().values())
                    .chain(std::iter::once(&state.gauge_multiplier()))
                    .chain(state.previous_face_volume_fluxes())
                {
                    bytes.extend_from_slice(&value.to_bits().to_be_bytes());
                }
            }
        }
        let identity = hex_bytes(&Sha256::digest(
            [b"eqiora.common-state/v1\0".as_slice(), bytes.as_slice()].concat(),
        ));
        Ok(Self {
            state_space_identity,
            identity,
            time_s,
            model,
            resources,
            kind,
        })
    }

    /// Exact identity of Model, Mesh, complete fields/spaces, gauge, layout, and history schema.
    #[must_use]
    pub fn state_space_identity(&self) -> &str {
        &self.state_space_identity
    }

    /// Content identity of this exact accepted state occurrence.
    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// Exact coherent-SI model time.
    #[must_use]
    pub const fn time_s(&self) -> f64 {
        self.time_s
    }

    #[must_use]
    pub fn velocity_vertex_values(&self) -> Option<&[[f64; 2]]> {
        match &self.kind {
            CommonStateKind::MiniP1(state) => Some(state.velocity().vertex_values()),
            CommonStateKind::CellCentered(_) => None,
        }
    }

    #[must_use]
    pub fn velocity_cell_values(&self) -> &[[f64; 2]] {
        match &self.kind {
            CommonStateKind::MiniP1(state) => state.velocity().cell_bubble_values(),
            CommonStateKind::CellCentered(state) => state.velocity().values(),
        }
    }

    #[must_use]
    pub fn pressure_vertex_values(&self) -> Option<&[f64]> {
        match &self.kind {
            CommonStateKind::MiniP1(state) => Some(state.pressure().vertex_values()),
            CommonStateKind::CellCentered(_) => None,
        }
    }

    #[must_use]
    pub fn pressure_cell_values(&self) -> Option<&[f64]> {
        match &self.kind {
            CommonStateKind::MiniP1(_) => None,
            CommonStateKind::CellCentered(state) => Some(state.pressure().values()),
        }
    }

    #[cfg(test)]
    fn method_history_values(&self) -> &[f64] {
        match &self.kind {
            CommonStateKind::MiniP1(_) => &[],
            CommonStateKind::CellCentered(state) => state.previous_face_volume_fluxes(),
        }
    }
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

/// Resolve Model mathematics first, then admit the requested numerical policies.
pub fn resolve_common_plan(
    model: &ModelEnvelope,
    owner: AuthenticatedCommonMesh,
    spatial: CommonSpatialPolicy,
    solve: CommonSolvePolicy,
    scaling: Option<IncompressibleScalingRequest2d>,
    temporal: Option<CommonBackwardEuler>,
    stokes_backend: &dyn LinearSolverBackend,
) -> Result<ResolvedCommonPlan, Diagnostic> {
    let recognized = RecognizedNativeAdmission::recognize(model, owner)?;
    let requested_linear = match solve {
        CommonSolvePolicy::Linear(linear) | CommonSolvePolicy::Newton { linear, .. } => linear,
    };
    if requested_linear.algorithm() != LinearSolver::ConjugateGradient
        || requested_linear.preconditioner() != PreconditionerPolicy::Identity
        || requested_linear.reduction() != ReductionPolicy::Reproducible
    {
        return Err(invalid(
            "common Linear request must contain identity-preconditioned reproducible controls",
        ));
    }
    match recognized.capability {
        NativeCapability::ScalarElliptic => {
            let CommonSolvePolicy::Linear(solve) = solve else {
                return Err(invalid(
                    "scalar-elliptic mathematics requires Linear solve policy",
                ));
            };
            if temporal.is_some() {
                return Err(invalid(
                    "steady scalar-elliptic mathematics does not admit a temporal policy",
                ));
            }
            if scaling.is_some() {
                return Err(invalid(
                    "scalar-elliptic Model mathematics does not admit incompressible-flow scaling",
                ));
            }
            let spatial = match spatial {
                CommonSpatialPolicy::Q1 => NativeSpatialPolicy::ScalarQ1,
                CommonSpatialPolicy::CellCenteredTpfa => NativeSpatialPolicy::ScalarTpfa,
                CommonSpatialPolicy::MiniP1 => {
                    return Err(invalid(
                        "scalar-elliptic Model mathematics is incompatible with MINI/P1",
                    ));
                }
                CommonSpatialPolicy::CellCentered => {
                    return Err(invalid(
                        "scalar-elliptic Model mathematics is incompatible with incompressible CellCentered",
                    ));
                }
            };
            let linear = NativeLinearPolicy::exact(solve, &REFERENCE_LINEAR_SOLVER)?;
            let admission = recognized.complete(spatial, linear, None, None)?;
            CommonScalarPlan::from_admission(model, admission).map(|plan| ResolvedCommonPlan {
                kind: ResolvedCommonPlanKind::Scalar(Box::new(plan)),
            })
        }
        NativeCapability::IsotropicElasticity => {
            let CommonSolvePolicy::Linear(solve) = solve else {
                return Err(invalid(
                    "linear-elasticity mathematics requires Linear solve policy",
                ));
            };
            if temporal.is_some() {
                return Err(invalid(
                    "steady linear-elasticity mathematics does not admit a temporal policy",
                ));
            }
            if scaling.is_some() {
                return Err(invalid(
                    "linear-elasticity mathematics does not admit incompressible-flow scaling",
                ));
            }
            if spatial != CommonSpatialPolicy::Q1 {
                return Err(invalid(
                    "linear-elasticity mathematics requires the admitted Cartesian Q1 policy",
                ));
            }
            let linear = NativeLinearPolicy::exact(solve, &REFERENCE_LINEAR_SOLVER)?;
            let admission =
                recognized.complete(NativeSpatialPolicy::ElasticityQ1, linear, None, None)?;
            CommonElasticityPlan::from_admission(model, admission).map(|plan| ResolvedCommonPlan {
                kind: ResolvedCommonPlanKind::Elasticity(Box::new(plan)),
            })
        }
        NativeCapability::SteadyIncompressibleStokes => {
            let CommonSolvePolicy::Linear(solve) = solve else {
                return Err(invalid(
                    "steady-Stokes mathematics requires Linear solve policy",
                ));
            };
            if temporal.is_some() {
                return Err(invalid(
                    "steady-Stokes mathematics does not admit a temporal policy",
                ));
            }
            if spatial != CommonSpatialPolicy::MiniP1 {
                return Err(invalid(
                    "steady-Stokes Model mathematics requires the admitted MINI/P1 policy",
                ));
            }
            let RecognizedNativeModel::Stokes(binding) = &recognized.recognized else {
                unreachable!("steady-Stokes capability recognition returns a Stokes binding")
            };
            let scaling = binding.resolve_incompressible_scaling(model, scaling)?;
            let effective_solve = SolverPlan::new(
                LinearSolver::SparseLu,
                solve.relative_tolerance(),
                solve.absolute_tolerance(),
                solve.maximum_iterations(),
            )?
            .with_reduction(ReductionPolicy::Fast);
            let linear = NativeLinearPolicy::exact(effective_solve, stokes_backend)?;
            let admission = recognized.complete(
                NativeSpatialPolicy::StokesMiniP1(scaling.scales()),
                linear,
                None,
                None,
            )?;
            CommonSteadyStokesPlan::from_admission(model, admission, scaling).map(|plan| {
                ResolvedCommonPlan {
                    kind: ResolvedCommonPlanKind::SteadyStokes(Box::new(plan)),
                }
            })
        }
        NativeCapability::TransientIncompressibleFlow => {
            let CommonSolvePolicy::Newton { nonlinear, linear } = solve else {
                return Err(invalid(
                    "transient incompressible-flow mathematics requires Newton(linear=...) policy",
                ));
            };
            let temporal = temporal.ok_or_else(|| {
                invalid("transient incompressible-flow mathematics requires BackwardEuler")
            })?;
            let (geometry, mesh, correspondence, _) =
                resource_artifact_digests(&recognized.resources)?;
            let scaling = resolve_complete_manual_incompressible_scaling_2d(
                scaling,
                model.digest()?,
                geometry,
                correspondence,
                mesh,
            )?;
            let effective_linear = SolverPlan::new(
                LinearSolver::BiConjugateGradientStabilized,
                linear.relative_tolerance(),
                linear.absolute_tolerance(),
                linear.maximum_iterations(),
            )?
            .with_preconditioner(PreconditionerPolicy::Identity)
            .with_reduction(match spatial {
                CommonSpatialPolicy::MiniP1 => ReductionPolicy::Fast,
                CommonSpatialPolicy::CellCentered => ReductionPolicy::Reproducible,
                CommonSpatialPolicy::Q1 | CommonSpatialPolicy::CellCenteredTpfa => {
                    return Err(invalid(
                        "transient incompressible-flow mathematics requires MINI/P1 or CellCentered",
                    ));
                }
            });
            let linear_backend: &dyn LinearSolverBackend = match spatial {
                CommonSpatialPolicy::MiniP1 => stokes_backend,
                CommonSpatialPolicy::CellCentered => &REFERENCE_LINEAR_SOLVER,
                CommonSpatialPolicy::Q1 | CommonSpatialPolicy::CellCenteredTpfa => unreachable!(),
            };
            let linear = NativeLinearPolicy::exact(effective_linear, linear_backend)?;
            let native_spatial = match spatial {
                CommonSpatialPolicy::MiniP1 => {
                    NativeSpatialPolicy::TransientMiniP1(scaling.scales())
                }
                CommonSpatialPolicy::CellCentered => {
                    NativeSpatialPolicy::TransientCellCentered(scaling.scales())
                }
                CommonSpatialPolicy::Q1 | CommonSpatialPolicy::CellCenteredTpfa => unreachable!(),
            };
            let admission =
                recognized.complete(native_spatial, linear, Some(temporal), Some(nonlinear))?;
            CommonTransientFlowPlan::from_admission(model, admission, scaling, temporal, nonlinear)
                .map(|plan| ResolvedCommonPlan {
                    kind: ResolvedCommonPlanKind::TransientFlow(Box::new(plan)),
                })
        }
    }
}

impl CommonSteadyStokesPlan {
    fn from_admission(
        model: &ModelEnvelope,
        admission: NativeNumericalAdmission,
        scaling: ResolvedIncompressibleScaling2d,
    ) -> Result<Self, Diagnostic> {
        let model_reference = model.artifact_reference()?;
        let model_id = model_reference.model().ulid().to_string();
        let binding = admission.stokes_binding()?;
        let (resolved, realization, velocity_space, pressure_space) =
            admission.resolve_stokes(&binding)?;
        let (geometry_digest, mesh_digest, correspondence_digest, production_digest) =
            resource_digests(admission.resources())?;
        let mut velocity_field_id = None;
        let mut pressure_field_id = None;
        for field in resolved.plan().spatial().field_spaces() {
            match field.space().family() {
                SpaceFamily::SimplexP1Bubble => {
                    velocity_field_id = Some(field.field().ulid().to_string());
                }
                SpaceFamily::ContinuousLagrange { order } if order == std::num::NonZeroU16::MIN => {
                    pressure_field_id = Some(field.field().ulid().to_string());
                }
                _ => {}
            }
        }
        let (velocity_field_id, pressure_field_id) = velocity_field_id
            .zip(pressure_field_id)
            .ok_or_else(|| invalid("steady-Stokes Plan omitted its MINI/P1 Field identities"))?;
        let realization_digest = realization.digest()?.to_string();
        let scaling_provenance_digest = scaling.receipt().provenance_digest();
        let mut identity_bytes = Vec::new();
        for value in [
            admission.model_digest(),
            geometry_digest.as_str(),
            mesh_digest.as_str(),
            correspondence_digest.as_str(),
            production_digest.as_str(),
            realization_digest.as_str(),
            admission.policy_identity(),
            scaling_provenance_digest.as_str(),
        ] {
            push_framed(&mut identity_bytes, value.as_bytes());
        }
        let identity = hex_bytes(&Sha256::digest(
            [
                b"eqiora.common-steady-stokes-plan/v1\0".as_slice(),
                identity_bytes.as_slice(),
            ]
            .concat(),
        ));
        Ok(Self {
            admission,
            binding,
            resolved,
            realization,
            scaling,
            realization_digest,
            identity,
            model_id,
            model_revision: model_reference.semantic_revision().get(),
            geometry_digest,
            mesh_digest,
            correspondence_digest,
            production_digest,
            velocity_field_id,
            pressure_field_id,
            velocity_space,
            pressure_space,
        })
    }

    /// Execute solely from the state retained by this Plan.
    pub fn run(
        &self,
        backend: &dyn LinearSolverBackend,
    ) -> Result<crate::fluid::SteadyStokesMiniSolution2d, Diagnostic> {
        self.admission.revalidate()?;
        if backend.provider() != self.admission.linear.provider
            || backend.capabilities() != self.admission.linear.capabilities
        {
            return Err(invalid(
                "steady-Stokes execution backend differs from the admitted provider or capabilities",
            ));
        }
        let solution = solve_resolved_steady_stokes_geometry_mini_2d(
            &self.admission.program,
            &self.resolved,
            &self.binding,
            backend,
        )?;
        if solution.scales() != self.scaling.scales() {
            return Err(invalid(
                "steady-Stokes execution changed the Plan-owned effective scaling",
            ));
        }
        Ok(solution)
    }

    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }
    #[must_use]
    pub fn model_id(&self) -> &str {
        &self.model_id
    }
    #[must_use]
    pub const fn model_revision(&self) -> u64 {
        self.model_revision
    }
    #[must_use]
    pub fn model_digest(&self) -> &str {
        self.admission.model_digest()
    }
    #[must_use]
    pub fn geometry_digest(&self) -> &str {
        &self.geometry_digest
    }
    #[must_use]
    pub fn mesh_digest(&self) -> &str {
        &self.mesh_digest
    }
    #[must_use]
    pub fn correspondence_digest(&self) -> &str {
        &self.correspondence_digest
    }
    #[must_use]
    pub fn production_digest(&self) -> &str {
        &self.production_digest
    }
    pub fn realization_digest(&self) -> &str {
        &self.realization_digest
    }
    #[must_use]
    pub const fn scaling_receipt(&self) -> &IncompressibleScalingReceipt2d {
        self.scaling.receipt()
    }
    #[must_use]
    pub fn velocity_field_id(&self) -> &str {
        &self.velocity_field_id
    }
    #[must_use]
    pub fn pressure_field_id(&self) -> &str {
        &self.pressure_field_id
    }
    #[must_use]
    pub const fn velocity_space(&self) -> Space {
        self.velocity_space
    }
    #[must_use]
    pub const fn pressure_space(&self) -> Space {
        self.pressure_space
    }
    #[must_use]
    pub const fn scales(&self) -> IncompressibleFlowScaleProfile2d {
        self.scaling.scales()
    }
    #[must_use]
    pub const fn linear(&self) -> SolverPlan {
        self.admission.linear.solver
    }
}

impl CommonTransientFlowPlan {
    fn from_admission(
        model: &ModelEnvelope,
        admission: NativeNumericalAdmission,
        scaling: ResolvedIncompressibleScaling2d,
        temporal: CommonBackwardEuler,
        nonlinear: NonlinearSolvePlan,
    ) -> Result<Self, Diagnostic> {
        let model_reference = model.artifact_reference()?;
        let model_id = model_reference.model().ulid().to_string();
        let RecognizedNativeModel::Transient(transient) = &admission.recognized else {
            return Err(invalid(
                "transient Plan admission omitted recognized transient Model meaning",
            ));
        };
        let velocity_field_id = transient.velocity().ulid().to_string();
        let pressure_field_id = transient.pressure().ulid().to_string();
        let solver = admission.linear.solver;
        let (resolved, velocity_space, pressure_space, gauge) =
            match (admission.spatial, &admission.resources) {
                (
                    NativeSpatialPolicy::TransientMiniP1(scales),
                    NativeMeshResources::AffineTriangleSimplicial { mesh, .. },
                ) => {
                    let plan = transient_navier_stokes_mini_plan_2d(
                        transient,
                        mesh.artifact_reference()?,
                        scales,
                        temporal.step(),
                        nonlinear,
                        solver,
                    )?;
                    let capabilities = transient_realization_capabilities(
                        DiscretizationMethod::ContinuousGalerkin,
                        MeshKind::ImportedAffineSimplicial,
                        solver,
                        &admission.linear.capabilities,
                    )?;
                    let resolved = resolve_transient_fieldwise(
                        &TransientFieldwiseRealizationRequest::explicit(
                            admission.program.model(),
                            SemanticRevision::new(admission.program.revision().0),
                            RealizationRevision::new(TRANSIENT_REALIZATION_REVISION),
                            plan,
                        ),
                        transient_navier_stokes_fieldwise_requirements_2d(transient),
                        &capabilities,
                    )?;
                    let gauge = if resolved
                        .plan()
                        .fieldwise()
                        .spatial()
                        .constraints()
                        .is_empty()
                    {
                        CommonPressureGauge2d::BoundaryTraction
                    } else {
                        CommonPressureGauge2d::ZeroIntegral
                    };
                    (
                        CommonTransientResolvedSpatial::MiniP1(resolved),
                        Space::simplex_p1_bubble(),
                        Space::continuous_lagrange(std::num::NonZeroU16::MIN),
                        gauge,
                    )
                }
                (
                    NativeSpatialPolicy::TransientCellCentered(scales),
                    NativeMeshResources::Cartesian { mesh, .. },
                ) => {
                    let artifact = mesh.artifact_reference()?;
                    let cells =
                        [
                            NonZeroUsize::new(mesh.mesh().axis_cell_count(0).ok_or_else(|| {
                                invalid("supplied Cartesian mesh omitted x cells")
                            })?)
                            .ok_or_else(|| invalid("supplied Cartesian x cell count is zero"))?,
                            NonZeroUsize::new(mesh.mesh().axis_cell_count(1).ok_or_else(|| {
                                invalid("supplied Cartesian mesh omitted y cells")
                            })?)
                            .ok_or_else(|| invalid("supplied Cartesian y cell count is zero"))?,
                        ];
                    let plan = transient_navier_stokes_cell_centered_plan_2d(
                        transient,
                        MeshPolicy::SuppliedCartesian { artifact, cells },
                        scales,
                        temporal.step(),
                        nonlinear,
                        solver,
                    )?;
                    let capabilities = transient_realization_capabilities(
                        DiscretizationMethod::CellCenteredFiniteVolume,
                        MeshKind::SuppliedCartesian,
                        solver,
                        &admission.linear.capabilities,
                    )?;
                    let resolved = resolve_transient_cell_centered_incompressible_flow(
                        &TransientCellCenteredIncompressibleFlowRealizationRequest::explicit(
                            admission.program.model(),
                            SemanticRevision::new(admission.program.revision().0),
                            RealizationRevision::new(TRANSIENT_REALIZATION_REVISION),
                            plan,
                        ),
                        transient_navier_stokes_cell_centered_requirements_2d(transient),
                        &TransientCellCenteredIncompressibleFlowCapabilities::new(capabilities),
                    )?;
                    (
                        CommonTransientResolvedSpatial::CellCentered(resolved),
                        Space::cell_constant(),
                        Space::cell_constant(),
                        CommonPressureGauge2d::ZeroIntegral,
                    )
                }
                _ => {
                    return Err(invalid(
                        "transient spatial policy and exact caller Mesh are cross-wired",
                    ));
                }
            };
        let (geometry_digest, mesh_digest, correspondence_digest, production_digest) =
            resource_digests(&admission.resources)?;
        let receipt_digest = scaling.receipt().provenance_digest();
        let mut identity_bytes = Vec::new();
        for value in [
            admission.model_digest(),
            model_id.as_str(),
            geometry_digest.as_str(),
            mesh_digest.as_str(),
            correspondence_digest.as_str(),
            production_digest.as_str(),
            admission.policy_identity(),
            receipt_digest.as_str(),
            velocity_field_id.as_str(),
            pressure_field_id.as_str(),
        ] {
            push_framed(&mut identity_bytes, value.as_bytes());
        }
        identity_bytes.extend_from_slice(&model_reference.semantic_revision().get().to_be_bytes());
        identity_bytes.extend_from_slice(&TRANSIENT_REALIZATION_REVISION.to_be_bytes());
        identity_bytes.extend_from_slice(&COMMON_TRANSIENT_RESOLVER_EPOCH.to_be_bytes());
        match &admission.resources {
            NativeMeshResources::AffineTriangleSimplicial { mesh, .. } => {
                push_framed(&mut identity_bytes, b"imported-affine-simplicial");
                push_framed(
                    &mut identity_bytes,
                    mesh.artifact_reference()?.sha256().as_slice(),
                );
            }
            NativeMeshResources::Cartesian { mesh, .. } => {
                push_framed(&mut identity_bytes, b"supplied-cartesian");
                push_framed(
                    &mut identity_bytes,
                    mesh.artifact_reference()?.sha256().as_slice(),
                );
                for axis in 0..2 {
                    let count = mesh.mesh().axis_cell_count(axis).ok_or_else(|| {
                        invalid("supplied Cartesian transient Mesh omitted an axis")
                    })?;
                    identity_bytes.extend_from_slice(&count.to_be_bytes());
                }
            }
            NativeMeshResources::ReferenceSimplicial { .. }
            | NativeMeshResources::GmshSimplicial { .. } => {
                return Err(invalid(
                    "transient common Plan requires the exact caller affine-triangle or supplied-Cartesian envelope",
                ));
            }
        }
        push_framed(&mut identity_bytes, space_identity(velocity_space));
        push_framed(&mut identity_bytes, space_identity(pressure_space));
        push_framed(
            &mut identity_bytes,
            match gauge {
                CommonPressureGauge2d::ZeroIntegral => b"zero-integral",
                CommonPressureGauge2d::BoundaryTraction => b"boundary-traction",
            },
        );
        for discriminant in [
            b"f64".as_slice(),
            b"replicated".as_slice(),
            b"general-operator".as_slice(),
            b"host-cpu".as_slice(),
            b"host-serial".as_slice(),
            b"offline".as_slice(),
        ] {
            push_framed(&mut identity_bytes, discriminant);
        }
        let identity = hex_bytes(&Sha256::digest(
            [
                b"eqiora.common-transient-flow-plan/v1\0".as_slice(),
                identity_bytes.as_slice(),
            ]
            .concat(),
        ));
        Ok(Self {
            admission,
            resolved,
            scaling,
            temporal,
            nonlinear,
            identity,
            model_id,
            model_revision: model_reference.semantic_revision().get(),
            geometry_digest,
            mesh_digest,
            correspondence_digest,
            production_digest,
            velocity_field_id,
            pressure_field_id,
            velocity_space,
            pressure_space,
            gauge,
        })
    }

    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }
    #[must_use]
    pub fn model_id(&self) -> &str {
        &self.model_id
    }
    #[must_use]
    pub const fn model_revision(&self) -> u64 {
        self.model_revision
    }
    #[must_use]
    pub fn model_digest(&self) -> &str {
        self.admission.model_digest()
    }
    #[must_use]
    pub fn geometry_digest(&self) -> &str {
        &self.geometry_digest
    }
    #[must_use]
    pub fn mesh_digest(&self) -> &str {
        &self.mesh_digest
    }
    #[must_use]
    pub fn correspondence_digest(&self) -> &str {
        &self.correspondence_digest
    }
    #[must_use]
    pub fn production_digest(&self) -> &str {
        &self.production_digest
    }
    #[must_use]
    pub const fn scaling_receipt(&self) -> &IncompressibleScalingReceipt2d {
        self.scaling.receipt()
    }
    #[must_use]
    pub const fn scales(&self) -> IncompressibleFlowScaleProfile2d {
        self.scaling.scales()
    }
    #[must_use]
    pub const fn temporal(&self) -> CommonBackwardEuler {
        self.temporal
    }
    #[must_use]
    pub const fn nonlinear(&self) -> NonlinearSolvePlan {
        self.nonlinear
    }
    #[must_use]
    pub const fn linear(&self) -> SolverPlan {
        self.admission.linear.solver
    }
    #[must_use]
    pub fn velocity_field_id(&self) -> &str {
        &self.velocity_field_id
    }
    #[must_use]
    pub fn pressure_field_id(&self) -> &str {
        &self.pressure_field_id
    }
    #[must_use]
    pub fn domain_id(&self) -> String {
        match &self.admission.recognized {
            RecognizedNativeModel::Transient(model) => model.domain().ulid().to_string(),
            _ => unreachable!("common transient Plan retains transient Model meaning"),
        }
    }
    #[must_use]
    pub const fn velocity_space(&self) -> Space {
        self.velocity_space
    }
    #[must_use]
    pub const fn pressure_space(&self) -> Space {
        self.pressure_space
    }
    #[must_use]
    pub const fn gauge(&self) -> CommonPressureGauge2d {
        self.gauge
    }
    #[must_use]
    pub const fn spatial(&self) -> CommonSpatialPolicy {
        match self.resolved {
            CommonTransientResolvedSpatial::MiniP1(_) => CommonSpatialPolicy::MiniP1,
            CommonTransientResolvedSpatial::CellCentered(_) => CommonSpatialPolicy::CellCentered,
        }
    }

    /// Identity of the complete restartable state space, excluding solve and Run controls.
    #[must_use]
    pub fn state_space_identity(&self) -> String {
        let mut bytes = Vec::new();
        for value in [
            self.model_digest(),
            self.geometry_digest(),
            self.mesh_digest(),
            self.correspondence_digest(),
            self.production_digest(),
            self.velocity_field_id(),
            self.pressure_field_id(),
            "f64",
            "replicated",
        ] {
            push_framed(&mut bytes, value.as_bytes());
        }
        push_framed(&mut bytes, space_identity(self.velocity_space()));
        push_framed(&mut bytes, space_identity(self.pressure_space()));
        push_framed(
            &mut bytes,
            match self.gauge() {
                CommonPressureGauge2d::ZeroIntegral => b"zero-integral",
                CommonPressureGauge2d::BoundaryTraction => b"boundary-traction",
            },
        );
        push_framed(
            &mut bytes,
            match self.spatial() {
                CommonSpatialPolicy::MiniP1 => b"mini-p1/backward-euler/no-extra-history/v1",
                CommonSpatialPolicy::CellCentered => {
                    b"cell-centered/backward-euler/bdf1-previous-accepted-face-volume-flux/v1"
                }
                _ => unreachable!("closed transient spatial policy"),
            },
        );
        hex_bytes(&Sha256::digest(
            [
                b"eqiora.common-state-space/v1\0".as_slice(),
                bytes.as_slice(),
            ]
            .concat(),
        ))
    }

    /// Construct the sole explicit homogeneous-zero bootstrap for this transient Plan.
    pub fn zero_state(&self, time_s: f64) -> Result<CommonState, Diagnostic> {
        self.admission.revalidate()?;
        let RecognizedNativeModel::Transient(model) = &self.admission.recognized else {
            return Err(invalid("State.zero requires a transient Plan"));
        };
        crate::canonical_stokes::require_complete_zero_trace(model)?;
        let time = DynQuantity::new(time_s, TIME);
        let kind = match (&self.resolved, &self.admission.resources) {
            (
                CommonTransientResolvedSpatial::MiniP1(_),
                NativeMeshResources::AffineTriangleSimplicial { mesh, .. },
            ) => {
                let mesh_data = mesh.mesh().clone();
                let vertex_count = mesh_data.vertices().len();
                let cell_count = mesh_data.entity_count(2).ok_or_else(|| {
                    invalid("affine-triangle transient Mesh omitted two-dimensional cells")
                })?;
                let reference = match self.gauge {
                    CommonPressureGauge2d::ZeroIntegral => {
                        SteadyStokesPressureReference2d::ZeroIntegral { multiplier: 0.0 }
                    }
                    CommonPressureGauge2d::BoundaryTraction => {
                        SteadyStokesPressureReference2d::BoundaryTraction
                    }
                };
                CommonStateKind::MiniP1(Box::new(TransientNavierStokesInitialState2d::new(
                    model,
                    time,
                    mesh.artifact_reference()?,
                    SimplicialMiniVelocityField2d::new(
                        mesh_data.clone(),
                        vec![[0.0; 2]; vertex_count],
                        vec![[0.0; 2]; cell_count],
                    )?,
                    SimplicialP1Field::new(mesh_data, vec![0.0; vertex_count])?,
                    reference,
                )?))
            }
            (
                CommonTransientResolvedSpatial::CellCentered(_),
                NativeMeshResources::Cartesian { mesh, .. },
            ) => {
                let mesh_data = mesh.mesh().clone();
                let cell_count = mesh_data.entity_count(2).ok_or_else(|| {
                    invalid("Cartesian transient Mesh omitted two-dimensional cells")
                })?;
                let facet_count =
                    crate::cartesian_fvm_geometry::cartesian_fvm_geometry_2d(&mesh_data)?
                        .1
                        .len();
                CommonStateKind::CellCentered(Box::new(
                    CellCenteredNavierStokesInitialState2d::new(
                        model,
                        time,
                        CellCenteredVelocityField2d::new(
                            mesh_data.clone(),
                            vec![[0.0; 2]; cell_count],
                        )?,
                        CellCenteredPressureField2d::new(mesh_data, vec![0.0; cell_count])?,
                        0.0,
                        vec![0.0; facet_count],
                    )?,
                ))
            }
            _ => {
                return Err(invalid(
                    "transient Plan lost its exact caller Mesh envelope",
                ));
            }
        };
        CommonState::new(
            self.state_space_identity(),
            time_s,
            Arc::new(self.admission.model.clone()),
            Arc::new(self.admission.resources.clone()),
            kind,
        )
    }

    /// Advance exactly one accepted Backward-Euler step from a compatible complete State.
    pub fn advance_one(
        &self,
        state: &CommonState,
        backend: &dyn LinearSolverBackend,
    ) -> Result<CommonState, Diagnostic> {
        self.admission.revalidate()?;
        if state.state_space_identity != self.state_space_identity() {
            return Err(invalid(
                "State belongs to a different exact common state space",
            ));
        }
        if backend.provider() != self.admission.linear.provider
            || backend.capabilities() != self.admission.linear.capabilities
        {
            return Err(invalid(
                "transient execution backend differs from the admitted provider or capabilities",
            ));
        }
        let run = TransientNavierStokesRun2d::new(NonZeroStepCount::new(NonZeroUsize::MIN));
        let next_kind = match (&self.resolved, &self.admission.resources, &state.kind) {
            (
                CommonTransientResolvedSpatial::MiniP1(resolved),
                NativeMeshResources::AffineTriangleSimplicial { mesh, .. },
                CommonStateKind::MiniP1(initial),
            ) => {
                let trajectory = advance_resolved_transient_navier_stokes_mini_2d(
                    &self.admission.program,
                    resolved,
                    mesh,
                    initial.as_ref().clone(),
                    run,
                    backend,
                )?;
                let accepted = trajectory
                    .states()
                    .last()
                    .ok_or_else(|| invalid("MINI transient step returned no accepted State"))?;
                CommonStateKind::MiniP1(Box::new(mini_initial_from_resolved(self, mesh, accepted)?))
            }
            (
                CommonTransientResolvedSpatial::CellCentered(resolved),
                NativeMeshResources::Cartesian { mesh, .. },
                CommonStateKind::CellCentered(initial),
            ) => {
                let trajectory = advance_resolved_transient_navier_stokes_cell_centered_2d(
                    &self.admission.program,
                    resolved,
                    mesh,
                    initial.as_ref().clone(),
                    run,
                    backend,
                )?;
                let accepted = trajectory.states().last().ok_or_else(|| {
                    invalid("cell-centered transient step returned no accepted State")
                })?;
                CommonStateKind::CellCentered(Box::new(cell_centered_initial_from_resolved(
                    self, accepted,
                )?))
            }
            _ => {
                return Err(invalid(
                    "State method history is incompatible with this Plan",
                ));
            }
        };
        let next_time = state.time_s + self.temporal.step().value();
        CommonState::new(
            self.state_space_identity(),
            next_time,
            Arc::clone(&state.model),
            Arc::clone(&state.resources),
            next_kind,
        )
    }
}

fn mini_initial_from_resolved(
    plan: &CommonTransientFlowPlan,
    mesh: &SimplicialMeshEnvelopeV1,
    state: &ResolvedTransientNavierStokesState2d,
) -> Result<TransientNavierStokesInitialState2d, Diagnostic> {
    let RecognizedNativeModel::Transient(model) = &plan.admission.recognized else {
        return Err(invalid("transient Plan lost recognized Model meaning"));
    };
    TransientNavierStokesInitialState2d::new(
        model,
        state.time(),
        mesh.artifact_reference()?,
        state.velocity().clone(),
        state.pressure().clone(),
        state.pressure_reference(),
    )
}

fn cell_centered_initial_from_resolved(
    plan: &CommonTransientFlowPlan,
    state: &ResolvedCellCenteredNavierStokesState2d,
) -> Result<CellCenteredNavierStokesInitialState2d, Diagnostic> {
    let RecognizedNativeModel::Transient(model) = &plan.admission.recognized else {
        return Err(invalid("transient Plan lost recognized Model meaning"));
    };
    CellCenteredNavierStokesInitialState2d::new(
        model,
        state.time(),
        state.velocity().clone(),
        state.pressure().clone(),
        state.gauge_multiplier(),
        state.previous_face_volume_fluxes().to_vec(),
    )
}

fn transient_realization_capabilities(
    method: DiscretizationMethod,
    mesh: MeshKind,
    solver: SolverPlan,
    backend: &SolverCapabilities,
) -> Result<RealizationCapabilities, Diagnostic> {
    backend.require_problem(solver, ScalarType::F64, LinearOperatorProperties::General)?;
    RealizationCapabilities::cartesian_product(
        [method],
        [(
            mesh,
            SpatialDimensionSupport::exact(NonZeroUsize::new(2).expect("two")),
        )],
        [VectorLayoutKind::Replicated],
        SolverCapabilities::exact([SolverCapability {
            algorithm: solver.algorithm(),
            operator_properties: LinearOperatorProperties::General,
            preconditioner: solver.preconditioner(),
            reduction: solver.reduction(),
            scalar_type: ScalarType::F64,
        }])?,
        TargetCapabilities::none().with_host_cpu(NonZeroUsize::MIN),
    )
}

impl CommonElasticityPlan {
    fn from_admission(
        model: &ModelEnvelope,
        admission: NativeNumericalAdmission,
    ) -> Result<Self, Diagnostic> {
        let model_reference = model.artifact_reference()?;
        let NativeMeshResources::Cartesian { mesh, .. } = &admission.resources else {
            return Err(invalid(
                "linear-elasticity common Plan requires an authenticated Cartesian Mesh",
            ));
        };
        let cells = [
            mesh.mesh()
                .axis_cell_count(0)
                .ok_or_else(|| invalid("elasticity Plan Mesh omitted x-axis cells"))?,
            mesh.mesh()
                .axis_cell_count(1)
                .ok_or_else(|| invalid("elasticity Plan Mesh omitted y-axis cells"))?,
        ];
        let RecognizedNativeModel::Elasticity(lowered) = &admission.recognized else {
            return Err(invalid(
                "common elasticity Plan omitted recognized elasticity meaning",
            ));
        };
        let displacement_field_id = lowered.displacement().ulid().to_string();
        let (geometry_digest, mesh_digest, correspondence_digest, production_digest) =
            resource_digests(admission.resources())?;
        let mut identity_bytes = Vec::new();
        for value in [
            admission.model_digest(),
            geometry_digest.as_str(),
            mesh_digest.as_str(),
            correspondence_digest.as_str(),
            production_digest.as_str(),
            admission.policy_identity(),
        ] {
            push_framed(&mut identity_bytes, value.as_bytes());
        }
        let identity = hex_bytes(&Sha256::digest(
            [
                b"eqiora.common-linear-elasticity-plan/v1\0".as_slice(),
                identity_bytes.as_slice(),
            ]
            .concat(),
        ));
        Ok(Self {
            admission,
            identity,
            model_id: model_reference.model().ulid().to_string(),
            model_revision: model_reference.semantic_revision().get(),
            geometry_digest,
            mesh_digest,
            correspondence_digest,
            production_digest,
            displacement_field_id,
            cells,
        })
    }

    pub fn run(&self) -> Result<CartesianLinearElasticity2dSolution, Diagnostic> {
        self.admission.execute_elasticity(&REFERENCE_LINEAR_SOLVER)
    }

    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }
    #[must_use]
    pub fn model_id(&self) -> &str {
        &self.model_id
    }
    #[must_use]
    pub const fn model_revision(&self) -> u64 {
        self.model_revision
    }
    #[must_use]
    pub fn model_digest(&self) -> &str {
        self.admission.model_digest()
    }
    #[must_use]
    pub fn geometry_digest(&self) -> &str {
        &self.geometry_digest
    }
    #[must_use]
    pub fn mesh_digest(&self) -> &str {
        &self.mesh_digest
    }
    #[must_use]
    pub fn correspondence_digest(&self) -> &str {
        &self.correspondence_digest
    }
    #[must_use]
    pub fn production_digest(&self) -> &str {
        &self.production_digest
    }
    #[must_use]
    pub fn displacement_field_id(&self) -> &str {
        &self.displacement_field_id
    }
    #[must_use]
    pub const fn cells(&self) -> [usize; 2] {
        self.cells
    }
    #[must_use]
    pub const fn linear(&self) -> SolverPlan {
        self.admission.linear.solver
    }
}

fn resolve_common_scalar_portable(
    admission: &NativeNumericalAdmission,
    lowered: &ScalarEllipticCartesianModel,
    mesh: &CartesianMeshEnvelopeV1,
    cells: [usize; 2],
) -> Result<PortableRealizationGraph, Diagnostic> {
    let cells = cells
        .map(|count| NonZeroUsize::new(count).expect("validated Cartesian cells are non-zero"));
    let (method, space, quadrature) = match admission.spatial {
        NativeSpatialPolicy::ScalarQ1 => (
            DiscretizationMethod::ContinuousGalerkin,
            Space::continuous_lagrange(std::num::NonZeroU16::MIN),
            QuadraturePolicy::GaussLegendre {
                points_per_axis: NonZeroUsize::new(2).expect("two is non-zero"),
            },
        ),
        NativeSpatialPolicy::ScalarTpfa => (
            DiscretizationMethod::CellCenteredFiniteVolume,
            Space::cell_constant(),
            QuadraturePolicy::CellCentroid,
        ),
        NativeSpatialPolicy::ElasticityQ1
        | NativeSpatialPolicy::StokesMiniP1(_)
        | NativeSpatialPolicy::TransientMiniP1(_)
        | NativeSpatialPolicy::TransientCellCentered(_) => {
            return Err(invalid(
                "common scalar portable graph received a non-scalar spatial policy",
            ));
        }
    };
    let solver = admission.linear.solver;
    let plan = RealizationPlan::new(
        space,
        Discretization::new(
            method,
            MeshPolicy::SuppliedCartesian {
                artifact: mesh.artifact_reference()?,
                cells,
            },
            quadrature,
        ),
        solver,
        Target::HostCpu {
            threads: admission.linear.workers,
        },
        ExecutionSchedule::Offline,
    )?;
    admission.linear.capabilities.require_problem(
        solver,
        ScalarType::F64,
        LinearOperatorProperties::SymmetricPositiveDefinite,
    )?;
    let capabilities = RealizationCapabilities::cartesian_product(
        [method],
        [(
            MeshKind::SuppliedCartesian,
            SpatialDimensionSupport::exact(NonZeroUsize::new(2).expect("two is non-zero")),
        )],
        [VectorLayoutKind::Replicated],
        SolverCapabilities::exact([SolverCapability {
            algorithm: solver.algorithm(),
            operator_properties: LinearOperatorProperties::SymmetricPositiveDefinite,
            preconditioner: solver.preconditioner(),
            reduction: solver.reduction(),
            scalar_type: ScalarType::F64,
        }])?,
        TargetCapabilities::none().with_host_cpu(admission.linear.workers),
    )?;
    let resolved = resolve(
        &RealizationRequest::explicit(
            admission.program.model(),
            SemanticRevision::new(admission.program.revision().0),
            RealizationRevision::new(COMMON_SCALAR_REALIZATION_REVISION),
            plan,
        ),
        RealizationRequirements::new(
            NonZeroUsize::new(2).expect("two is non-zero"),
            ScalarType::F64,
            VectorLayoutKind::Replicated,
        ),
        &capabilities,
    )?;
    resolved.portable_graph(SingleFieldOperatorClaim::new(
        lowered.domain_id(),
        lowered.field_id(),
        LinearOperatorProperties::SymmetricPositiveDefinite,
    ))
}

impl CommonScalarPlan {
    fn from_admission(
        model: &ModelEnvelope,
        admission: NativeNumericalAdmission,
    ) -> Result<Self, Diagnostic> {
        let model_reference = model.artifact_reference()?;
        let NativeMeshResources::Cartesian {
            geometry,
            mesh,
            correspondence,
            production,
        } = &admission.resources
        else {
            return Err(invalid(
                "scalar Q1/TPFA common Plan requires an authenticated Cartesian Mesh",
            ));
        };
        let geometry_digest = hex_bytes(&geometry.digest_bytes());
        let mesh_digest = mesh.digest()?.to_string();
        let correspondence_digest = correspondence.digest()?.to_string();
        let production_digest = production.digest()?.to_string();
        let cells = [
            mesh.mesh()
                .axis_cell_count(0)
                .ok_or_else(|| invalid("common Plan Mesh omitted x-axis cells"))?,
            mesh.mesh()
                .axis_cell_count(1)
                .ok_or_else(|| invalid("common Plan Mesh omitted y-axis cells"))?,
        ];
        let RecognizedNativeModel::Scalar(lowered) = &admission.recognized else {
            return Err(invalid(
                "common scalar Plan admitted non-scalar mathematics",
            ));
        };
        let portable = resolve_common_scalar_portable(&admission, lowered, mesh, cells)?;
        let field = lowered.field_id();
        let field_id = field.ulid().to_string();
        let mut identity_bytes = Vec::new();
        for value in [
            admission.model_digest(),
            geometry_digest.as_str(),
            mesh_digest.as_str(),
            correspondence_digest.as_str(),
            production_digest.as_str(),
            admission.policy_identity(),
        ] {
            push_framed(&mut identity_bytes, value.as_bytes());
        }
        let identity = hex_bytes(&Sha256::digest(
            [
                b"eqiora.common-scalar-plan/v1\0".as_slice(),
                identity_bytes.as_slice(),
            ]
            .concat(),
        ));
        Ok(Self {
            admission,
            portable,
            identity,
            model_id: model_reference.model().ulid().to_string(),
            model_revision: model_reference.semantic_revision().get(),
            geometry_digest,
            mesh_digest,
            correspondence_digest,
            production_digest,
            field,
            field_id,
            cells,
        })
    }

    /// Execute solely from retained Plan state.
    pub fn run(&self) -> Result<ResolvedScalarEllipticCartesianSolution, Diagnostic> {
        self.admission.execute_scalar(&REFERENCE_LINEAR_SOLVER)
    }

    /// Accept one selected Parameter point through this Plan's exact supplied Mesh and policies.
    ///
    /// `values=None` selects the Model's canonical values. Otherwise only the ordered selected
    /// Parameter values vary; Model structure and every numerical resource remain Plan-owned.
    pub fn differentiate(
        &self,
        selected: &[eqiora_core::Id<eqiora_core::entity::kinds::Parameter>],
        values: Option<&[f64]>,
    ) -> Result<CommonScalarDifferentiationPoint, Diagnostic> {
        self.admission.revalidate()?;
        let RecognizedNativeModel::Scalar(template) = &self.admission.recognized else {
            return Err(invalid(
                "common scalar Plan lost its recognized mathematics",
            ));
        };
        let selected_values = selected
            .iter()
            .map(|field| {
                template
                    .parameter_fields()
                    .iter()
                    .position(|candidate| candidate == field)
                    .map(|index| template.parameter_values()[index])
                    .ok_or_else(|| {
                        invalid(
                            "selected differentiable Parameter is frozen or absent from this Plan",
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let bound = template
            .bind_selected_parameters(selected, values.unwrap_or(selected_values.as_slice()))?;
        let NativeMeshResources::Cartesian { mesh, .. } = &self.admission.resources else {
            return Err(invalid(
                "common scalar differentiation requires exact Cartesian resources",
            ));
        };
        let mesh = mesh.mesh();
        let source = |coordinates: &[f64]| bound.source().evaluate(coordinates).unwrap_or(f64::NAN);
        let boundary = |coordinates: &[f64]| {
            bound
                .essential_boundary_jvp(
                    coordinates,
                    &vec![0.0; coordinates.len()],
                    &vec![0.0; bound.parameter_fields().len()],
                )
                .map(|value| value.0)
                .unwrap_or(f64::NAN)
        };
        let solver = self.admission.linear.solver;
        let target = Target::HostCpu {
            threads: NonZeroUsize::MIN,
        };
        let finalized = match self.admission.spatial {
            NativeSpatialPolicy::ScalarQ1 => {
                let quadrature = QuadratureRule::tensor_product_gauss_legendre(2, 2)?;
                let assembly = finalize_scalar_elliptic_cartesian_fem(
                    mesh,
                    bound.coefficient(),
                    &source,
                    &boundary,
                    &quadrature,
                    &REFERENCE_ASSEMBLY_BACKEND,
                    None,
                )?;
                FinalizedScalarEllipticCartesianProblem::finite_element(
                    self.portable.clone(),
                    solver,
                    VectorLayoutKind::Replicated,
                    target,
                    assembly,
                )?
            }
            NativeSpatialPolicy::ScalarTpfa => {
                let cell = QuadratureRule::tensor_product_gauss_legendre(2, 1)?;
                let facet = QuadratureRule::gauss_legendre(1)?;
                let assembly = finalize_scalar_elliptic_cartesian_fvm(
                    mesh,
                    bound.coefficient(),
                    &source,
                    &boundary,
                    &cell,
                    &facet,
                    &REFERENCE_ASSEMBLY_BACKEND,
                )?;
                FinalizedScalarEllipticCartesianProblem::finite_volume(
                    self.portable.clone(),
                    solver,
                    VectorLayoutKind::Replicated,
                    target,
                    assembly,
                )?
            }
            NativeSpatialPolicy::ElasticityQ1
            | NativeSpatialPolicy::StokesMiniP1(_)
            | NativeSpatialPolicy::TransientMiniP1(_)
            | NativeSpatialPolicy::TransientCellCentered(_) => {
                return Err(invalid(
                    "common scalar differentiation received a non-scalar spatial policy",
                ));
            }
        };
        let executor = HostExecutorDescriptor::new(
            self.admission.linear.provider,
            self.admission.linear.execution,
            self.admission.linear.workers,
            self.admission.linear.capabilities.clone(),
        );
        let binding = DeploymentBinding::bind_host(&self.portable, executor)?;
        let admitted = AdmittedExecution::admit_host_linear(
            &self.portable,
            finalized.canonical_csr_system_view(),
            binding,
        )?;
        let produced = REFERENCE_LINEAR_SOLVER.solve(&finalized.linear_problem()?, solver)?;
        let accepted = admitted.accept(produced)?;
        let (solution, receipt) = accepted.into_parts();
        let solution = finalized.finish(solution)?;
        let coordinates = selected
            .iter()
            .copied()
            .map(SpatialDesignCoordinate::ModelParameter)
            .collect::<Vec<_>>();
        let (relation, output) = match &solution {
            ResolvedScalarEllipticCartesianSolution::FiniteElement(solution) => {
                let quadrature = QuadratureRule::tensor_product_gauss_legendre(2, 2)?;
                (
                    linearize_scalar_elliptic_cartesian_fem(
                        &bound,
                        mesh,
                        solution,
                        &quadrature,
                        &coordinates,
                    )?,
                    linearize_scalar_elliptic_cartesian_fem_output(
                        &bound,
                        mesh,
                        solution,
                        &coordinates,
                    )?,
                )
            }
            ResolvedScalarEllipticCartesianSolution::FiniteVolume(solution) => {
                let cell = QuadratureRule::tensor_product_gauss_legendre(2, 1)?;
                let facet = QuadratureRule::gauss_legendre(1)?;
                (
                    linearize_scalar_elliptic_cartesian_fvm(
                        &bound,
                        mesh,
                        solution,
                        &cell,
                        &facet,
                        &coordinates,
                    )?,
                    linearize_scalar_elliptic_cartesian_fvm_output(
                        &bound,
                        mesh,
                        solution,
                        &coordinates,
                    )?,
                )
            }
        };
        if relation.state_jacobian().agreement_fingerprint() != receipt.operator() {
            return Err(invalid(
                "common Plan solve receipt differs from its differentiated state system",
            ));
        }
        Ok(CommonScalarDifferentiationPoint {
            relation,
            output,
            receipt,
        })
    }

    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }

    #[must_use]
    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    #[must_use]
    pub const fn model_revision(&self) -> u64 {
        self.model_revision
    }

    #[must_use]
    pub fn model_digest(&self) -> &str {
        self.admission.model_digest()
    }

    /// Exact canonical Model artifact selected by this Plan.
    pub fn model_reference(&self) -> Result<eqiora_artifact::ModelArtifactReference, Diagnostic> {
        self.admission.model.artifact_reference()
    }

    #[must_use]
    pub fn geometry_digest(&self) -> &str {
        &self.geometry_digest
    }

    #[must_use]
    pub fn mesh_digest(&self) -> &str {
        &self.mesh_digest
    }

    #[must_use]
    pub fn correspondence_digest(&self) -> &str {
        &self.correspondence_digest
    }

    #[must_use]
    pub fn production_digest(&self) -> &str {
        &self.production_digest
    }

    #[must_use]
    pub fn field_id(&self) -> &str {
        &self.field_id
    }

    /// Exact scalar Field represented by this Plan.
    #[must_use]
    pub const fn field(&self) -> eqiora_core::Id<eqiora_core::entity::kinds::Field> {
        self.field
    }

    #[must_use]
    pub const fn cells(&self) -> [usize; 2] {
        self.cells
    }

    #[must_use]
    pub fn spatial(&self) -> CommonSpatialPolicy {
        match self.admission.spatial {
            NativeSpatialPolicy::ScalarQ1 => CommonSpatialPolicy::Q1,
            NativeSpatialPolicy::ScalarTpfa => CommonSpatialPolicy::CellCenteredTpfa,
            NativeSpatialPolicy::ElasticityQ1 => {
                unreachable!("common scalar Plan cannot own elasticity policy")
            }
            NativeSpatialPolicy::StokesMiniP1(_) => {
                unreachable!("common scalar Plan cannot own Stokes policy")
            }
            NativeSpatialPolicy::TransientMiniP1(_)
            | NativeSpatialPolicy::TransientCellCentered(_) => {
                unreachable!("common scalar Plan cannot own transient-flow policy")
            }
        }
    }

    #[must_use]
    pub const fn linear(&self) -> SolverPlan {
        self.admission.linear.solver
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NativeCapability {
    ScalarElliptic,
    IsotropicElasticity,
    SteadyIncompressibleStokes,
    TransientIncompressibleFlow,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum NativeSpatialPolicy {
    ScalarQ1,
    ScalarTpfa,
    ElasticityQ1,
    StokesMiniP1(IncompressibleFlowScaleProfile2d),
    TransientMiniP1(IncompressibleFlowScaleProfile2d),
    TransientCellCentered(IncompressibleFlowScaleProfile2d),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct NativeLinearPolicy {
    solver: SolverPlan,
    provider: SolverProvider,
    capabilities: SolverCapabilities,
    execution: ExecutionProvider,
    workers: NonZeroUsize,
}

impl NativeLinearPolicy {
    pub(super) fn exact(
        solver: SolverPlan,
        backend: &dyn LinearSolverBackend,
    ) -> Result<Self, Diagnostic> {
        if solver.relative_tolerance().to_bits() == (-0.0_f64).to_bits()
            || solver.absolute_tolerance().to_bits() == (-0.0_f64).to_bits()
        {
            return Err(invalid(
                "linear policy contains signed-zero tolerance ambiguity",
            ));
        }
        let provider = backend.provider();
        provider.validate()?;
        SERIAL_EXECUTION_PROVIDER.validate()?;
        Ok(Self {
            solver,
            provider,
            capabilities: backend.capabilities(),
            execution: SERIAL_EXECUTION_PROVIDER,
            workers: NonZeroUsize::MIN,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum NativeMeshResources {
    Cartesian {
        geometry: CanonicalGeometryV1,
        mesh: CartesianMeshEnvelopeV1,
        correspondence: GeometryMeshCorrespondenceEnvelopeV1,
        production: MeshProductionLineageEnvelopeV1,
    },
    ReferenceSimplicial {
        geometry: CanonicalGeometryV1,
        mesh: SimplicialMeshEnvelopeV1,
        correspondence: GeometryMeshCorrespondenceEnvelopeV1,
        production: MeshProductionLineageEnvelopeV1,
    },
    AffineTriangleSimplicial {
        geometry: CanonicalGeometryV1,
        mesh: SimplicialMeshEnvelopeV1,
        correspondence: GeometryMeshCorrespondenceEnvelopeV1,
        production: MeshProductionLineageEnvelopeV1,
    },
    GmshSimplicial {
        geometry: CanonicalGeometryV1,
        policy: eqiora_artifact::PlanarMeshQualityV1,
        provider_output: Box<[u8]>,
        mesh: SimplicialMeshEnvelopeV1,
        correspondence: GeometryMeshCorrespondenceEnvelopeV1,
        production: MeshProductionLineageEnvelopeV1,
    },
}

/// Authenticated in-process owner of one exact common Geometry/Mesh occurrence.
#[derive(Debug, Clone, PartialEq)]
pub struct AuthenticatedCommonMesh {
    resources: NativeMeshResources,
}

impl AuthenticatedCommonMesh {
    /// Authenticate and own one structured-Cartesian rectangle occurrence.
    pub fn structured_cartesian(
        geometry: CanonicalGeometryV1,
        mesh: CartesianMeshEnvelopeV1,
        correspondence: GeometryMeshCorrespondenceEnvelopeV1,
        production: MeshProductionLineageEnvelopeV1,
    ) -> Result<Self, Diagnostic> {
        let resources = NativeMeshResources::Cartesian {
            geometry,
            mesh,
            correspondence,
            production,
        };
        validate_cartesian_resources(&resources)?;
        Ok(Self { resources })
    }

    /// Authenticate and own one deterministic reference simplicial occurrence.
    pub fn planar_reference(
        geometry: CanonicalGeometryV1,
        mesh: SimplicialMeshEnvelopeV1,
        correspondence: GeometryMeshCorrespondenceEnvelopeV1,
        production: MeshProductionLineageEnvelopeV1,
    ) -> Result<Self, Diagnostic> {
        let resources = NativeMeshResources::ReferenceSimplicial {
            geometry,
            mesh,
            correspondence,
            production,
        };
        validate_simplicial_resources(&resources)?;
        Ok(Self { resources })
    }

    /// Authenticate and own one fixed-diagonal affine-triangle rectangle occurrence.
    pub fn affine_triangle_rectangle(
        geometry: CanonicalGeometryV1,
        mesh: SimplicialMeshEnvelopeV1,
        correspondence: GeometryMeshCorrespondenceEnvelopeV1,
        production: MeshProductionLineageEnvelopeV1,
    ) -> Result<Self, Diagnostic> {
        let resources = NativeMeshResources::AffineTriangleSimplicial {
            geometry,
            mesh,
            correspondence,
            production,
        };
        validate_simplicial_resources(&resources)?;
        Ok(Self { resources })
    }

    /// Re-import and own one exact bounded Gmsh 4.15.2 provider observation.
    pub fn gmsh_4152(
        geometry: CanonicalGeometryV1,
        policy: eqiora_artifact::PlanarMeshQualityV1,
        provider_output: Vec<u8>,
    ) -> Result<Self, Diagnostic> {
        let resources = derive_gmsh_resources(geometry, policy, provider_output)?;
        Ok(Self { resources })
    }
}

impl NativeMeshResources {
    fn geometry(&self) -> &CanonicalGeometryV1 {
        match self {
            Self::Cartesian { geometry, .. }
            | Self::ReferenceSimplicial { geometry, .. }
            | Self::AffineTriangleSimplicial { geometry, .. }
            | Self::GmshSimplicial { geometry, .. } => geometry,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct NativeNumericalAdmission {
    model: ModelEnvelope,
    model_digest: String,
    program: KernelProgram,
    capability: NativeCapability,
    recognized: RecognizedNativeModel,
    resources: NativeMeshResources,
    spatial: NativeSpatialPolicy,
    linear: NativeLinearPolicy,
    policy_identity: String,
    temporal: Option<CommonBackwardEuler>,
    nonlinear: Option<NonlinearSolvePlan>,
}

#[derive(Debug, Clone, PartialEq)]
enum RecognizedNativeModel {
    Scalar(Box<ScalarEllipticCartesianModel>),
    Elasticity(Box<IsotropicElasticityCartesianModel2d>),
    Stokes(Box<SteadyStokesGeometryBinding2d>),
    Transient(Box<TransientIncompressibleNavierStokesCartesianModel2d>),
}

struct RecognizedNativeAdmission {
    model: ModelEnvelope,
    model_digest: String,
    program: KernelProgram,
    capability: NativeCapability,
    recognized: RecognizedNativeModel,
    resources: NativeMeshResources,
}

impl RecognizedNativeAdmission {
    fn recognize(
        model: &ModelEnvelope,
        owner: AuthenticatedCommonMesh,
    ) -> Result<Self, Diagnostic> {
        let resources = owner.resources;
        let program = replay_program(model, resources.geometry())?;
        let transient = lower_transient_incompressible_navier_stokes_cartesian_2d(&program);
        let capability = recognize_capability(&program, &transient)?;
        let recognized = recognize_exact_model(capability, &program, &resources, transient)?;
        let model_digest = model.digest()?.to_string();
        Ok(Self {
            model: model.clone(),
            model_digest,
            program,
            capability,
            recognized,
            resources,
        })
    }

    fn complete(
        self,
        spatial: NativeSpatialPolicy,
        linear: NativeLinearPolicy,
        temporal: Option<CommonBackwardEuler>,
        nonlinear: Option<NonlinearSolvePlan>,
    ) -> Result<NativeNumericalAdmission, Diagnostic> {
        require_policy_compatibility(self.capability, spatial, &linear)?;
        validate_resources(self.capability, spatial, &self.resources)?;
        let policy_identity = policy_identity(spatial, &linear, temporal, nonlinear);
        Ok(NativeNumericalAdmission {
            model: self.model,
            model_digest: self.model_digest,
            program: self.program,
            capability: self.capability,
            recognized: self.recognized,
            resources: self.resources,
            spatial,
            linear,
            policy_identity,
            temporal,
            nonlinear,
        })
    }
}

impl NativeNumericalAdmission {
    #[cfg(test)]
    pub(super) fn admit(
        model: &ModelEnvelope,
        owner: AuthenticatedCommonMesh,
        spatial: NativeSpatialPolicy,
        linear: NativeLinearPolicy,
    ) -> Result<Self, Diagnostic> {
        RecognizedNativeAdmission::recognize(model, owner)?.complete(spatial, linear, None, None)
    }

    pub(super) fn revalidate(&self) -> Result<(), Diagnostic> {
        let replayed = RecognizedNativeAdmission::recognize(
            &self.model,
            AuthenticatedCommonMesh {
                resources: self.resources.clone(),
            },
        )?
        .complete(
            self.spatial,
            self.linear.clone(),
            self.temporal,
            self.nonlinear,
        )?;
        if &replayed != self {
            return Err(invalid(
                "native numerical admission changed during exact internal replay",
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) const fn model(&self) -> &ModelEnvelope {
        &self.model
    }

    pub(super) fn model_digest(&self) -> &str {
        &self.model_digest
    }

    pub(super) fn policy_identity(&self) -> &str {
        &self.policy_identity
    }

    #[cfg(test)]
    pub(super) const fn capability(&self) -> NativeCapability {
        self.capability
    }

    pub(super) const fn resources(&self) -> &NativeMeshResources {
        &self.resources
    }

    pub(super) fn stokes_binding(&self) -> Result<SteadyStokesGeometryBinding2d, Diagnostic> {
        let RecognizedNativeModel::Stokes(binding) = &self.recognized else {
            return Err(invalid(
                "native numerical admission does not own recognized steady-Stokes meaning",
            ));
        };
        Ok((**binding).clone())
    }

    pub(super) fn resolve_stokes(
        &self,
        binding: &SteadyStokesGeometryBinding2d,
    ) -> Result<
        (
            ResolvedFieldwiseRealization,
            RealizationEnvelopeV2,
            Space,
            Space,
        ),
        Diagnostic,
    > {
        let NativeSpatialPolicy::StokesMiniP1(scales) = self.spatial else {
            return Err(invalid(
                "steady-Stokes admission has a non-Stokes spatial policy",
            ));
        };
        let (NativeMeshResources::ReferenceSimplicial { mesh, .. }
        | NativeMeshResources::GmshSimplicial { mesh, .. }) = &self.resources
        else {
            return Err(invalid(
                "steady Stokes requires exact supplied simplicial resources",
            ));
        };
        let solver = self.linear.solver;
        let fieldwise = binding.mini_plan(mesh.artifact_reference()?, scales, solver)?;
        let selected_solver = SolverCapabilities::exact([SolverCapability {
            algorithm: solver.algorithm(),
            operator_properties: LinearOperatorProperties::SymmetricIndefinite,
            preconditioner: solver.preconditioner(),
            reduction: solver.reduction(),
            scalar_type: ScalarType::F64,
        }])?;
        let capabilities = RealizationCapabilities::cartesian_product(
            [DiscretizationMethod::ContinuousGalerkin],
            [(
                MeshKind::ImportedAffineSimplicial,
                SpatialDimensionSupport::exact(NonZeroUsize::new(2).expect("two is nonzero")),
            )],
            [VectorLayoutKind::Replicated],
            selected_solver,
            TargetCapabilities::none().with_host_cpu(self.linear.workers),
        )?;
        let resolved = resolve_fieldwise(
            &FieldwiseRealizationRequest::explicit(
                self.program.model(),
                SemanticRevision::new(self.program.revision().0),
                RealizationRevision::new(APPLICATION_REALIZATION_REVISION),
                fieldwise,
            ),
            binding.fieldwise_requirements(),
            &capabilities,
        )?;
        let realization = RealizationEnvelopeV2::from_resolved(
            &self.model,
            &resolved,
            eqiora_artifact::LayoutArtifacts::Replicated,
        )?;
        let mut velocity = None;
        let mut pressure = None;
        for field in resolved.plan().spatial().field_spaces() {
            match field.space().family() {
                SpaceFamily::SimplexP1Bubble if velocity.replace(field.space()).is_none() => {}
                SpaceFamily::ContinuousLagrange { order }
                    if order == std::num::NonZeroU16::MIN
                        && pressure.replace(field.space()).is_none() => {}
                _ => {
                    return Err(invalid(
                        "steady-Stokes resolved space inventory is not MINI/P1",
                    ));
                }
            }
        }
        let (velocity, pressure) = velocity
            .zip(pressure)
            .ok_or_else(|| invalid("steady-Stokes resolved space inventory is incomplete"))?;
        Ok((resolved, realization, velocity, pressure))
    }

    pub(super) fn execute_scalar(
        &self,
        backend: &dyn LinearSolverBackend,
    ) -> Result<ResolvedScalarEllipticCartesianSolution, Diagnostic> {
        self.revalidate()?;
        if backend.provider() != self.linear.provider
            || backend.capabilities() != self.linear.capabilities
        {
            return Err(invalid(
                "scalar execution backend differs from admitted provider or capabilities",
            ));
        }
        let NativeMeshResources::Cartesian { mesh, .. } = &self.resources else {
            return Err(invalid(
                "scalar elliptic execution requires Cartesian resources",
            ));
        };
        let RecognizedNativeModel::Scalar(lowered) = &self.recognized else {
            return Err(invalid(
                "native numerical admission does not own recognized scalar-elliptic meaning",
            ));
        };
        let source =
            |coordinates: &[f64]| lowered.source().evaluate(coordinates).unwrap_or(f64::NAN);
        let boundary = |coordinates: &[f64]| {
            lowered
                .essential_boundary_jvp(
                    coordinates,
                    &vec![0.0; coordinates.len()],
                    &vec![0.0; lowered.parameter_fields().len()],
                )
                .map(|value| value.0)
                .unwrap_or(f64::NAN)
        };
        let solve = LinearSolveRequest::new(backend, self.linear.solver);
        match self.spatial {
            NativeSpatialPolicy::ScalarQ1 => {
                let quadrature = QuadratureRule::tensor_product_gauss_legendre(2, 2)?;
                solve_scalar_elliptic_cartesian_fem(
                    mesh.mesh(),
                    lowered.coefficient(),
                    &source,
                    &boundary,
                    &quadrature,
                    solve,
                )
                .map(ResolvedScalarEllipticCartesianSolution::FiniteElement)
            }
            NativeSpatialPolicy::ScalarTpfa => {
                let cell = QuadratureRule::tensor_product_gauss_legendre(2, 1)?;
                let facet = QuadratureRule::gauss_legendre(1)?;
                solve_scalar_elliptic_cartesian_fvm(
                    mesh.mesh(),
                    lowered.coefficient(),
                    &source,
                    &boundary,
                    &cell,
                    &facet,
                    solve,
                )
                .map(ResolvedScalarEllipticCartesianSolution::FiniteVolume)
            }
            NativeSpatialPolicy::ElasticityQ1 => Err(invalid(
                "scalar execution received an elasticity spatial policy",
            )),
            NativeSpatialPolicy::StokesMiniP1(_) => Err(invalid(
                "scalar execution received a steady-Stokes spatial policy",
            )),
            NativeSpatialPolicy::TransientMiniP1(_)
            | NativeSpatialPolicy::TransientCellCentered(_) => Err(invalid(
                "scalar execution received a transient-flow spatial policy",
            )),
        }
    }

    pub(super) fn execute_elasticity(
        &self,
        backend: &dyn LinearSolverBackend,
    ) -> Result<CartesianLinearElasticity2dSolution, Diagnostic> {
        self.revalidate()?;
        if backend.provider() != self.linear.provider
            || backend.capabilities() != self.linear.capabilities
        {
            return Err(invalid(
                "elasticity execution backend differs from admitted provider or capabilities",
            ));
        }
        let NativeMeshResources::Cartesian { mesh, .. } = &self.resources else {
            return Err(invalid("elasticity execution requires Cartesian resources"));
        };
        let RecognizedNativeModel::Elasticity(lowered) = &self.recognized else {
            return Err(invalid(
                "native numerical admission does not own recognized elasticity meaning",
            ));
        };
        let finalized = finalize_isotropic_elasticity_cartesian_q1_on_mesh(
            lowered,
            mesh.mesh(),
            self.linear.solver,
            &REFERENCE_ASSEMBLY_BACKEND,
        )?;
        let solved = backend.solve(&finalized.linear_problem()?, finalized.solver_plan())?;
        finalized.finish(solved)
    }
}

fn resource_digests(
    resources: &NativeMeshResources,
) -> Result<(String, String, String, String), Diagnostic> {
    let (geometry, mesh, correspondence, production) = match resources {
        NativeMeshResources::Cartesian {
            geometry,
            mesh,
            correspondence,
            production,
        } => (
            geometry,
            mesh.digest()?,
            correspondence.digest()?,
            production.digest()?,
        ),
        NativeMeshResources::ReferenceSimplicial {
            geometry,
            mesh,
            correspondence,
            production,
        }
        | NativeMeshResources::AffineTriangleSimplicial {
            geometry,
            mesh,
            correspondence,
            production,
        }
        | NativeMeshResources::GmshSimplicial {
            geometry,
            mesh,
            correspondence,
            production,
            ..
        } => (
            geometry,
            mesh.digest()?,
            correspondence.digest()?,
            production.digest()?,
        ),
    };
    Ok((
        hex_bytes(&geometry.digest_bytes()),
        mesh.to_string(),
        correspondence.to_string(),
        production.to_string(),
    ))
}

fn resource_artifact_digests(
    resources: &NativeMeshResources,
) -> Result<
    (
        eqiora_artifact::ArtifactDigest,
        eqiora_artifact::ArtifactDigest,
        eqiora_artifact::ArtifactDigest,
        eqiora_artifact::ArtifactDigest,
    ),
    Diagnostic,
> {
    let (geometry, mesh, correspondence, production) = match resources {
        NativeMeshResources::Cartesian {
            geometry,
            mesh,
            correspondence,
            production,
        } => (
            geometry,
            mesh.digest()?,
            correspondence.digest()?,
            production.digest()?,
        ),
        NativeMeshResources::ReferenceSimplicial {
            geometry,
            mesh,
            correspondence,
            production,
        }
        | NativeMeshResources::AffineTriangleSimplicial {
            geometry,
            mesh,
            correspondence,
            production,
        }
        | NativeMeshResources::GmshSimplicial {
            geometry,
            mesh,
            correspondence,
            production,
            ..
        } => (
            geometry,
            mesh.digest()?,
            correspondence.digest()?,
            production.digest()?,
        ),
    };
    Ok((
        eqiora_artifact::ArtifactDigest::from_sha256(geometry.digest_bytes()),
        mesh,
        correspondence,
        production,
    ))
}

fn recognize_capability(
    program: &KernelProgram,
    transient: &Result<TransientIncompressibleNavierStokesCartesianModel2d, Diagnostic>,
) -> Result<NativeCapability, Diagnostic> {
    let scalar = recognize_scalar_elliptic_geometry_mathematics(program);
    let elasticity = recognize_isotropic_elasticity_geometry_mathematics(program);
    let stokes = recognize_steady_incompressible_stokes_geometry_mathematics(program);
    let recognized = [
        scalar.is_ok(),
        elasticity.is_ok(),
        stokes.is_ok(),
        transient.is_ok(),
    ];
    if recognized.into_iter().filter(|matched| *matched).count() > 1 {
        return Err(invalid(
            "Model mathematical meaning is ambiguous across native capabilities",
        ));
    }
    if scalar.is_ok() {
        return Ok(NativeCapability::ScalarElliptic);
    }
    if elasticity.is_ok() {
        return Ok(NativeCapability::IsotropicElasticity);
    }
    if stokes.is_ok() {
        return Ok(NativeCapability::SteadyIncompressibleStokes);
    }
    if transient.is_ok() {
        return Ok(NativeCapability::TransientIncompressibleFlow);
    }
    let scalar = scalar.unwrap_err();
    let elasticity = elasticity.unwrap_err();
    let stokes = stokes.unwrap_err();
    let transient = transient.as_ref().unwrap_err();
    Err(invalid(format!(
        "Model mathematical meaning matches no native capability: scalar [{}: {}]; elasticity [{}: {}]; Stokes [{}: {}]; transient flow [{}: {}]",
        scalar.code(),
        scalar.message(),
        elasticity.code(),
        elasticity.message(),
        stokes.code(),
        stokes.message(),
        transient.code(),
        transient.message(),
    )))
}

fn recognize_exact_model(
    capability: NativeCapability,
    program: &KernelProgram,
    resources: &NativeMeshResources,
    transient: Result<TransientIncompressibleNavierStokesCartesianModel2d, Diagnostic>,
) -> Result<RecognizedNativeModel, Diagnostic> {
    match (capability, resources) {
        (
            NativeCapability::ScalarElliptic,
            NativeMeshResources::Cartesian {
                geometry,
                mesh,
                correspondence,
                ..
            },
        ) => {
            lower_scalar_elliptic_cartesian_with_resources(program, geometry, mesh, correspondence)
                .map(Box::new)
                .map(RecognizedNativeModel::Scalar)
        }
        (
            NativeCapability::IsotropicElasticity,
            NativeMeshResources::Cartesian {
                geometry,
                mesh,
                correspondence,
                ..
            },
        ) => lower_isotropic_elasticity_geometry_2d(program, geometry, mesh, correspondence)
            .map(Box::new)
            .map(RecognizedNativeModel::Elasticity),
        (
            NativeCapability::SteadyIncompressibleStokes,
            NativeMeshResources::ReferenceSimplicial {
                geometry,
                mesh,
                correspondence,
                ..
            }
            | NativeMeshResources::GmshSimplicial {
                geometry,
                mesh,
                correspondence,
                ..
            },
        ) => SteadyStokesGeometryBinding2d::new_authenticated(
            program,
            geometry,
            mesh,
            correspondence,
        )
        .map(Box::new)
        .map(RecognizedNativeModel::Stokes),
        (NativeCapability::TransientIncompressibleFlow, _) => {
            let transient = transient?;
            let exact_bounds = resources
                .geometry()
                .planar_rectangle_bounds()
                .ok_or_else(|| {
                    invalid("transient flow requires an exact planar rectangle Geometry")
                })?;
            if !exact_bounds
                .iter()
                .zip(transient.bounds())
                .all(|(caller, model)| {
                    caller[0].to_bits() == model[0].to_bits()
                        && caller[1].to_bits() == model[1].to_bits()
                })
            {
                return Err(invalid(
                    "caller Mesh Geometry bounds differ from Model-owned transient Domain",
                ));
            }
            Ok(RecognizedNativeModel::Transient(Box::new(transient)))
        }
        _ => Err(invalid(
            "recognized Model capability and authenticated common Mesh kind are cross-wired",
        )),
    }
}

fn require_policy_compatibility(
    capability: NativeCapability,
    spatial: NativeSpatialPolicy,
    linear: &NativeLinearPolicy,
) -> Result<(), Diagnostic> {
    let (algorithm, properties, preconditioner, reduction) = match (capability, spatial) {
        (
            NativeCapability::ScalarElliptic,
            NativeSpatialPolicy::ScalarQ1 | NativeSpatialPolicy::ScalarTpfa,
        ) => (
            LinearSolver::ConjugateGradient,
            LinearOperatorProperties::SymmetricPositiveDefinite,
            PreconditionerPolicy::Identity,
            ReductionPolicy::Reproducible,
        ),
        (NativeCapability::IsotropicElasticity, NativeSpatialPolicy::ElasticityQ1) => (
            LinearSolver::ConjugateGradient,
            LinearOperatorProperties::SymmetricPositiveDefinite,
            PreconditionerPolicy::Identity,
            ReductionPolicy::Reproducible,
        ),
        (NativeCapability::SteadyIncompressibleStokes, NativeSpatialPolicy::StokesMiniP1(_)) => (
            LinearSolver::SparseLu,
            LinearOperatorProperties::SymmetricIndefinite,
            PreconditionerPolicy::Identity,
            ReductionPolicy::Fast,
        ),
        (
            NativeCapability::TransientIncompressibleFlow,
            NativeSpatialPolicy::TransientMiniP1(_),
        ) => (
            LinearSolver::BiConjugateGradientStabilized,
            LinearOperatorProperties::General,
            PreconditionerPolicy::Identity,
            ReductionPolicy::Fast,
        ),
        (
            NativeCapability::TransientIncompressibleFlow,
            NativeSpatialPolicy::TransientCellCentered(_),
        ) => (
            LinearSolver::BiConjugateGradientStabilized,
            LinearOperatorProperties::General,
            PreconditionerPolicy::Identity,
            ReductionPolicy::Reproducible,
        ),
        _ => {
            return Err(invalid(
                "Model capability and spatial policy are cross-wired",
            ));
        }
    };
    if linear.solver.algorithm() != algorithm
        || linear.solver.preconditioner() != preconditioner
        || linear.solver.reduction() != reduction
        || linear.execution != SERIAL_EXECUTION_PROVIDER
        || linear.workers != NonZeroUsize::MIN
    {
        return Err(invalid(
            "linear solver, preconditioner, reduction, or placement is unsupported",
        ));
    }
    linear
        .capabilities
        .require_problem(linear.solver, ScalarType::F64, properties)
}

fn validate_resources(
    capability: NativeCapability,
    spatial: NativeSpatialPolicy,
    resources: &NativeMeshResources,
) -> Result<(), Diagnostic> {
    match (capability, spatial, resources) {
        (
            NativeCapability::ScalarElliptic,
            NativeSpatialPolicy::ScalarQ1 | NativeSpatialPolicy::ScalarTpfa,
            resources @ NativeMeshResources::Cartesian { .. },
        ) => validate_cartesian_resources(resources),
        (
            NativeCapability::IsotropicElasticity,
            NativeSpatialPolicy::ElasticityQ1,
            resources @ NativeMeshResources::Cartesian { .. },
        ) => validate_cartesian_resources(resources),
        (
            NativeCapability::SteadyIncompressibleStokes,
            NativeSpatialPolicy::StokesMiniP1(_),
            resources @ (NativeMeshResources::ReferenceSimplicial { .. }
            | NativeMeshResources::GmshSimplicial { .. }),
        ) => validate_simplicial_resources(resources),
        (
            NativeCapability::TransientIncompressibleFlow,
            NativeSpatialPolicy::TransientMiniP1(_),
            resources @ NativeMeshResources::AffineTriangleSimplicial { .. },
        ) => validate_simplicial_resources(resources),
        (
            NativeCapability::TransientIncompressibleFlow,
            NativeSpatialPolicy::TransientCellCentered(_),
            resources @ NativeMeshResources::Cartesian { .. },
        ) => validate_cartesian_resources(resources),
        _ => Err(invalid(
            "Model capability, spatial policy, and common Mesh kind are cross-wired",
        )),
    }
}

fn validate_cartesian_resources(resources: &NativeMeshResources) -> Result<(), Diagnostic> {
    let NativeMeshResources::Cartesian {
        geometry,
        mesh,
        correspondence,
        production,
    } = resources
    else {
        return Err(invalid("authenticated owner is not Cartesian"));
    };
    let policy = production
        .cartesian_cells()
        .ok_or_else(|| invalid("Cartesian resource has a non-Cartesian policy"))?;
    correspondence.validate_against_planar_rectangle_v2_cartesian(
        geometry,
        mesh,
        policy.cells(),
    )?;
    production.validate_against_structured_cartesian_v1_resources(
        policy,
        geometry,
        mesh,
        correspondence,
    )?;
    let [nx, ny] = policy.cells();
    if mesh.dimension() != 2
        || mesh.mesh().axis_cell_count(0) != Some(nx)
        || mesh.mesh().axis_cell_count(1) != Some(ny)
    {
        return Err(invalid(
            "Cartesian Mesh topology differs from its exact production policy",
        ));
    }
    Ok(())
}

fn validate_simplicial_resources(resources: &NativeMeshResources) -> Result<(), Diagnostic> {
    match resources {
        NativeMeshResources::ReferenceSimplicial {
            geometry,
            mesh,
            correspondence,
            production,
        } => {
            let policy = production.planar_mesh_quality().ok_or_else(|| {
                invalid("reference simplicial resource has a non-planar production policy")
            })?;
            correspondence.validate_against_planar_circular_hole_v2_reference(
                geometry,
                mesh,
                policy.maximum_boundary_error_m(),
                policy.maximum_boundary_facets(),
            )?;
            production.validate_against_planar_circular_hole_reference_v1_resources(
                policy,
                geometry,
                mesh,
                correspondence,
            )?;
        }
        NativeMeshResources::AffineTriangleSimplicial {
            geometry,
            mesh,
            correspondence,
            production,
        } => {
            let policy = production.affine_triangle_cells().ok_or_else(|| {
                invalid("affine-triangle resource has a non-affine-triangle production policy")
            })?;
            correspondence.validate_against_planar_rectangle_v2_affine_triangles(
                geometry,
                mesh,
                policy.cells(),
            )?;
            production.validate_against_affine_triangle_rectangle_v1_resources(
                policy,
                geometry,
                mesh,
                correspondence,
            )?;
        }
        NativeMeshResources::GmshSimplicial {
            geometry,
            policy,
            provider_output,
            ..
        } => {
            let replayed =
                derive_gmsh_resources(geometry.clone(), *policy, provider_output.to_vec())?;
            if &replayed != resources {
                return Err(invalid(
                    "Gmsh common Mesh resources differ from exact provider-output replay",
                ));
            }
        }
        NativeMeshResources::Cartesian { .. } => {
            return Err(invalid("authenticated owner is not simplicial"));
        }
    }
    let mesh = match resources {
        NativeMeshResources::ReferenceSimplicial { mesh, .. }
        | NativeMeshResources::AffineTriangleSimplicial { mesh, .. }
        | NativeMeshResources::GmshSimplicial { mesh, .. } => mesh,
        NativeMeshResources::Cartesian { .. } => unreachable!("rejected above"),
    };
    if mesh.dimension() != 2 {
        return Err(invalid(
            "steady Stokes requires a two-dimensional common Mesh",
        ));
    }
    Ok(())
}

fn derive_gmsh_resources(
    geometry: CanonicalGeometryV1,
    policy: eqiora_artifact::PlanarMeshQualityV1,
    provider_output: Vec<u8>,
) -> Result<NativeMeshResources, Diagnostic> {
    CanonicalGeometryV1::decode_planar_circular_hole_v2_canonical(
        geometry.canonical_bytes(),
        eqiora_geometry::CanonicalGeometryLimits::default(),
    )
    .map_err(|_| invalid("Gmsh provider observation requires exact planar circular-hole v2"))?;
    let quality = eqiora_meshing::MeshQualityGate::new(policy.minimum_mean_ratio())?;
    let importer = GmshSimplexImporter::new(2, quality, GmshImportLimits::default())?;
    let imported = importer.import_ascii_bytes_with_entities(&provider_output)?;
    let (tagged_facets, tagged_cells) = derive_entity_assignments(&imported)?;
    let expected_tags = [1_u32, 5, 6, 7, 8]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    if tagged_facets
        .keys()
        .copied()
        .collect::<std::collections::BTreeSet<_>>()
        != expected_tags
    {
        return Err(invalid(
            "Gmsh provider observation has a foreign boundary entity-tag inventory",
        ));
    }
    if tagged_cells.keys().copied().collect::<Vec<_>>() != [1] {
        return Err(invalid(
            "Gmsh provider observation has a foreign source-face entity-tag inventory",
        ));
    }
    let mut source_edge_facets: [Vec<usize>; 5] = std::array::from_fn(|_| Vec::new());
    for (tag, source_edge) in [(1_u32, 4_usize), (5, 2), (6, 1), (7, 3), (8, 0)] {
        source_edge_facets[source_edge] = tagged_facets
            .get(&tag)
            .expect("exact tag inventory checked")
            .clone();
    }
    let mesh = SimplicialMeshEnvelopeV1::from_mesh(imported.mesh())?;
    let correspondence =
        GeometryMeshCorrespondenceEnvelopeV1::from_planar_circular_hole_v2_mesh_assignments(
            &geometry,
            &mesh,
            source_edge_facets,
        )?;
    let production = MeshProductionLineageEnvelopeV1::from_gmsh_4152_resources(
        policy,
        &geometry,
        &mesh,
        &correspondence,
    )?;
    Ok(NativeMeshResources::GmshSimplicial {
        geometry,
        policy,
        provider_output: provider_output.into_boxed_slice(),
        mesh,
        correspondence,
        production,
    })
}

fn derive_entity_assignments(
    imported: &GmshSimplicialImport,
) -> Result<TaggedMeshAssignments, Diagnostic> {
    let mesh = imported.mesh();
    let dimension = mesh.topological_dimension();
    let facet_dimension = dimension
        .checked_sub(1)
        .ok_or_else(|| invalid("Gmsh simplex Mesh has no boundary stratum"))?;
    let facet_count = mesh
        .entity_count(facet_dimension)
        .ok_or_else(|| invalid("Gmsh simplex Mesh omitted its facet stratum"))?;
    let mut facet_by_vertices = BTreeMap::new();
    let mut boundary_facets = BTreeSet::new();
    for facet_index in 0..facet_count {
        let facet = MeshEntity::new(facet_dimension, facet_index);
        let mut vertices = mesh
            .entity_vertices(facet)
            .ok_or_else(|| invalid("Gmsh Mesh facet omitted its vertex closure"))?
            .into_iter()
            .map(MeshEntity::index)
            .collect::<Vec<_>>();
        vertices.sort_unstable();
        if facet_by_vertices.insert(vertices, facet_index).is_some() {
            return Err(invalid(
                "Gmsh Mesh has duplicate canonical facet connectivity",
            ));
        }
        let parents = mesh
            .incidence(facet, dimension)
            .ok_or_else(|| invalid("Gmsh Mesh facet omitted parent incidence"))?;
        if parents.len() == 1 {
            boundary_facets.insert(facet_index);
        }
    }
    let mut tagged_facets: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    let mut assigned_facets = BTreeSet::new();
    for block in imported
        .element_blocks()
        .iter()
        .filter(|block| block.dimension() == facet_dimension)
    {
        let facets = tagged_facets.entry(block.entity_tag()).or_default();
        for element in block.elements() {
            let mut vertices = element.clone();
            vertices.sort_unstable();
            let facet = *facet_by_vertices
                .get(&vertices)
                .ok_or_else(|| invalid("Gmsh boundary element is absent from Mesh topology"))?;
            if !boundary_facets.contains(&facet) || !assigned_facets.insert(facet) {
                return Err(invalid(
                    "Gmsh boundary assignment is interior or duplicated",
                ));
            }
            facets.push(facet);
        }
    }
    for facets in tagged_facets.values_mut() {
        facets.sort_unstable();
    }
    if assigned_facets != boundary_facets {
        return Err(invalid(
            "Gmsh entity blocks do not assign every Mesh boundary facet",
        ));
    }

    let mut cell_by_vertices = BTreeMap::new();
    for (cell_index, cell) in mesh.cells().iter().enumerate() {
        let mut vertices = cell.clone();
        vertices.sort_unstable();
        if cell_by_vertices.insert(vertices, cell_index).is_some() {
            return Err(invalid("Gmsh Mesh has duplicate canonical cells"));
        }
    }
    let mut tagged_cells: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    let mut assigned_cells = BTreeSet::new();
    for block in imported
        .element_blocks()
        .iter()
        .filter(|block| block.dimension() == dimension)
    {
        let cells = tagged_cells.entry(block.entity_tag()).or_default();
        for element in block.elements() {
            let mut vertices = element.clone();
            vertices.sort_unstable();
            let cell = *cell_by_vertices
                .get(&vertices)
                .ok_or_else(|| invalid("Gmsh top element is absent from Mesh topology"))?;
            if !assigned_cells.insert(cell) {
                return Err(invalid("Gmsh top cell assignment is duplicated"));
            }
            cells.push(cell);
        }
    }
    for cells in tagged_cells.values_mut() {
        cells.sort_unstable();
    }
    if assigned_cells != (0..mesh.cells().len()).collect() {
        return Err(invalid(
            "Gmsh entity blocks do not assign every Mesh top cell",
        ));
    }
    Ok((tagged_facets, tagged_cells))
}

fn replay_program(
    model: &ModelEnvelope,
    geometry: &CanonicalGeometryV1,
) -> Result<KernelProgram, Diagnostic> {
    let reference = model.artifact_reference()?;
    let (transaction, model_id) = model.to_transaction().map_err(first)?;
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).map_err(first)?;
    let snapshot = store.snapshot();
    let program = if model.requires_geometry_admission()? {
        KernelProgram::from_snapshot_with_geometry(&snapshot, model_id, &[geometry.into()])
            .map_err(first)?
    } else {
        KernelProgram::from_snapshot(&snapshot, model_id).map_err(first)?
    };
    if program.model() != reference.model()
        || program.revision().0 != reference.semantic_revision().get()
    {
        return Err(invalid(
            "replayed Model identity differs from exact caller Model",
        ));
    }
    Ok(program)
}

fn policy_identity(
    spatial: NativeSpatialPolicy,
    linear: &NativeLinearPolicy,
    temporal: Option<CommonBackwardEuler>,
    nonlinear: Option<NonlinearSolvePlan>,
) -> String {
    let mut bytes = Vec::new();
    match spatial {
        NativeSpatialPolicy::ScalarQ1 => bytes.extend_from_slice(b"scalar-q1"),
        NativeSpatialPolicy::ScalarTpfa => bytes.extend_from_slice(b"scalar-tpfa"),
        NativeSpatialPolicy::ElasticityQ1 => bytes.extend_from_slice(b"elasticity-q1"),
        NativeSpatialPolicy::StokesMiniP1(scales) => {
            bytes.extend_from_slice(b"stokes-mini-p1");
            bytes.extend_from_slice(&scales.length().value().to_bits().to_be_bytes());
            bytes.extend_from_slice(&scales.velocity().value().to_bits().to_be_bytes());
            bytes.extend_from_slice(&scales.pressure().value().to_bits().to_be_bytes());
        }
        NativeSpatialPolicy::TransientMiniP1(scales)
        | NativeSpatialPolicy::TransientCellCentered(scales) => {
            bytes.extend_from_slice(match spatial {
                NativeSpatialPolicy::TransientMiniP1(_) => b"transient-mini-p1",
                NativeSpatialPolicy::TransientCellCentered(_) => b"transient-cell-centered",
                _ => unreachable!(),
            });
            bytes.extend_from_slice(&scales.length().value().to_bits().to_be_bytes());
            bytes.extend_from_slice(&scales.velocity().value().to_bits().to_be_bytes());
            bytes.extend_from_slice(&scales.pressure().value().to_bits().to_be_bytes());
        }
    }
    if let Some(temporal) = temporal {
        bytes.extend_from_slice(&temporal.step().value().to_bits().to_be_bytes());
    }
    if let Some(nonlinear) = nonlinear {
        bytes.extend_from_slice(&nonlinear.relative_tolerance().to_bits().to_be_bytes());
        bytes.extend_from_slice(&nonlinear.absolute_tolerance().to_bits().to_be_bytes());
        bytes.extend_from_slice(&nonlinear.maximum_iterations().get().to_be_bytes());
        bytes.extend_from_slice(&nonlinear.maximum_line_search_steps().to_be_bytes());
    }
    push_framed(
        &mut bytes,
        linear_solver_identity(linear.solver.algorithm()),
    );
    push_framed(
        &mut bytes,
        preconditioner_identity(linear.solver.preconditioner()),
    );
    push_framed(&mut bytes, reduction_identity(linear.solver.reduction()));
    bytes.extend_from_slice(&linear.solver.relative_tolerance().to_bits().to_be_bytes());
    bytes.extend_from_slice(&linear.solver.absolute_tolerance().to_bits().to_be_bytes());
    bytes.extend_from_slice(&linear.solver.maximum_iterations().get().to_be_bytes());
    push_framed(&mut bytes, linear.provider.id().as_str().as_bytes());
    push_framed(
        &mut bytes,
        linear.provider.implementation_version().as_bytes(),
    );
    for library in linear.provider.libraries() {
        push_framed(&mut bytes, library.name().as_bytes());
        push_framed(&mut bytes, library.version().as_bytes());
    }
    push_framed(&mut bytes, linear.execution.id().as_str().as_bytes());
    push_framed(
        &mut bytes,
        linear.execution.implementation_version().as_bytes(),
    );
    bytes.extend_from_slice(&linear.workers.get().to_be_bytes());
    let digest = Sha256::digest([POLICY_DOMAIN, bytes.as_slice()].concat());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

const fn linear_solver_identity(value: LinearSolver) -> &'static [u8] {
    match value {
        LinearSolver::ConjugateGradient => b"conjugate-gradient",
        LinearSolver::MinimumResidual => b"minimum-residual",
        LinearSolver::BiConjugateGradientStabilized => b"bicgstab",
        LinearSolver::SparseLu => b"sparse-lu",
    }
}

const fn preconditioner_identity(value: PreconditionerPolicy) -> &'static [u8] {
    match value {
        PreconditionerPolicy::Identity => b"identity",
        PreconditionerPolicy::Jacobi => b"jacobi",
    }
}

const fn reduction_identity(value: ReductionPolicy) -> &'static [u8] {
    match value {
        ReductionPolicy::Reproducible => b"reproducible",
        ReductionPolicy::Fast => b"fast",
    }
}

fn push_framed(target: &mut Vec<u8>, value: &[u8]) {
    target.extend_from_slice(&value.len().to_be_bytes());
    target.extend_from_slice(value);
}

fn space_identity(space: Space) -> &'static [u8] {
    match space.family() {
        SpaceFamily::SimplexP1Bubble => b"simplex-p1-bubble",
        SpaceFamily::CellConstant => b"cell-constant",
        SpaceFamily::ContinuousLagrange { order } if order.get() == 1 => b"continuous-lagrange-p1",
        SpaceFamily::ContinuousLagrange { .. } => {
            unreachable!("closed common transient resolver only admits continuous P1")
        }
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn first(diagnostics: Vec<Diagnostic>) -> Diagnostic {
    diagnostics
        .into_iter()
        .next()
        .unwrap_or_else(|| invalid("Model replay failed without a diagnostic"))
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use eqiora_artifact::{
        AffineTriangleMeshCellsV1, CartesianMeshCellsV1, GeometryDecoderLimits,
        GeometryMeshCorrespondenceEnvelopeV1, MeshProductionLineageEnvelopeV1, ModelDecoderLimits,
        PlanarMeshQualityV1,
    };
    use eqiora_core::{DimExponents, DynQuantity};
    use eqiora_geometry::{
        CadAuthoredGraph, CanonicalGeometryRef, ConstrainedRectangleV1, NamedEntitySet,
        PlanarOperationGraph, PlanarTopologyHandle,
    };
    use eqiora_meshing::{CartesianMesh, MeshEntity, MeshQualityGate};
    use eqiora_solver::{
        BackendId, LinearProblem, LinearSolution, REFERENCE_LINEAR_SOLVER,
        ReplicatedLinearExecution, SolverPlan,
    };

    use super::*;
    use eqiora_compiler::CompiledModel;

    const COMPONENT: &str = r#"
public component PoissonRectangle {
  public support region: volume(ambient_dimension = 2);
  public support left: boundary(parent = region);
  public support right: boundary(parent = region);
  public support bottom: boundary(parent = region);
  public support top: boundary(parent = region);
  public parameter wave_number: 1 / m;
  public parameter source_scale: 1 / m ^ 2;
  representation space = continuum;
  field potential on region as space: 1 = 0;
  relation balance continuous on region {
    -div(grad(potential))
      - source_scale * sin(wave_number * coordinate(0))
        * sin(wave_number * coordinate(1)) = 0;
  }
  relation left_value continuous on left { trace(potential) = 0; }
  relation right_value continuous on right { trace(potential) = 0; }
  relation bottom_value continuous on bottom { trace(potential) = 0; }
  relation top_value continuous on top { trace(potential) = 0; }
}
"#;

    const STOKES_COMPONENT: &str =
        include_str!("../../eqiora-api/src/steady_stokes/accepted_component.eqi");
    const ELASTICITY_COMPONENT: &str = r#"
public component MixedBoundaryElasticity {
  public support region: volume(ambient_dimension = 2);
  public support left: boundary(parent = region);
  public support right: boundary(parent = region);
  public support bottom: boundary(parent = region);
  public support top: boundary(parent = region);
  public parameter mu: kg / (m * s ^ 2);
  public parameter lambda: kg / (m * s ^ 2);
  public parameter length_scale: m;
  representation space = continuum;
  field displacement on region as space: m shape spatial_vector;
  field load_potential on region as space: kg / (m * s ^ 2) = 0;
  relation load continuous on region {
    load_potential - 2 * mu * coordinate(0) / length_scale = 0;
  }
  relation balance continuous on region {
    -div(
      2 * mu * symmetric_part(grad(displacement))
      + lambda * isotropic_lift(div(displacement))
    ) - grad(load_potential) = 0;
  }
  relation left_fixed continuous on left { trace(displacement) = 0; }
  relation right_free continuous on right {
    normal(2 * mu * symmetric_part(grad(displacement))
      + lambda * isotropic_lift(div(displacement))) = 0;
  }
  relation bottom_free continuous on bottom {
    normal(2 * mu * symmetric_part(grad(displacement))
      + lambda * isotropic_lift(div(displacement))) = 0;
  }
  relation top_free continuous on top {
    normal(2 * mu * symmetric_part(grad(displacement))
      + lambda * isotropic_lift(div(displacement))) = 0;
  }
}
"#;
    const TRANSIENT_SOURCE: &str = include_str!(
        "../../../verify/fluid/fixed-domain-transient-navier-stokes-2d/models/direct.eqi"
    );

    type SupportBinding<'a> = (
        &'a str,
        &'a NamedEntitySet,
        Option<(&'a str, &'a NamedEntitySet)>,
    );

    fn compile_model(
        filename: &str,
        source: &str,
        geometry: &CanonicalGeometryV1,
        model: &str,
        component: &str,
        supports: &[SupportBinding<'_>],
        parameters: &[(&str, DynQuantity)],
    ) -> ModelEnvelope {
        let compiled = CompiledModel::compile_external_component(
            filename,
            source,
            model,
            component,
            CanonicalGeometryRef::from(geometry),
            supports,
            parameters,
        )
        .unwrap();
        let (transaction, model, _) = compiled.into_parts();
        let mut store = InMemoryGraphStore::new();
        store.commit(transaction).unwrap();
        let program = KernelProgram::from_snapshot_with_geometry(
            &store.snapshot(),
            model,
            &[CanonicalGeometryRef::from(geometry)],
        )
        .unwrap();
        ModelEnvelope::from_program(&program).unwrap()
    }

    #[derive(Debug)]
    struct ResolveOnlyBackend;

    impl LinearSolverBackend for ResolveOnlyBackend {
        fn provider(&self) -> SolverProvider {
            SolverProvider::new(BackendId::new("eqiora.test-resolve-only"), "1", &[])
        }

        fn capabilities(&self) -> SolverCapabilities {
            SolverCapabilities::exact([
                SolverCapability {
                    algorithm: LinearSolver::SparseLu,
                    operator_properties: LinearOperatorProperties::SymmetricIndefinite,
                    preconditioner: PreconditionerPolicy::Identity,
                    reduction: ReductionPolicy::Fast,
                    scalar_type: ScalarType::F64,
                },
                SolverCapability {
                    algorithm: LinearSolver::BiConjugateGradientStabilized,
                    operator_properties: LinearOperatorProperties::General,
                    preconditioner: PreconditionerPolicy::Identity,
                    reduction: ReductionPolicy::Fast,
                    scalar_type: ScalarType::F64,
                },
                SolverCapability {
                    algorithm: LinearSolver::BiConjugateGradientStabilized,
                    operator_properties: LinearOperatorProperties::General,
                    preconditioner: PreconditionerPolicy::Identity,
                    reduction: ReductionPolicy::Reproducible,
                    scalar_type: ScalarType::F64,
                },
            ])
            .unwrap()
        }

        fn solve_with_execution(
            &self,
            _problem: &LinearProblem<'_>,
            _plan: SolverPlan,
            _execution: &dyn ReplicatedLinearExecution,
        ) -> Result<LinearSolution, Diagnostic> {
            unreachable!("resolution test must not execute")
        }
    }

    #[derive(Debug)]
    struct AlternateScalarBackend;

    impl LinearSolverBackend for AlternateScalarBackend {
        fn provider(&self) -> SolverProvider {
            SolverProvider::new(BackendId::new("eqiora.test-alternate-scalar"), "1", &[])
        }

        fn capabilities(&self) -> SolverCapabilities {
            REFERENCE_LINEAR_SOLVER.capabilities()
        }

        fn solve_with_execution(
            &self,
            _problem: &LinearProblem<'_>,
            _plan: SolverPlan,
            _execution: &dyn ReplicatedLinearExecution,
        ) -> Result<LinearSolution, Diagnostic> {
            unreachable!("provider mismatch must reject before execution")
        }
    }

    fn rectangle() -> CanonicalGeometryV1 {
        let graph = PlanarOperationGraph::new();
        let rectangle = graph.rectangle([0.0, 1.0], [0.0, 1.0]).unwrap();
        let edges = rectangle.boundaries();
        graph
            .build(
                &rectangle,
                &BTreeMap::from([
                    ("region".to_owned(), vec![rectangle.region().into()]),
                    (
                        "left".to_owned(),
                        vec![PlanarTopologyHandle::from(edges[0])],
                    ),
                    (
                        "right".to_owned(),
                        vec![PlanarTopologyHandle::from(edges[1])],
                    ),
                    (
                        "bottom".to_owned(),
                        vec![PlanarTopologyHandle::from(edges[2])],
                    ),
                    ("top".to_owned(), vec![PlanarTopologyHandle::from(edges[3])]),
                ]),
            )
            .unwrap()
    }

    #[test]
    fn affine_triangle_common_owner_reauthenticates_exact_resource_occurrence() {
        let geometry = rectangle();
        let policy = AffineTriangleMeshCellsV1::new([2, 3]).unwrap();
        let (mesh, correspondence) =
            GeometryMeshCorrespondenceEnvelopeV1::from_planar_rectangle_v2_affine_triangles(
                &geometry,
                policy.cells(),
            )
            .unwrap();
        let production =
            MeshProductionLineageEnvelopeV1::from_affine_triangle_rectangle_v1_resources(
                policy,
                &geometry,
                &mesh,
                &correspondence,
            )
            .unwrap();
        let exact_owner = AuthenticatedCommonMesh::affine_triangle_rectangle(
            geometry.clone(),
            mesh.clone(),
            correspondence.clone(),
            production.clone(),
        )
        .unwrap();
        let stokes_scales = IncompressibleFlowScaleProfile2d::new(
            DynQuantity::new(
                1.0,
                DimExponents {
                    length: 1,
                    ..DimExponents::DIMENSIONLESS
                },
            ),
            DynQuantity::new(
                1.0,
                DimExponents {
                    length: 1,
                    time: -1,
                    ..DimExponents::DIMENSIONLESS
                },
            ),
            DynQuantity::new(
                1.0,
                DimExponents {
                    mass: 1,
                    length: -1,
                    time: -2,
                    ..DimExponents::DIMENSIONLESS
                },
            ),
        )
        .unwrap();
        assert!(
            validate_resources(
                NativeCapability::SteadyIncompressibleStokes,
                NativeSpatialPolicy::StokesMiniP1(stokes_scales),
                &exact_owner.resources,
            )
            .is_err(),
            "#574 publishes physics-independent Mesh resources but does not admit Stokes"
        );

        let alternate_policy = AffineTriangleMeshCellsV1::new([3, 2]).unwrap();
        let (alternate_mesh, alternate_correspondence) =
            GeometryMeshCorrespondenceEnvelopeV1::from_planar_rectangle_v2_affine_triangles(
                &geometry,
                alternate_policy.cells(),
            )
            .unwrap();
        let alternate_production =
            MeshProductionLineageEnvelopeV1::from_affine_triangle_rectangle_v1_resources(
                alternate_policy,
                &geometry,
                &alternate_mesh,
                &alternate_correspondence,
            )
            .unwrap();
        assert!(
            AuthenticatedCommonMesh::affine_triangle_rectangle(
                geometry.clone(),
                alternate_mesh,
                correspondence.clone(),
                production.clone(),
            )
            .is_err()
        );
        assert!(
            AuthenticatedCommonMesh::affine_triangle_rectangle(
                geometry.clone(),
                mesh.clone(),
                alternate_correspondence,
                production.clone(),
            )
            .is_err()
        );
        assert!(
            AuthenticatedCommonMesh::affine_triangle_rectangle(
                geometry.clone(),
                mesh.clone(),
                correspondence.clone(),
                alternate_production,
            )
            .is_err()
        );

        let graph = PlanarOperationGraph::new();
        let foreign_rectangle = graph.rectangle([0.0, 2.0], [0.0, 1.0]).unwrap();
        let foreign_edges = foreign_rectangle.boundaries();
        let foreign_geometry = graph
            .build(
                &foreign_rectangle,
                &BTreeMap::from([
                    ("region".to_owned(), vec![foreign_rectangle.region().into()]),
                    ("left".to_owned(), vec![foreign_edges[0].into()]),
                    ("right".to_owned(), vec![foreign_edges[1].into()]),
                    ("bottom".to_owned(), vec![foreign_edges[2].into()]),
                    ("top".to_owned(), vec![foreign_edges[3].into()]),
                ]),
            )
            .unwrap();
        assert!(
            AuthenticatedCommonMesh::affine_triangle_rectangle(
                foreign_geometry,
                mesh,
                correspondence,
                production,
            )
            .is_err()
        );
    }

    fn model(geometry: &CanonicalGeometryV1) -> ModelEnvelope {
        scalar_model_from_source(geometry, COMPONENT)
    }

    fn scalar_model_from_source(geometry: &CanonicalGeometryV1, source: &str) -> ModelEnvelope {
        let region = geometry.entity_set("region").unwrap();
        let supports = [
            ("region", region, None),
            (
                "left",
                geometry.entity_set("left").unwrap(),
                Some(("region", region)),
            ),
            (
                "right",
                geometry.entity_set("right").unwrap(),
                Some(("region", region)),
            ),
            (
                "bottom",
                geometry.entity_set("bottom").unwrap(),
                Some(("region", region)),
            ),
            (
                "top",
                geometry.entity_set("top").unwrap(),
                Some(("region", region)),
            ),
        ];
        let parameters = [
            (
                "wave_number",
                DynQuantity::new(
                    std::f64::consts::PI,
                    DimExponents {
                        length: -1,
                        ..DimExponents::DIMENSIONLESS
                    },
                ),
            ),
            (
                "source_scale",
                DynQuantity::new(
                    2.0 * std::f64::consts::PI.powi(2),
                    DimExponents {
                        length: -2,
                        ..DimExponents::DIMENSIONLESS
                    },
                ),
            ),
        ];
        compile_model(
            "poisson-rectangle.eqi",
            source,
            geometry,
            "PoissonRectangleModel",
            "PoissonRectangle",
            &supports,
            &parameters,
        )
    }

    fn elasticity_model(geometry: &CanonicalGeometryV1, mu: f64) -> ModelEnvelope {
        let region = geometry.entity_set("region").unwrap();
        let supports = [
            ("region", region, None),
            (
                "left",
                geometry.entity_set("left").unwrap(),
                Some(("region", region)),
            ),
            (
                "right",
                geometry.entity_set("right").unwrap(),
                Some(("region", region)),
            ),
            (
                "bottom",
                geometry.entity_set("bottom").unwrap(),
                Some(("region", region)),
            ),
            (
                "top",
                geometry.entity_set("top").unwrap(),
                Some(("region", region)),
            ),
        ];
        let pressure = DimExponents {
            mass: 1,
            length: -1,
            time: -2,
            ..DimExponents::DIMENSIONLESS
        };
        let parameters = [
            ("mu", DynQuantity::new(mu, pressure)),
            ("lambda", DynQuantity::new(0.0, pressure)),
            (
                "length_scale",
                DynQuantity::new(
                    1.0,
                    DimExponents {
                        length: 1,
                        ..DimExponents::DIMENSIONLESS
                    },
                ),
            ),
        ];
        compile_model(
            "mixed-boundary-elasticity.eqi",
            ELASTICITY_COMPONENT,
            geometry,
            "MixedBoundaryElasticityModel",
            "MixedBoundaryElasticity",
            &supports,
            &parameters,
        )
    }

    fn resources(geometry: &CanonicalGeometryV1) -> AuthenticatedCommonMesh {
        let cells = CartesianMeshCellsV1::new([2, 3]).unwrap();
        let (mesh, correspondence) =
            GeometryMeshCorrespondenceEnvelopeV1::from_planar_rectangle_v2_cartesian(
                geometry,
                cells.cells(),
            )
            .unwrap();
        let production = MeshProductionLineageEnvelopeV1::from_structured_cartesian_v1_resources(
            cells,
            geometry,
            &mesh,
            &correspondence,
        )
        .unwrap();
        AuthenticatedCommonMesh::structured_cartesian(
            geometry.clone(),
            mesh,
            correspondence,
            production,
        )
        .unwrap()
    }

    fn affine_resources(geometry: &CanonicalGeometryV1) -> AuthenticatedCommonMesh {
        let cells = AffineTriangleMeshCellsV1::new([2, 3]).unwrap();
        let (mesh, correspondence) =
            GeometryMeshCorrespondenceEnvelopeV1::from_planar_rectangle_v2_affine_triangles(
                geometry,
                cells.cells(),
            )
            .unwrap();
        let production =
            MeshProductionLineageEnvelopeV1::from_affine_triangle_rectangle_v1_resources(
                cells,
                geometry,
                &mesh,
                &correspondence,
            )
            .unwrap();
        AuthenticatedCommonMesh::affine_triangle_rectangle(
            geometry.clone(),
            mesh,
            correspondence,
            production,
        )
        .unwrap()
    }

    fn transient_model() -> ModelEnvelope {
        let compiled = eqiora_compiler::compile("transient-direct.eqi", TRANSIENT_SOURCE)
            .unwrap()
            .pop()
            .unwrap();
        let (transaction, model, _) = compiled.into_parts();
        let mut store = InMemoryGraphStore::new();
        store.commit(transaction).unwrap();
        let program = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
        ModelEnvelope::from_program(&program).unwrap()
    }

    fn gmsh_provider_output(
        mesh: &SimplicialMeshEnvelopeV1,
        source_edge_facets: &[Vec<usize>; 5],
    ) -> Vec<u8> {
        let native = mesh.mesh();
        let vertex_count = native.vertices().len();
        let boundary_count = source_edge_facets.iter().map(Vec::len).sum::<usize>();
        let element_count = boundary_count + native.cells().len();
        let mut output = String::from("$MeshFormat\n4.1 0 8\n$EndMeshFormat\n$Nodes\n");
        writeln!(output, "1 {vertex_count} 1 {vertex_count}").unwrap();
        writeln!(output, "2 1 0 {vertex_count}").unwrap();
        for tag in 1..=vertex_count {
            writeln!(output, "{tag}").unwrap();
        }
        for coordinate in native.vertices() {
            writeln!(output, "{:?} {:?} 0", coordinate[0], coordinate[1]).unwrap();
        }
        output.push_str("$EndNodes\n$Elements\n");
        writeln!(output, "6 {element_count} 1 {element_count}").unwrap();
        let mut element_tag = 1;
        for (entity_tag, source_edge) in [(1, 4), (5, 2), (6, 1), (7, 3), (8, 0)] {
            let facets = &source_edge_facets[source_edge];
            writeln!(output, "1 {entity_tag} 1 {}", facets.len()).unwrap();
            for &facet_index in facets {
                let vertices = native
                    .entity_vertices(MeshEntity::new(1, facet_index))
                    .unwrap();
                writeln!(
                    output,
                    "{element_tag} {} {}",
                    vertices[0].index() + 1,
                    vertices[1].index() + 1,
                )
                .unwrap();
                element_tag += 1;
            }
        }
        writeln!(output, "2 1 2 {}", native.cells().len()).unwrap();
        for cell in native.cells() {
            writeln!(
                output,
                "{element_tag} {} {} {}",
                cell[0] + 1,
                cell[1] + 1,
                cell[2] + 1,
            )
            .unwrap();
            element_tag += 1;
        }
        output.push_str("$EndElements\n");
        assert_eq!(element_tag, element_count + 1);
        output.into_bytes()
    }

    fn linear() -> NativeLinearPolicy {
        NativeLinearPolicy::exact(
            SolverPlan::new(
                LinearSolver::ConjugateGradient,
                1.0e-10,
                1.0e-13,
                NonZeroUsize::new(1000).unwrap(),
            )
            .unwrap(),
            &REFERENCE_LINEAR_SOLVER,
        )
        .unwrap()
    }

    fn stokes_geometry() -> CanonicalGeometryV1 {
        let predecessor = CadAuthoredGraph::new(
            ConstrainedRectangleV1::new((0.0, 2.2), (0.0, 0.41), 0.0).unwrap(),
            1.0,
            1.0e-10,
        )
        .unwrap();
        let end_cap = predecessor.face_handle("end-cap").unwrap();
        let x_lower = predecessor.face_handle("profile-x-lower").unwrap();
        let x_upper = predecessor.face_handle("profile-x-upper").unwrap();
        let y_lower = predecessor.face_handle("profile-y-lower").unwrap();
        let y_upper = predecessor.face_handle("profile-y-upper").unwrap();
        let graph = predecessor
            .circular_through_cut([0.2, 0.2], 0.05, 1.0e-10)
            .unwrap();
        let cut_wall = graph.face_handle("cut-wall").unwrap();
        graph
            .planar_result()
            .unwrap()
            .with_named_topology(&BTreeMap::from([
                ("fluid".to_owned(), vec![end_cap]),
                ("inlet".to_owned(), vec![x_lower]),
                ("outlet".to_owned(), vec![x_upper]),
                ("walls".to_owned(), vec![y_lower, y_upper]),
                ("cylinder".to_owned(), vec![cut_wall]),
            ]))
            .unwrap()
    }

    fn stokes_model(geometry: &CanonicalGeometryV1) -> ModelEnvelope {
        stokes_model_from_source(geometry, STOKES_COMPONENT)
    }

    fn stokes_model_from_source(geometry: &CanonicalGeometryV1, source: &str) -> ModelEnvelope {
        let fluid = geometry.entity_set("fluid").unwrap();
        let supports = [
            ("fluid", fluid, None),
            (
                "inlet",
                geometry.entity_set("inlet").unwrap(),
                Some(("fluid", fluid)),
            ),
            (
                "outlet",
                geometry.entity_set("outlet").unwrap(),
                Some(("fluid", fluid)),
            ),
            (
                "walls",
                geometry.entity_set("walls").unwrap(),
                Some(("fluid", fluid)),
            ),
            (
                "cylinder",
                geometry.entity_set("cylinder").unwrap(),
                Some(("fluid", fluid)),
            ),
        ];
        let parameters = [
            (
                "dynamic_viscosity",
                DynQuantity::new(
                    0.001,
                    DimExponents {
                        mass: 1,
                        length: -1,
                        time: -1,
                        ..DimExponents::DIMENSIONLESS
                    },
                ),
            ),
            (
                "zero_pressure",
                DynQuantity::new(
                    0.0,
                    DimExponents {
                        mass: 1,
                        length: -1,
                        time: -2,
                        ..DimExponents::DIMENSIONLESS
                    },
                ),
            ),
            (
                "inlet_speed",
                DynQuantity::new(
                    0.3,
                    DimExponents {
                        length: 1,
                        time: -1,
                        ..DimExponents::DIMENSIONLESS
                    },
                ),
            ),
            (
                "channel_height",
                DynQuantity::new(
                    0.41,
                    DimExponents {
                        length: 1,
                        ..DimExponents::DIMENSIONLESS
                    },
                ),
            ),
        ];
        compile_model(
            "steady-flow-past-cylinder.eqi",
            source,
            geometry,
            "SteadyFlowPastCylinderModel",
            "SteadyFlowPastCylinder",
            &supports,
            &parameters,
        )
    }

    #[test]
    fn scalar_q1_and_tpfa_consume_one_exact_anisotropic_common_mesh() {
        let geometry = rectangle();
        let model = model(&geometry);
        let exact_owner = resources(&geometry);
        let caller_resources = exact_owner.resources.clone();
        let q1 = NativeNumericalAdmission::admit(
            &model,
            exact_owner.clone(),
            NativeSpatialPolicy::ScalarQ1,
            linear(),
        )
        .unwrap();
        let q1_repeat = NativeNumericalAdmission::admit(
            &model,
            resources(&geometry),
            NativeSpatialPolicy::ScalarQ1,
            linear(),
        )
        .unwrap();
        let tpfa = NativeNumericalAdmission::admit(
            &model,
            exact_owner,
            NativeSpatialPolicy::ScalarTpfa,
            linear(),
        )
        .unwrap();
        let alternate_provider = NativeNumericalAdmission::admit(
            &model,
            resources(&geometry),
            NativeSpatialPolicy::ScalarQ1,
            NativeLinearPolicy::exact(
                SolverPlan::new(
                    LinearSolver::ConjugateGradient,
                    1.0e-10,
                    1.0e-13,
                    NonZeroUsize::new(1000).unwrap(),
                )
                .unwrap(),
                &AlternateScalarBackend,
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(q1.model(), &model);
        assert_eq!(q1.model_digest(), model.digest().unwrap().to_string());
        assert_ne!(q1.policy_identity(), tpfa.policy_identity());
        assert_eq!(q1.policy_identity(), q1_repeat.policy_identity());
        assert_ne!(q1.policy_identity(), alternate_provider.policy_identity());
        assert_eq!(q1.resources(), &caller_resources);
        assert_eq!(tpfa.resources(), &caller_resources);
        assert_eq!(q1.resources(), q1_repeat.resources());
        assert!(q1.execute_scalar(&AlternateScalarBackend).is_err());
        assert_eq!(
            q1.execute_scalar(&REFERENCE_LINEAR_SOLVER)
                .unwrap()
                .into_primary_field_values()
                .len(),
            12
        );
        assert_eq!(
            tpfa.execute_scalar(&REFERENCE_LINEAR_SOLVER)
                .unwrap()
                .into_primary_field_values()
                .len(),
            6
        );
    }

    #[test]
    fn common_scalar_plan_owns_exact_lineage_and_executes_without_repeated_inputs() {
        let geometry = rectangle();
        let model = model(&geometry);
        let linear = SolverPlan::new(
            LinearSolver::ConjugateGradient,
            1.0e-10,
            1.0e-12,
            NonZeroUsize::new(10_000).unwrap(),
        )
        .unwrap();
        let resolve_scalar = |spatial, solve| {
            resolve_common_plan(
                &model,
                resources(&geometry),
                spatial,
                CommonSolvePolicy::Linear(solve),
                None,
                None,
                &ResolveOnlyBackend,
            )
            .unwrap()
            .project(
                |_| panic!("spatial Model resolved as no-Mesh ODE"),
                |plan| plan,
                |_| panic!("scalar Model resolved as elasticity"),
                |_| panic!("scalar Model resolved as another capability"),
                |_| panic!("scalar Model resolved as transient capability"),
            )
        };
        let q1 = resolve_scalar(CommonSpatialPolicy::Q1, linear);
        let repeat = resolve_scalar(CommonSpatialPolicy::Q1, linear);
        let tpfa = resolve_scalar(CommonSpatialPolicy::CellCenteredTpfa, linear);
        let alternate_tolerance = resolve_scalar(
            CommonSpatialPolicy::Q1,
            SolverPlan::new(
                LinearSolver::ConjugateGradient,
                1.0e-9,
                1.0e-12,
                NonZeroUsize::new(10_000).unwrap(),
            )
            .unwrap(),
        );

        assert_eq!(q1.identity(), repeat.identity());
        assert_ne!(q1.identity(), tpfa.identity());
        assert_ne!(q1.identity(), alternate_tolerance.identity());
        assert_eq!(q1.model_digest(), model.digest().unwrap().to_string());
        assert_eq!(q1.cells(), [2, 3]);
        assert_eq!(q1.run().unwrap().into_primary_field_values().len(), 12);
        assert_eq!(tpfa.run().unwrap().into_primary_field_values().len(), 6);
        assert!(
            resolve_common_plan(
                &model,
                resources(&geometry),
                CommonSpatialPolicy::Q1,
                CommonSolvePolicy::Linear(
                    SolverPlan::new(
                        LinearSolver::MinimumResidual,
                        1.0e-10,
                        1.0e-12,
                        NonZeroUsize::new(10_000).unwrap(),
                    )
                    .unwrap()
                ),
                None,
                None,
                &ResolveOnlyBackend,
            )
            .is_err()
        );
        assert!(
            resolve_common_plan(
                &model,
                resources(&geometry),
                CommonSpatialPolicy::MiniP1,
                CommonSolvePolicy::Linear(linear),
                None,
                None,
                &ResolveOnlyBackend,
            )
            .is_err()
        );
    }

    #[test]
    fn common_elasticity_plan_consumes_exact_mesh_and_model_meaning() {
        let geometry = rectangle();
        let model = elasticity_model(&geometry, 3.0);
        let alternate_material = elasticity_model(&geometry, 4.0);
        let solve = SolverPlan::new(
            LinearSolver::ConjugateGradient,
            1.0e-10,
            1.0e-12,
            NonZeroUsize::new(10_000).unwrap(),
        )
        .unwrap();
        let resolve_elasticity = |model: &ModelEnvelope| {
            resolve_common_plan(
                model,
                resources(&geometry),
                CommonSpatialPolicy::Q1,
                CommonSolvePolicy::Linear(solve),
                None,
                None,
                &ResolveOnlyBackend,
            )
            .unwrap()
            .project(
                |_| panic!("spatial Model resolved as no-Mesh ODE"),
                |_| panic!("elasticity Model resolved as scalar"),
                |plan| plan,
                |_| panic!("elasticity Model resolved as Stokes"),
                |_| panic!("elasticity Model resolved as transient flow"),
            )
        };
        let plan = resolve_elasticity(&model);
        let repeat = resolve_elasticity(&model);
        let alternate = resolve_elasticity(&alternate_material);
        assert_eq!(plan.identity(), repeat.identity());
        assert_ne!(plan.identity(), alternate.identity());
        assert_eq!(plan.model_digest(), model.digest().unwrap().to_string());
        assert_eq!(plan.cells(), [2, 3]);
        let result = plan.run().unwrap();
        assert_eq!(result.displacement().mesh().axis_cell_count(0), Some(2));
        assert_eq!(result.displacement().mesh().axis_cell_count(1), Some(3));
        assert_eq!(result.displacement().values().len(), 24);
        assert!(
            resolve_common_plan(
                &model,
                resources(&geometry),
                CommonSpatialPolicy::CellCenteredTpfa,
                CommonSolvePolicy::Linear(solve),
                None,
                None,
                &ResolveOnlyBackend,
            )
            .is_err()
        );
    }

    #[test]
    fn admission_rejects_policy_and_resource_cross_wires() {
        let geometry = rectangle();
        let model = model(&geometry);
        assert!(
            NativeLinearPolicy::exact(
                SolverPlan::new(
                    LinearSolver::ConjugateGradient,
                    -0.0,
                    1.0e-13,
                    NonZeroUsize::new(1000).unwrap(),
                )
                .unwrap(),
                &REFERENCE_LINEAR_SOLVER,
            )
            .is_err()
        );
        for solver in [
            SolverPlan::new(
                LinearSolver::ConjugateGradient,
                1.0e-10,
                1.0e-13,
                NonZeroUsize::new(1000).unwrap(),
            )
            .unwrap()
            .with_preconditioner(PreconditionerPolicy::Jacobi),
            SolverPlan::new(
                LinearSolver::ConjugateGradient,
                1.0e-10,
                1.0e-13,
                NonZeroUsize::new(1000).unwrap(),
            )
            .unwrap()
            .with_reduction(ReductionPolicy::Fast),
            SolverPlan::new(
                LinearSolver::BiConjugateGradientStabilized,
                1.0e-10,
                1.0e-13,
                NonZeroUsize::new(1000).unwrap(),
            )
            .unwrap(),
        ] {
            assert!(
                NativeNumericalAdmission::admit(
                    &model,
                    resources(&geometry),
                    NativeSpatialPolicy::ScalarQ1,
                    NativeLinearPolicy::exact(solver, &REFERENCE_LINEAR_SOLVER).unwrap(),
                )
                .is_err()
            );
        }
        assert!(
            NativeNumericalAdmission::admit(
                &model,
                resources(&geometry),
                NativeSpatialPolicy::StokesMiniP1(
                    IncompressibleFlowScaleProfile2d::new(
                        DynQuantity::new(
                            1.0,
                            DimExponents {
                                length: 1,
                                ..DimExponents::DIMENSIONLESS
                            }
                        ),
                        DynQuantity::new(
                            1.0,
                            DimExponents {
                                length: 1,
                                time: -1,
                                ..DimExponents::DIMENSIONLESS
                            }
                        ),
                        DynQuantity::new(
                            1.0,
                            DimExponents {
                                mass: 1,
                                length: -1,
                                time: -2,
                                ..DimExponents::DIMENSIONLESS
                            }
                        ),
                    )
                    .unwrap(),
                ),
                linear(),
            )
            .is_err()
        );

        let owner = resources(&geometry);
        let NativeMeshResources::Cartesian {
            geometry: exact_geometry,
            mesh,
            correspondence,
            production,
        } = owner.resources
        else {
            unreachable!()
        };
        let substituted_mesh = CartesianMeshEnvelopeV1::from_mesh(
            &CartesianMesh::from_axes(vec![vec![0.0, 0.25, 1.0], vec![0.0, 0.2, 0.8, 1.0]])
                .unwrap(),
        )
        .unwrap();
        assert!(
            AuthenticatedCommonMesh::structured_cartesian(
                exact_geometry.clone(),
                substituted_mesh,
                correspondence.clone(),
                production.clone(),
            )
            .is_err()
        );
        let mut correspondence_value: serde_json::Value =
            serde_json::from_slice(&correspondence.canonical_json().unwrap()).unwrap();
        let frontiers = correspondence_value["frontiers"].as_array_mut().unwrap();
        let left = frontiers[0]["facet_indices"].clone();
        let right = frontiers[1]["facet_indices"].clone();
        frontiers[0]["facet_indices"] = right;
        frontiers[1]["facet_indices"] = left;
        let relabelled = GeometryMeshCorrespondenceEnvelopeV1::from_json(
            &serde_json::to_vec(&correspondence_value).unwrap(),
            GeometryDecoderLimits::default(),
        )
        .unwrap();
        assert!(
            AuthenticatedCommonMesh::structured_cartesian(
                exact_geometry.clone(),
                mesh.clone(),
                relabelled,
                production.clone(),
            )
            .is_err()
        );
        let production_json = String::from_utf8(production.canonical_json().unwrap()).unwrap();
        let provider_mutation = production_json.replace(
            "\"identity\":\"eqiora.structured-cartesian\",\"version\":\"1\"",
            "\"identity\":\"eqiora.gmsh-cli\",\"version\":\"4.15.2\"",
        );
        assert!(MeshProductionLineageEnvelopeV1::from_json(provider_mutation.as_bytes()).is_err());
        let foreign_production_json = production_json.replace(
            &correspondence.digest().unwrap().to_string(),
            &"00".repeat(32),
        );
        let foreign_production =
            MeshProductionLineageEnvelopeV1::from_json(foreign_production_json.as_bytes()).unwrap();
        assert!(
            AuthenticatedCommonMesh::structured_cartesian(
                exact_geometry,
                mesh,
                correspondence,
                foreign_production,
            )
            .is_err()
        );

        let reaction_source = COMPONENT.replace(
            "-div(grad(potential))\n      - source_scale * sin(wave_number * coordinate(0))\n        * sin(wave_number * coordinate(1)) = 0;",
            "potential - 1 = 0;",
        );
        let reaction = scalar_model_from_source(&geometry, &reaction_source);
        let reaction_program = replay_program(&reaction, &geometry).unwrap();
        let reaction_transient =
            lower_transient_incompressible_navier_stokes_cartesian_2d(&reaction_program);
        assert!(recognize_capability(&reaction_program, &reaction_transient).is_err());

        let stokes_geometry = stokes_geometry();
        let non_stokes_source =
            STOKES_COMPONENT.replace("div(velocity) = 0;", "pressure - zero_pressure = 0;");
        let non_stokes = stokes_model_from_source(&stokes_geometry, &non_stokes_source);
        let non_stokes_program = replay_program(&non_stokes, &stokes_geometry).unwrap();
        let non_stokes_transient =
            lower_transient_incompressible_navier_stokes_cartesian_2d(&non_stokes_program);
        assert!(recognize_capability(&non_stokes_program, &non_stokes_transient).is_err());

        let foreign = rectangle();
        let mut foreign_resources = resources(&foreign);
        if let NativeMeshResources::Cartesian { geometry, .. } = &mut foreign_resources.resources {
            *geometry = {
                let graph = PlanarOperationGraph::new();
                let rectangle = graph.rectangle([0.0, 2.0], [0.0, 1.0]).unwrap();
                let edges = rectangle.boundaries();
                graph
                    .build(
                        &rectangle,
                        &BTreeMap::from([
                            ("region".to_owned(), vec![rectangle.region().into()]),
                            ("left".to_owned(), vec![edges[0].into()]),
                            ("right".to_owned(), vec![edges[1].into()]),
                            ("bottom".to_owned(), vec![edges[2].into()]),
                            ("top".to_owned(), vec![edges[3].into()]),
                        ]),
                    )
                    .unwrap()
            };
        }
        assert!(
            NativeNumericalAdmission::admit(
                &model,
                foreign_resources,
                NativeSpatialPolicy::ScalarQ1,
                linear(),
            )
            .is_err()
        );
    }

    #[test]
    fn stokes_resolution_consumes_exact_source_owned_common_mesh() {
        let geometry = stokes_geometry();
        let model = stokes_model(&geometry);
        let policy = PlanarMeshQualityV1::new(1.0e-4, 1.0e-5, 50).unwrap();
        let (mesh, correspondence) =
            GeometryMeshCorrespondenceEnvelopeV1::from_planar_circular_hole_v2_reference(
                &geometry,
                policy.maximum_boundary_error_m(),
                policy.maximum_boundary_facets(),
                MeshQualityGate::new(policy.minimum_mean_ratio()).unwrap(),
            )
            .unwrap();
        let production =
            MeshProductionLineageEnvelopeV1::from_planar_circular_hole_reference_v1_resources(
                policy,
                &geometry,
                &mesh,
                &correspondence,
            )
            .unwrap();
        let correspondence_value: serde_json::Value =
            serde_json::from_slice(&correspondence.canonical_json().unwrap()).unwrap();
        let frontiers = correspondence_value["frontiers"].as_array().unwrap();
        let assignment_proof: [Vec<usize>; 5] = std::array::from_fn(|edge| {
            frontiers[edge]["facet_indices"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| usize::try_from(value.as_u64().unwrap()).unwrap())
                .collect()
        });
        let provider_output = gmsh_provider_output(&mesh, &assignment_proof);
        let exact_gmsh =
            AuthenticatedCommonMesh::gmsh_4152(geometry.clone(), policy, provider_output.clone())
                .unwrap();
        let mut relabelled_assignments = assignment_proof.clone();
        relabelled_assignments.swap(0, 1);
        let relabelled_output = gmsh_provider_output(&mesh, &relabelled_assignments);
        let relabelled_gmsh =
            AuthenticatedCommonMesh::gmsh_4152(geometry.clone(), policy, relabelled_output)
                .unwrap();
        assert_ne!(exact_gmsh, relabelled_gmsh);
        let NativeMeshResources::GmshSimplicial {
            correspondence: exact_correspondence,
            production: exact_production,
            ..
        } = &exact_gmsh.resources
        else {
            unreachable!("Gmsh factory returns Gmsh resources")
        };
        let NativeMeshResources::GmshSimplicial {
            correspondence: relabelled_correspondence,
            production: relabelled_production,
            ..
        } = &relabelled_gmsh.resources
        else {
            unreachable!("Gmsh factory returns Gmsh resources")
        };
        assert_ne!(exact_correspondence, relabelled_correspondence);
        assert_ne!(exact_production, relabelled_production);
        let malformed_output = provider_output
            .windows(b"1 5 1".len())
            .position(|window| window == b"1 5 1")
            .map(|offset| {
                let mut mutated = provider_output.clone();
                mutated[offset + 2] = b'9';
                mutated
            })
            .unwrap();
        assert!(
            AuthenticatedCommonMesh::gmsh_4152(geometry.clone(), policy, malformed_output).is_err()
        );
        let resources = AuthenticatedCommonMesh::planar_reference(
            geometry,
            mesh.clone(),
            correspondence,
            production,
        )
        .unwrap();
        let solver = SolverPlan::new(
            LinearSolver::ConjugateGradient,
            1.0e-6,
            1.0e-13,
            NonZeroUsize::new(10_000).unwrap(),
        )
        .unwrap();
        let common = resolve_common_plan(
            &model,
            resources.clone(),
            CommonSpatialPolicy::MiniP1,
            CommonSolvePolicy::Linear(solver),
            None,
            None,
            &ResolveOnlyBackend,
        )
        .unwrap()
        .project(
            |_| panic!("spatial Model resolved as no-Mesh ODE"),
            |_| panic!("steady-Stokes Model resolved as another capability"),
            |_| panic!("steady-Stokes Model resolved as elasticity"),
            |plan| plan,
            |_| panic!("steady-Stokes Model resolved as transient capability"),
        );
        let gmsh_common = resolve_common_plan(
            &model,
            exact_gmsh,
            CommonSpatialPolicy::MiniP1,
            CommonSolvePolicy::Linear(solver),
            None,
            None,
            &ResolveOnlyBackend,
        )
        .unwrap()
        .project(
            |_| panic!("spatial Model resolved as no-Mesh ODE"),
            |_| panic!("steady-Stokes Model resolved as another capability"),
            |_| panic!("steady-Stokes Model resolved as elasticity"),
            |plan| plan,
            |_| panic!("steady-Stokes Model resolved as transient capability"),
        );
        assert_eq!(common.model_digest(), model.digest().unwrap().to_string());
        assert_eq!(common.mesh_digest(), mesh.digest().unwrap().to_string());
        assert_eq!(
            common.scales().length().value().to_bits(),
            0.41_f64.to_bits()
        );
        assert_eq!(
            common.scales().velocity().value().to_bits(),
            0.3_f64.to_bits()
        );
        assert_eq!(
            common.scales().pressure().value().to_bits(),
            (0.001_f64 * 0.3 / 0.41).to_bits()
        );
        assert_eq!(
            [
                common.scales().length().value().to_bits(),
                common.scales().velocity().value().to_bits(),
                common.scales().pressure().value().to_bits(),
                common.scales().gauge().value().to_bits(),
                common.scales().weak_functional().value().to_bits(),
            ],
            [
                gmsh_common.scales().length().value().to_bits(),
                gmsh_common.scales().velocity().value().to_bits(),
                gmsh_common.scales().pressure().value().to_bits(),
                gmsh_common.scales().gauge().value().to_bits(),
                gmsh_common.scales().weak_functional().value().to_bits(),
            ],
            "reference and Gmsh occurrences of one exact source must resolve bit-equal automatic scales",
        );
        assert_eq!(common.linear().algorithm(), LinearSolver::SparseLu);
        assert_eq!(common.linear().reduction(), ReductionPolicy::Fast);
        assert_eq!(
            common.linear().relative_tolerance(),
            solver.relative_tolerance()
        );
        assert_eq!(
            common.linear().absolute_tolerance(),
            solver.absolute_tolerance()
        );
        assert_eq!(
            common.linear().maximum_iterations(),
            solver.maximum_iterations()
        );
        let admission = NativeNumericalAdmission::admit(
            &model,
            resources,
            NativeSpatialPolicy::StokesMiniP1(common.scales()),
            NativeLinearPolicy::exact(common.linear(), &ResolveOnlyBackend).unwrap(),
        )
        .unwrap();
        assert_eq!(
            admission.capability(),
            NativeCapability::SteadyIncompressibleStokes
        );
        assert_eq!(admission.model(), &model);
        let binding = admission.stokes_binding().unwrap();
        let (_resolved, realization, velocity, pressure) =
            admission.resolve_stokes(&binding).unwrap();
        assert_eq!(
            realization.mesh_artifact().unwrap(),
            Some(mesh.digest().unwrap())
        );
        assert_eq!(velocity.family(), SpaceFamily::SimplexP1Bubble);
        assert!(matches!(
            pressure.family(),
            SpaceFamily::ContinuousLagrange { .. }
        ));
    }

    #[test]
    fn transient_common_plan_resolves_exact_mini_and_supplied_cartesian_resources() {
        let geometry = rectangle();
        let model = transient_model();
        let replayed = ModelEnvelope::from_json(
            &model.canonical_json().unwrap(),
            ModelDecoderLimits::default(),
        )
        .unwrap();
        let linear = SolverPlan::new(
            LinearSolver::ConjugateGradient,
            1.0e-10,
            1.0e-12,
            NonZeroUsize::new(2_000).unwrap(),
        )
        .unwrap();
        let temporal = CommonBackwardEuler::from_seconds(0.01).unwrap();
        let nonlinear =
            NonlinearSolvePlan::new(1.0e-9, 1.0e-11, NonZeroUsize::new(16).unwrap(), 12).unwrap();
        let scaling =
            IncompressibleScalingRequest2d::from_si(Some(1.0), Some(2.0), Some(3.0)).unwrap();
        let resolve = |model: &ModelEnvelope, owner, spatial| {
            resolve_common_plan(
                model,
                owner,
                spatial,
                CommonSolvePolicy::newton(linear, nonlinear),
                Some(scaling),
                Some(temporal),
                &ResolveOnlyBackend,
            )
            .unwrap()
            .project(
                |_| panic!("spatial Model resolved as no-Mesh ODE"),
                |_| panic!("transient Model resolved as scalar"),
                |_| panic!("transient Model resolved as elasticity"),
                |_| panic!("transient Model resolved as steady Stokes"),
                |plan| plan,
            )
        };
        let mini = resolve(
            &model,
            affine_resources(&geometry),
            CommonSpatialPolicy::MiniP1,
        );
        let mini_replay = resolve(
            &replayed,
            affine_resources(&geometry),
            CommonSpatialPolicy::MiniP1,
        );
        let fvm = resolve(
            &model,
            resources(&geometry),
            CommonSpatialPolicy::CellCentered,
        );
        let custom_nonlinear =
            NonlinearSolvePlan::new(2.0e-9, 3.0e-11, NonZeroUsize::new(19).unwrap(), 7).unwrap();
        let custom = resolve_common_plan(
            &model,
            affine_resources(&geometry),
            CommonSpatialPolicy::MiniP1,
            CommonSolvePolicy::newton(linear, custom_nonlinear),
            Some(scaling),
            Some(temporal),
            &ResolveOnlyBackend,
        )
        .unwrap()
        .project(
            |_| panic!("spatial Model resolved as no-Mesh ODE"),
            |_| panic!("transient Model resolved as scalar"),
            |_| panic!("transient Model resolved as elasticity"),
            |_| panic!("transient Model resolved as steady Stokes"),
            |plan| plan,
        );
        let alternate_scaling =
            IncompressibleScalingRequest2d::from_si(Some(4.0), Some(5.0), Some(6.0)).unwrap();
        let fvm_alternate_scaling = resolve_common_plan(
            &model,
            resources(&geometry),
            CommonSpatialPolicy::CellCentered,
            CommonSolvePolicy::newton(linear, nonlinear),
            Some(alternate_scaling),
            Some(temporal),
            &ResolveOnlyBackend,
        )
        .unwrap()
        .project(
            |_| panic!("spatial Model resolved as no-Mesh ODE"),
            |_| panic!("transient Model resolved as scalar"),
            |_| panic!("transient Model resolved as elasticity"),
            |_| panic!("transient Model resolved as steady Stokes"),
            |plan| plan,
        );

        assert_eq!(mini.identity(), mini_replay.identity());
        assert_ne!(mini.identity(), fvm.identity());
        assert_ne!(mini.identity(), custom.identity());
        assert_eq!(custom.nonlinear(), custom_nonlinear);
        assert_eq!(mini.model_digest(), model.digest().unwrap().to_string());
        assert_eq!(mini.velocity_field_id(), fvm.velocity_field_id());
        assert_eq!(mini.pressure_field_id(), fvm.pressure_field_id());
        assert_eq!(mini.velocity_space().family(), SpaceFamily::SimplexP1Bubble);
        assert!(
            matches!(mini.pressure_space().family(), SpaceFamily::ContinuousLagrange { order } if order.get() == 1)
        );
        assert_eq!(fvm.velocity_space().family(), SpaceFamily::CellConstant);
        assert_eq!(fvm.pressure_space().family(), SpaceFamily::CellConstant);
        assert_eq!(mini.temporal().step().value().to_bits(), 0.01_f64.to_bits());
        assert_eq!(mini.scales().length().value().to_bits(), 1.0_f64.to_bits());
        assert_eq!(
            mini.scales().velocity().value().to_bits(),
            2.0_f64.to_bits()
        );
        assert_eq!(
            mini.scales().pressure().value().to_bits(),
            3.0_f64.to_bits()
        );
        assert_eq!(
            mini.linear().algorithm(),
            LinearSolver::BiConjugateGradientStabilized
        );
        assert_eq!(mini.linear().reduction(), ReductionPolicy::Fast);
        assert_eq!(fvm.linear().reduction(), ReductionPolicy::Reproducible);

        let mini_zero = mini.zero_state(0.0).unwrap();
        let fvm_zero = fvm.zero_state(0.0).unwrap();
        assert_eq!(mini_zero.velocity_vertex_values().unwrap().len(), 12);
        assert_eq!(mini_zero.velocity_cell_values().len(), 12);
        assert_eq!(mini_zero.pressure_vertex_values().unwrap().len(), 12);
        assert!(mini_zero.method_history_values().is_empty());
        assert_eq!(fvm_zero.velocity_cell_values().len(), 6);
        assert_eq!(fvm_zero.pressure_cell_values().unwrap().len(), 6);
        assert!(!fvm_zero.method_history_values().is_empty());
        assert_eq!(custom.state_space_identity(), mini.state_space_identity());
        assert_eq!(
            fvm_alternate_scaling.state_space_identity(),
            fvm.state_space_identity(),
            "coherent-SI State compatibility excludes numerical scaling",
        );
        assert!(CommonTransientRunRequest::from_steps(
            custom.clone(),
            mini_zero.clone(),
            2,
            vec![1, 2],
        )
        .is_ok());
        assert!(
            CommonTransientRunRequest::from_steps(mini.clone(), fvm_zero, 1, vec![1],).is_err()
        );
        let by_steps =
            CommonTransientRunRequest::from_steps(mini.clone(), mini_zero.clone(), 2, vec![1, 2])
                .unwrap();
        let by_times =
            CommonTransientRunRequest::from_times(mini.clone(), mini_zero, 0.02, vec![0.01, 0.02])
                .unwrap();
        assert_eq!(by_steps.identity(), by_times.identity());
        assert!(
            CommonTransientRunRequest::from_steps(mini, by_times.state().clone(), 2, vec![2, 1],)
                .is_err()
        );

        assert!(
            resolve_common_plan(
                &model,
                affine_resources(&geometry),
                CommonSpatialPolicy::MiniP1,
                CommonSolvePolicy::Linear(linear),
                Some(scaling),
                Some(temporal),
                &ResolveOnlyBackend,
            )
            .is_err()
        );
        assert!(
            resolve_common_plan(
                &model,
                affine_resources(&geometry),
                CommonSpatialPolicy::MiniP1,
                CommonSolvePolicy::newton(linear, nonlinear),
                Some(IncompressibleScalingRequest2d::from_si(Some(1.0), None, Some(3.0)).unwrap()),
                Some(temporal),
                &ResolveOnlyBackend,
            )
            .is_err()
        );
        assert!(
            resolve_common_plan(
                &model,
                affine_resources(&geometry),
                CommonSpatialPolicy::MiniP1,
                CommonSolvePolicy::newton(linear, nonlinear),
                Some(scaling),
                None,
                &ResolveOnlyBackend,
            )
            .is_err()
        );
        assert!(
            resolve_common_plan(
                &model,
                resources(&geometry),
                CommonSpatialPolicy::MiniP1,
                CommonSolvePolicy::newton(linear, nonlinear),
                Some(scaling),
                Some(temporal),
                &ResolveOnlyBackend,
            )
            .is_err()
        );
    }

    #[test]
    fn registered_model_driven_common_mesh_admission_evidence() {
        scalar_q1_and_tpfa_consume_one_exact_anisotropic_common_mesh();
        admission_rejects_policy_and_resource_cross_wires();
        stokes_resolution_consumes_exact_source_owned_common_mesh();
        transient_common_plan_resolves_exact_mini_and_supplied_cartesian_resources();
    }
}
