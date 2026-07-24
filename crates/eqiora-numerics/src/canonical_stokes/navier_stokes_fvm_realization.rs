use std::num::NonZeroUsize;

use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, DimExponents, DynQuantity, Id};
use eqiora_realization::{
    AlgebraicBlock, AlgebraicBlockScale, AlgebraicConstraint, BackwardEulerRelationStep,
    CartesianCentralNewtonianTraction, CoordinateTreatment, Discretization, DiscretizationMethod,
    ExecutionSchedule, FieldSpaceBinding, FieldwiseRealizationPlan,
    FieldwiseRealizationRequirements, FieldwiseSpatialDiscretization,
    ImplicitCenteredMomentumConvection, MeshPolicy, MomentumWeightedLinearExactCoupling,
    NonlinearSolvePlan, PortableRealizationGraph, PositiveMomentumDiagonal, PositivePhysicalScale,
    QuadraturePolicy, RealizationRequirements,
    ResolvedTransientCellCenteredIncompressibleFlowRealization, ScalarType, Space,
    SymmetricCongruenceScaling, SystemBlock, Target, TransformationNode,
    TransientCellCenteredIncompressibleFlowRealizationPlan,
    TransientCellCenteredIncompressibleFlowRealizationRequirements, TransientFaceFluxHistory,
    VectorLayoutKind,
};
use eqiora_sem::KernelProgram;
use eqiora_solver::{
    BackendId, LinearOperatorProperties, LinearSolverBackend, SolveReport, SolverPlan,
};

use super::{
    IncompressibleFlowScaleProfile2d, TransientIncompressibleNavierStokesCartesianModel2d,
    TransientNavierStokesRun2d,
};
use crate::CartesianMesh;
use crate::cartesian_fvm_geometry::{CartesianFacetAdjacency2d, cartesian_fvm_geometry_2d};
use crate::cartesian_incompressible::{
    CartesianIncompressibleOperator2d, CellCenteredPressureField2d, CellCenteredVelocityField2d,
    CollocatedNewtonEvidence2d, CollocatedPoint2d, solve_collocated_step_2d,
};

const DIMENSION: usize = 2;
const TIME: DimExponents = DimExponents {
    time: 1,
    ..DimExponents::DIMENSIONLESS
};

/// Coherent-SI cell-centered initial state bound to exact canonical Fields.
#[derive(Debug, Clone, PartialEq)]
pub struct CellCenteredNavierStokesInitialState2d {
    time: DynQuantity,
    velocity_field: Id<kinds::Field>,
    pressure_field: Id<kinds::Field>,
    velocity: CellCenteredVelocityField2d,
    pressure: CellCenteredPressureField2d,
    gauge_multiplier: f64,
}

impl CellCenteredNavierStokesInitialState2d {
    /// Bind one physical cell state to the exact lowered flow identities.
    ///
    /// # Errors
    /// Returns `EQ0807` for invalid time, different velocity/pressure meshes,
    /// non-finite gauge evidence, or a non-zero pressure integral.
    pub fn new(
        model: &TransientIncompressibleNavierStokesCartesianModel2d,
        time: DynQuantity,
        velocity: CellCenteredVelocityField2d,
        pressure: CellCenteredPressureField2d,
        gauge_multiplier: f64,
    ) -> Result<Self, Diagnostic> {
        if time.dim() != TIME || !time.value().is_finite() || time.value() < 0.0 {
            return Err(invalid_realization(
                "cell-centered transient initial time must be finite, non-negative, and physical time",
            ));
        }
        if velocity.mesh() != pressure.mesh() || !gauge_multiplier.is_finite() {
            return Err(invalid_realization(
                "cell-centered initial velocity and pressure must share one mesh and finite gauge evidence",
            ));
        }
        let pressure_integral = pressure.volume_integral()?;
        let pressure_scale = pressure
            .values()
            .iter()
            .fold(0.0_f64, |scale, value| scale.max(value.abs()));
        let volume = pressure
            .mesh()
            .axis_bounds(0)
            .zip(pressure.mesh().axis_bounds(1))
            .map(|(x, y)| (x[1] - x[0]) * (y[1] - y[0]))
            .ok_or_else(|| {
                invalid_realization("cell-centered initial mesh is not two-dimensional")
            })?;
        let tolerance = 128.0 * f64::EPSILON * volume * pressure_scale.max(1.0);
        if pressure_integral.abs() > tolerance {
            return Err(invalid_realization(format!(
                "cell-centered initial pressure integral {pressure_integral:e} exceeds zero-integral tolerance {tolerance:e}"
            )));
        }
        Ok(Self {
            time,
            velocity_field: velocity_id(model),
            pressure_field: pressure_id(model),
            velocity,
            pressure,
            gauge_multiplier,
        })
    }
}

/// One accepted coherent-SI collocated velocity--pressure state.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedCellCenteredNavierStokesState2d {
    time: DynQuantity,
    velocity_field: Id<kinds::Field>,
    pressure_field: Id<kinds::Field>,
    velocity: CellCenteredVelocityField2d,
    pressure: CellCenteredPressureField2d,
    gauge_multiplier: f64,
}

impl ResolvedCellCenteredNavierStokesState2d {
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

    /// Physical cell-centered velocity.
    #[must_use]
    pub const fn velocity(&self) -> &CellCenteredVelocityField2d {
        &self.velocity
    }

    /// Physical cell-centered zero-integral pressure.
    #[must_use]
    pub const fn pressure(&self) -> &CellCenteredPressureField2d {
        &self.pressure
    }

    /// Physical pressure-constraint multiplier.
    #[must_use]
    pub const fn gauge_multiplier(&self) -> f64 {
        self.gauge_multiplier
    }
}

/// Inspectable acceptance evidence for one monolithic collocated step.
#[derive(Debug, Clone, PartialEq)]
pub struct CellCenteredNavierStokesStepEvidence2d {
    iterations: usize,
    initial_residual_norm: f64,
    initial_momentum_norm: f64,
    initial_continuity_norm: f64,
    residual_target: f64,
    momentum_residual_target: f64,
    continuity_residual_target: f64,
    gauge_residual_target: f64,
    momentum_residual_norm: f64,
    continuity_residual_norm: f64,
    gauge_residual: f64,
    maximum_momentum_replay_defect: f64,
    maximum_continuity_replay_defect: f64,
    maximum_centered_jvp_defect: f64,
    maximum_face_cancellation_defect: f64,
    maximum_flux_reuse_defect: f64,
    global_mass_defect: f64,
    replay_tolerance: f64,
    maximum_affine_pressure_correction: f64,
    minimum_checkerboard_correction_norm: f64,
    linear_solves: Vec<SolveReport>,
}

impl CellCenteredNavierStokesStepEvidence2d {
    /// Accepted Newton update count.
    #[must_use]
    pub const fn iterations(&self) -> usize {
        self.iterations
    }
    /// Final independent momentum residual norm.
    #[must_use]
    pub const fn momentum_residual_norm(&self) -> f64 {
        self.momentum_residual_norm
    }
    /// Momentum-block acceptance target derived only from its initial norm.
    #[must_use]
    pub const fn momentum_residual_target(&self) -> f64 {
        self.momentum_residual_target
    }
    /// Final independent continuity residual norm.
    #[must_use]
    pub const fn continuity_residual_norm(&self) -> f64 {
        self.continuity_residual_norm
    }
    /// Physical mass-block acceptance target derived only from its initial norm.
    #[must_use]
    pub const fn continuity_residual_target(&self) -> f64 {
        self.continuity_residual_target
    }
    /// Final zero-integral pressure constraint residual.
    #[must_use]
    pub const fn gauge_residual(&self) -> f64 {
        self.gauge_residual
    }
    /// Pressure-gauge acceptance target derived only from its initial residual.
    #[must_use]
    pub const fn gauge_residual_target(&self) -> f64 {
        self.gauge_residual_target
    }
    /// Maximum momentum-entry defect from retained-face independent replay.
    #[must_use]
    pub const fn maximum_momentum_replay_defect(&self) -> f64 {
        self.maximum_momentum_replay_defect
    }
    /// Maximum physical-continuity entry defect from retained-face replay.
    #[must_use]
    pub const fn maximum_continuity_replay_defect(&self) -> f64 {
        self.maximum_continuity_replay_defect
    }
    /// Maximum analytic-JVP versus centered-reassembly defect over all columns.
    #[must_use]
    pub const fn maximum_centered_jvp_defect(&self) -> f64 {
        self.maximum_centered_jvp_defect
    }
    /// Largest equal-and-opposite interior face scatter defect.
    #[must_use]
    pub const fn maximum_face_cancellation_defect(&self) -> f64 {
        self.maximum_face_cancellation_defect
    }
    /// Largest mismatch between the sole retained face flux and convection.
    #[must_use]
    pub const fn maximum_flux_reuse_defect(&self) -> f64 {
        self.maximum_flux_reuse_defect
    }
    /// Absolute sum of independently replayed physical cell mass residuals.
    #[must_use]
    pub const fn global_mass_defect(&self) -> f64 {
        self.global_mass_defect
    }
    /// Floating-point tolerance used for retained-face replay acceptance.
    #[must_use]
    pub const fn replay_tolerance(&self) -> f64 {
        self.replay_tolerance
    }
    /// Largest pressure correction produced by constant or affine pressure.
    #[must_use]
    pub const fn maximum_affine_pressure_correction(&self) -> f64 {
        self.maximum_affine_pressure_correction
    }
    /// Smallest correction norm among the registered Cartesian checkerboards.
    #[must_use]
    pub const fn minimum_checkerboard_correction_norm(&self) -> f64 {
        self.minimum_checkerboard_correction_norm
    }
    /// Exact linear solve reports used by accepted Newton updates.
    #[must_use]
    pub fn linear_solves(&self) -> &[SolveReport] {
        &self.linear_solves
    }
    /// Initial complete residual norm.
    #[must_use]
    pub const fn initial_residual_norm(&self) -> f64 {
        self.initial_residual_norm
    }
    /// Initial momentum-block norm before Newton.
    #[must_use]
    pub const fn initial_momentum_norm(&self) -> f64 {
        self.initial_momentum_norm
    }
    /// Continuity norm of the accepted Run-side initial guess before Newton.
    #[must_use]
    pub const fn initial_continuity_norm(&self) -> f64 {
        self.initial_continuity_norm
    }
    /// Nonlinear acceptance target.
    #[must_use]
    pub const fn residual_target(&self) -> f64 {
        self.residual_target
    }
}

/// Physical trajectory and exact two-layer provenance for the collocated path.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedCellCenteredNavierStokesTrajectory2d {
    model: TransientIncompressibleNavierStokesCartesianModel2d,
    realization: ResolvedTransientCellCenteredIncompressibleFlowRealization,
    realization_graph: PortableRealizationGraph,
    solver_backend: BackendId,
    scales: IncompressibleFlowScaleProfile2d,
    states: Vec<ResolvedCellCenteredNavierStokesState2d>,
    steps: Vec<CellCenteredNavierStokesStepEvidence2d>,
}

impl ResolvedCellCenteredNavierStokesTrajectory2d {
    /// Exact package-neutral canonical fluid roles.
    #[must_use]
    pub const fn model(&self) -> &TransientIncompressibleNavierStokesCartesianModel2d {
        &self.model
    }
    /// Exact accepted collocated Realization.
    #[must_use]
    pub const fn realization(&self) -> &ResolvedTransientCellCenteredIncompressibleFlowRealization {
        &self.realization
    }
    /// Portable graph that drove every step.
    #[must_use]
    pub const fn realization_graph(&self) -> &PortableRealizationGraph {
        &self.realization_graph
    }
    /// Exact solver adapter identity.
    #[must_use]
    pub const fn solver_backend(&self) -> BackendId {
        self.solver_backend
    }
    /// Initial state followed by accepted positive-time states.
    #[must_use]
    pub fn states(&self) -> &[ResolvedCellCenteredNavierStokesState2d] {
        &self.states
    }
    /// Per-step residual, coupling, JVP, and solve evidence.
    #[must_use]
    pub fn steps(&self) -> &[CellCenteredNavierStokesStepEvidence2d] {
        &self.steps
    }
    /// Exact coherent-SI normalization profile.
    #[must_use]
    pub const fn scales(&self) -> IncompressibleFlowScaleProfile2d {
        self.scales
    }
}

/// Exact lowerer requirements for the cell-centered incompressible-flow path.
#[must_use]
pub fn transient_navier_stokes_cell_centered_requirements_2d(
    model: &TransientIncompressibleNavierStokesCartesianModel2d,
) -> TransientCellCenteredIncompressibleFlowRealizationRequirements {
    let velocity = velocity_id(model);
    let pressure = pressure_id(model);
    let fieldwise = FieldwiseRealizationRequirements::new(
        domain_id(model),
        [velocity, pressure],
        RealizationRequirements::new(
            NonZeroUsize::new(DIMENSION).expect("two is non-zero"),
            ScalarType::F64,
            VectorLayoutKind::Replicated,
        ),
    )
    .expect("a lowered transient flow model owns two distinct algebraic Fields");
    TransientCellCenteredIncompressibleFlowRealizationRequirements::new(
        fieldwise,
        momentum_id(model),
        incompressibility_id(model),
        velocity,
        pressure,
    )
    .expect("the canonical transient-flow identities are exact and distinct")
}

/// Build the bounded 2D collocated cell-centered reference Realization.
///
/// The plan selects generated Cartesian cells, cell-constant velocity and
/// pressure, a zero-integral pressure representative, backward Euler,
/// implicit centered convection, centered Newtonian traction, and the
/// linearly exact momentum-weighted face-flux coupling from RFC 0072.
/// Repeated step count remains a Run choice.
///
/// # Errors
/// Returns `EQ0807` for invalid scaling, nonlinear, solver, or composed method
/// choices. No alternate coupling or pressure correction is substituted.
pub fn transient_navier_stokes_cell_centered_plan_2d(
    model: &TransientIncompressibleNavierStokesCartesianModel2d,
    cells_per_axis: NonZeroUsize,
    scales: IncompressibleFlowScaleProfile2d,
    time_step: DynQuantity,
    nonlinear: NonlinearSolvePlan,
    solver: SolverPlan,
) -> Result<TransientCellCenteredIncompressibleFlowRealizationPlan, Diagnostic> {
    let velocity = velocity_id(model);
    let pressure = pressure_id(model);
    let momentum = momentum_id(model);
    let spatial = FieldwiseSpatialDiscretization::new(
        domain_id(model),
        PositivePhysicalScale::new(scales.length()).map_err(realization_error)?,
        [
            FieldSpaceBinding::new(velocity, Space::cell_constant()),
            FieldSpaceBinding::new(pressure, Space::cell_constant()),
        ],
        [AlgebraicConstraint::ZeroIntegral { field: pressure }],
        Discretization::new(
            DiscretizationMethod::CellCenteredFiniteVolume,
            MeshPolicy::GeneratedUniform { cells_per_axis },
            QuadraturePolicy::CellCentroid,
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
    TransientCellCenteredIncompressibleFlowRealizationPlan::new(
        fieldwise,
        BackwardEulerRelationStep::new(momentum, velocity, time_step).map_err(realization_error)?,
        ImplicitCenteredMomentumConvection::new(momentum, velocity),
        CartesianCentralNewtonianTraction::new(momentum, velocity, pressure),
        MomentumWeightedLinearExactCoupling::new(
            momentum,
            incompressibility_id(model),
            velocity,
            pressure,
            PositiveMomentumDiagonal::BackwardEulerMassAndLocalNewtonian,
            TransientFaceFluxHistory::Bdf1PreviousAccepted,
        ),
        nonlinear,
    )
    .map_err(realization_error)
}

/// Advance the exact canonical fluid through the bounded collocated FVM path.
///
/// # Errors
/// Rejects Model/revision, boundary, generated-mesh, Field identity, scaling,
/// portable-graph, nonlinear, linear-solver, JVP, checkerboard, or conservation
/// drift. A failed step never mutates the last accepted state.
pub fn advance_resolved_transient_navier_stokes_cell_centered_2d(
    program: &KernelProgram,
    resolved: &ResolvedTransientCellCenteredIncompressibleFlowRealization,
    initial: CellCenteredNavierStokesInitialState2d,
    run: TransientNavierStokesRun2d,
    solver: &dyn LinearSolverBackend,
) -> Result<ResolvedCellCenteredNavierStokesTrajectory2d, Diagnostic> {
    if program.model() != resolved.model()
        || program.revision().0 != resolved.semantic_revision().get()
    {
        return Err(invalid_realization(
            "resolved collocated flow realization does not reference this exact Semantic Model revision",
        ));
    }
    let model = super::lower_transient_incompressible_navier_stokes_cartesian_2d(program)?;
    super::navier_stokes_realization::require_complete_zero_trace(&model)?;
    let graph = resolved.portable_graph()?;
    let scales = require_exact_cell_centered_plan(&model, resolved, &graph)?;
    let cells_per_axis = match resolved
        .plan()
        .fieldwise()
        .spatial()
        .discretization()
        .mesh()
    {
        MeshPolicy::GeneratedUniform { cells_per_axis } => cells_per_axis,
        MeshPolicy::ImportedSimplicial { .. } => {
            return Err(invalid_realization(
                "collocated flow requires generated Cartesian cells",
            ));
        }
    };
    let cell_count = cells_per_axis.get();
    let physical_mesh = CartesianMesh::uniform(model.bounds(), &[cell_count, cell_count])?;
    if initial.velocity_field != velocity_id(&model)
        || initial.pressure_field != pressure_id(&model)
        || initial.velocity.mesh() != &physical_mesh
        || initial.pressure.mesh() != &physical_mesh
    {
        return Err(invalid_realization(
            "collocated initial state differs from the exact Model Fields or generated mesh",
        ));
    }
    let length = scales.length_value();
    let normalized_bounds = model
        .bounds()
        .map(|bounds| [0.0, (bounds[1] - bounds[0]) / length]);
    let normalized_mesh = CartesianMesh::uniform(&normalized_bounds, &[cell_count, cell_count])?;
    let (normalized_cells, normalized_facets) = cartesian_fvm_geometry_2d(&normalized_mesh)?;
    if normalized_facets.is_empty() {
        return Err(invalid_realization(
            "collocated generated mesh must contain physical facets",
        ));
    }
    let body_force = normalized_cells
        .iter()
        .map(|cell| {
            let coordinate = [
                model.bounds()[0][0] + length * cell.center[0],
                model.bounds()[1][0] + length * cell.center[1],
            ];
            model
                .conservative_body_force(&coordinate)
                .map(|force| force.map(|value| length * value / scales.pressure_value()))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let duration = resolved.plan().time_step().duration();
    let dimensionless_duration = duration.value() * scales.velocity_value() / length;
    let density = model.mass_density() * scales.velocity_value().powi(2) / scales.pressure_value();
    let viscosity =
        model.dynamic_viscosity() * scales.velocity_value() / (scales.pressure_value() * length);
    let step_count = run.step_count();
    let mut numerical = CollocatedPoint2d {
        velocity: initial
            .velocity
            .values()
            .iter()
            .map(|value| value.map(|component| component / scales.velocity_value()))
            .collect(),
        pressure: initial
            .pressure
            .values()
            .iter()
            .map(|value| value / scales.pressure_value())
            .collect(),
        gauge_multiplier: initial.gauge_multiplier / scales.gauge_value(),
    };
    let mut previous_face_volume_fluxes = normalized_facets
        .iter()
        .map(|facet| match facet.adjacency {
            CartesianFacetAdjacency2d::Interior { lower, upper, .. } => {
                facet.measure
                    * 0.5
                    * (numerical.velocity[lower][facet.normal_axis]
                        + numerical.velocity[upper][facet.normal_axis])
            }
            CartesianFacetAdjacency2d::Boundary { .. } => 0.0,
        })
        .collect::<Vec<_>>();
    let mut time = initial.time;
    let mut states = vec![ResolvedCellCenteredNavierStokesState2d {
        time,
        velocity_field: initial.velocity_field,
        pressure_field: initial.pressure_field,
        velocity: initial.velocity,
        pressure: initial.pressure,
        gauge_multiplier: initial.gauge_multiplier,
    }];
    let mut steps = Vec::with_capacity(step_count.get());
    for _ in 0..step_count.get() {
        let operator = CartesianIncompressibleOperator2d::new(
            normalized_mesh.clone(),
            density,
            viscosity,
            dimensionless_duration,
            numerical.velocity.clone(),
            previous_face_volume_fluxes.clone(),
            body_force.clone(),
        )?;
        let (accepted, residual, newton) = solve_collocated_step_2d(
            &operator,
            numerical.clone(),
            resolved.plan().nonlinear(),
            resolved.plan().fieldwise().solver(),
            solver,
        )?;
        let evidence = accept_collocated_step(
            &normalized_mesh,
            &normalized_cells,
            &operator,
            &accepted,
            &residual,
            newton,
        )?;
        previous_face_volume_fluxes = residual.face_volume_fluxes();
        time = DynQuantity::new(time.value() + duration.value(), TIME);
        if !time.value().is_finite() {
            return Err(invalid_realization(
                "collocated transient time cannot be represented after an accepted step",
            ));
        }
        states.push(ResolvedCellCenteredNavierStokesState2d {
            time,
            velocity_field: velocity_id(&model),
            pressure_field: pressure_id(&model),
            velocity: CellCenteredVelocityField2d::new(
                physical_mesh.clone(),
                accepted
                    .velocity
                    .iter()
                    .map(|value| value.map(|component| component * scales.velocity_value()))
                    .collect(),
            )?,
            pressure: CellCenteredPressureField2d::new(
                physical_mesh.clone(),
                accepted
                    .pressure
                    .iter()
                    .map(|value| value * scales.pressure_value())
                    .collect(),
            )?,
            gauge_multiplier: accepted.gauge_multiplier * scales.gauge_value(),
        });
        numerical = accepted;
        steps.push(evidence);
    }
    Ok(ResolvedCellCenteredNavierStokesTrajectory2d {
        model,
        realization: resolved.clone(),
        realization_graph: graph,
        solver_backend: solver.id(),
        scales,
        states,
        steps,
    })
}

fn require_exact_cell_centered_plan(
    model: &TransientIncompressibleNavierStokesCartesianModel2d,
    resolved: &ResolvedTransientCellCenteredIncompressibleFlowRealization,
    graph: &PortableRealizationGraph,
) -> Result<IncompressibleFlowScaleProfile2d, Diagnostic> {
    if resolved.requirements() != &transient_navier_stokes_cell_centered_requirements_2d(model) {
        return Err(invalid_realization(
            "collocated flow requirements differ from the exact canonical lowerer inventory",
        ));
    }
    if resolved.plan().fieldwise().target()
        != (Target::HostCpu {
            threads: NonZeroUsize::MIN,
        })
        || resolved.plan().fieldwise().schedule() != ExecutionSchedule::Offline
    {
        return Err(invalid_realization(
            "the bounded collocated reference executor requires exactly one host worker and offline scheduling",
        ));
    }
    let velocity = velocity_id(model);
    let pressure = pressure_id(model);
    if graph.lineage().model() != resolved.model()
        || graph.lineage().semantic_revision() != resolved.semantic_revision()
        || graph.domains().len() != 1
        || graph.domains()[0].domain() != domain_id(model)
        || graph.systems().len() != 1
        || graph.linear_solves().len() != 1
        || graph.nonlinear_solves().len() != 1
    {
        return Err(invalid_realization(
            "collocated portable graph differs from the exact canonical FVM method",
        ));
    }
    let system = &graph.systems()[0];
    let representation_for = |field| {
        system.blocks().iter().find_map(|block| match *block {
            SystemBlock::Field(id) if graph.field(id).is_some_and(|node| node.field() == field) => {
                Some(id)
            }
            _ => None,
        })
    };
    let velocity_representation = representation_for(velocity)
        .ok_or_else(|| invalid_realization("collocated graph has no velocity Field block"))?;
    let pressure_representation = representation_for(pressure)
        .ok_or_else(|| invalid_realization("collocated graph has no pressure Field block"))?;
    let expected = [
        TransformationNode::BackwardEulerDerivative {
            relation: momentum_id(model),
            state: velocity_representation,
            duration: resolved.plan().time_step().duration(),
        },
        TransformationNode::ImplicitCenteredMomentumConvection {
            relation: momentum_id(model),
            velocity: velocity_representation,
        },
        TransformationNode::CartesianCentralNewtonianTraction {
            relation: momentum_id(model),
            velocity: velocity_representation,
            pressure: pressure_representation,
        },
        TransformationNode::MomentumWeightedLinearExactCoupling {
            momentum_relation: momentum_id(model),
            incompressibility_relation: incompressibility_id(model),
            velocity: velocity_representation,
            pressure: pressure_representation,
            positive_diagonal: PositiveMomentumDiagonal::BackwardEulerMassAndLocalNewtonian,
            transient_history: TransientFaceFluxHistory::Bdf1PreviousAccepted,
        },
    ];
    if graph.transformations() != expected {
        return Err(invalid_realization(
            "collocated portable graph differs from the exact canonical FVM method",
        ));
    }
    let scaling = system
        .congruence_scaling()
        .ok_or_else(|| invalid_realization("collocated graph requires congruence scaling"))?;
    let scale_for = |block| {
        scaling
            .block_scales()
            .iter()
            .find(|entry| entry.block() == block)
            .map(|entry| entry.scale().quantity())
            .ok_or_else(|| invalid_realization("collocated graph is missing an exact block scale"))
    };
    let length = match graph.domains()[0].coordinates() {
        CoordinateTreatment::Scaled(scale) => scale.quantity(),
        CoordinateTreatment::Physical => {
            return Err(invalid_realization(
                "collocated coherent-SI graph requires an explicit length scale",
            ));
        }
    };
    let scales = IncompressibleFlowScaleProfile2d::new(
        length,
        scale_for(AlgebraicBlock::Field(velocity))?,
        scale_for(AlgebraicBlock::Field(pressure))?,
    )?;
    if scale_for(AlgebraicBlock::ConstraintMultiplier { field: pressure })? != scales.gauge()
        || scaling.weak_functional_scale().quantity() != scales.weak_functional()
    {
        return Err(invalid_realization(
            "collocated gauge and weak-functional scales must derive exactly from L/U/P",
        ));
    }
    Ok(scales)
}

fn accept_collocated_step(
    mesh: &CartesianMesh,
    cells: &[crate::cartesian_fvm_geometry::CartesianCellMetrics2d],
    operator: &CartesianIncompressibleOperator2d,
    accepted: &CollocatedPoint2d,
    residual: &crate::cartesian_incompressible::CollocatedResidual2d,
    newton: CollocatedNewtonEvidence2d,
) -> Result<CellCenteredNavierStokesStepEvidence2d, Diagnostic> {
    let replay = operator.replay(accepted, residual)?;
    let constant = vec![1.0; operator.cell_count()];
    let affine = cells
        .iter()
        .map(|cell| 1.0 + 2.0 * cell.center[0] - 3.0 * cell.center[1])
        .collect::<Vec<_>>();
    let maximum_affine_pressure_correction = constant
        .iter()
        .map(|_| 0.0)
        .chain(operator.pressure_corrections(&constant)?)
        .chain(operator.pressure_corrections(&affine)?)
        .fold(0.0_f64, |maximum, value| maximum.max(value.abs()));
    let mut checkerboard_norms = Vec::new();
    for axes in [[true, false], [false, true], [true, true]] {
        let pressure = (0..operator.cell_count())
            .map(|cell| {
                let indices = mesh
                    .cell_multi_index(eqiora_meshing::MeshEntity::new(DIMENSION, cell))
                    .expect("accepted Cartesian cell owns its multi-index");
                let parity = usize::from(axes[0]) * indices[0] + usize::from(axes[1]) * indices[1];
                if parity & 1 == 0 { 1.0 } else { -1.0 }
            })
            .collect::<Vec<_>>();
        let squared = operator
            .pressure_corrections(&pressure)?
            .iter()
            .map(|value| value * value)
            .sum::<f64>();
        checkerboard_norms.push(squared.sqrt());
    }
    let minimum_checkerboard_correction_norm =
        checkerboard_norms.into_iter().fold(f64::INFINITY, f64::min);
    let scale = operator
        .momentum_diagonal()
        .iter()
        .flatten()
        .copied()
        .fold(1.0_f64, f64::max);
    let affine_tolerance = 1024.0 * f64::EPSILON * scale;
    require_pressure_coupling_evidence(
        maximum_affine_pressure_correction,
        minimum_checkerboard_correction_norm,
        affine_tolerance,
    )?;
    if newton.maximum_centered_jvp_defect > 2.0e-7
        || residual.momentum_norm > newton.momentum_target
        || residual.continuity_norm > newton.continuity_target
        || residual.gauge_residual.abs() > newton.gauge_target
    {
        return Err(invalid_realization(
            "collocated step failed residual, JVP, affine-pressure, checkerboard, gauge, or face-cancellation acceptance",
        ));
    }
    Ok(CellCenteredNavierStokesStepEvidence2d {
        iterations: newton.iterations,
        initial_residual_norm: newton.initial_residual_norm,
        initial_momentum_norm: newton.initial_momentum_norm,
        initial_continuity_norm: newton.initial_continuity_norm,
        residual_target: newton.residual_target,
        momentum_residual_target: newton.momentum_target,
        continuity_residual_target: newton.continuity_target,
        gauge_residual_target: newton.gauge_target,
        momentum_residual_norm: residual.momentum_norm,
        continuity_residual_norm: residual.continuity_norm,
        gauge_residual: residual.gauge_residual,
        maximum_momentum_replay_defect: replay.maximum_momentum_defect,
        maximum_continuity_replay_defect: replay.maximum_continuity_defect,
        maximum_centered_jvp_defect: newton.maximum_centered_jvp_defect,
        maximum_face_cancellation_defect: replay.maximum_face_cancellation_defect,
        maximum_flux_reuse_defect: replay.maximum_flux_reuse_defect,
        global_mass_defect: replay.global_mass_defect,
        replay_tolerance: replay.tolerance,
        maximum_affine_pressure_correction,
        minimum_checkerboard_correction_norm,
        linear_solves: newton.linear_solves,
    })
}

fn require_pressure_coupling_evidence(
    maximum_affine_correction: f64,
    minimum_checkerboard_action: f64,
    affine_tolerance: f64,
) -> Result<(), Diagnostic> {
    if !maximum_affine_correction.is_finite()
        || maximum_affine_correction > affine_tolerance
        || !minimum_checkerboard_action.is_finite()
        || minimum_checkerboard_action <= 1024.0 * f64::EPSILON
    {
        Err(invalid_realization(
            "collocated pressure coupling failed affine exactness or admitted an unstabilized checkerboard null action",
        ))
    } else {
        Ok(())
    }
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

fn incompressibility_id(
    model: &TransientIncompressibleNavierStokesCartesianModel2d,
) -> Id<kinds::Relation> {
    model
        .incompressibility_relation()
        .downcast()
        .expect("transient lowerer retains an incompressibility Relation identity")
}

fn realization_error(error: Diagnostic) -> Diagnostic {
    invalid_realization(error.message())
}

fn invalid_realization(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(eqiora_core::diagnostic::codes::INVALID_REALIZATION, message)
}

#[cfg(test)]
mod tests {
    use super::require_pressure_coupling_evidence;

    #[test]
    fn omitted_checkerboard_action_is_an_active_acceptance_falsifier() {
        assert!(require_pressure_coupling_evidence(0.0, 0.0, 1.0e-12).is_err());
        assert!(require_pressure_coupling_evidence(0.0, 1.0e-6, 1.0e-12).is_ok());
    }
}
