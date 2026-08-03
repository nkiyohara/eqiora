#![cfg(feature = "faer")]

use std::num::NonZeroUsize;

use eqiora::api::TransientNavierStokesReference2d;
use eqiora::artifact::SimplicialMeshEnvelopeV1;
use eqiora::backends::faer::FaerLinearSolver;
use eqiora::meshing::{MeshQualityGate, MeshTopology, SimplicialMesh};
use eqiora::realization::{
    DiscretizationMethod, MeshKind, NonlinearSolvePlan, RealizationCapabilities,
    RealizationRevision, SemanticRevision, SpatialDimensionSupport, TargetCapabilities,
    TransientCellCenteredIncompressibleFlowCapabilities,
    TransientCellCenteredIncompressibleFlowRealizationRequest, VectorLayoutKind,
    resolve_transient_cell_centered_incompressible_flow,
};
use eqiora::sem::KernelProgram;
use eqiora::solver::{
    LinearOperatorProperties, LinearSolver, PreconditionerPolicy, REFERENCE_LINEAR_SOLVER,
    ReductionPolicy, ScalarType, SolverCapabilities, SolverCapability, SolverPlan,
};
use eqiora::{DimExponents, DynQuantity};
use eqiora_meshing::CartesianMesh;
use eqiora_numerics::{
    common::NonZeroStepCount, common::SimplicialP1Field,
    fluid::CellCenteredNavierStokesInitialState2d, fluid::CellCenteredPressureField2d,
    fluid::CellCenteredVelocityField2d, fluid::IncompressibleFlowScaleProfile2d,
    fluid::ResolvedCellCenteredNavierStokesTrajectory2d,
    fluid::ResolvedTransientNavierStokesTrajectory2d, fluid::SimplicialMiniVelocityField2d,
    fluid::SteadyStokesPressureReference2d, fluid::TransientNavierStokesRun2d,
    fluid::advance_resolved_transient_navier_stokes_cell_centered_2d,
    fluid::lower_transient_incompressible_navier_stokes_cartesian_2d,
    fluid::transient_navier_stokes_cell_centered_plan_2d,
    fluid::transient_navier_stokes_cell_centered_requirements_2d,
};

const SOURCE: &str = include_str!(
    "../../../verify/fluid/same-program-fem-fvm-navier-stokes-2d/models/hydrostatic.eqi"
);
const CELLS_PER_AXIS: usize = 4;
const TIME_STEP: f64 = 0.01;

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
fn one_borrowed_program_drives_fem_and_fvm_to_the_same_affine_pressure_equilibrium() {
    let document = eqiora::api::ModelDocument::compile(
        "verify/fluid/same-program-fem-fvm-navier-stokes-2d/models/hydrostatic.eqi",
        SOURCE,
    )
    .unwrap();
    let program = document.program();
    let model = lower_transient_incompressible_navier_stokes_cartesian_2d(program).unwrap();
    assert_eq!(
        model.conservative_body_force(&[0.25, 0.75]).unwrap(),
        [1.0, 0.0]
    );

    let fem = run_fem(program);
    let fvm = run_fvm(program);

    assert_eq!(fem.model(), &model);
    assert_eq!(fvm.model(), &model);
    let semantic_revision = SemanticRevision::new(program.revision().0);
    assert_eq!(fem.realization().model(), program.model());
    assert_eq!(fvm.realization().model(), program.model());
    assert_eq!(fem.realization().semantic_revision(), semantic_revision);
    assert_eq!(fvm.realization().semantic_revision(), semantic_revision);
    assert_ne!(
        fem.realization().realization_revision(),
        fvm.realization().realization_revision()
    );
    assert_eq!(
        fem.model().boundary_inventory(),
        fvm.model().boundary_inventory()
    );
    assert_eq!(
        fem.realization()
            .plan()
            .fieldwise()
            .spatial()
            .discretization()
            .method(),
        DiscretizationMethod::ContinuousGalerkin
    );
    assert_eq!(
        fvm.realization()
            .plan()
            .fieldwise()
            .spatial()
            .discretization()
            .method(),
        DiscretizationMethod::CellCenteredFiniteVolume
    );
    assert_ne!(
        fem.realization()
            .plan()
            .fieldwise()
            .spatial()
            .field_spaces(),
        fvm.realization()
            .plan()
            .fieldwise()
            .spatial()
            .field_spaces()
    );
    assert_eq!(fem.states().len(), 2);
    assert_eq!(fvm.states().len(), 2);
    assert!(fem.steps()[0].initial_residual_norm() > 1.0e-6);
    assert!(fvm.steps()[0].initial_residual_norm() > 1.0e-6);
    assert!(fem.steps()[0].nonlinear_iterations() > 0);
    assert!(fvm.steps()[0].iterations() > 0);

    let fem_state = fem.states().last().unwrap();
    let fvm_state = fvm.states().last().unwrap();
    assert_eq!(fem_state.velocity_field(), fvm_state.velocity_field());
    assert_eq!(fem_state.pressure_field(), fvm_state.pressure_field());
    assert_eq!(fem_state.time(), fvm_state.time());
    assert_eq!(
        fem.realization().plan().time_step().duration(),
        DynQuantity::new(TIME_STEP, TIME)
    );
    assert_eq!(
        fvm.realization().plan().time_step().duration(),
        DynQuantity::new(TIME_STEP, TIME)
    );

    let fem_velocity_maximum = fem_state
        .velocity()
        .vertex_values()
        .iter()
        .chain(fem_state.velocity().cell_bubble_values())
        .flatten()
        .map(|value| value.abs())
        .fold(0.0, f64::max);
    let fvm_velocity_maximum = fvm_state
        .velocity()
        .values()
        .iter()
        .flatten()
        .map(|value| value.abs())
        .fold(0.0, f64::max);
    assert!(
        fem_velocity_maximum < 2.0e-9,
        "FEM equilibrium velocity was {fem_velocity_maximum:e}"
    );
    assert!(
        fvm_velocity_maximum < 2.0e-9,
        "FVM equilibrium velocity was {fvm_velocity_maximum:e}"
    );

    let fem_pressure = fem_state.pressure().vertex_values();
    let maximum_fem_vertex_oracle_defect = fem_state
        .pressure()
        .mesh()
        .vertices()
        .iter()
        .zip(fem_pressure)
        .map(|(point, pressure)| (pressure - (point[0] - 0.5)).abs())
        .fold(0.0_f64, f64::max);
    assert!(
        maximum_fem_vertex_oracle_defect < 2.0e-9,
        "FEM vertex affine-pressure defect was {maximum_fem_vertex_oracle_defect:e}"
    );
    let fvm_mesh = fvm_state.pressure().mesh();
    let fvm_pressure = fvm_state.pressure().values();
    let vertex_width = CELLS_PER_AXIS + 1;
    let mut maximum_fem_oracle_defect = 0.0_f64;
    let mut maximum_fvm_oracle_defect = 0.0_f64;
    let mut maximum_cross_method_defect = 0.0_f64;
    for y in 0..CELLS_PER_AXIS {
        for x in 0..CELLS_PER_AXIS {
            let cell = fvm_mesh.cell_at(&[x, y]).unwrap().index();
            let lower_left = y * vertex_width + x;
            let upper_right = (y + 1) * vertex_width + x + 1;
            let fem_at_center = 0.5 * (fem_pressure[lower_left] + fem_pressure[upper_right]);
            let exact = (x as f64 + 0.5) / CELLS_PER_AXIS as f64 - 0.5;
            maximum_fem_oracle_defect =
                maximum_fem_oracle_defect.max((fem_at_center - exact).abs());
            maximum_fvm_oracle_defect =
                maximum_fvm_oracle_defect.max((fvm_pressure[cell] - exact).abs());
            maximum_cross_method_defect =
                maximum_cross_method_defect.max((fem_at_center - fvm_pressure[cell]).abs());
        }
    }
    assert!(
        maximum_fem_oracle_defect < 2.0e-9,
        "FEM affine-pressure defect was {maximum_fem_oracle_defect:e}"
    );
    assert!(
        maximum_fvm_oracle_defect < 2.0e-9,
        "FVM affine-pressure defect was {maximum_fvm_oracle_defect:e}"
    );
    assert!(
        maximum_cross_method_defect < 2.0e-9,
        "FEM/FVM common-cell pressure defect was {maximum_cross_method_defect:e}"
    );
    assert!(
        fvm_pressure
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max)
            - fvm_pressure.iter().copied().fold(f64::INFINITY, f64::min)
            > 0.7
    );
}

fn run_fem(program: &KernelProgram) -> ResolvedTransientNavierStokesTrajectory2d {
    let mesh = unit_square_triangles(CELLS_PER_AXIS);
    let cell_count = mesh.entity_count(2).unwrap();
    let velocity = SimplicialMiniVelocityField2d::new(
        mesh.clone(),
        vec![[0.0; 2]; mesh.vertices().len()],
        vec![[0.0; 2]; cell_count],
    )
    .unwrap();
    let pressure = SimplicialP1Field::new(mesh.clone(), vec![0.0; mesh.vertices().len()]).unwrap();
    let reference = TransientNavierStokesReference2d::prepare(
        program,
        SimplicialMeshEnvelopeV1::from_mesh(&mesh).unwrap(),
        scales(),
        DynQuantity::new(TIME_STEP, TIME),
        nonlinear_plan(),
        linear_plan(ReductionPolicy::Fast),
        RealizationRevision::new(1),
        FaerLinearSolver,
    )
    .unwrap();
    let initial = reference
        .initial_condition(
            DynQuantity::new(0.0, TIME),
            velocity,
            pressure,
            SteadyStokesPressureReference2d::ZeroIntegral { multiplier: 0.0 },
        )
        .unwrap();
    reference.advance(initial, NonZeroUsize::MIN).unwrap()
}

fn run_fvm(program: &KernelProgram) -> ResolvedCellCenteredNavierStokesTrajectory2d {
    let model = lower_transient_incompressible_navier_stokes_cartesian_2d(program).unwrap();
    let plan = transient_navier_stokes_cell_centered_plan_2d(
        &model,
        NonZeroUsize::new(CELLS_PER_AXIS).unwrap(),
        scales(),
        DynQuantity::new(TIME_STEP, TIME),
        nonlinear_plan(),
        linear_plan(ReductionPolicy::Reproducible),
    )
    .unwrap();
    let request = TransientCellCenteredIncompressibleFlowRealizationRequest::explicit(
        program.model(),
        SemanticRevision::new(program.revision().0),
        RealizationRevision::new(2),
        plan,
    );
    let resolved = resolve_transient_cell_centered_incompressible_flow(
        &request,
        transient_navier_stokes_cell_centered_requirements_2d(&model),
        &fvm_capabilities(),
    )
    .unwrap();
    let mesh = CartesianMesh::uniform(model.bounds(), &[CELLS_PER_AXIS; 2]).unwrap();
    let initial = CellCenteredNavierStokesInitialState2d::new(
        &model,
        DynQuantity::new(0.0, TIME),
        CellCenteredVelocityField2d::new(
            mesh.clone(),
            vec![[0.0; 2]; CELLS_PER_AXIS * CELLS_PER_AXIS],
        )
        .unwrap(),
        CellCenteredPressureField2d::new(mesh, vec![0.0; CELLS_PER_AXIS * CELLS_PER_AXIS]).unwrap(),
        0.0,
    )
    .unwrap();
    advance_resolved_transient_navier_stokes_cell_centered_2d(
        program,
        &resolved,
        initial,
        TransientNavierStokesRun2d::new(NonZeroStepCount::new(NonZeroUsize::MIN)),
        &REFERENCE_LINEAR_SOLVER,
    )
    .unwrap()
}

fn scales() -> IncompressibleFlowScaleProfile2d {
    IncompressibleFlowScaleProfile2d::new(
        DynQuantity::new(1.0, LENGTH),
        DynQuantity::new(1.0, VELOCITY),
        DynQuantity::new(1.0, PRESSURE),
    )
    .unwrap()
}

fn nonlinear_plan() -> NonlinearSolvePlan {
    NonlinearSolvePlan::new(1.0e-10, 1.0e-12, NonZeroUsize::new(16).unwrap(), 12).unwrap()
}

fn linear_plan(reduction: ReductionPolicy) -> SolverPlan {
    SolverPlan::new(
        LinearSolver::BiConjugateGradientStabilized,
        1.0e-12,
        1.0e-13,
        NonZeroUsize::new(4_000).unwrap(),
    )
    .unwrap()
    .with_reduction(reduction)
}

fn fvm_capabilities() -> TransientCellCenteredIncompressibleFlowCapabilities {
    let solver = SolverCapabilities::exact([SolverCapability {
        algorithm: LinearSolver::BiConjugateGradientStabilized,
        operator_properties: LinearOperatorProperties::General,
        preconditioner: PreconditionerPolicy::Identity,
        reduction: ReductionPolicy::Reproducible,
        scalar_type: ScalarType::F64,
    }])
    .unwrap();
    TransientCellCenteredIncompressibleFlowCapabilities::new(
        RealizationCapabilities::cartesian_product(
            [DiscretizationMethod::CellCenteredFiniteVolume],
            [(
                MeshKind::GeneratedCartesian,
                SpatialDimensionSupport::exact(NonZeroUsize::new(2).unwrap()),
            )],
            [VectorLayoutKind::Replicated],
            solver,
            TargetCapabilities::none().with_host_cpu(NonZeroUsize::MIN),
        )
        .unwrap(),
    )
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
