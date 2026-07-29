#![cfg(feature = "faer")]

use std::num::NonZeroUsize;

use eqiora::api::TransientNavierStokesReference2d;
use eqiora::artifact::SimplicialMeshEnvelopeV1;
use eqiora::assembly::{AssemblyBackend, AssemblyPlan, AssemblyResult, AssemblyWork};
use eqiora::backends::faer::FaerLinearSolver;
use eqiora::compatibility::ExactModelCodec;
use eqiora::meshing::{
    MeshEntity, MeshQualityGate, MeshTopology, SimplicialMesh, simplex_centroid_rule,
    triangle_duffy_gauss_legendre,
};
use eqiora::realization::{
    AlgebraicConstraint, DiscretizationMethod, MeshKind, NonlinearSolvePlan,
    RealizationCapabilities, RealizationRevision, SemanticRevision, SpatialDimensionSupport,
    SystemBlock, TargetCapabilities, TransientFieldwiseRealizationRequest, VectorLayoutKind,
    resolve_transient_fieldwise,
};
use eqiora::solver::{
    LinearOperatorProperties, LinearSolveRequest, LinearSolver, REFERENCE_LINEAR_SOLVER,
    ReductionPolicy, ScalarType, SolverCapabilities, SolverCapability, SolverPlan,
};
use eqiora::{DimExponents, DynQuantity};
use eqiora_numerics::{
    common::NonZeroStepCount, common::PhysicalBoundaryDisposition,
    common::PhysicalBoundaryQuantity, common::SimplicialP1Field,
    fluid::IncompressibleFlowScaleProfile2d, fluid::SimplicialMiniStokesBoundary2d,
    fluid::SimplicialMiniStokesBoundaryCondition2d, fluid::SimplicialMiniStokesBoundaryFacet2d,
    fluid::SimplicialMiniStokesPressureReference2d, fluid::SimplicialMiniVelocityField2d,
    fluid::SteadyStokesPressureReference2d, fluid::TransientNavierStokesInitialState2d,
    fluid::TransientNavierStokesRun2d,
    fluid::advance_resolved_transient_navier_stokes_mini_2d_with_assembly,
    fluid::lower_transient_incompressible_navier_stokes_cartesian_2d,
    fluid::solve_simplicial_mini_stokes_2d_with_boundary,
    fluid::transient_navier_stokes_fieldwise_requirements_2d,
    fluid::transient_navier_stokes_mini_plan_2d,
};

const SOURCE: &str =
    include_str!("../../../verify/fluid/canonical-inlet-outlet-navier-stokes-2d/models/direct.eqi");
const HOMOGENEOUS_SOURCE: &str =
    include_str!("../../../verify/fluid/fixed-domain-transient-navier-stokes-2d/models/direct.eqi");

const OUTLET_RELATION: &str = r#"  relation outlet_traction continuous on x_upper {
    normal(
      2 * dynamic_viscosity * symmetric_part(grad(velocity))
      - isotropic_lift(pressure)
    ) = 0;
  }"#;
const LOWER_WALL: &str = "  relation lower_wall continuous on y_lower { trace(velocity) = 0; }";
const X_UPPER_TRACE: &str =
    "  relation outlet_traction continuous on x_upper { trace(velocity) = 0; }";

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
fn canonical_run_admits_nonzero_inlet_and_traction_outlet_without_gauge() {
    let document = compile("inlet-outlet.eqi", SOURCE);
    let model = lower_transient_incompressible_navier_stokes_cartesian_2d(document.program())
        .expect("mixed transient boundary lowers");
    let inlet = model
        .boundary_inventory()
        .boundary(0, eqiora::kernel::BoundarySide::Lower)
        .expect("inlet side");
    assert!(matches!(
        inlet.disposition(),
        PhysicalBoundaryDisposition::Prescribed(law)
            if law.quantity() == PhysicalBoundaryQuantity::Trace
    ));
    let outlet = model
        .boundary_inventory()
        .boundary(0, eqiora::kernel::BoundarySide::Upper)
        .expect("outlet side");
    assert_eq!(outlet.disposition(), PhysicalBoundaryDisposition::FluxZero);

    let mesh = unit_square_triangles(2);
    let envelope = SimplicialMeshEnvelopeV1::from_mesh(&mesh).expect("mesh envelope");
    let reference = prepare(document.program(), envelope);
    assert!(
        reference
            .realization()
            .plan()
            .fieldwise()
            .spatial()
            .constraints()
            .is_empty()
    );
    assert!(!has_gauge_block(reference.realization_graph()));

    let boundary = inlet_outlet_boundary(&mesh);
    let steady = solve_simplicial_mini_stokes_2d_with_boundary(
        &mesh,
        0.05,
        &zero_force,
        &boundary,
        &inlet_velocity,
        &triangle_duffy_gauss_legendre(3).unwrap(),
        &simplex_centroid_rule(1).unwrap(),
        LinearSolveRequest::new(&REFERENCE_LINEAR_SOLVER, stokes_solver()),
    )
    .expect("mixed steady state initializes the transient run");
    assert_eq!(
        steady.pressure_reference(),
        SimplicialMiniStokesPressureReference2d::BoundaryTraction
    );
    let initial = reference
        .initial_condition(
            DynQuantity::new(0.0, TIME),
            steady.velocity().clone(),
            steady.pressure().clone(),
            SteadyStokesPressureReference2d::BoundaryTraction,
        )
        .expect("boundary-determined initial pressure");
    let trajectory = reference
        .advance(initial, NonZeroUsize::MIN)
        .expect("canonical inlet/outlet run advances");

    assert_eq!(
        trajectory.states().last().unwrap().pressure_reference(),
        SteadyStokesPressureReference2d::BoundaryTraction
    );
    let inlet_midpoint = mesh
        .vertices()
        .iter()
        .position(|point| point == &[0.0, 0.5])
        .expect("mesh owns inlet midpoint");
    assert!(
        trajectory
            .states()
            .last()
            .unwrap()
            .velocity()
            .vertex_values()[inlet_midpoint][0]
            > 0.09
    );
    assert!(trajectory.steps()[0].assembly_report().packet_count() > 0);
}

#[test]
fn open_boundary_convective_identity_accepts_ordinary_positive_inertia() {
    let source = SOURCE.replace(
        "parameter density: kg / m ^ 3 = 0.0000000001;",
        "parameter density: kg / m ^ 3 = 1;",
    );
    assert_ne!(source, SOURCE, "density witness replacement must apply");
    let document = compile("ordinary-inertia-inlet-outlet.eqi", &source);
    let mesh = unit_square_triangles(2);
    let envelope = SimplicialMeshEnvelopeV1::from_mesh(&mesh).expect("mesh envelope");
    let reference = prepare(document.program(), envelope);
    let steady = solve_simplicial_mini_stokes_2d_with_boundary(
        &mesh,
        0.05,
        &zero_force,
        &inlet_outlet_boundary(&mesh),
        &inlet_velocity,
        &triangle_duffy_gauss_legendre(3).unwrap(),
        &simplex_centroid_rule(1).unwrap(),
        LinearSolveRequest::new(&REFERENCE_LINEAR_SOLVER, stokes_solver()),
    )
    .expect("steady mixed state initializes the ordinary-inertia run");
    let initial = reference
        .initial_condition(
            DynQuantity::new(0.0, TIME),
            steady.velocity().clone(),
            steady.pressure().clone(),
            SteadyStokesPressureReference2d::BoundaryTraction,
        )
        .expect("boundary-determined initial pressure");
    let trajectory = reference
        .advance(initial, NonZeroUsize::MIN)
        .expect("open-boundary identity admits ordinary positive inertia");
    let evidence = &trajectory.steps()[0];
    assert!(evidence.convective_residual_norm() > 1.0e-6);
    assert!(evidence.conservative_advection_defect_norm() > 1.0e-6);
    assert!(evidence.convective_power().abs() < 1.0e-12);
}

#[test]
fn all_essential_regime_retains_and_uses_the_pressure_gauge() {
    let document = compile("homogeneous.eqi", HOMOGENEOUS_SOURCE);
    let mesh = unit_square_triangles(2);
    let envelope = SimplicialMeshEnvelopeV1::from_mesh(&mesh).expect("mesh envelope");
    let reference = prepare(document.program(), envelope);
    assert_eq!(
        reference
            .realization()
            .plan()
            .fieldwise()
            .spatial()
            .constraints(),
        [AlgebraicConstraint::ZeroIntegral {
            field: reference.model().pressure().downcast().unwrap(),
        }]
    );
    assert!(has_gauge_block(reference.realization_graph()));

    let initial = reference
        .initial_condition(
            DynQuantity::new(0.0, TIME),
            zero_velocity(&mesh),
            zero_pressure(&mesh),
            SteadyStokesPressureReference2d::ZeroIntegral { multiplier: 0.0 },
        )
        .expect("zero gauge state");
    let trajectory = reference
        .advance(initial, NonZeroUsize::MIN)
        .expect("gauge regime advances");
    assert!(matches!(
        trajectory.states().last().unwrap().pressure_reference(),
        SteadyStokesPressureReference2d::ZeroIntegral { multiplier: 0.0 }
    ));
}

#[test]
fn nonzero_net_parent_outward_flux_is_rejected_before_assembly() {
    let source = SOURCE.replace(OUTLET_RELATION, X_UPPER_TRACE);
    let document = compile("incompatible-flux.eqi", &source);
    let model = lower_transient_incompressible_navier_stokes_cartesian_2d(document.program())
        .expect("all-essential prescribed trace lowers");
    let mesh = unit_square_triangles(2);
    let envelope = SimplicialMeshEnvelopeV1::from_mesh(&mesh).expect("mesh envelope");
    let resolved = resolve(document.program(), &model, &envelope);
    let initial = TransientNavierStokesInitialState2d::new(
        &model,
        DynQuantity::new(0.0, TIME),
        envelope.artifact_reference().unwrap(),
        zero_velocity(&mesh),
        zero_pressure(&mesh),
        SteadyStokesPressureReference2d::ZeroIntegral { multiplier: 0.0 },
    )
    .unwrap();
    let diagnostic = advance_resolved_transient_navier_stokes_mini_2d_with_assembly(
        document.program(),
        &resolved,
        &envelope,
        initial,
        TransientNavierStokesRun2d::new(NonZeroStepCount::new(NonZeroUsize::MIN)),
        &RejectUnexpectedAssembly,
        &FaerLinearSolver,
    )
    .expect_err("incompatible prescribed flux must fail");
    assert!(
        diagnostic
            .message()
            .contains("non-zero net parent-outward flux"),
        "unexpected diagnostic: {diagnostic}"
    );
    assert!(!diagnostic.message().contains("reached assembly"));
}

#[test]
fn all_traction_boundary_is_rejected_as_translation_indeterminate() {
    let mut source = HOMOGENEOUS_SOURCE.to_owned();
    for (old, name, boundary) in [
        (
            "  relation x_lower_value continuous on x_lower { trace(velocity) = 0; }",
            "x_lower_value",
            "x_lower",
        ),
        (
            "  relation x_upper_value continuous on x_upper { trace(velocity) = 0; }",
            "x_upper_value",
            "x_upper",
        ),
        (
            "  relation y_lower_value continuous on y_lower { trace(velocity) = 0; }",
            "y_lower_value",
            "y_lower",
        ),
        (
            "  relation y_upper_value continuous on y_upper { trace(velocity) = 0; }",
            "y_upper_value",
            "y_upper",
        ),
    ] {
        source = source.replace(old, &zero_traction_relation(name, boundary));
    }
    let document = compile("all-traction.eqi", &source);
    let model = lower_transient_incompressible_navier_stokes_cartesian_2d(document.program())
        .expect("all-traction meaning lowers");
    let diagnostic = transient_navier_stokes_mini_plan_2d(
        &model,
        eqiora::realization::MeshArtifactReference::from_sha256([0x91; 32]),
        scales(),
        DynQuantity::new(0.01, TIME),
        nonlinear(),
        navier_stokes_solver(),
    )
    .expect_err("all traction must not select a realization");
    assert!(
        diagnostic
            .message()
            .contains("velocity is otherwise determined only up to a constant"),
        "unexpected diagnostic: {diagnostic}"
    );
}

#[test]
fn incomplete_and_overlapping_boundary_conditions_are_rejected() {
    let incomplete = compile("incomplete.eqi", &SOURCE.replace(LOWER_WALL, ""));
    let diagnostic =
        lower_transient_incompressible_navier_stokes_cartesian_2d(incomplete.program())
            .expect_err("uncovered side must fail");
    assert!(diagnostic.message().contains("boundary"));

    let overlapping_source = SOURCE.replace(
        OUTLET_RELATION,
        &format!(
            "  relation duplicate_inlet continuous on x_lower {{ trace(velocity) = 0; }}\n{OUTLET_RELATION}"
        ),
    );
    let overlapping = compile("overlapping.eqi", &overlapping_source);
    let diagnostic =
        lower_transient_incompressible_navier_stokes_cartesian_2d(overlapping.program())
            .expect_err("two conditions on one side must fail");
    assert!(diagnostic.message().contains("ambiguous"));
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
            "incompatible flux check reached assembly unexpectedly",
        ))
    }
}

fn compile(name: &str, source: &str) -> eqiora::api::ModelDocument {
    ExactModelCodec::V5
        .compile(name, source)
        .expect("transient source compiles")
}

fn zero_traction_relation(name: &str, boundary: &str) -> String {
    format!(
        "  relation {name} continuous on {boundary} {{\n    normal(\n      2 * dynamic_viscosity * symmetric_part(grad(velocity))\n      - isotropic_lift(pressure)\n    ) = 0;\n  }}"
    )
}

fn prepare(
    program: &eqiora::sem::KernelProgram,
    mesh: SimplicialMeshEnvelopeV1,
) -> TransientNavierStokesReference2d {
    TransientNavierStokesReference2d::prepare(
        program,
        mesh,
        scales(),
        DynQuantity::new(0.01, TIME),
        nonlinear(),
        navier_stokes_solver(),
        RealizationRevision::new(91),
        FaerLinearSolver,
    )
    .expect("reference prepares")
}

fn resolve(
    program: &eqiora::sem::KernelProgram,
    model: &eqiora_numerics::fluid::TransientIncompressibleNavierStokesCartesianModel2d,
    mesh: &SimplicialMeshEnvelopeV1,
) -> eqiora::realization::ResolvedTransientFieldwiseRealization {
    let solver = navier_stokes_solver();
    let plan = transient_navier_stokes_mini_plan_2d(
        model,
        mesh.artifact_reference().unwrap(),
        scales(),
        DynQuantity::new(0.01, TIME),
        nonlinear(),
        solver,
    )
    .unwrap();
    let selected_solver = SolverCapabilities::exact([SolverCapability {
        algorithm: solver.algorithm(),
        operator_properties: LinearOperatorProperties::General,
        preconditioner: solver.preconditioner(),
        reduction: solver.reduction(),
        scalar_type: ScalarType::F64,
    }])
    .unwrap();
    let capabilities = RealizationCapabilities::cartesian_product(
        [DiscretizationMethod::ContinuousGalerkin],
        [(
            MeshKind::ImportedAffineSimplicial,
            SpatialDimensionSupport::exact(NonZeroUsize::new(2).unwrap()),
        )],
        [VectorLayoutKind::Replicated],
        selected_solver,
        TargetCapabilities::none().with_host_cpu(NonZeroUsize::MIN),
    )
    .unwrap();
    resolve_transient_fieldwise(
        &TransientFieldwiseRealizationRequest::explicit(
            program.model(),
            SemanticRevision::new(program.revision().0),
            RealizationRevision::new(91),
            plan,
        ),
        transient_navier_stokes_fieldwise_requirements_2d(model),
        &capabilities,
    )
    .unwrap()
}

fn has_gauge_block(graph: &eqiora::realization::PortableRealizationGraph) -> bool {
    graph.systems().iter().any(|system| {
        system
            .blocks()
            .iter()
            .any(|block| matches!(block, SystemBlock::ConstraintMultiplier(_)))
    })
}

fn scales() -> IncompressibleFlowScaleProfile2d {
    IncompressibleFlowScaleProfile2d::new(
        DynQuantity::new(1.0, LENGTH),
        DynQuantity::new(1.0, VELOCITY),
        DynQuantity::new(1.0, PRESSURE),
    )
    .unwrap()
}

fn nonlinear() -> NonlinearSolvePlan {
    NonlinearSolvePlan::new(1.0e-9, 1.0e-11, NonZeroUsize::new(16).unwrap(), 12).unwrap()
}

fn navier_stokes_solver() -> SolverPlan {
    SolverPlan::new(
        LinearSolver::BiConjugateGradientStabilized,
        1.0e-11,
        1.0e-12,
        NonZeroUsize::new(2000).unwrap(),
    )
    .unwrap()
    .with_reduction(ReductionPolicy::Fast)
}

fn stokes_solver() -> SolverPlan {
    SolverPlan::new(
        LinearSolver::MinimumResidual,
        1.0e-11,
        1.0e-12,
        NonZeroUsize::new(2000).unwrap(),
    )
    .unwrap()
}

fn inlet_outlet_boundary(mesh: &SimplicialMesh) -> SimplicialMiniStokesBoundary2d {
    let facets = (0..mesh.entity_count(1).unwrap()).filter_map(|index| {
        let facet = MeshEntity::new(1, index);
        mesh.is_boundary_entity(facet).unwrap().then(|| {
            let on_outlet = mesh
                .entity_vertices(facet)
                .unwrap()
                .iter()
                .all(|vertex| mesh.vertices()[vertex.index()][0] == 1.0);
            let condition = if on_outlet {
                SimplicialMiniStokesBoundaryCondition2d::ConstantTraction { value: [0.0; 2] }
            } else {
                SimplicialMiniStokesBoundaryCondition2d::EssentialVelocity
            };
            SimplicialMiniStokesBoundaryFacet2d::new(facet, condition)
        })
    });
    SimplicialMiniStokesBoundary2d::new(mesh, facets).unwrap()
}

fn inlet_velocity(point: [f64; 2]) -> Result<[f64; 2], eqiora::Diagnostic> {
    Ok(if point[0] == 0.0 {
        [0.4 * point[1] * (1.0 - point[1]), 0.0]
    } else {
        [0.0; 2]
    })
}

fn zero_force(_: [f64; 2]) -> Result<[f64; 2], eqiora::Diagnostic> {
    Ok([0.0; 2])
}

fn zero_velocity(mesh: &SimplicialMesh) -> SimplicialMiniVelocityField2d {
    SimplicialMiniVelocityField2d::new(
        mesh.clone(),
        vec![[0.0; 2]; mesh.vertices().len()],
        vec![[0.0; 2]; mesh.entity_count(2).unwrap()],
    )
    .unwrap()
}

fn zero_pressure(mesh: &SimplicialMesh) -> SimplicialP1Field {
    SimplicialP1Field::new(mesh.clone(), vec![0.0; mesh.vertices().len()]).unwrap()
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
