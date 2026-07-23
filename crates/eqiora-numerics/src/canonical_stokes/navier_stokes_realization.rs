//! Coherent-SI reference realization of the fixed-domain transient flow subset.

use std::num::{NonZeroU16, NonZeroUsize};

use eqiora_artifact::SimplicialMeshEnvelopeV1;
use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, DimExponents, DynQuantity, Id, RawId, ValueShape};
use eqiora_realization::{
    AlgebraicBlock, AlgebraicBlockScale, AlgebraicConstraint, BackwardEulerRelationStep,
    CoordinateTreatment, Discretization, DiscretizationMethod, EnergySkewConvection,
    ExecutionSchedule, FieldSpaceBinding, FieldwiseRealizationPlan,
    FieldwiseRealizationRequirements, FieldwiseSpatialDiscretization, MeshArtifactReference,
    MeshPolicy, NonlinearSolvePlan, PlacementRequirementNode, PortableRealizationGraph,
    PositivePhysicalScale, QuadraturePolicy, RealizationRequirements,
    ResolvedTransientFieldwiseRealization, ScalarType, SolveRoot, Space,
    SymmetricCongruenceScaling, SystemBlock, Target, TransformationNode,
    TransientFieldwiseRealizationPlan, TransientFieldwiseRealizationRequirements, VectorLayoutKind,
};
use eqiora_schema::kernel::ValueFrame;
use eqiora_sem::KernelProgram;
use eqiora_solver::{LinearOperatorProperties, LinearSolverBackend, SolverPlan};

use crate::discrete_block::{
    AlgebraicClosure, AuxiliaryBlock, BlockRealizationIdentity, BlockSupport, BlockTransformation,
    ContributionBatch, ContributionTerm, DiscreteBlockContext, DiscreteBlockSystem, FieldBlock,
    FieldBlockRole, RelationBlock, RelationDisposition, ResidualBlock, ResidualOrigin,
};
use crate::{
    AssemblyBackend, MeshTopology, NonZeroStepCount, REFERENCE_ASSEMBLY_BACKEND, SimplicialMesh,
    SimplicialMiniNavierStokesState2d, SimplicialMiniNavierStokesStepEvidence2d,
    SimplicialMiniStokesBoundary2d, SimplicialMiniStokesPressureReference2d,
    SimplicialMiniVelocityField2d, SimplicialP1Field,
    advance_simplicial_mini_navier_stokes_2d_with_assembly, simplex_centroid_rule,
    triangle_duffy_gauss_legendre,
};

use super::realization::normalize_cartesian_mesh;
use super::{
    IncompressibleFlowScaleProfile2d, TransientIncompressibleNavierStokesCartesianModel2d,
    lower_transient_incompressible_navier_stokes_cartesian_2d,
};

const DIMENSION: usize = 2;
const DUFFY_POINTS_PER_AXIS: usize = 5;
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

/// Repeated-step request, deliberately separate from Realization identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransientNavierStokesRun2d {
    step_count: NonZeroStepCount,
}

impl TransientNavierStokesRun2d {
    /// Request a non-zero number of accepted applications of one finalized step.
    #[must_use]
    pub const fn new(step_count: NonZeroStepCount) -> Self {
        Self { step_count }
    }

    /// Requested accepted-step count.
    #[must_use]
    pub const fn step_count(self) -> NonZeroStepCount {
        self.step_count
    }
}

/// Coherent-SI initial state admitted from an already accepted steady solve.
///
/// This type cannot be passed to the dimensionless MINI kernel. It retains
/// exact Field and mesh identities so scaling and provenance drift fail before
/// numerical normalization.
#[derive(Debug, Clone, PartialEq)]
pub struct TransientNavierStokesInitialState2d {
    time: DynQuantity,
    mesh_artifact: MeshArtifactReference,
    velocity_field: Id<kinds::Field>,
    pressure_field: Id<kinds::Field>,
    velocity: SimplicialMiniVelocityField2d,
    pressure: SimplicialP1Field,
    pressure_reference: super::SteadyStokesPressureReference2d,
}

impl TransientNavierStokesInitialState2d {
    /// Bind coherent-SI fields to exact transient Model and mesh identities.
    ///
    /// This constructor validates shape and provenance only. The selected
    /// Realization reassembles continuity, pressure-mean, and gauge conditions
    /// before the first Newton iteration.
    ///
    /// # Errors
    /// Returns `EQ0807` for invalid time or mismatched velocity/pressure mesh.
    pub fn new(
        model: &TransientIncompressibleNavierStokesCartesianModel2d,
        time: DynQuantity,
        mesh_artifact: MeshArtifactReference,
        velocity: SimplicialMiniVelocityField2d,
        pressure: SimplicialP1Field,
        pressure_reference: super::SteadyStokesPressureReference2d,
    ) -> Result<Self, Diagnostic> {
        if time.dim() != time_dimension() || !time.value().is_finite() || time.value() < 0.0 {
            return Err(invalid_realization(
                "transient initial time must be finite, non-negative, and have physical time dimension",
            ));
        }
        if velocity.mesh() != pressure.mesh() {
            return Err(invalid_realization(
                "transient initial velocity and pressure must share one exact mesh",
            ));
        }
        if pressure_reference
            .gauge_multiplier()
            .is_some_and(|value| !value.is_finite())
        {
            return Err(invalid_realization(
                "transient initial pressure-reference evidence must be finite",
            ));
        }
        Ok(Self {
            time,
            mesh_artifact,
            velocity_field: velocity_id(model),
            pressure_field: pressure_id(model),
            velocity,
            pressure,
            pressure_reference,
        })
    }
}

/// One accepted coherent-SI transient state with exact semantic provenance.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedTransientNavierStokesState2d {
    time: DynQuantity,
    velocity_field: Id<kinds::Field>,
    pressure_field: Id<kinds::Field>,
    velocity: SimplicialMiniVelocityField2d,
    pressure: SimplicialP1Field,
    pressure_reference: super::SteadyStokesPressureReference2d,
}

impl ResolvedTransientNavierStokesState2d {
    /// Accepted physical model time.
    #[must_use]
    pub const fn time(&self) -> DynQuantity {
        self.time
    }

    /// Exact Semantic velocity Field.
    #[must_use]
    pub const fn velocity_field(&self) -> Id<kinds::Field> {
        self.velocity_field
    }

    /// Exact Semantic pressure Field.
    #[must_use]
    pub const fn pressure_field(&self) -> Id<kinds::Field> {
        self.pressure_field
    }

    /// Physical MINI velocity in metres per second.
    #[must_use]
    pub const fn velocity(&self) -> &SimplicialMiniVelocityField2d {
        &self.velocity
    }

    /// Physical P1 pressure in pascals.
    #[must_use]
    pub const fn pressure(&self) -> &SimplicialP1Field {
        &self.pressure
    }

    /// Physical pressure closure retained at acceptance.
    #[must_use]
    pub const fn pressure_reference(&self) -> super::SteadyStokesPressureReference2d {
        self.pressure_reference
    }
}

/// Exact lowerer requirements for the admitted transient MINI path.
#[must_use]
pub fn transient_navier_stokes_fieldwise_requirements_2d(
    model: &TransientIncompressibleNavierStokesCartesianModel2d,
) -> TransientFieldwiseRealizationRequirements {
    let fieldwise = FieldwiseRealizationRequirements::new(
        domain_id(model),
        [velocity_id(model), pressure_id(model)],
        RealizationRequirements::new(
            NonZeroUsize::new(DIMENSION).expect("two is non-zero"),
            ScalarType::F64,
            VectorLayoutKind::Replicated,
        ),
    )
    .expect("a lowered transient flow model owns two distinct algebraic Fields");
    TransientFieldwiseRealizationRequirements::new(
        fieldwise,
        momentum_id(model),
        velocity_id(model),
    )
    .expect("the transient velocity is part of the exact Field inventory")
}

/// Build the sole admitted transient MINI/P1 reference Realization.
///
/// The result owns method, mesh, quadrature, scaling, time discretization,
/// nonlinear policy, linear solver, and placement. Repeated step count remains
/// a separate [`TransientNavierStokesRun2d`].
///
/// # Errors
/// Returns `EQ0807` for an invalid scale, solver, or composed transient plan.
pub fn transient_navier_stokes_mini_plan_2d(
    model: &TransientIncompressibleNavierStokesCartesianModel2d,
    mesh: MeshArtifactReference,
    scales: IncompressibleFlowScaleProfile2d,
    time_step: DynQuantity,
    nonlinear: NonlinearSolvePlan,
    solver: SolverPlan,
) -> Result<TransientFieldwiseRealizationPlan, Diagnostic> {
    let velocity = velocity_id(model);
    let pressure = pressure_id(model);
    let spatial = FieldwiseSpatialDiscretization::new(
        domain_id(model),
        PositivePhysicalScale::new(scales.length()).map_err(realization_error)?,
        [
            FieldSpaceBinding::new(velocity, Space::simplex_p1_bubble()),
            FieldSpaceBinding::new(pressure, Space::continuous_lagrange(NonZeroU16::MIN)),
        ],
        [AlgebraicConstraint::ZeroIntegral { field: pressure }],
        Discretization::new(
            DiscretizationMethod::ContinuousGalerkin,
            MeshPolicy::ImportedSimplicial { artifact: mesh },
            QuadraturePolicy::TriangleDuffyGaussLegendre {
                points_per_axis: NonZeroUsize::new(DUFFY_POINTS_PER_AXIS)
                    .expect("five is non-zero"),
            },
        ),
    )
    .map_err(realization_error)?;
    let scaling = SymmetricCongruenceScaling::new(
        [
            AlgebraicBlockScale::new(
                AlgebraicBlock::Field(velocity),
                PositivePhysicalScale::new(scales.velocity()).map_err(realization_error)?,
            ),
            AlgebraicBlockScale::new(
                AlgebraicBlock::Field(pressure),
                PositivePhysicalScale::new(scales.pressure()).map_err(realization_error)?,
            ),
            AlgebraicBlockScale::new(
                AlgebraicBlock::ConstraintMultiplier { field: pressure },
                PositivePhysicalScale::new(scales.gauge()).map_err(realization_error)?,
            ),
        ],
        PositivePhysicalScale::new(scales.weak_functional()).map_err(realization_error)?,
    )
    .map_err(realization_error)?;
    let fieldwise = FieldwiseRealizationPlan::new(
        spatial,
        scaling,
        LinearOperatorProperties::General,
        solver,
        Target::HostCpu {
            threads: NonZeroUsize::MIN,
        },
        ExecutionSchedule::Offline,
    )
    .map_err(realization_error)?;
    TransientFieldwiseRealizationPlan::new(
        fieldwise,
        BackwardEulerRelationStep::new(momentum_id(model), velocity, time_step)
            .map_err(realization_error)?,
        EnergySkewConvection::new(momentum_id(model), velocity),
        nonlinear,
    )
    .map_err(realization_error)
}

/// Coherent-SI states plus dimensionless nonlinear acceptance evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedTransientNavierStokesTrajectory2d {
    model: TransientIncompressibleNavierStokesCartesianModel2d,
    realization: ResolvedTransientFieldwiseRealization,
    realization_graph: PortableRealizationGraph,
    solver_backend: eqiora_solver::BackendId,
    mesh_artifact: MeshArtifactReference,
    scales: IncompressibleFlowScaleProfile2d,
    states: Vec<ResolvedTransientNavierStokesState2d>,
    steps: Vec<SimplicialMiniNavierStokesStepEvidence2d>,
    validated_block_materializations: usize,
}

impl ResolvedTransientNavierStokesTrajectory2d {
    /// Exact package-neutral model roles used by every step.
    #[must_use]
    pub const fn model(&self) -> &TransientIncompressibleNavierStokesCartesianModel2d {
        &self.model
    }

    /// Exact resolved Realization and its independent revision provenance.
    #[must_use]
    pub const fn realization(&self) -> &ResolvedTransientFieldwiseRealization {
        &self.realization
    }

    /// Common portable DAG that drove numerical admission for every step.
    #[must_use]
    pub const fn realization_graph(&self) -> &PortableRealizationGraph {
        &self.realization_graph
    }

    /// Exact solver adapter that executed every accepted Newton linearization.
    #[must_use]
    pub const fn solver_backend(&self) -> eqiora_solver::BackendId {
        self.solver_backend
    }

    /// Fixed imported mesh content identity.
    #[must_use]
    pub const fn mesh_artifact(&self) -> MeshArtifactReference {
        self.mesh_artifact
    }

    /// Coherent-SI initial state followed by accepted states.
    #[must_use]
    pub fn states(&self) -> &[ResolvedTransientNavierStokesState2d] {
        &self.states
    }

    /// Dimensionless true-residual and conservation evidence per step.
    #[must_use]
    pub fn steps(&self) -> &[SimplicialMiniNavierStokesStepEvidence2d] {
        &self.steps
    }

    /// Number of CSR assemblies bound to and revalidated against one exact
    /// private block-system identity during this run.
    #[must_use]
    pub const fn validated_block_materialization_count(&self) -> usize {
        self.validated_block_materializations
    }

    /// Exact scaling used for numerical coordinates and reconstruction.
    #[must_use]
    pub const fn scales(&self) -> IncompressibleFlowScaleProfile2d {
        self.scales
    }
}

/// Advance one exact canonical model through the reference assembler.
///
/// # Errors
/// Rejects semantic, boundary, mesh, initial-state, scaling, nonlinear, JVP,
/// Krylov, or reconstruction drift before returning accepted evidence.
#[allow(clippy::too_many_arguments)]
pub fn advance_resolved_transient_navier_stokes_mini_2d(
    program: &KernelProgram,
    resolved: &ResolvedTransientFieldwiseRealization,
    mesh: &SimplicialMeshEnvelopeV1,
    initial: TransientNavierStokesInitialState2d,
    run: TransientNavierStokesRun2d,
    solver: &dyn LinearSolverBackend,
) -> Result<ResolvedTransientNavierStokesTrajectory2d, Diagnostic> {
    advance_resolved_transient_navier_stokes_mini_2d_with_assembly(
        program,
        resolved,
        mesh,
        initial,
        run,
        &REFERENCE_ASSEMBLY_BACKEND,
        solver,
    )
}

/// Advance through an explicit assembly adapter and common linear backend.
///
/// The accepted envelope keeps content identity and reconstructed mesh data
/// indivisible at this public boundary.
///
/// # Errors
/// Preserves all canonical, fixed-mesh, block-system, assembly, nonlinear,
/// linearization, solver, and physical reconstruction diagnostics.
#[allow(clippy::too_many_arguments)]
pub fn advance_resolved_transient_navier_stokes_mini_2d_with_assembly(
    program: &KernelProgram,
    resolved: &ResolvedTransientFieldwiseRealization,
    mesh: &SimplicialMeshEnvelopeV1,
    initial: TransientNavierStokesInitialState2d,
    run: TransientNavierStokesRun2d,
    assembly: &dyn AssemblyBackend,
    solver: &dyn LinearSolverBackend,
) -> Result<ResolvedTransientNavierStokesTrajectory2d, Diagnostic> {
    let mesh_artifact = mesh.artifact_reference()?;
    let mesh_data = mesh.mesh();
    if program.model() != resolved.model()
        || program.revision().0 != resolved.semantic_revision().get()
    {
        return Err(invalid_realization(
            "resolved transient realization does not reference this exact Semantic Model revision",
        ));
    }
    let model = lower_transient_incompressible_navier_stokes_cartesian_2d(program)?;
    require_complete_zero_trace(&model)?;
    let realization_graph = resolved.portable_graph()?;
    let (scales, numerical_plan) =
        require_exact_transient_plan(&model, resolved, &realization_graph, mesh_artifact)?;
    if initial.mesh_artifact != mesh_artifact
        || initial.velocity_field != velocity_id(&model)
        || initial.pressure_field != pressure_id(&model)
    {
        return Err(invalid_realization(
            "transient initial state identity differs from the resolved Model or mesh revision",
        ));
    }
    if initial.velocity.mesh() != mesh_data || initial.pressure.mesh() != mesh_data {
        return Err(invalid_realization(
            "transient Navier--Stokes initial fields are stale for the selected mesh artifact",
        ));
    }
    let normalized = normalize_cartesian_mesh(
        model.bounds(),
        mesh_data,
        scales.length_value(),
        "Navier--Stokes",
    )?;
    let numerical_initial = normalize_state(&initial, &normalized.mesh, scales)?;
    let boundary = SimplicialMiniStokesBoundary2d::all_essential(&normalized.mesh)
        .map_err(|error| invalid_realization(error.message()))?;
    let block_system = transient_block_system(
        program,
        &model,
        mesh_artifact,
        &normalized.mesh,
        &boundary,
        resolved,
        scales,
    )?;
    let checked_assembly = block_system.checked_backend(assembly);
    let lower = [model.bounds()[0][0], model.bounds()[1][0]];
    let length = scales.length_value();
    let pressure = scales.pressure_value();
    let body_force = |coordinate_hat: [f64; DIMENSION]| {
        let coordinate = [
            lower[0] + length * coordinate_hat[0],
            lower[1] + length * coordinate_hat[1],
        ];
        let force = model.conservative_body_force(&coordinate)?;
        Ok([length * force[0] / pressure, length * force[1] / pressure])
    };
    let zero_velocity = |_| Ok([0.0; DIMENSION]);
    let numerical = advance_simplicial_mini_navier_stokes_2d_with_assembly(
        &normalized.mesh,
        &boundary,
        &zero_velocity,
        &body_force,
        numerical_initial,
        run.step_count,
        numerical_plan,
        &triangle_duffy_gauss_legendre(DUFFY_POINTS_PER_AXIS)?,
        &simplex_centroid_rule(DIMENSION - 1)?,
        &checked_assembly,
        solver,
    )?;
    let validated_block_materializations = checked_assembly.validated_materialization_count();
    if validated_block_materializations == 0 {
        return Err(invalid_realization(
            "transient execution returned without a validated block materialization",
        ));
    }
    let states = numerical
        .states()
        .iter()
        .map(|state| reconstruct_state(state, mesh_data, &model, scales))
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    Ok(ResolvedTransientNavierStokesTrajectory2d {
        model,
        realization: resolved.clone(),
        realization_graph,
        solver_backend: solver.id(),
        mesh_artifact,
        scales,
        states,
        steps: numerical.steps().to_vec(),
        validated_block_materializations,
    })
}

fn normalize_state(
    state: &TransientNavierStokesInitialState2d,
    mesh: &SimplicialMesh,
    scales: IncompressibleFlowScaleProfile2d,
) -> Result<SimplicialMiniNavierStokesState2d, Diagnostic> {
    let velocity = SimplicialMiniVelocityField2d::new(
        mesh.clone(),
        state
            .velocity
            .vertex_values()
            .iter()
            .map(|value| {
                [
                    value[0] / scales.velocity_value(),
                    value[1] / scales.velocity_value(),
                ]
            })
            .collect(),
        state
            .velocity
            .cell_bubble_values()
            .iter()
            .map(|value| {
                [
                    value[0] / scales.velocity_value(),
                    value[1] / scales.velocity_value(),
                ]
            })
            .collect(),
    )?;
    let pressure = SimplicialP1Field::new(
        mesh.clone(),
        state
            .pressure
            .vertex_values()
            .iter()
            .map(|value| value / scales.pressure_value())
            .collect(),
    )?;
    let pressure_reference = match state.pressure_reference {
        super::SteadyStokesPressureReference2d::ZeroIntegral { multiplier } => {
            SimplicialMiniStokesPressureReference2d::ZeroIntegral {
                multiplier: multiplier / scales.gauge_value(),
            }
        }
        super::SteadyStokesPressureReference2d::BoundaryTraction => {
            return Err(invalid_realization(
                "complete zero-velocity boundary requires a zero-integral initial pressure policy",
            ));
        }
    };
    SimplicialMiniNavierStokesState2d::new(
        state.time.value() * scales.velocity_value() / scales.length_value(),
        velocity,
        pressure,
        pressure_reference,
    )
    .map_err(|error| invalid_realization(error.message()))
}

fn reconstruct_state(
    state: &SimplicialMiniNavierStokesState2d,
    mesh: &SimplicialMesh,
    model: &TransientIncompressibleNavierStokesCartesianModel2d,
    scales: IncompressibleFlowScaleProfile2d,
) -> Result<ResolvedTransientNavierStokesState2d, Diagnostic> {
    let velocity = SimplicialMiniVelocityField2d::new(
        mesh.clone(),
        state
            .velocity()
            .vertex_values()
            .iter()
            .map(|value| {
                [
                    value[0] * scales.velocity_value(),
                    value[1] * scales.velocity_value(),
                ]
            })
            .collect(),
        state
            .velocity()
            .cell_bubble_values()
            .iter()
            .map(|value| {
                [
                    value[0] * scales.velocity_value(),
                    value[1] * scales.velocity_value(),
                ]
            })
            .collect(),
    )?;
    let pressure = SimplicialP1Field::new(
        mesh.clone(),
        state
            .pressure()
            .vertex_values()
            .iter()
            .map(|value| value * scales.pressure_value())
            .collect(),
    )?;
    let pressure_reference = match state.pressure_reference() {
        SimplicialMiniStokesPressureReference2d::ZeroIntegral { multiplier } => {
            super::SteadyStokesPressureReference2d::ZeroIntegral {
                multiplier: multiplier * scales.gauge_value(),
            }
        }
        SimplicialMiniStokesPressureReference2d::BoundaryTraction => {
            return Err(invalid_realization(
                "reference transient reconstruction lost its zero-integral pressure policy",
            ));
        }
    };
    let time = DynQuantity::new(
        state.time() * scales.length_value() / scales.velocity_value(),
        time_dimension(),
    );
    if !time.value().is_finite() {
        return Err(invalid_realization(
            "transient physical reconstruction produced non-finite time",
        ));
    }
    Ok(ResolvedTransientNavierStokesState2d {
        time,
        velocity_field: velocity_id(model),
        pressure_field: pressure_id(model),
        velocity,
        pressure,
        pressure_reference,
    })
}

pub(super) fn require_complete_zero_trace(
    model: &TransientIncompressibleNavierStokesCartesianModel2d,
) -> Result<(), Diagnostic> {
    let entries = model.boundary_inventory().entries().collect::<Vec<_>>();
    if entries.len() != 2 * DIMENSION
        || entries
            .iter()
            .any(|(_, entry)| entry.disposition() != crate::PhysicalBoundaryDisposition::TraceZero)
    {
        return Err(invalid_realization(
            "bounded transient Navier--Stokes realization requires complete homogeneous velocity trace",
        ));
    }
    Ok(())
}

fn require_exact_transient_plan(
    model: &TransientIncompressibleNavierStokesCartesianModel2d,
    resolved: &ResolvedTransientFieldwiseRealization,
    graph: &PortableRealizationGraph,
    mesh_artifact: MeshArtifactReference,
) -> Result<
    (
        IncompressibleFlowScaleProfile2d,
        crate::MiniNavierStokesStepPlan2d,
    ),
    Diagnostic,
> {
    let expected_requirements = transient_navier_stokes_fieldwise_requirements_2d(model);
    if resolved.requirements() != &expected_requirements {
        return Err(invalid_realization(
            "transient Realization requirements differ from the exact flow lowerer inventory",
        ));
    }
    let velocity = velocity_id(model);
    let pressure = pressure_id(model);
    if graph.lineage().model() != resolved.model()
        || graph.lineage().semantic_revision() != resolved.semantic_revision()
        || graph.domains().len() != 1
        || graph.domains()[0].domain() != domain_id(model)
        || graph.domains()[0].discretization()
            != Discretization::new(
                DiscretizationMethod::ContinuousGalerkin,
                MeshPolicy::ImportedSimplicial {
                    artifact: mesh_artifact,
                },
                QuadraturePolicy::TriangleDuffyGaussLegendre {
                    points_per_axis: NonZeroUsize::new(DUFFY_POINTS_PER_AXIS)
                        .expect("five is non-zero"),
                },
            )
    {
        return Err(invalid_realization(
            "transient portable graph lineage or Domain discretization differs from the exact flow lowerer",
        ));
    }
    let exact_space = |field, expected| {
        graph
            .fields()
            .iter()
            .find(|binding| binding.field() == field)
            .is_some_and(|binding| binding.space() == expected)
    };
    let [system] = graph.systems() else {
        return Err(invalid_realization(
            "transient portable graph must contain one monolithic algebraic system",
        ));
    };
    let algebraic_blocks = system
        .blocks()
        .iter()
        .map(|block| match *block {
            SystemBlock::Field(id) => graph
                .field(id)
                .map(|field| AlgebraicBlock::Field(field.field()))
                .ok_or_else(|| invalid_realization("portable system has an absent Field block")),
            SystemBlock::ConstraintMultiplier(constraint) => {
                Ok(AlgebraicBlock::ConstraintMultiplier {
                    field: constraint.field(),
                })
            }
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let mut expected_blocks = vec![
        AlgebraicBlock::Field(velocity),
        AlgebraicBlock::Field(pressure),
        AlgebraicBlock::ConstraintMultiplier { field: pressure },
    ];
    expected_blocks.sort_by_key(|block| match *block {
        AlgebraicBlock::Field(field) => (0_u8, field.ulid()),
        AlgebraicBlock::ConstraintMultiplier { field } => (1_u8, field.ulid()),
    });
    if !exact_space(velocity, Space::simplex_p1_bubble())
        || !exact_space(pressure, Space::continuous_lagrange(NonZeroU16::MIN))
        || algebraic_blocks != expected_blocks
        || system.operator_properties() != LinearOperatorProperties::General
        || system.scalar_type() != ScalarType::F64
        || system.partition() != VectorLayoutKind::Replicated
    {
        return Err(invalid_realization(
            "transient flow requires exact MINI/P1, zero-integral, replicated-f64 monolithic Realization graph choices",
        ));
    }
    let SolveRoot::Nonlinear(nonlinear_id) = graph.root() else {
        return Err(invalid_realization(
            "transient flow requires a nonlinear portable solve root",
        ));
    };
    let nonlinear_node = graph
        .nonlinear_solve(nonlinear_id)
        .ok_or_else(|| invalid_realization("transient nonlinear root is absent"))?;
    let linear_node = graph
        .linear_solve(nonlinear_node.linearization())
        .ok_or_else(|| invalid_realization("transient linearization solve is absent"))?;
    let placement = graph
        .placement(linear_node.placement())
        .ok_or_else(|| invalid_realization("transient placement requirement is absent"))?;
    if placement
        != (PlacementRequirementNode::HostWorkers {
            workers_per_partition: NonZeroUsize::MIN,
        })
        || linear_node.schedule() != ExecutionSchedule::Offline
    {
        return Err(invalid_realization(
            "transient reference requires one portable host worker and offline scheduling",
        ));
    }
    let velocity_representation = system
        .blocks()
        .iter()
        .find_map(|block| match *block {
            SystemBlock::Field(id)
                if graph
                    .field(id)
                    .is_some_and(|field| field.field() == velocity) =>
            {
                Some(id)
            }
            _ => None,
        })
        .ok_or_else(|| invalid_realization("portable graph has no velocity system block"))?;
    if graph.transformations()
        != [
            TransformationNode::BackwardEulerDerivative {
                relation: momentum_id(model),
                state: velocity_representation,
                duration: resolved.plan().time_step().duration(),
            },
            TransformationNode::EnergySkewConvection {
                relation: momentum_id(model),
                velocity: velocity_representation,
            },
        ]
    {
        return Err(invalid_realization(
            "transient portable graph must bind Backward Euler and energy-skew transformations to the exact momentum Relation and velocity Field",
        ));
    }
    let scale_for = |block| {
        system
            .congruence_scaling()
            .ok_or_else(|| {
                invalid_realization("transient portable graph requires congruence scaling")
            })?
            .block_scales()
            .iter()
            .find(|entry| entry.block() == block)
            .map(|entry| entry.scale().quantity())
            .ok_or_else(|| invalid_realization("transient plan is missing an exact block scale"))
    };
    let scales = IncompressibleFlowScaleProfile2d::new(
        match graph.domains()[0].coordinates() {
            CoordinateTreatment::Scaled(scale) => scale.quantity(),
            CoordinateTreatment::Physical => {
                return Err(invalid_realization(
                    "transient coherent-SI graph requires an explicit coordinate length scale",
                ));
            }
        },
        scale_for(AlgebraicBlock::Field(velocity))?,
        scale_for(AlgebraicBlock::Field(pressure))?,
    )?;
    if scale_for(AlgebraicBlock::ConstraintMultiplier { field: pressure })? != scales.gauge()
        || system
            .congruence_scaling()
            .ok_or_else(|| {
                invalid_realization("transient portable graph requires congruence scaling")
            })?
            .weak_functional_scale()
            .quantity()
            != scales.weak_functional()
    {
        return Err(invalid_realization(
            "transient plan gauge or weak-functional scale is not derived exactly from L/U/P",
        ));
    }
    let nonlinear = nonlinear_node.plan();
    let density = model.mass_density() * scales.velocity_value().powi(2) / scales.pressure_value();
    let viscosity = model.dynamic_viscosity() * scales.velocity_value()
        / (scales.pressure_value() * scales.length_value());
    let time_step = resolved.plan().time_step().duration().value() * scales.velocity_value()
        / scales.length_value();
    let numerical = crate::MiniNavierStokesStepPlan2d::new(
        density,
        viscosity,
        time_step,
        nonlinear.relative_tolerance(),
        nonlinear.absolute_tolerance(),
        nonlinear.maximum_iterations(),
        nonlinear.maximum_line_search_steps(),
        linear_node.plan(),
        resolved.plan().fieldwise().target(),
    )
    .map_err(|error| invalid_realization(error.message()))?;
    Ok((scales, numerical))
}

fn transient_block_system(
    program: &KernelProgram,
    model: &TransientIncompressibleNavierStokesCartesianModel2d,
    mesh_artifact: MeshArtifactReference,
    mesh: &SimplicialMesh,
    boundary: &SimplicialMiniStokesBoundary2d,
    resolved: &ResolvedTransientFieldwiseRealization,
    scales: IncompressibleFlowScaleProfile2d,
) -> Result<DiscreteBlockSystem, Diagnostic> {
    let domain = downcast::<kinds::Domain>(model.domain(), "Domain")?;
    let velocity = downcast::<kinds::Field>(model.velocity(), "velocity Field")?;
    let pressure = downcast::<kinds::Field>(model.pressure(), "pressure Field")?;
    let force = downcast::<kinds::Field>(model.force_potential(), "force-potential Field")?;
    let momentum = downcast::<kinds::Relation>(model.momentum_relation(), "momentum Relation")?;
    let incompressibility = downcast::<kinds::Relation>(
        model.incompressibility_relation(),
        "incompressibility Relation",
    )?;
    let force_definition = downcast::<kinds::Relation>(
        model.force_potential_definition(),
        "force-potential definition Relation",
    )?;
    let fields = vec![
        FieldBlock::discrete(
            domain,
            velocity,
            Space::simplex_p1_bubble(),
            ValueShape::new([2_u32]).expect("2D velocity shape is representable"),
            VELOCITY,
            ValueFrame::SpatialCartesian,
            scales.velocity(),
            FieldBlockRole::Algebraic,
        )?,
        FieldBlock::discrete(
            domain,
            pressure,
            Space::continuous_lagrange(NonZeroU16::MIN),
            ValueShape::scalar(),
            PRESSURE,
            ValueFrame::Invariant,
            scales.pressure(),
            FieldBlockRole::Algebraic,
        )?,
        FieldBlock::coefficient(
            domain,
            force,
            ValueShape::scalar(),
            PRESSURE,
            ValueFrame::Invariant,
        ),
    ];
    let constraint = AlgebraicConstraint::ZeroIntegral { field: pressure };
    let auxiliaries = vec![AuxiliaryBlock::new(constraint, scales.gauge())?];
    let mut relations = vec![
        RelationBlock::new(
            force_definition,
            BlockSupport::Volume(domain),
            RelationDisposition::CoefficientDefinition { field: force },
        ),
        RelationBlock::new(
            momentum,
            BlockSupport::Volume(domain),
            RelationDisposition::Residual {
                tested: AlgebraicBlock::Field(velocity),
            },
        ),
        RelationBlock::new(
            incompressibility,
            BlockSupport::Volume(domain),
            RelationDisposition::Residual {
                tested: AlgebraicBlock::Field(pressure),
            },
        ),
    ];
    let mut essential_relations = Vec::new();
    for binding in model.boundary_relations() {
        let relation = downcast::<kinds::Relation>(binding.relation(), "boundary Relation")?;
        let support = downcast::<kinds::Domain>(binding.boundary(), "boundary Domain")?;
        essential_relations.push(relation);
        relations.push(RelationBlock::new(
            relation,
            BlockSupport::Boundary(support),
            RelationDisposition::BoundaryCondition {
                field: velocity,
                treatment: crate::discrete_block::BoundaryTreatment::EssentialElimination,
            },
        ));
    }
    let residuals = vec![
        ResidualBlock::new(
            AlgebraicBlock::Field(velocity),
            BlockSupport::Volume(domain),
            [ResidualOrigin::Relation(momentum)],
        )?,
        ResidualBlock::new(
            AlgebraicBlock::Field(pressure),
            BlockSupport::Volume(domain),
            [ResidualOrigin::Relation(incompressibility)],
        )?,
        ResidualBlock::new(
            AlgebraicBlock::ConstraintMultiplier { field: pressure },
            BlockSupport::Volume(domain),
            [ResidualOrigin::AlgebraicConstraint(constraint)],
        )?,
    ];
    let transformations = vec![
        BlockTransformation::EssentialElimination {
            field: velocity,
            boundary_relations: essential_relations.clone(),
        },
        BlockTransformation::BackwardEulerDerivative {
            relation: momentum,
            state: velocity,
            duration: resolved.plan().time_step().duration(),
        },
        BlockTransformation::EnergySkewConvection {
            relation: momentum,
            velocity,
        },
    ];
    let closures = vec![
        AlgebraicClosure::EssentialBoundary {
            field: velocity,
            relations: essential_relations,
        },
        AlgebraicClosure::ZeroIntegral { field: pressure },
    ];
    let cell_count = mesh
        .entity_count(DIMENSION)
        .expect("accepted 2D mesh owns cells");
    let parameters = parameter_inventory([
        model.mass_density_expression().parameter_fields(),
        model.dynamic_viscosity_expression().parameter_fields(),
        model.force_potential_expression().parameter_fields(),
    ]);
    let contributions = vec![
        ContributionBatch::new(
            [BlockSupport::Volume(domain)],
            0..cell_count,
            [0, 1],
            [
                ResidualOrigin::Relation(force_definition),
                ResidualOrigin::Relation(momentum),
                ResidualOrigin::Relation(incompressibility),
            ],
            parameters,
            [
                AlgebraicBlock::Field(velocity),
                AlgebraicBlock::Field(pressure),
            ],
            [
                AlgebraicBlock::Field(velocity),
                AlgebraicBlock::Field(pressure),
            ],
            [
                ContributionTerm::Mass,
                ContributionTerm::Advection,
                ContributionTerm::Stiffness,
                ContributionTerm::MixedConstraint,
                ContributionTerm::Load,
            ],
        )?,
        ContributionBatch::new(
            [BlockSupport::Volume(domain)],
            cell_count..2 * cell_count,
            [0, 1],
            [ResidualOrigin::AlgebraicConstraint(constraint)],
            [],
            [
                AlgebraicBlock::Field(pressure),
                AlgebraicBlock::ConstraintMultiplier { field: pressure },
            ],
            [
                AlgebraicBlock::Field(pressure),
                AlgebraicBlock::ConstraintMultiplier { field: pressure },
            ],
            [ContributionTerm::AlgebraicConstraint],
        )?,
    ];
    let expected_packets = 2 * cell_count;
    if boundary.facets().is_empty() {
        return Err(invalid_realization(
            "transient block system requires a nonempty boundary closure",
        ));
    }
    DiscreteBlockSystem::new(
        DiscreteBlockContext::new(
            program.model(),
            eqiora_realization::SemanticRevision::new(program.revision().0),
            BlockRealizationIdentity::Explicit(resolved.realization_revision()),
            Some(mesh_artifact),
        ),
        fields,
        auxiliaries,
        relations,
        residuals,
        transformations,
        closures,
        contributions,
        expected_packets,
        2,
        0,
        LinearOperatorProperties::General,
    )
}

fn parameter_inventory<'a>(
    fields: impl IntoIterator<Item = &'a [Id<kinds::Parameter>]>,
) -> Vec<Id<kinds::Parameter>> {
    let mut result = fields.into_iter().flatten().copied().collect::<Vec<_>>();
    result.sort_by_key(Id::ulid);
    result.dedup();
    result
}

fn downcast<E: eqiora_core::Entity>(id: RawId, label: &str) -> Result<Id<E>, Diagnostic> {
    id.downcast::<E>().ok_or_else(|| {
        invalid_realization(format!(
            "transient Navier--Stokes block inventory expected {label} identity, received {id}"
        ))
    })
}

fn domain_id(model: &TransientIncompressibleNavierStokesCartesianModel2d) -> Id<kinds::Domain> {
    model
        .domain()
        .downcast()
        .expect("transient lowerer retains a Domain identity")
}

fn velocity_id(model: &TransientIncompressibleNavierStokesCartesianModel2d) -> Id<kinds::Field> {
    model
        .velocity()
        .downcast()
        .expect("transient lowerer retains a velocity Field identity")
}

fn pressure_id(model: &TransientIncompressibleNavierStokesCartesianModel2d) -> Id<kinds::Field> {
    model
        .pressure()
        .downcast()
        .expect("transient lowerer retains a pressure Field identity")
}

fn momentum_id(model: &TransientIncompressibleNavierStokesCartesianModel2d) -> Id<kinds::Relation> {
    model
        .momentum_relation()
        .downcast()
        .expect("transient lowerer retains a momentum Relation identity")
}

const fn time_dimension() -> DimExponents {
    DimExponents {
        time: 1,
        ..DimExponents::DIMENSIONLESS
    }
}

fn realization_error(error: Diagnostic) -> Diagnostic {
    invalid_realization(error.message())
}

fn invalid_realization(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}
