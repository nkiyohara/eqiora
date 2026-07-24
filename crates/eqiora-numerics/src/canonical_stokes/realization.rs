use std::num::{NonZeroU16, NonZeroUsize};

use eqiora_assembly::{AssemblyBackend, REFERENCE_ASSEMBLY_BACKEND};
use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, DimExponents, DynQuantity, Id};
use eqiora_meshing::{
    MeshEntity, MeshTopology, SimplicialMesh, simplex_centroid_rule, triangle_duffy_gauss_legendre,
};
use eqiora_realization::{
    AlgebraicBlock, AlgebraicBlockScale, AlgebraicConstraint, Discretization, DiscretizationMethod,
    ExecutionSchedule, FieldSpaceBinding, FieldwiseRealizationPlan,
    FieldwiseRealizationRequirements, FieldwiseSpatialDiscretization, MeshArtifactReference,
    MeshPolicy, PlacementRequirementNode, PortableRealizationGraph, PositivePhysicalScale,
    QuadraturePolicy, RealizationRequirements, ResolvedFieldwiseRealization, SolveRoot, Space,
    SymmetricCongruenceScaling, Target, VectorLayoutKind,
};
use eqiora_schema::kernel::BoundarySide;
use eqiora_sem::KernelProgram;
use eqiora_solver::{
    LinearOperatorProperties, LinearSolver, LinearSolverBackend, PreconditionerPolicy,
    ReductionPolicy, ScalarType, SolverPlan,
};

use super::{
    FinalizedSteadyStokesMini2dProblem, SteadyIncompressibleStokesCartesianModel2d,
    SteadyStokesMiniSolution2d, lower_steady_incompressible_stokes_cartesian_2d,
};
use crate::{
    PhysicalBoundaryDisposition, SimplicialMiniStokesBoundary2d,
    SimplicialMiniStokesBoundaryCondition2d, SimplicialMiniStokesBoundaryFacet2d,
    finalize_simplicial_mini_stokes_2d_with_boundary_and_assembly,
};

const DIMENSION: usize = 2;
const DUFFY_POINTS_PER_AXIS: usize = 3;

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

/// Three native characteristic quantities for coherent-SI incompressible flow.
///
/// Gauge and weak-functional scales are derived exactly as `G = U / L` and
/// `Theta = P U L`; callers cannot supply either value independently.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IncompressibleFlowScaleProfile2d {
    length: PositivePhysicalScale,
    velocity: PositivePhysicalScale,
    pressure: PositivePhysicalScale,
    gauge: PositivePhysicalScale,
    weak_functional: PositivePhysicalScale,
}

impl IncompressibleFlowScaleProfile2d {
    /// Validate positive finite `L`, `U`, and `P` and derive `G` and `Theta`.
    ///
    /// # Errors
    /// Returns `EQ0807` for a non-positive, non-finite, or dimensionally
    /// incompatible quantity, including overflow in a derived scale.
    pub fn new(
        length: DynQuantity,
        velocity: DynQuantity,
        pressure: DynQuantity,
    ) -> Result<Self, Diagnostic> {
        require_dimension(length, LENGTH, "Stokes length scale L")?;
        require_dimension(velocity, VELOCITY, "Stokes velocity scale U")?;
        require_dimension(pressure, PRESSURE, "Stokes pressure scale P")?;
        let length = PositivePhysicalScale::new(length).map_err(realization_error)?;
        let velocity = PositivePhysicalScale::new(velocity).map_err(realization_error)?;
        let pressure = PositivePhysicalScale::new(pressure).map_err(realization_error)?;
        let gauge = PositivePhysicalScale::new(velocity.quantity() / length.quantity())
            .map_err(realization_error)?;
        let weak_functional = PositivePhysicalScale::new(
            pressure.quantity() * velocity.quantity() * length.quantity(),
        )
        .map_err(realization_error)?;
        Ok(Self {
            length,
            velocity,
            pressure,
            gauge,
            weak_functional,
        })
    }

    /// Characteristic physical length `L`.
    #[must_use]
    pub const fn length(self) -> DynQuantity {
        self.length.quantity()
    }

    /// Characteristic physical velocity `U`.
    #[must_use]
    pub const fn velocity(self) -> DynQuantity {
        self.velocity.quantity()
    }

    /// Characteristic physical pressure `P`.
    #[must_use]
    pub const fn pressure(self) -> DynQuantity {
        self.pressure.quantity()
    }

    /// Derived gauge-multiplier scale `G = U / L`.
    #[must_use]
    pub const fn gauge(self) -> DynQuantity {
        self.gauge.quantity()
    }

    /// Derived intrinsic-2D weak-functional scale `Theta = P U L`.
    #[must_use]
    pub const fn weak_functional(self) -> DynQuantity {
        self.weak_functional.quantity()
    }

    pub(super) const fn length_value(self) -> f64 {
        self.length.quantity().value()
    }

    pub(super) const fn velocity_value(self) -> f64 {
        self.velocity.quantity().value()
    }

    pub(super) const fn pressure_value(self) -> f64 {
        self.pressure.quantity().value()
    }

    pub(super) const fn gauge_value(self) -> f64 {
        self.gauge.quantity().value()
    }
}

/// Compatibility name for the existing steady-Stokes realization surface.
pub type SteadyStokesScaleProfile2d = IncompressibleFlowScaleProfile2d;

/// Exact lowerer requirements for the admitted mixed Stokes path.
#[must_use]
pub fn steady_stokes_fieldwise_requirements_2d(
    model: &SteadyIncompressibleStokesCartesianModel2d,
) -> FieldwiseRealizationRequirements {
    FieldwiseRealizationRequirements::new(
        domain_id(model),
        [velocity_id(model), pressure_id(model)],
        RealizationRequirements::new(
            NonZeroUsize::new(DIMENSION).expect("two is non-zero"),
            ScalarType::F64,
            VectorLayoutKind::Replicated,
        ),
    )
    .expect("a lowered Stokes model owns two distinct algebraic Fields")
}

/// Build the sole admitted field-wise MINI plan from exact canonical roles.
///
/// `q` remains immutable canonical coefficient data and receives no discrete
/// unknown block. The plan derives gauge and functional scales from `L/U/P`,
/// fixes positive degree-four Duffy assembly, and retains one exact MINRES
/// policy without a fluid-named generic space tag.
///
/// # Errors
/// Returns `EQ0807` for an unsupported solver tuple or invalid derived plan.
pub fn steady_stokes_mini_plan_2d(
    model: &SteadyIncompressibleStokesCartesianModel2d,
    mesh: MeshArtifactReference,
    scales: SteadyStokesScaleProfile2d,
    solver: SolverPlan,
) -> Result<FieldwiseRealizationPlan, Diagnostic> {
    let with_zero_integral_constraint = requires_zero_integral_constraint(model)?;
    require_reference_solver(solver)?;
    let velocity = velocity_id(model);
    let pressure = pressure_id(model);
    let constraints = with_zero_integral_constraint
        .then_some(AlgebraicConstraint::ZeroIntegral { field: pressure })
        .into_iter()
        .collect::<Vec<_>>();
    let spatial = FieldwiseSpatialDiscretization::new(
        domain_id(model),
        scales.length,
        [
            FieldSpaceBinding::new(velocity, Space::simplex_p1_bubble()),
            FieldSpaceBinding::new(pressure, Space::continuous_lagrange(NonZeroU16::MIN)),
        ],
        constraints,
        Discretization::new(
            DiscretizationMethod::ContinuousGalerkin,
            MeshPolicy::ImportedSimplicial { artifact: mesh },
            QuadraturePolicy::TriangleDuffyGaussLegendre {
                points_per_axis: NonZeroUsize::new(DUFFY_POINTS_PER_AXIS)
                    .expect("three is non-zero"),
            },
        ),
    )
    .map_err(realization_error)?;
    let mut block_scales = vec![
        AlgebraicBlockScale::new(AlgebraicBlock::Field(velocity), scales.velocity),
        AlgebraicBlockScale::new(AlgebraicBlock::Field(pressure), scales.pressure),
    ];
    if with_zero_integral_constraint {
        block_scales.push(AlgebraicBlockScale::new(
            AlgebraicBlock::ConstraintMultiplier { field: pressure },
            scales.gauge,
        ));
    }
    let scaling = SymmetricCongruenceScaling::new(block_scales, scales.weak_functional)
        .map_err(realization_error)?;
    FieldwiseRealizationPlan::new(
        spatial,
        scaling,
        LinearOperatorProperties::SymmetricIndefinite,
        solver,
        Target::HostCpu {
            threads: NonZeroUsize::MIN,
        },
        ExecutionSchedule::Offline,
    )
    .map_err(realization_error)
}

/// Finalize one resolved coherent-SI Stokes Model through reference assembly.
///
/// `mesh_artifact` authenticates the reference retained by `resolved`; a bare
/// [`SimplicialMesh`] has no content digest. Artifact-backed callers must first
/// validate the Realization against a mesh envelope, then pass the reference
/// and mesh reconstructed from that same accepted envelope.
///
/// # Errors
/// Rejects Model/Field/mesh/space/constraint/scale/solver drift before local
/// assembly and preserves numerical diagnostics from the dimensionless path.
pub fn finalize_resolved_steady_stokes_mini_2d(
    program: &KernelProgram,
    resolved: &ResolvedFieldwiseRealization,
    mesh_artifact: MeshArtifactReference,
    mesh: &SimplicialMesh,
) -> Result<
    (
        SteadyIncompressibleStokesCartesianModel2d,
        FinalizedSteadyStokesMini2dProblem,
    ),
    Diagnostic,
> {
    finalize_resolved_steady_stokes_mini_2d_with_assembly(
        program,
        resolved,
        mesh_artifact,
        mesh,
        &REFERENCE_ASSEMBLY_BACKEND,
    )
}

/// Finalize one resolved coherent-SI Stokes Model through explicit assembly.
///
/// The production local operator is assembled directly on normalized
/// coordinates with dimensionless viscosity and force-potential gradient. No
/// raw matrix with heterogeneous SI row dimensions is materialized.
/// `mesh_artifact` proves reference agreement, while the caller retains
/// responsibility for obtaining `mesh` from that exact validated artifact.
///
/// # Errors
/// Preserves all reference finalization failures plus the selected assembly
/// adapter's complete-operation diagnostic.
pub fn finalize_resolved_steady_stokes_mini_2d_with_assembly(
    program: &KernelProgram,
    resolved: &ResolvedFieldwiseRealization,
    mesh_artifact: MeshArtifactReference,
    mesh: &SimplicialMesh,
    assembly: &dyn AssemblyBackend,
) -> Result<
    (
        SteadyIncompressibleStokesCartesianModel2d,
        FinalizedSteadyStokesMini2dProblem,
    ),
    Diagnostic,
> {
    if program.model() != resolved.model()
        || program.revision().0 != resolved.semantic_revision().get()
    {
        return Err(invalid_realization(
            "resolved field-wise realization does not reference this exact Semantic Model revision",
        ));
    }
    let model = lower_steady_incompressible_stokes_cartesian_2d(program)?;
    requires_zero_integral_constraint(&model)?;
    let realization_graph = resolved.portable_graph()?;
    let scales = require_exact_plan(&model, resolved, &realization_graph, mesh_artifact)?;
    let normalized = normalize_mesh(&model, mesh, scales.length_value())?;
    let boundary = numerical_boundary(&model, &normalized, scales.pressure_value())?;
    let quadrature = triangle_duffy_gauss_legendre(DUFFY_POINTS_PER_AXIS)?;
    let facet_quadrature = simplex_centroid_rule(DIMENSION - 1)?;
    let dimensionless_viscosity = model.dynamic_viscosity() * scales.velocity_value()
        / (scales.pressure_value() * scales.length_value());
    if !dimensionless_viscosity.is_finite() || dimensionless_viscosity <= 0.0 {
        return Err(invalid_realization(
            "derived dimensionless Stokes viscosity is not finite and positive",
        ));
    }
    let lower = [model.bounds()[0][0], model.bounds()[1][0]];
    let parameter_tangent = vec![0.0; model.force_potential_expression().parameter_fields().len()];
    let force_potential = model.force_potential_expression();
    let dimensionless_force = |coordinate_hat: [f64; DIMENSION]| {
        let coordinate = [
            lower[0] + scales.length_value() * coordinate_hat[0],
            lower[1] + scales.length_value() * coordinate_hat[1],
        ];
        let mut result = [0.0; DIMENSION];
        for axis in 0..DIMENSION {
            let mut coordinate_tangent = [0.0; DIMENSION];
            coordinate_tangent[axis] = scales.length_value();
            let (_, derivative_hat) = force_potential.evaluate_jvp(
                &coordinate,
                &coordinate_tangent,
                &parameter_tangent,
            )?;
            result[axis] = derivative_hat / scales.pressure_value();
        }
        Ok(result)
    };
    let zero_velocity = |_| Ok([0.0; DIMENSION]);
    let block_system = super::block::steady_stokes_block_system(
        &model,
        resolved,
        mesh_artifact,
        &normalized.mesh,
        &boundary,
        scales,
    )?;
    let checked_assembly = block_system.checked_backend(assembly);
    let SolveRoot::Linear(root) = realization_graph.root() else {
        return Err(invalid_realization(
            "steady Stokes requires a linear portable Realization root",
        ));
    };
    let linear = realization_graph
        .linear_solve(root)
        .ok_or_else(|| invalid_realization("steady Stokes portable linear root is absent"))?;
    let inner = finalize_simplicial_mini_stokes_2d_with_boundary_and_assembly(
        &normalized.mesh,
        dimensionless_viscosity,
        &dimensionless_force,
        &boundary,
        &zero_velocity,
        &quadrature,
        &facet_quadrature,
        &checked_assembly,
        linear.plan(),
        realization_graph.systems()[0].partition(),
        resolved.plan().target(),
    )?;
    let inner = inner.with_block_system(&block_system)?;
    let finalized = FinalizedSteadyStokesMini2dProblem::new(
        inner,
        mesh.clone(),
        velocity_id(&model),
        pressure_id(&model),
        force_potential_id(&model),
        scales,
    );
    Ok((model, finalized))
}

/// Solve one resolved coherent-SI Stokes Model through reference assembly.
///
/// # Errors
/// Preserves exact finalization, backend, residual, and reconstruction diagnostics.
pub fn solve_resolved_steady_stokes_mini_2d(
    program: &KernelProgram,
    resolved: &ResolvedFieldwiseRealization,
    mesh_artifact: MeshArtifactReference,
    mesh: &SimplicialMesh,
    backend: &dyn LinearSolverBackend,
) -> Result<
    (
        SteadyIncompressibleStokesCartesianModel2d,
        SteadyStokesMiniSolution2d,
    ),
    Diagnostic,
> {
    solve_resolved_steady_stokes_mini_2d_with_assembly(
        program,
        resolved,
        mesh_artifact,
        mesh,
        &REFERENCE_ASSEMBLY_BACKEND,
        backend,
    )
}

/// Solve through independently selected assembly and linear execution adapters.
///
/// # Errors
/// Preserves every typed admission, adapter, and physical reconstruction failure.
pub fn solve_resolved_steady_stokes_mini_2d_with_assembly(
    program: &KernelProgram,
    resolved: &ResolvedFieldwiseRealization,
    mesh_artifact: MeshArtifactReference,
    mesh: &SimplicialMesh,
    assembly: &dyn AssemblyBackend,
    backend: &dyn LinearSolverBackend,
) -> Result<
    (
        SteadyIncompressibleStokesCartesianModel2d,
        SteadyStokesMiniSolution2d,
    ),
    Diagnostic,
> {
    let (model, finalized) = finalize_resolved_steady_stokes_mini_2d_with_assembly(
        program,
        resolved,
        mesh_artifact,
        mesh,
        assembly,
    )?;
    let solved = backend.solve(&finalized.linear_problem()?, finalized.solver_plan())?;
    Ok((model, finalized.finish(solved)?))
}

fn require_exact_plan(
    model: &SteadyIncompressibleStokesCartesianModel2d,
    resolved: &ResolvedFieldwiseRealization,
    graph: &PortableRealizationGraph,
    mesh_artifact: MeshArtifactReference,
) -> Result<SteadyStokesScaleProfile2d, Diagnostic> {
    let expected_requirements = steady_stokes_fieldwise_requirements_2d(model);
    if resolved.requirements() != &expected_requirements {
        return Err(invalid_realization(
            "field-wise requirements differ from the exact Stokes Domain and unknown-Field inventory",
        ));
    }
    let SolveRoot::Linear(root) = graph.root() else {
        return Err(invalid_realization(
            "steady Stokes portable graph requires one linear solve root",
        ));
    };
    let linear = graph
        .linear_solve(root)
        .ok_or_else(|| invalid_realization("steady Stokes graph linear root is absent"))?;
    if graph.lineage().model() != resolved.model()
        || graph.lineage().semantic_revision() != resolved.semantic_revision()
        || graph.domains().len() != 1
        || graph.domains()[0].domain() != domain_id(model)
        || graph.fields().len() != 2
        || graph.systems().len() != 1
        || graph.placement(linear.placement())
            != Some(PlacementRequirementNode::HostWorkers {
                workers_per_partition: NonZeroUsize::MIN,
            })
    {
        return Err(invalid_realization(
            "steady Stokes portable graph lineage, exact Field inventory, or placement drifted",
        ));
    }
    let plan = resolved.plan();
    let velocity = velocity_id(model);
    let pressure = pressure_id(model);
    let scale_for = |block| {
        graph.systems()[0]
            .congruence_scaling()
            .ok_or_else(|| invalid_realization("steady Stokes graph requires congruence scaling"))?
            .block_scales()
            .iter()
            .find(|entry| entry.block() == block)
            .map(|entry| entry.scale().quantity())
            .ok_or_else(|| {
                invalid_realization("Stokes plan is missing an exact algebraic block scale")
            })
    };
    let scales = SteadyStokesScaleProfile2d::new(
        plan.spatial().coordinate_length_scale().quantity(),
        scale_for(AlgebraicBlock::Field(velocity))?,
        scale_for(AlgebraicBlock::Field(pressure))?,
    )?;
    let expected = steady_stokes_mini_plan_2d(model, mesh_artifact, scales, plan.solver())?;
    if plan != &expected {
        return Err(invalid_realization(
            "field-wise plan differs from the exact coherent-SI MINI Stokes contract",
        ));
    }
    Ok(scales)
}

pub(super) struct NormalizedCartesianSimplicialMesh2d {
    pub(super) mesh: SimplicialMesh,
    pub(super) boundary_facets: Vec<(MeshEntity, usize, BoundarySide)>,
}

fn normalize_mesh(
    model: &SteadyIncompressibleStokesCartesianModel2d,
    mesh: &SimplicialMesh,
    length: f64,
) -> Result<NormalizedCartesianSimplicialMesh2d, Diagnostic> {
    normalize_cartesian_mesh(model.bounds(), mesh, length, "Stokes")
}

pub(super) fn normalize_cartesian_mesh(
    bounds: &[[f64; 2]; DIMENSION],
    mesh: &SimplicialMesh,
    length: f64,
    equation_family: &str,
) -> Result<NormalizedCartesianSimplicialMesh2d, Diagnostic> {
    if mesh.topological_dimension() != DIMENSION {
        return Err(invalid_realization(format!(
            "coherent-SI MINI {equation_family} requires an intrinsic 2D mesh"
        )));
    }
    let mut side_coverage = [[false; 2]; DIMENSION];
    for (vertex, coordinates) in mesh.vertices().iter().enumerate() {
        if coordinates.len() != DIMENSION
            || coordinates
                .iter()
                .enumerate()
                .any(|(axis, value)| *value < bounds[axis][0] || *value > bounds[axis][1])
        {
            return Err(invalid_realization(format!(
                "imported {equation_family} mesh has a vertex outside the exact Cartesian Domain"
            )));
        }
        let on_box = (0..DIMENSION).any(|axis| {
            coordinates[axis] == bounds[axis][0] || coordinates[axis] == bounds[axis][1]
        });
        let on_mesh_boundary = mesh
            .is_boundary_entity(MeshEntity::new(0, vertex))
            .expect("mesh owns every vertex");
        if on_box != on_mesh_boundary {
            return Err(invalid_realization(format!(
                "imported {equation_family} mesh boundary vertices do not coincide exactly with the Cartesian Domain boundary"
            )));
        }
    }
    let facet_count = mesh
        .entity_count(DIMENSION - 1)
        .expect("2D mesh owns edge entities");
    let mut boundary_facets = Vec::new();
    for facet in 0..facet_count {
        let facet = MeshEntity::new(DIMENSION - 1, facet);
        if !mesh
            .is_boundary_entity(facet)
            .expect("mesh owns every edge")
        {
            continue;
        }
        let vertices = mesh
            .entity_vertices(facet)
            .expect("accepted boundary edge owns vertices");
        let mut matched = None;
        for (axis, axis_bounds) in bounds.iter().enumerate() {
            for (side, bound) in axis_bounds.iter().enumerate() {
                if vertices
                    .iter()
                    .all(|vertex| mesh.vertices()[vertex.index()][axis] == *bound)
                {
                    if matched.is_some() {
                        return Err(invalid_realization(format!(
                            "a {equation_family} boundary facet ambiguously belongs to multiple Cartesian sides"
                        )));
                    }
                    matched = Some((axis, side));
                }
            }
        }
        let Some((axis, side)) = matched else {
            return Err(invalid_realization(format!(
                "a {equation_family} mesh boundary facet does not lie on an exact Cartesian side"
            )));
        };
        side_coverage[axis][side] = true;
        boundary_facets.push((
            facet,
            axis,
            if side == 0 {
                BoundarySide::Lower
            } else {
                BoundarySide::Upper
            },
        ));
    }
    if side_coverage.iter().flatten().any(|covered| !covered) {
        return Err(invalid_realization(format!(
            "imported {equation_family} mesh does not cover every exact Cartesian boundary side"
        )));
    }
    let vertices = mesh
        .vertices()
        .iter()
        .map(|coordinates| {
            (0..DIMENSION)
                .map(|axis| (coordinates[axis] - bounds[axis][0]) / length)
                .collect()
        })
        .collect();
    let mesh = SimplicialMesh::new(
        DIMENSION,
        vertices,
        mesh.cells().to_vec(),
        mesh.quality_gate(),
    )
    .map_err(|error| invalid_realization(error.message()))?;
    Ok(NormalizedCartesianSimplicialMesh2d {
        mesh,
        boundary_facets,
    })
}

fn numerical_boundary(
    model: &SteadyIncompressibleStokesCartesianModel2d,
    normalized: &NormalizedCartesianSimplicialMesh2d,
    pressure_scale: f64,
) -> Result<SimplicialMiniStokesBoundary2d, Diagnostic> {
    let facets = normalized
        .boundary_facets
        .iter()
        .map(|(facet, axis, side)| {
            let entry = model
                .boundary_inventory()
                .boundary(*axis, *side)
                .ok_or_else(|| {
                    invalid_realization(format!(
                        "lowered 2D Stokes boundary inventory omits axis {axis} {side:?}"
                    ))
                })?;
            let condition = match entry.disposition() {
                PhysicalBoundaryDisposition::TraceZero => {
                    SimplicialMiniStokesBoundaryCondition2d::EssentialVelocity
                }
                PhysicalBoundaryDisposition::FluxZero
                | PhysicalBoundaryDisposition::Prescribed(_) => {
                    let pressure = model
                        .normal_pressure(*axis, *side)
                        .ok_or_else(|| {
                            invalid_realization(format!(
                                "axis {axis} {side:?} is not an admitted normal-pressure boundary"
                            ))
                        })?
                        .expression()
                        .constant_value()
                        .ok_or_else(|| {
                            invalid_realization(format!(
                                "axis {axis} {side:?} normal pressure is coordinate-dependent; this realization admits only a spatial constant"
                            ))
                        })?;
                    let mut traction = [0.0; DIMENSION];
                    traction[*axis] = match side {
                        BoundarySide::Lower => pressure / pressure_scale,
                        BoundarySide::Upper => -pressure / pressure_scale,
                    };
                    SimplicialMiniStokesBoundaryCondition2d::ConstantTraction {
                        value: traction,
                    }
                }
                PhysicalBoundaryDisposition::PortBinding { connection, port } => {
                    return Err(invalid_realization(format!(
                        "live Stokes PortBinding {connection} through Port {port} on axis {axis} {side:?} requires an explicit trace-space interface Realization"
                    )));
                }
            };
            Ok(SimplicialMiniStokesBoundaryFacet2d::new(*facet, condition))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    SimplicialMiniStokesBoundary2d::new(&normalized.mesh, facets)
        .map_err(|error| invalid_realization(error.message()))
}

fn require_reference_solver(solver: SolverPlan) -> Result<(), Diagnostic> {
    if solver.algorithm() != LinearSolver::MinimumResidual
        || solver.preconditioner() != PreconditionerPolicy::Identity
        || solver.reduction() != ReductionPolicy::Reproducible
    {
        return Err(invalid_realization(
            "coherent-SI MINI Stokes requires reproducible identity-preconditioned MINRES",
        ));
    }
    Ok(())
}

fn requires_zero_integral_constraint(
    model: &SteadyIncompressibleStokesCartesianModel2d,
) -> Result<bool, Diagnostic> {
    let mut trace_sides = 0_usize;
    let mut pressure_sides = 0_usize;
    for axis in 0..DIMENSION {
        for side in [BoundarySide::Lower, BoundarySide::Upper] {
            let disposition = model
                .boundary_inventory()
                .boundary(axis, side)
                .ok_or_else(|| {
                    invalid_realization(format!(
                        "lowered 2D Stokes boundary inventory omits axis {axis} {side:?}"
                    ))
                })?
                .disposition();
            match disposition {
                PhysicalBoundaryDisposition::TraceZero => trace_sides += 1,
                PhysicalBoundaryDisposition::FluxZero => {
                    if model.normal_pressure(axis, side).is_none() {
                        return Err(invalid_realization(format!(
                            "flux-zero Stokes boundary on axis {axis} {side:?} is not an admitted normal-pressure law"
                        )));
                    }
                    pressure_sides += 1;
                }
                PhysicalBoundaryDisposition::Prescribed(law) => {
                    if model.normal_pressure(axis, side).is_none() {
                        return Err(invalid_realization(format!(
                            "prescribed Stokes {:?} law {} on axis {axis} {side:?} is outside the normal-pressure realization",
                            law.quantity(),
                            law.relation()
                        )));
                    }
                    pressure_sides += 1;
                }
                PhysicalBoundaryDisposition::PortBinding { connection, port } => {
                    return Err(invalid_realization(format!(
                        "live Stokes PortBinding {connection} through Port {port} on axis {axis} {side:?} requires an explicit trace-space interface Realization"
                    )));
                }
            }
        }
    }
    let side_count = 2 * DIMENSION;
    match (trace_sides, pressure_sides) {
        (trace, 0) if trace == side_count => Ok(true),
        (trace, pressure) if trace > 0 && pressure > 0 && trace + pressure == side_count => {
            Ok(false)
        }
        (0, pressure) if pressure == side_count => Err(invalid_realization(
            "the bounded MINI Stokes realization requires positive-measure essential velocity boundary; pure traction remains unsupported",
        )),
        _ => Err(invalid_realization(
            "Stokes boundary meaning is neither complete velocity trace nor an admitted velocity/static-pressure partition",
        )),
    }
}

fn require_dimension(
    value: DynQuantity,
    expected: DimExponents,
    label: &str,
) -> Result<(), Diagnostic> {
    if value.dim() != expected {
        return Err(invalid_realization(format!(
            "{label} has incompatible physical dimension {:?}",
            value.dim()
        )));
    }
    Ok(())
}

fn domain_id(model: &SteadyIncompressibleStokesCartesianModel2d) -> Id<kinds::Domain> {
    model
        .domain()
        .downcast()
        .expect("lowered Stokes Domain retains its entity kind")
}

fn velocity_id(model: &SteadyIncompressibleStokesCartesianModel2d) -> Id<kinds::Field> {
    model
        .velocity()
        .downcast()
        .expect("lowered Stokes velocity retains its Field kind")
}

fn pressure_id(model: &SteadyIncompressibleStokesCartesianModel2d) -> Id<kinds::Field> {
    model
        .pressure()
        .downcast()
        .expect("lowered Stokes pressure retains its Field kind")
}

fn force_potential_id(model: &SteadyIncompressibleStokesCartesianModel2d) -> Id<kinds::Field> {
    model
        .force_potential()
        .downcast()
        .expect("lowered Stokes force potential retains its Field kind")
}

fn realization_error(error: Diagnostic) -> Diagnostic {
    invalid_realization(error.message())
}

fn invalid_realization(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}
