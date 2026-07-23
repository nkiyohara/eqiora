#![cfg(feature = "faer")]

use std::f64::consts::PI;
use std::num::NonZeroUsize;

use eqiora::api::TransientNavierStokesReference2d;
use eqiora::artifact::SimplicialMeshEnvelopeV1;
use eqiora::assembly::{
    AssemblyBackend, AssemblyPacket, AssemblyPlan, AssemblyResult, AssemblyWork, LocalContribution,
    REFERENCE_ASSEMBLY_BACKEND,
};
use eqiora::backends::faer::FaerLinearSolver;
use eqiora::compatibility::ExactModelCodec;
use eqiora::meshing::{
    MeshEntity, MeshGeometry, MeshQualityGate, MeshTopology, SimplicialMesh, simplex_centroid_rule,
    triangle_duffy_gauss_legendre,
};
use eqiora::numerics::{
    DiscreteSpace, IncompressibleFlowScaleProfile2d, MiniNavierStokesStepPlan2d, NonZeroStepCount,
    ResolvedTransientNavierStokesState2d, ResolvedTransientNavierStokesTrajectory2d,
    SimplexP1BubbleSpace, SimplicialMiniNavierStokesState2d,
    SimplicialMiniNavierStokesTrajectory2d, SimplicialMiniStokesBoundary2d,
    SimplicialMiniStokesBoundaryCondition2d, SimplicialMiniStokesBoundaryFacet2d,
    SimplicialMiniVelocityField2d, SteadyStokesPressureReference2d,
    advance_simplicial_mini_navier_stokes_2d,
    advance_simplicial_mini_navier_stokes_2d_with_assembly,
    lower_transient_incompressible_navier_stokes_cartesian_2d, solve_simplicial_mini_stokes_2d,
};
use eqiora::realization::{
    NonlinearSolvePlan, PlacementRequirementNode, RealizationRevision, SolveRoot, Target,
    TransformationNode,
};
use eqiora::solver::{
    LinearSolveRequest, LinearSolver, REFERENCE_LINEAR_SOLVER, ReductionPolicy, SolverPlan,
};
use eqiora::{DimExponents, DynQuantity};

const SOURCE: &str =
    include_str!("../../../verify/fluid/fixed-domain-transient-navier-stokes-2d/models/direct.eqi");

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
const TIME: DimExponents = DimExponents {
    time: 1,
    ..DimExponents::DIMENSIONLESS
};

#[test]
fn canonical_model_advances_through_one_scaled_block_system() {
    let fixture = fixture(3);
    let trajectory = canonical_advance(&fixture, 2, 0.01).unwrap();
    let non_unit = canonical_advance_with_policy(
        &fixture,
        2,
        0.01,
        flow_scales(2.0, 3.0, 5.0),
        1.0e-8,
        1.0e-10,
    )
    .unwrap();

    assert_eq!(trajectory.states().len(), 3);
    assert_eq!(trajectory.steps().len(), 2);
    assert_ne!(trajectory.mesh_artifact().sha256(), [0; 32]);
    assert_eq!(trajectory.model().mass_density(), 1.0);
    assert_eq!(trajectory.model().dynamic_viscosity(), 0.05);
    assert_eq!(
        trajectory.realization().realization_revision(),
        RealizationRevision::new(1)
    );
    assert_eq!(
        trajectory
            .realization()
            .plan()
            .time_step()
            .duration()
            .value(),
        0.01
    );
    assert_eq!(trajectory.solver_backend().as_str(), "eqiora.faer");
    assert_eq!(
        trajectory.realization_graph(),
        &trajectory.realization().portable_graph().unwrap()
    );
    assert!(matches!(
        trajectory.realization_graph().root(),
        SolveRoot::Nonlinear(_)
    ));
    assert_eq!(
        trajectory.realization_graph().placements(),
        [PlacementRequirementNode::HostWorkers {
            workers_per_partition: NonZeroUsize::MIN,
        }]
    );
    assert!(matches!(
        trajectory.realization_graph().transformations(),
        [
            TransformationNode::BackwardEulerDerivative { .. },
            TransformationNode::EnergySkewConvection { .. }
        ]
    ));
    assert!(trajectory.validated_block_materialization_count() > 0);
    assert_eq!(trajectory.states().last().unwrap().time().value(), 0.02);
    assert!(trajectory.states().iter().all(|state| {
        state.velocity().mesh() == &fixture.mesh && state.pressure().mesh() == &fixture.mesh
    }));
    assert!(resolved_final_velocity_distance(&trajectory, &non_unit) < 1.0e-8);
}

#[test]
fn fixed_domain_skew_mini_advances_two_nonlinear_steps() {
    let fixture = fixture(3);
    let trajectory = advance(&fixture, fixture.initial.clone(), 2, 0.01).unwrap();

    assert_eq!(trajectory.states().len(), 3);
    assert_eq!(trajectory.steps().len(), 2);
    assert!(
        trajectory
            .states()
            .windows(2)
            .all(|pair| pair[0].time() < pair[1].time())
    );
    for step in trajectory.steps() {
        assert!(step.nonlinear_iterations() > 0);
        assert!(step.final_residual_norm() <= step.residual_target());
        assert!(step.continuity_residual_norm() <= 2.0 * step.residual_target());
        assert!(step.pressure_integral().abs() <= 2.0 * step.residual_target());
        assert!(step.convective_residual_norm() > 1.0e-8);
        assert!(step.convective_power().abs() < 1.0e-10);
        assert!(!step.linear_solves().is_empty());
        assert!(step.assembly_report().packet_count() > 0);
    }
}

#[test]
fn backward_euler_has_first_order_fixed_mesh_time_refinement() {
    let fixture = fixture(3);
    let coarse = canonical_advance(&fixture, 2, 0.02).unwrap();
    let medium = canonical_advance(&fixture, 4, 0.01).unwrap();
    let fine = canonical_advance(&fixture, 8, 0.005).unwrap();
    let tight_fine =
        canonical_advance_with_policy(&fixture, 8, 0.005, reference_scales(), 1.0e-9, 1.0e-11)
            .unwrap();

    let coarse_medium = resolved_final_velocity_distance(&coarse, &medium);
    let medium_fine = resolved_final_velocity_distance(&medium, &fine);
    let ratio = coarse_medium / medium_fine;
    assert!(coarse_medium.is_finite() && medium_fine > 0.0);
    assert!(
        ratio > 1.7 && ratio < 2.4,
        "backward-Euler step-doubling ratio {ratio} from errors {coarse_medium:e}, {medium_fine:e}"
    );
    let nonlinear_sensitivity = resolved_final_velocity_distance(&fine, &tight_fine);
    assert!(nonlinear_sensitivity < 1.0e-3 * medium_fine);
}

#[test]
fn transient_admission_fails_closed_on_near_misses() {
    let fixture = fixture(3);

    let document = ExactModelCodec::V5
        .compile("transient-foreign-mesh.eqi", SOURCE)
        .unwrap();
    let foreign_mesh = SimplicialMeshEnvelopeV1::from_mesh(&unit_square_triangles(4)).unwrap();
    let foreign_reference = TransientNavierStokesReference2d::prepare(
        document.program(),
        foreign_mesh,
        reference_scales(),
        DynQuantity::new(0.01, TIME),
        NonlinearSolvePlan::new(1.0e-9, 1.0e-11, NonZeroUsize::new(12).unwrap(), 12).unwrap(),
        linear_plan(1.0e-11, 1.0e-12, 2000),
        RealizationRevision::new(2),
        FaerLinearSolver,
    )
    .unwrap();
    let foreign_initial = foreign_reference
        .initial_condition(
            DynQuantity::new(0.0, TIME),
            fixture.initial.velocity().clone(),
            fixture.initial.pressure().clone(),
            physical_pressure_reference(&fixture.initial),
        )
        .unwrap_err();
    assert!(
        foreign_initial
            .to_string()
            .contains("authenticated mesh revision")
    );

    let low_quadrature = advance_simplicial_mini_navier_stokes_2d(
        &fixture.mesh,
        &fixture.boundary,
        &zero_trace,
        &zero_force,
        fixture.initial.clone(),
        NonZeroStepCount::new(NonZeroUsize::MIN),
        plan(0.01, 12),
        &triangle_duffy_gauss_legendre(4).unwrap(),
        &simplex_centroid_rule(1).unwrap(),
        &FaerLinearSolver,
    )
    .unwrap_err();
    assert!(low_quadrature.to_string().contains("quadrature exactness"));

    let other_mesh = unit_square_triangles(4);
    let other_boundary = SimplicialMiniStokesBoundary2d::all_essential(&other_mesh).unwrap();
    let stale_mesh = advance_simplicial_mini_navier_stokes_2d(
        &other_mesh,
        &other_boundary,
        &zero_trace,
        &zero_force,
        fixture.initial.clone(),
        NonZeroStepCount::new(NonZeroUsize::MIN),
        plan(0.01, 12),
        &triangle_duffy_gauss_legendre(5).unwrap(),
        &simplex_centroid_rule(1).unwrap(),
        &FaerLinearSolver,
    )
    .unwrap_err();
    assert!(stale_mesh.to_string().contains("stale or moving mesh"));

    let mixed_boundary = one_traction_boundary(&fixture.mesh);
    let ambiguous_pressure = advance_simplicial_mini_navier_stokes_2d(
        &fixture.mesh,
        &mixed_boundary,
        &zero_trace,
        &zero_force,
        fixture.initial.clone(),
        NonZeroStepCount::new(NonZeroUsize::MIN),
        plan(0.01, 12),
        &triangle_duffy_gauss_legendre(5).unwrap(),
        &simplex_centroid_rule(1).unwrap(),
        &FaerLinearSolver,
    )
    .unwrap_err();
    assert!(
        ambiguous_pressure
            .to_string()
            .contains("pressure closure differs")
    );

    let inconsistent = inconsistent_boundary_state(&fixture.initial);
    let inconsistent = advance_simplicial_mini_navier_stokes_2d(
        &fixture.mesh,
        &fixture.boundary,
        &zero_trace,
        &zero_force,
        inconsistent,
        NonZeroStepCount::new(NonZeroUsize::MIN),
        plan(0.01, 12),
        &triangle_duffy_gauss_legendre(5).unwrap(),
        &simplex_centroid_rule(1).unwrap(),
        &FaerLinearSolver,
    )
    .unwrap_err();
    assert!(
        inconsistent
            .to_string()
            .contains("violates an essential boundary")
    );

    for (inconsistent, expected) in [
        (
            inconsistent_interior_velocity_state(&fixture.initial),
            "weak continuity residual",
        ),
        (
            inconsistent_pressure_mean_state(&fixture.initial),
            "pressure integral",
        ),
        (
            inconsistent_gauge_state(&fixture.initial),
            "gauge multiplier",
        ),
    ] {
        let diagnostic =
            advance_with_assembly(&fixture, inconsistent, 1, 0.01, &RejectUnexpectedAssembly)
                .unwrap_err();
        assert!(
            diagnostic.to_string().contains(expected),
            "expected {expected} falsifier, received {diagnostic}"
        );
    }

    assert!(
        MiniNavierStokesStepPlan2d::new(
            1.0,
            0.05,
            0.01,
            1.0,
            1.0e-11,
            NonZeroUsize::MIN,
            0,
            linear_plan(1.0e-11, 1.0e-12, 2000),
            Target::HostCpu {
                threads: NonZeroUsize::MIN,
            },
        )
        .unwrap_err()
        .to_string()
        .contains("below one")
    );

    let nonconvergent = advance_simplicial_mini_navier_stokes_2d(
        &fixture.mesh,
        &fixture.boundary,
        &zero_trace,
        &zero_force,
        fixture.initial.clone(),
        NonZeroStepCount::new(NonZeroUsize::MIN),
        strict_plan(0.01, 1),
        &triangle_duffy_gauss_legendre(5).unwrap(),
        &simplex_centroid_rule(1).unwrap(),
        &FaerLinearSolver,
    )
    .unwrap_err();
    assert!(nonconvergent.to_string().contains("Newton solve reached"));

    let wrong_jacobian = advance_simplicial_mini_navier_stokes_2d_with_assembly(
        &fixture.mesh,
        &fixture.boundary,
        &zero_trace,
        &zero_force,
        zero_state(&fixture.mesh),
        NonZeroStepCount::new(NonZeroUsize::MIN),
        plan(0.01, 20),
        &triangle_duffy_gauss_legendre(5).unwrap(),
        &simplex_centroid_rule(1).unwrap(),
        &CorruptJacobianAssembly,
        &FaerLinearSolver,
    )
    .unwrap_err();
    assert!(
        wrong_jacobian.to_string().contains("Jacobian column"),
        "wrong analytic Jacobian must fail its column check: {wrong_jacobian}"
    );

    let unrepresentable_time = SimplicialMiniNavierStokesState2d::new(
        f64::MAX,
        fixture.initial.velocity().clone(),
        fixture.initial.pressure().clone(),
        fixture.initial.pressure_reference(),
    )
    .unwrap();
    let nonmonotone = advance(&fixture, unrepresentable_time, 1, 0.01).unwrap_err();
    assert!(
        nonmonotone
            .to_string()
            .contains("cannot advance representable model time")
    );
}

#[derive(Debug)]
struct RejectUnexpectedAssembly;

impl AssemblyBackend for RejectUnexpectedAssembly {
    fn assemble(
        &self,
        _plan: &AssemblyPlan,
        _work: &dyn AssemblyWork,
    ) -> Result<AssemblyResult, eqiora::Diagnostic> {
        Err(eqiora::Diagnostic::error(
            eqiora::diagnostic::codes::INVALID_REALIZATION,
            "initial consistency check reached assembly unexpectedly",
        ))
    }
}

#[derive(Debug)]
struct CorruptJacobianAssembly;

impl AssemblyBackend for CorruptJacobianAssembly {
    fn assemble(
        &self,
        plan: &AssemblyPlan,
        work: &dyn AssemblyWork,
    ) -> Result<AssemblyResult, eqiora::Diagnostic> {
        REFERENCE_ASSEMBLY_BACKEND.assemble(plan, &CorruptJacobianWork { inner: work })
    }
}

#[derive(Debug)]
struct CorruptJacobianWork<'a> {
    inner: &'a dyn AssemblyWork,
}

impl AssemblyWork for CorruptJacobianWork<'_> {
    fn packet_set_identity(&self) -> eqiora::assembly::AssemblyPacketSetIdentityV1 {
        self.inner.packet_set_identity()
    }

    fn packet_count(&self) -> usize {
        self.inner.packet_count()
    }

    fn evaluate(&self, packet_index: usize) -> Result<AssemblyPacket, eqiora::Diagnostic> {
        let packet = self.inner.evaluate(packet_index)?;
        let mut matrix = packet.local().matrix().to_vec();
        matrix[0] += 1.0e-5;
        AssemblyPacket::new(
            LocalContribution::new(
                packet.local().rows(),
                packet.local().columns(),
                matrix,
                packet.local().rhs().to_vec(),
            )?,
            packet.mappings().to_vec(),
        )
    }
}

#[test]
fn canonical_flow_falsifiers_are_part_of_registered_evidence() {
    assert_canonical_flow_rejected(&SOURCE.replace(
        "outer_product(velocity, velocity)",
        "outer_product(velocity, velocity + velocity)",
    ));

    let distinct_density = SOURCE
        .replace(
            "parameter density: kg / m ^ 3 = 1;",
            "parameter density: kg / m ^ 3 = 1;\n  parameter flux_density: kg / m ^ 3 = 1;",
        )
        .replace(
            "div(density * outer_product(velocity, velocity))",
            "div(flux_density * outer_product(velocity, velocity))",
        );
    assert_canonical_flow_rejected(&distinct_density);

    let hidden_ale = SOURCE
        .replace(
            "field pressure on body as space: kg / (m * s ^ 2) = 0;",
            "field mesh_velocity on body as space: m / s shape spatial_vector;\n  field pressure on body as space: kg / (m * s ^ 2) = 0;",
        )
        .replace(
            "outer_product(velocity, velocity)",
            "outer_product(velocity - mesh_velocity, velocity)",
        );
    assert_canonical_flow_rejected(&hidden_ale);

    assert_canonical_flow_rejected(&SOURCE.replace(
        "  relation x_lower_value continuous on x_lower { trace(velocity) = 0; }\n",
        "",
    ));
}

fn assert_canonical_flow_rejected(source: &str) {
    let document = ExactModelCodec::V5
        .compile("transient-falsifier.eqi", source)
        .expect("near-miss remains a valid typed Model");
    assert!(
        lower_transient_incompressible_navier_stokes_cartesian_2d(document.program()).is_err(),
        "canonical near-miss must fail closed"
    );
}

struct Fixture {
    mesh: SimplicialMesh,
    boundary: SimplicialMiniStokesBoundary2d,
    initial: SimplicialMiniNavierStokesState2d,
}

fn fixture(subdivisions: usize) -> Fixture {
    let mesh = unit_square_triangles(subdivisions);
    let initial_force =
        |point: [f64; 2]| Ok([(PI * point[1]).sin(), -0.75 * (2.0 * PI * point[0]).sin()]);
    let initial = solve_simplicial_mini_stokes_2d(
        &mesh,
        0.05,
        &initial_force,
        &zero_trace,
        &triangle_duffy_gauss_legendre(3).unwrap(),
        LinearSolveRequest::new(
            &REFERENCE_LINEAR_SOLVER,
            SolverPlan::new(
                LinearSolver::MinimumResidual,
                1.0e-11,
                1.0e-12,
                NonZeroUsize::new(1000).unwrap(),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let initial = SimplicialMiniNavierStokesState2d::from_stokes_solution(0.0, &initial).unwrap();
    let boundary = SimplicialMiniStokesBoundary2d::all_essential(&mesh).unwrap();
    Fixture {
        mesh,
        boundary,
        initial,
    }
}

fn canonical_advance(
    fixture: &Fixture,
    steps: usize,
    dt: f64,
) -> Result<ResolvedTransientNavierStokesTrajectory2d, eqiora::Diagnostic> {
    canonical_advance_with_policy(fixture, steps, dt, reference_scales(), 1.0e-8, 1.0e-10)
}

fn canonical_advance_with_policy(
    fixture: &Fixture,
    steps: usize,
    dt: f64,
    scales: IncompressibleFlowScaleProfile2d,
    nonlinear_relative_tolerance: f64,
    nonlinear_absolute_tolerance: f64,
) -> Result<ResolvedTransientNavierStokesTrajectory2d, eqiora::Diagnostic> {
    let document = ExactModelCodec::V5
        .compile(
            "verify/fluid/fixed-domain-transient-navier-stokes-2d/models/direct.eqi",
            SOURCE,
        )
        .unwrap();
    assert_eq!(document.exact_codec(), ExactModelCodec::V5);
    let mesh = SimplicialMeshEnvelopeV1::from_mesh(&fixture.mesh).unwrap();
    let reference = TransientNavierStokesReference2d::prepare(
        document.program(),
        mesh,
        scales,
        DynQuantity::new(dt, TIME),
        NonlinearSolvePlan::new(
            nonlinear_relative_tolerance,
            nonlinear_absolute_tolerance,
            NonZeroUsize::new(16).unwrap(),
            12,
        )?,
        linear_plan(1.0e-11, 1.0e-12, 2000),
        RealizationRevision::new(1),
        FaerLinearSolver,
    )?;
    let initial = reference.initial_condition(
        DynQuantity::new(0.0, TIME),
        fixture.initial.velocity().clone(),
        fixture.initial.pressure().clone(),
        physical_pressure_reference(&fixture.initial),
    )?;
    reference.advance(
        initial,
        NonZeroUsize::new(steps).expect("registered runs request at least one step"),
    )
}

fn reference_scales() -> IncompressibleFlowScaleProfile2d {
    flow_scales(1.0, 1.0, 1.0)
}

fn flow_scales(length: f64, velocity: f64, pressure: f64) -> IncompressibleFlowScaleProfile2d {
    IncompressibleFlowScaleProfile2d::new(
        DynQuantity::new(length, LENGTH),
        DynQuantity::new(velocity, VELOCITY),
        DynQuantity::new(pressure, PRESSURE),
    )
    .unwrap()
}

fn physical_pressure_reference(
    initial: &SimplicialMiniNavierStokesState2d,
) -> SteadyStokesPressureReference2d {
    match initial.pressure_reference() {
        eqiora::numerics::SimplicialMiniStokesPressureReference2d::ZeroIntegral { multiplier } => {
            SteadyStokesPressureReference2d::ZeroIntegral { multiplier }
        }
        eqiora::numerics::SimplicialMiniStokesPressureReference2d::BoundaryTraction => {
            SteadyStokesPressureReference2d::BoundaryTraction
        }
    }
}

fn advance(
    fixture: &Fixture,
    initial: SimplicialMiniNavierStokesState2d,
    steps: usize,
    dt: f64,
) -> Result<SimplicialMiniNavierStokesTrajectory2d, eqiora::Diagnostic> {
    advance_simplicial_mini_navier_stokes_2d(
        &fixture.mesh,
        &fixture.boundary,
        &zero_trace,
        &zero_force,
        initial,
        NonZeroStepCount::new(NonZeroUsize::new(steps).unwrap()),
        plan(dt, 12),
        &triangle_duffy_gauss_legendre(5).unwrap(),
        &simplex_centroid_rule(1).unwrap(),
        &FaerLinearSolver,
    )
}

fn advance_with_assembly(
    fixture: &Fixture,
    initial: SimplicialMiniNavierStokesState2d,
    steps: usize,
    dt: f64,
    assembly: &dyn AssemblyBackend,
) -> Result<SimplicialMiniNavierStokesTrajectory2d, eqiora::Diagnostic> {
    advance_simplicial_mini_navier_stokes_2d_with_assembly(
        &fixture.mesh,
        &fixture.boundary,
        &zero_trace,
        &zero_force,
        initial,
        NonZeroStepCount::new(NonZeroUsize::new(steps).unwrap()),
        plan(dt, 12),
        &triangle_duffy_gauss_legendre(5).unwrap(),
        &simplex_centroid_rule(1).unwrap(),
        assembly,
        &FaerLinearSolver,
    )
}

fn plan(dt: f64, maximum_newton_iterations: usize) -> MiniNavierStokesStepPlan2d {
    MiniNavierStokesStepPlan2d::new(
        1.0,
        0.05,
        dt,
        1.0e-9,
        1.0e-11,
        NonZeroUsize::new(maximum_newton_iterations).unwrap(),
        12,
        linear_plan(1.0e-11, 1.0e-12, 2000),
        Target::HostCpu {
            threads: NonZeroUsize::MIN,
        },
    )
    .unwrap()
}

fn strict_plan(dt: f64, maximum_newton_iterations: usize) -> MiniNavierStokesStepPlan2d {
    MiniNavierStokesStepPlan2d::new(
        1.0,
        0.05,
        dt,
        1.0e-14,
        1.0e-14,
        NonZeroUsize::new(maximum_newton_iterations).unwrap(),
        0,
        linear_plan(1.0e-13, 1.0e-14, 4000),
        Target::HostCpu {
            threads: NonZeroUsize::MIN,
        },
    )
    .unwrap()
}

fn linear_plan(relative: f64, absolute: f64, iterations: usize) -> SolverPlan {
    SolverPlan::new(
        LinearSolver::BiConjugateGradientStabilized,
        relative,
        absolute,
        NonZeroUsize::new(iterations).unwrap(),
    )
    .unwrap()
    .with_reduction(ReductionPolicy::Fast)
}

fn resolved_final_velocity_distance(
    left: &ResolvedTransientNavierStokesTrajectory2d,
    right: &ResolvedTransientNavierStokesTrajectory2d,
) -> f64 {
    velocity_distance(
        left.states().last().unwrap(),
        right.states().last().unwrap(),
    )
}

fn velocity_distance(
    left: &ResolvedTransientNavierStokesState2d,
    right: &ResolvedTransientNavierStokesState2d,
) -> f64 {
    assert_eq!(left.velocity().mesh(), right.velocity().mesh());
    let mesh = left.velocity().mesh();
    let space = SimplexP1BubbleSpace::new(2).unwrap();
    let quadrature = triangle_duffy_gauss_legendre(5).unwrap();
    let mut squared = 0.0;
    for cell in 0..mesh.entity_count(2).unwrap() {
        let entity = MeshEntity::new(2, cell);
        let vertices = mesh.entity_vertices(entity).unwrap();
        let geometry = mesh.geometry_map(entity).unwrap();
        for point in quadrature.points() {
            let basis = space.tabulate(&point.coordinates).unwrap();
            let mut difference = [0.0; 2];
            for (local, basis_value) in basis.values().iter().enumerate() {
                let left_value = if local < 3 {
                    left.velocity().vertex_values()[vertices[local].index()]
                } else {
                    left.velocity().cell_bubble_values()[cell]
                };
                let right_value = if local < 3 {
                    right.velocity().vertex_values()[vertices[local].index()]
                } else {
                    right.velocity().cell_bubble_values()[cell]
                };
                for component in 0..2 {
                    difference[component] +=
                        basis_value * (left_value[component] - right_value[component]);
                }
            }
            squared += point.weight
                * geometry.measure_scale()
                * difference.iter().map(|value| value * value).sum::<f64>();
        }
    }
    squared.sqrt()
}

fn zero_state(mesh: &SimplicialMesh) -> SimplicialMiniNavierStokesState2d {
    let velocity = SimplicialMiniVelocityField2d::new(
        mesh.clone(),
        vec![[0.0; 2]; mesh.vertices().len()],
        vec![[0.0; 2]; mesh.entity_count(2).unwrap()],
    )
    .unwrap();
    let pressure =
        eqiora::numerics::SimplicialP1Field::new(mesh.clone(), vec![0.0; mesh.vertices().len()])
            .unwrap();
    SimplicialMiniNavierStokesState2d::new(
        0.0,
        velocity,
        pressure,
        eqiora::numerics::SimplicialMiniStokesPressureReference2d::ZeroIntegral { multiplier: 0.0 },
    )
    .unwrap()
}

fn inconsistent_boundary_state(
    source: &SimplicialMiniNavierStokesState2d,
) -> SimplicialMiniNavierStokesState2d {
    let mut vertices = source.velocity().vertex_values().to_vec();
    vertices[0][0] = 1.0;
    let velocity = SimplicialMiniVelocityField2d::new(
        source.velocity().mesh().clone(),
        vertices,
        source.velocity().cell_bubble_values().to_vec(),
    )
    .unwrap();
    SimplicialMiniNavierStokesState2d::new(
        source.time(),
        velocity,
        source.pressure().clone(),
        source.pressure_reference(),
    )
    .unwrap()
}

fn inconsistent_interior_velocity_state(
    source: &SimplicialMiniNavierStokesState2d,
) -> SimplicialMiniNavierStokesState2d {
    let mut bubbles = source.velocity().cell_bubble_values().to_vec();
    bubbles[0][0] += 1.0;
    let velocity = SimplicialMiniVelocityField2d::new(
        source.velocity().mesh().clone(),
        source.velocity().vertex_values().to_vec(),
        bubbles,
    )
    .unwrap();
    SimplicialMiniNavierStokesState2d::new(
        source.time(),
        velocity,
        source.pressure().clone(),
        source.pressure_reference(),
    )
    .unwrap()
}

fn inconsistent_pressure_mean_state(
    source: &SimplicialMiniNavierStokesState2d,
) -> SimplicialMiniNavierStokesState2d {
    let pressure = eqiora::numerics::SimplicialP1Field::new(
        source.pressure().mesh().clone(),
        source
            .pressure()
            .vertex_values()
            .iter()
            .map(|value| value + 1.0)
            .collect(),
    )
    .unwrap();
    SimplicialMiniNavierStokesState2d::new(
        source.time(),
        source.velocity().clone(),
        pressure,
        source.pressure_reference(),
    )
    .unwrap()
}

fn inconsistent_gauge_state(
    source: &SimplicialMiniNavierStokesState2d,
) -> SimplicialMiniNavierStokesState2d {
    SimplicialMiniNavierStokesState2d::new(
        source.time(),
        source.velocity().clone(),
        source.pressure().clone(),
        eqiora::numerics::SimplicialMiniStokesPressureReference2d::ZeroIntegral { multiplier: 1.0 },
    )
    .unwrap()
}

fn one_traction_boundary(mesh: &SimplicialMesh) -> SimplicialMiniStokesBoundary2d {
    let facet_count = mesh.entity_count(1).unwrap();
    let mut used_traction = false;
    let facets = (0..facet_count).filter_map(|index| {
        let facet = MeshEntity::new(1, index);
        mesh.is_boundary_entity(facet).unwrap().then(|| {
            let condition = if used_traction {
                SimplicialMiniStokesBoundaryCondition2d::EssentialVelocity
            } else {
                used_traction = true;
                SimplicialMiniStokesBoundaryCondition2d::ConstantTraction { value: [0.0, 0.0] }
            };
            SimplicialMiniStokesBoundaryFacet2d::new(facet, condition)
        })
    });
    SimplicialMiniStokesBoundary2d::new(mesh, facets).unwrap()
}

fn zero_trace(_: [f64; 2]) -> Result<[f64; 2], eqiora::Diagnostic> {
    Ok([0.0, 0.0])
}

fn zero_force(_: [f64; 2]) -> Result<[f64; 2], eqiora::Diagnostic> {
    Ok([0.0, 0.0])
}

fn unit_square_triangles(subdivisions: usize) -> SimplicialMesh {
    let width = subdivisions + 1;
    let mut vertices = Vec::with_capacity(width * width);
    for row in 0..=subdivisions {
        for column in 0..=subdivisions {
            vertices.push(vec![
                column as f64 / subdivisions as f64,
                row as f64 / subdivisions as f64,
            ]);
        }
    }
    let mut cells = Vec::with_capacity(2 * subdivisions * subdivisions);
    for row in 0..subdivisions {
        for column in 0..subdivisions {
            let lower_left = row * width + column;
            let lower_right = lower_left + 1;
            let upper_left = lower_left + width;
            let upper_right = upper_left + 1;
            cells.push(vec![lower_left, lower_right, upper_right]);
            cells.push(vec![lower_left, upper_right, upper_left]);
        }
    }
    SimplicialMesh::new(2, vertices, cells, MeshQualityGate::new(0.5).unwrap()).unwrap()
}
