use std::collections::{BTreeSet, HashSet};
use std::num::{NonZeroU16, NonZeroUsize};
use std::sync::atomic::{AtomicU8, Ordering};

use eqiora_artifact::AcceptedCircularHoleChordalRealizationV1;
use eqiora_assembly::{
    AssemblyBackend, AssemblyPlan, AssemblyResult, AssemblyWork, REFERENCE_ASSEMBLY_BACKEND,
};
use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, DimExponents, DynQuantity};
use eqiora_geometry::{CanonicalGeometryV1, EDGE_DIMENSION, FACE_DIMENSION, NamedEntitySet};
use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora_meshing::{MeshEntity, MeshQualityGate, MeshTopology};
use eqiora_realization::{
    DiscretizationMethod, DomainConfiguration, ExecutionSchedule, MeshKind, MeshPolicy,
    NonlinearSolvePlan, PlacementRequirementNode, QuadraturePolicy, RealizationCapabilities,
    RealizationRevision, SemanticRevision, SolveRoot, Space, SpatialDimensionSupport, SystemBlock,
    TargetCapabilities, TransformationNode, TransientFieldwiseRealizationRequest, VectorLayoutKind,
    resolve_transient_fieldwise,
};
use eqiora_schema::ModelView;
use eqiora_schema::kernel::{BoundarySide, DomainDef, DomainKind, GeometryDigest, KernelNode};
use eqiora_sem::KernelProgram;
use eqiora_solver::{
    BackendId, ConvergenceReason, ExecutionReport, LinearOperator, LinearOperatorProperties,
    LinearProblem, LinearSolution, LinearSolveRequest, LinearSolver, LinearSolverBackend,
    PreconditionerPolicy, REFERENCE_LINEAR_SOLVER, ReductionPolicy, ReplicatedLinearExecution,
    ScalarType, SolverCapabilities, SolverCapability, SolverPlan, SolverProvider,
    accept_linear_solution_with_execution,
};

use super::TransientNavierStokesGeometryBinding2d;
use crate::canonical_stokes::{
    IncompressibleFlowScaleProfile2d, lower_transient_incompressible_navier_stokes_cartesian_2d,
};
use crate::simplicial_elliptic::SimplicialP1Field;
use crate::simplicial_navier_stokes::SimplicialMiniNavierStokesState2d;
use crate::simplicial_stokes::{
    SimplicialMiniStokesBoundary2d, SimplicialMiniStokesBoundaryCondition2d,
    SimplicialMiniStokesBoundaryFacet2d, SimplicialMiniStokesPressureReference2d,
    SimplicialMiniVelocityField2d, solve_simplicial_mini_stokes_2d_with_boundary,
};
use crate::step_count::NonZeroStepCount;

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
const BOUNDARY_NAMES: [&str; 4] = ["inlet", "outlet", "walls", "cylinder"];

const SOURCE: &str = r#"
public pure operator outer_product(left: spatial[1], right: spatial[1]) -> spatial[2]
  = component(left, 0) * component(right, 1);

model Main {
  domain body = box(0, 2.2, 0, 0.41);
  domain x_lower = boundary(body, axis = 0, side = lower);
  domain x_upper = boundary(body, axis = 0, side = upper);
  domain y_lower = boundary(body, axis = 1, side = lower);
  domain y_upper = boundary(body, axis = 1, side = upper);
  representation space = continuum;

  field velocity on body as space: m / s shape spatial_vector;
  field pressure on body as space: kg / (m * s ^ 2) = 0;
  field force_potential on body as space: kg / (m * s ^ 2) = 0;
  parameter density: kg / m ^ 3 = 1;
  parameter dynamic_viscosity: kg / (m * s) = 0.05;
  parameter zero_pressure: kg / (m * s ^ 2) = 0;

  relation force_definition continuous on body {
    force_potential - zero_pressure = 0;
  }
  relation momentum continuous on body {
    density * derivative(velocity)
      + div(density * outer_product(velocity, velocity))
      - div(
        2 * dynamic_viscosity * symmetric_part(grad(velocity))
        - isotropic_lift(pressure)
      ) - grad(force_potential) = 0;
  }
  relation incompressibility continuous on body { div(velocity) = 0; }

  relation inlet_velocity continuous on x_lower { trace(velocity) = 0; }
  relation outlet_traction continuous on x_upper {
    normal(
      2 * dynamic_viscosity * symmetric_part(grad(velocity))
      - isotropic_lift(pressure)
    ) = 0;
  }
  relation lower_wall continuous on y_lower { trace(velocity) = 0; }
  relation cylinder_wall continuous on y_upper { trace(velocity) = 0; }
}
"#;

#[test]
fn source_bound_positive_executes_exact_zero_step() {
    let source = exact_source();
    let owner = owner(&source, 1.0e-4, 50, 1.0e-5);
    let program = geometry_program(&source, SOURCE, BOUNDARY_NAMES);

    let cartesian_error = lower_transient_incompressible_navier_stokes_cartesian_2d(&program)
        .expect_err("the public Cartesian lowerer must reject GeometryRegion");
    assert_eq!(cartesian_error.code(), codes::INVALID_SPATIAL_LOWERING);
    assert_model_digest(&program, source.digest_bytes());
    assert_eq!(owner.source(), &source);
    owner.revalidate().expect("accepted owner replays");
    assert_eq!(
        owner.correspondence().mesh_artifact(),
        owner.mesh().digest().expect("mesh digest")
    );
    assert_named_partition(&owner);

    let binding = TransientNavierStokesGeometryBinding2d::new(&program, owner.clone())
        .expect("exact source-bound transient binding");
    let requirements = binding.fieldwise_requirements();
    let plan = binding
        .mini_plan(scales(), time_step(), nonlinear_plan(), solver_plan())
        .expect("bounded transient MINI/P1 plan");
    let mesh_reference = owner
        .mesh()
        .artifact_reference()
        .expect("exact mesh identity");
    assert!(plan.fieldwise().spatial().constraints().is_empty());
    assert_eq!(
        plan.fieldwise().spatial().discretization().mesh(),
        MeshPolicy::ImportedSimplicial {
            artifact: mesh_reference
        }
    );
    let resolved = resolve_transient_fieldwise(
        &TransientFieldwiseRealizationRequest::explicit(
            program.model(),
            SemanticRevision::new(program.revision().0),
            RealizationRevision::new(165),
            plan.clone(),
        ),
        requirements.clone(),
        &capabilities(),
    )
    .expect("the exact reference tuple resolves");
    assert_exact_graph(&resolved, &requirements, mesh_reference);

    let mesh = owner.mesh().mesh().clone();
    let initial = exact_zero_state(&mesh);
    let trajectory = binding
        .advance_with_assembly(
            &program,
            &resolved,
            initial,
            NonZeroStepCount::new(NonZeroUsize::MIN),
            &REFERENCE_ASSEMBLY_BACKEND,
            &REFERENCE_LINEAR_SOLVER,
        )
        .expect("one exact-zero step executes through checked assembly");
    assert_eq!(trajectory.states().len(), 2);
    assert_eq!(trajectory.steps().len(), 1);
    assert_eq!(trajectory.states()[0].time(), 0.0);
    assert_eq!(trajectory.states()[1].time(), time_step().value());
    for state in trajectory.states() {
        assert_eq!(
            state.pressure_reference(),
            SimplicialMiniStokesPressureReference2d::BoundaryTraction
        );
        assert!(
            state
                .velocity()
                .vertex_values()
                .iter()
                .flatten()
                .all(|value| value.is_finite() && *value == 0.0)
        );
        assert!(
            state
                .velocity()
                .cell_bubble_values()
                .iter()
                .flatten()
                .all(|value| value.is_finite() && *value == 0.0)
        );
        assert!(
            state
                .pressure()
                .vertex_values()
                .iter()
                .all(|value| value.is_finite() && *value == 0.0)
        );
    }
    let step = &trajectory.steps()[0];
    assert_eq!(step.final_residual_norm(), 0.0);
    assert_eq!(step.momentum_residual_norm(), 0.0);
    assert_eq!(step.continuity_residual_norm(), 0.0);
    assert_eq!(step.convective_residual_norm(), 0.0);
    assert_eq!(step.convective_power(), 0.0);
    assert_eq!(step.conservative_advection_defect_norm(), 0.0);
    assert!(step.assembly_report().packet_count() > 0);
}

#[test]
fn source_and_mesh_identity_mismatches_fail_before_materialization() {
    let source = exact_source();
    let accepted = owner(&source, 1.0e-4, 50, 1.0e-5);
    let cartesian = cartesian_program(SOURCE);
    let program = geometry_program_from_cartesian(&cartesian, &source, BOUNDARY_NAMES);
    let binding = TransientNavierStokesGeometryBinding2d::new(&program, accepted.clone()).unwrap();
    let resolved = resolve(&program, &binding);
    let initial = exact_zero_state(accepted.mesh().mesh());

    let foreign_source = circular_source(
        [0.21, 0.2],
        vec![
            named("fluid", FACE_DIMENSION, &[0]),
            named("walls", EDGE_DIMENSION, &[2, 3]),
            named("inlet", EDGE_DIMENSION, &[0]),
            named("cylinder", EDGE_DIMENSION, &[4]),
            named("outlet", EDGE_DIMENSION, &[1]),
        ],
    );
    let foreign_owner = owner(&foreign_source, 1.0e-4, 50, 1.0e-5);
    let foreign_program =
        geometry_program_from_cartesian(&cartesian, &foreign_source, BOUNDARY_NAMES);
    assert_eq!(foreign_program.model(), program.model());
    assert_eq!(foreign_program.revision(), program.revision());
    TransientNavierStokesGeometryBinding2d::new(&foreign_program, foreign_owner).unwrap();
    let source_error = binding
        .advance_with_assembly(
            &foreign_program,
            &resolved,
            initial.clone(),
            NonZeroStepCount::new(NonZeroUsize::MIN),
            &RejectAnyAssembly,
            &REFERENCE_LINEAR_SOLVER,
        )
        .expect_err("a second valid exact source must not cross the binding");
    assert_eq!(source_error.code(), codes::INVALID_REALIZATION);

    let other_owner = owner(&source, 0.2, 8, 1.0e-8);
    let other_binding =
        TransientNavierStokesGeometryBinding2d::new(&program, other_owner.clone()).unwrap();
    let other_resolved = resolve(&program, &other_binding);
    let mesh_error = binding
        .advance_with_assembly(
            &program,
            &other_resolved,
            initial,
            NonZeroStepCount::new(NonZeroUsize::MIN),
            &RejectAnyAssembly,
            &REFERENCE_LINEAR_SOLVER,
        )
        .expect_err("a second valid same-source mesh owner must not cross the binding");
    assert_eq!(mesh_error.code(), codes::INVALID_REALIZATION);
}

#[test]
fn authored_correspondence_names_and_partition_are_not_inferred() {
    for (source, names, reason) in [
        (
            circular_source(
                [0.2, 0.2],
                vec![
                    named("fluid", FACE_DIMENSION, &[0]),
                    named("outer", EDGE_DIMENSION, &[2, 3]),
                    named("left", EDGE_DIMENSION, &[0]),
                    named("hole", EDGE_DIMENSION, &[4]),
                    named("right", EDGE_DIMENSION, &[1]),
                ],
            ),
            ["left", "right", "outer", "hole"],
            "coordinate/topology relabel",
        ),
        (
            circular_source(
                [0.2, 0.2],
                vec![
                    named("fluid", FACE_DIMENSION, &[0]),
                    named("walls", EDGE_DIMENSION, &[2, 3, 4]),
                    named("inlet", EDGE_DIMENSION, &[0]),
                    named("outlet", EDGE_DIMENSION, &[1]),
                ],
            ),
            ["inlet", "outlet", "walls", "walls"],
            "omitted cylinder/default uncovered",
        ),
        (
            circular_source(
                [0.2, 0.2],
                vec![
                    named("fluid", FACE_DIMENSION, &[0]),
                    named("walls", EDGE_DIMENSION, &[2]),
                    named("inlet", EDGE_DIMENSION, &[0]),
                    named("cylinder", EDGE_DIMENSION, &[4]),
                    named("outlet", EDGE_DIMENSION, &[1]),
                ],
            ),
            BOUNDARY_NAMES,
            "uncovered exterior facet",
        ),
    ] {
        let accepted = owner(&source, 1.0e-4, 50, 1.0e-5);
        let program = geometry_program(&source, SOURCE, names);
        let error =
            TransientNavierStokesGeometryBinding2d::new(&program, accepted).expect_err(reason);
        assert_eq!(error.code(), codes::INVALID_REALIZATION, "{reason}");
    }

    let overlapping_source = circular_source(
        [0.2, 0.2],
        vec![
            named("fluid", FACE_DIMENSION, &[0]),
            named("walls", EDGE_DIMENSION, &[2, 3, 4]),
            named("inlet", EDGE_DIMENSION, &[0]),
            named("cylinder", EDGE_DIMENSION, &[4]),
            named("outlet", EDGE_DIMENSION, &[1]),
        ],
    );
    let overlap_error = AcceptedCircularHoleChordalRealizationV1::from_reference(
        &overlapping_source,
        1.0e-4,
        50,
        MeshQualityGate::new(1.0e-5).unwrap(),
    )
    .expect_err("the accepted owner rejects an ambiguous named facet before assembly");
    assert_eq!(overlap_error.code(), codes::INVALID_ARTIFACT);
    assert_eq!(
        overlap_error.message(),
        "mesh facet is ambiguous between region entity sets 'cylinder' and 'walls'"
    );
}

#[test]
fn exact_boundary_dispositions_and_closed_grammar_fail_closed() {
    let source = exact_source();
    let accepted = owner(&source, 1.0e-4, 50, 1.0e-5);
    let wrong_dispositions = SOURCE
        .replace(
            "relation outlet_traction continuous on x_upper {\n    normal(\n      2 * dynamic_viscosity * symmetric_part(grad(velocity))\n      - isotropic_lift(pressure)\n    ) = 0;\n  }",
            "relation outlet_traction continuous on x_upper { trace(velocity) = 0; }",
        )
        .replace(
            "relation cylinder_wall continuous on y_upper { trace(velocity) = 0; }",
            "relation cylinder_wall continuous on y_upper {\n    normal(\n      2 * dynamic_viscosity * symmetric_part(grad(velocity))\n      - isotropic_lift(pressure)\n    ) = 0;\n  }",
        );
    let wrong_program = geometry_program(&source, &wrong_dispositions, BOUNDARY_NAMES);
    let boundary_error =
        TransientNavierStokesGeometryBinding2d::new(&wrong_program, accepted.clone())
            .expect_err("outlet traction cannot become essential or migrate to cylinder");
    assert_eq!(boundary_error.code(), codes::INVALID_REALIZATION);

    let all_essential = SOURCE.replace(
        "relation outlet_traction continuous on x_upper {\n    normal(\n      2 * dynamic_viscosity * symmetric_part(grad(velocity))\n      - isotropic_lift(pressure)\n    ) = 0;\n  }",
        "relation outlet_traction continuous on x_upper { trace(velocity) = 0; }",
    );
    let all_essential_program = geometry_program(&source, &all_essential, BOUNDARY_NAMES);
    let gauge_error =
        TransientNavierStokesGeometryBinding2d::new(&all_essential_program, accepted.clone())
            .expect_err("the accepted path has BoundaryTraction and no gauge");
    assert_eq!(gauge_error.code(), codes::INVALID_REALIZATION);

    let extra = SOURCE.replace(
        "  representation space = continuum;",
        "  domain extra = boundary(body, axis = 0, side = lower);\n  representation space = continuum;",
    ).replace(
        "  relation inlet_velocity continuous on x_lower { trace(velocity) = 0; }",
        "  relation inlet_velocity continuous on x_lower { trace(velocity) = 0; }\n  relation extra_value continuous on extra { trace(velocity) = 0; }",
    );
    let extra_program = geometry_program(&source, &extra, BOUNDARY_NAMES);
    let extra_error = TransientNavierStokesGeometryBinding2d::new(&extra_program, accepted)
        .expect_err("an additional GeometryBoundary/Relation is not ignored");
    assert_eq!(extra_error.code(), codes::INVALID_SPATIAL_LOWERING);
}

#[test]
fn registered_nonbox_transient_oracle_executes_all_falsifiers() {
    source_bound_positive_executes_exact_zero_step();
    source_and_mesh_identity_mismatches_fail_before_materialization();
    authored_correspondence_names_and_partition_are_not_inferred();
    exact_boundary_dispositions_and_closed_grammar_fail_closed();
}

fn resolve(
    program: &KernelProgram,
    binding: &TransientNavierStokesGeometryBinding2d,
) -> eqiora_realization::ResolvedTransientFieldwiseRealization {
    let plan = binding
        .mini_plan(scales(), time_step(), nonlinear_plan(), solver_plan())
        .unwrap();
    resolve_transient_fieldwise(
        &TransientFieldwiseRealizationRequest::explicit(
            program.model(),
            SemanticRevision::new(program.revision().0),
            RealizationRevision::new(165),
            plan,
        ),
        binding.fieldwise_requirements(),
        &capabilities(),
    )
    .unwrap()
}

fn assert_exact_graph(
    resolved: &eqiora_realization::ResolvedTransientFieldwiseRealization,
    requirements: &eqiora_realization::TransientFieldwiseRealizationRequirements,
    mesh: eqiora_realization::MeshArtifactReference,
) {
    let graph = resolved.portable_graph().expect("portable graph");
    assert_eq!(graph.domains().len(), 1);
    assert_eq!(graph.fields().len(), 2);
    assert!(graph.geometry_actions().is_empty());
    assert_eq!(graph.transformations().len(), 2);
    assert_eq!(graph.systems().len(), 1);
    assert_eq!(graph.linear_solves().len(), 1);
    assert_eq!(graph.nonlinear_solves().len(), 1);
    assert_eq!(graph.placements().len(), 1);
    let execution = requirements.fieldwise().execution();
    assert_eq!(execution.spatial_dimension(), NonZeroUsize::new(2).unwrap());
    assert_eq!(execution.scalar_type(), ScalarType::F64);
    assert_eq!(execution.vector_layout(), VectorLayoutKind::Replicated);
    let domain = &graph.domains()[0];
    assert_eq!(domain.domain(), requirements.fieldwise().domain());
    assert!(matches!(
        domain.coordinates(),
        eqiora_realization::CoordinateTreatment::Scaled(_)
    ));
    assert_eq!(domain.configuration(), DomainConfiguration::FixedGeometry);
    let discretization = domain.discretization();
    assert_eq!(
        discretization.method(),
        DiscretizationMethod::ContinuousGalerkin
    );
    assert_eq!(
        discretization.mesh(),
        MeshPolicy::ImportedSimplicial { artifact: mesh }
    );
    assert_eq!(
        discretization.quadrature(),
        QuadraturePolicy::TriangleDuffyGaussLegendre {
            points_per_axis: NonZeroUsize::new(5).unwrap()
        }
    );
    let spaces = graph
        .fields()
        .iter()
        .map(|field| field.space())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        spaces,
        BTreeSet::from([
            Space::simplex_p1_bubble(),
            Space::continuous_lagrange(NonZeroU16::MIN),
        ])
    );
    assert_eq!(
        graph
            .fields()
            .iter()
            .map(|field| field.field())
            .collect::<HashSet<_>>(),
        requirements
            .fieldwise()
            .unknown_fields()
            .iter()
            .copied()
            .collect::<HashSet<_>>()
    );
    match graph.transformations() {
        [
            TransformationNode::BackwardEulerDerivative {
                relation,
                state,
                duration,
            },
            TransformationNode::EnergySkewConvection {
                relation: convection_relation,
                velocity,
            },
        ] => {
            assert_eq!(*relation, requirements.relation());
            assert_eq!(*convection_relation, requirements.relation());
            assert_eq!(graph.field(*state).unwrap().field(), requirements.state());
            assert_eq!(
                graph.field(*velocity).unwrap().field(),
                requirements.state()
            );
            assert_eq!(*duration, time_step());
        }
        transformations => panic!("unexpected transformation graph: {transformations:?}"),
    }
    let system = &graph.systems()[0];
    assert_eq!(system.blocks().len(), 2);
    assert!(
        system
            .blocks()
            .iter()
            .all(|block| matches!(block, SystemBlock::Field(_)))
    );
    assert_eq!(
        system.operator_properties(),
        LinearOperatorProperties::General
    );
    assert_eq!(system.scalar_type(), ScalarType::F64);
    assert_eq!(system.partition(), VectorLayoutKind::Replicated);
    let linear = graph.linear_solves()[0];
    assert_eq!(linear.plan(), solver_plan());
    assert_eq!(linear.schedule(), ExecutionSchedule::Offline);
    assert_eq!(
        graph.placements(),
        [PlacementRequirementNode::HostWorkers {
            workers_per_partition: NonZeroUsize::MIN
        }]
    );
    assert_eq!(graph.nonlinear_solves()[0].plan(), nonlinear_plan());
    assert!(matches!(graph.root(), SolveRoot::Nonlinear(_)));
}

fn assert_model_digest(program: &KernelProgram, expected: [u8; 32]) {
    let digests = program
        .nodes()
        .filter_map(|node| match node {
            KernelNode::Domain(domain) => match domain.kind() {
                DomainKind::GeometryRegion { geometry, .. } => Some(geometry.bytes()),
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(digests, [expected]);
}

fn assert_named_partition(owner: &AcceptedCircularHoleChordalRealizationV1) {
    let source_names = owner
        .source()
        .entity_sets()
        .iter()
        .map(|set| set.name())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        source_names,
        BTreeSet::from(["cylinder", "fluid", "inlet", "outlet", "walls"])
    );
    let mesh = owner.mesh().mesh();
    let correspondence = owner.correspondence();
    let geometry = owner.realized_geometry();
    let fluid = correspondence
        .region_entity_set_entities(geometry, "fluid")
        .unwrap();
    assert_eq!(
        fluid,
        (0..mesh.entity_count(2).unwrap())
            .map(|index| MeshEntity::new(2, index))
            .collect::<Vec<_>>()
    );
    let mut owned = BTreeSet::new();
    for name in BOUNDARY_NAMES {
        let facets = correspondence
            .region_entity_set_entities(geometry, name)
            .unwrap();
        assert!(!facets.is_empty());
        for facet in facets {
            assert_eq!(facet.dimension(), 1);
            assert_eq!(mesh.is_boundary_entity(facet), Some(true));
            assert!(
                owned.insert(facet),
                "facet occurs in more than one authored set"
            );
        }
    }
    let exterior = (0..mesh.entity_count(1).unwrap())
        .map(|index| MeshEntity::new(1, index))
        .filter(|entity| mesh.is_boundary_entity(*entity) == Some(true))
        .collect::<BTreeSet<_>>();
    assert_eq!(owned, exterior);
}

fn exact_zero_state(mesh: &eqiora_meshing::SimplicialMesh) -> SimplicialMiniNavierStokesState2d {
    let cells = mesh.entity_count(2).unwrap();
    SimplicialMiniNavierStokesState2d::new(
        0.0,
        SimplicialMiniVelocityField2d::new(
            mesh.clone(),
            vec![[0.0; 2]; mesh.vertices().len()],
            vec![[0.0; 2]; cells],
        )
        .unwrap(),
        SimplicialP1Field::new(mesh.clone(), vec![0.0; mesh.vertices().len()]).unwrap(),
        SimplicialMiniStokesPressureReference2d::BoundaryTraction,
    )
    .unwrap()
}

fn capabilities() -> RealizationCapabilities {
    let solver = solver_plan();
    RealizationCapabilities::cartesian_product(
        [DiscretizationMethod::ContinuousGalerkin],
        [(
            MeshKind::ImportedAffineSimplicial,
            SpatialDimensionSupport::exact(NonZeroUsize::new(2).unwrap()),
        )],
        [VectorLayoutKind::Replicated],
        SolverCapabilities::exact([SolverCapability {
            algorithm: solver.algorithm(),
            operator_properties: LinearOperatorProperties::General,
            preconditioner: solver.preconditioner(),
            reduction: solver.reduction(),
            scalar_type: ScalarType::F64,
        }])
        .expect("the precommitted General/Fast solver tuple is exact"),
        TargetCapabilities::none().with_host_cpu(NonZeroUsize::MIN),
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

fn time_step() -> DynQuantity {
    DynQuantity::new(0.01, TIME)
}

fn nonlinear_plan() -> NonlinearSolvePlan {
    NonlinearSolvePlan::new(1.0e-9, 1.0e-11, NonZeroUsize::new(16).unwrap(), 12).unwrap()
}

fn solver_plan() -> SolverPlan {
    SolverPlan::new(
        LinearSolver::BiConjugateGradientStabilized,
        1.0e-11,
        1.0e-12,
        NonZeroUsize::new(2000).unwrap(),
    )
    .unwrap()
    .with_preconditioner(PreconditionerPolicy::Identity)
    .with_reduction(ReductionPolicy::Fast)
}

fn exact_source() -> CanonicalGeometryV1 {
    circular_source(
        [0.2, 0.2],
        vec![
            named("fluid", FACE_DIMENSION, &[0]),
            named("walls", EDGE_DIMENSION, &[2, 3]),
            named("inlet", EDGE_DIMENSION, &[0]),
            named("cylinder", EDGE_DIMENSION, &[4]),
            named("outlet", EDGE_DIMENSION, &[1]),
        ],
    )
}

fn named(name: &str, dimension: usize, members: &[usize]) -> NamedEntitySet {
    NamedEntitySet::new(name, dimension, members.to_vec())
}

fn circular_source(center: [f64; 2], sets: Vec<NamedEntitySet>) -> CanonicalGeometryV1 {
    CanonicalGeometryV1::from_circular_hole([[0.0, 2.2], [0.0, 0.41]], center, 0.05, sets, 1.0e-12)
        .expect("valid exact circular source")
}

fn owner(
    source: &CanonicalGeometryV1,
    boundary_error: f64,
    segments: usize,
    quality: f64,
) -> AcceptedCircularHoleChordalRealizationV1 {
    AcceptedCircularHoleChordalRealizationV1::from_reference(
        source,
        boundary_error,
        segments,
        MeshQualityGate::new(quality).unwrap(),
    )
    .expect("valid exact-source owner")
}

fn cartesian_program(model_source: &str) -> KernelProgram {
    let mut compiled = eqiora_compiler::compile("non-box-transient-oracle.eqi", model_source)
        .expect("accepted transient grammar compiles");
    assert_eq!(compiled.len(), 1);
    let (transaction, model, _) = compiled.remove(0).into_parts();
    let mut store = InMemoryGraphStore::new();
    store
        .commit(transaction)
        .expect("compiled transaction commits");
    KernelProgram::from_snapshot(&store.snapshot(), model).expect("Cartesian scaffold admits")
}

fn geometry_program(
    source: &CanonicalGeometryV1,
    model_source: &str,
    names: [&str; 4],
) -> KernelProgram {
    let cartesian = cartesian_program(model_source);
    geometry_program_from_cartesian(&cartesian, source, names)
}

fn geometry_program_from_cartesian(
    cartesian: &KernelProgram,
    source: &CanonicalGeometryV1,
    names: [&str; 4],
) -> KernelProgram {
    let body = cartesian
        .nodes()
        .find_map(|node| match node {
            KernelNode::Domain(domain)
                if matches!(domain.kind(), DomainKind::CartesianBox { .. }) =>
            {
                Some(domain.id())
            }
            _ => None,
        })
        .expect("one Cartesian scaffold body");
    let mut nodes = Vec::new();
    for node in cartesian.nodes() {
        nodes.push(match node {
            KernelNode::Domain(domain) if domain.id() == body => KernelNode::from(
                DomainDef::geometry_region(
                    domain.id(),
                    GeometryDigest::new(source.digest_bytes()),
                    "fluid",
                )
                .unwrap(),
            ),
            KernelNode::Domain(domain) => match domain.kind() {
                DomainKind::CartesianBoundary { axis, side } => {
                    let name = match (*axis, *side) {
                        (0, BoundarySide::Lower) => names[0],
                        (0, BoundarySide::Upper) => names[1],
                        (1, BoundarySide::Lower) => names[2],
                        (1, BoundarySide::Upper) => names[3],
                        pair => panic!("unexpected scaffold side {pair:?}"),
                    };
                    KernelNode::from(DomainDef::geometry_boundary(domain.id(), name).unwrap())
                }
                _ => node.clone(),
            },
            _ => node.clone(),
        });
    }
    let members = nodes.iter().map(KernelNode::id).collect::<Vec<_>>();
    let mut transaction = Transaction::new("private non-box transient oracle");
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
    for node in cartesian.nodes() {
        if let Some(value) = cartesian.value(node.id()) {
            transaction.push(Op::SetValue {
                target: node.id(),
                value,
            });
        }
    }
    for edge in cartesian.edges() {
        transaction.push(Op::Connect {
            from: edge.from(),
            to: edge.to(),
            edge: if edge.kind() == EdgeKind::BoundaryOf {
                EdgeKind::BoundaryOf
            } else {
                edge.kind()
            },
        });
    }
    transaction.push(Op::DefineOntologyView {
        view: ModelView::new(cartesian.model(), members, None)
            .unwrap()
            .into(),
    });
    let mut store = InMemoryGraphStore::new();
    store
        .commit(transaction)
        .expect("geometry transaction commits");
    KernelProgram::from_snapshot_with_geometry(&store.snapshot(), cartesian.model(), &[source])
        .expect("exact geometry program admits")
}

#[derive(Debug)]
struct RejectAnyAssembly;

impl AssemblyBackend for RejectAnyAssembly {
    fn assemble(
        &self,
        _plan: &AssemblyPlan,
        _work: &dyn AssemblyWork,
    ) -> Result<AssemblyResult, Diagnostic> {
        panic!("identity mismatch reached materialization")
    }
}

/// Serial-host evidence executor for the production-mandated transient
/// `BiCGSTAB / General / Identity / Fast / F64` tuple, which the
/// Reproducible-only reference oracle deliberately does not declare
/// (`SolverCapabilities::reference()`). It mirrors the accepted ALE/FSI
/// `DenseGeneralSolver` evidence executor: the admitted plan is served by an
/// exact dense partial-pivot elimination, and every solution passes the
/// solver crate's independent true-residual acceptance against the unchanged
/// plan tolerances and the frozen iteration bound.
#[derive(Debug)]
struct DenseGeneralSolver;

impl LinearSolverBackend for DenseGeneralSolver {
    fn provider(&self) -> SolverProvider {
        SolverProvider::new(
            BackendId::new("eqiora.test.dense-general"),
            env!("CARGO_PKG_VERSION"),
            &[],
        )
    }

    fn capabilities(&self) -> SolverCapabilities {
        SolverCapabilities::exact([SolverCapability {
            algorithm: LinearSolver::BiConjugateGradientStabilized,
            operator_properties: LinearOperatorProperties::General,
            preconditioner: PreconditionerPolicy::Identity,
            reduction: ReductionPolicy::Fast,
            scalar_type: ScalarType::F64,
        }])
        .expect("the exact production transient solver tuple is admissible")
    }

    fn solve_with_execution(
        &self,
        problem: &LinearProblem<'_>,
        plan: SolverPlan,
        execution: &dyn ReplicatedLinearExecution,
    ) -> Result<LinearSolution, Diagnostic> {
        self.capabilities()
            .require_problem(plan, ScalarType::F64, problem.properties())?;
        if execution.report() != ExecutionReport::host_serial() {
            return Err(Diagnostic::error(
                codes::INVALID_REALIZATION,
                "dense test solver requires serial-host execution",
            ));
        }
        let dimension = problem.operator().columns();
        let mut matrix = vec![0.0; dimension * dimension];
        for column in 0..dimension {
            let mut basis = vec![0.0; dimension];
            basis[column] = 1.0;
            let mut action = vec![0.0; dimension];
            LinearOperator::apply(problem.operator(), &basis, &mut action)?;
            for (row, value) in action.into_iter().enumerate() {
                matrix[row * dimension + column] = value;
            }
        }
        let values = solve_dense(matrix, problem.right_hand_side().to_vec())?;
        accept_linear_solution_with_execution(
            problem,
            plan,
            self.provider(),
            ConvergenceReason::ResidualToleranceSatisfied,
            1,
            0.0,
            values,
            execution,
        )
    }
}

fn solve_dense(mut matrix: Vec<f64>, mut rhs: Vec<f64>) -> Result<Vec<f64>, Diagnostic> {
    let solve_failed =
        |message: &str| Diagnostic::error(codes::NUMERICAL_SOLVE_FAILED, message.to_owned());
    let dimension = rhs.len();
    for pivot in 0..dimension {
        let selected = (pivot..dimension)
            .max_by(|left, right| {
                matrix[*left * dimension + pivot]
                    .abs()
                    .total_cmp(&matrix[*right * dimension + pivot].abs())
            })
            .expect("nonempty pivot suffix");
        let pivot_value = matrix[selected * dimension + pivot];
        if !pivot_value.is_finite() || pivot_value.abs() <= f64::MIN_POSITIVE {
            return Err(solve_failed(
                "dense test solver encountered a singular pivot",
            ));
        }
        if selected != pivot {
            for column in 0..dimension {
                matrix.swap(pivot * dimension + column, selected * dimension + column);
            }
            rhs.swap(pivot, selected);
        }
        let diagonal = matrix[pivot * dimension + pivot];
        for row in pivot + 1..dimension {
            let factor = matrix[row * dimension + pivot] / diagonal;
            matrix[row * dimension + pivot] = 0.0;
            for column in pivot + 1..dimension {
                matrix[row * dimension + column] -= factor * matrix[pivot * dimension + column];
            }
            rhs[row] -= factor * rhs[pivot];
        }
    }
    let mut solution = vec![0.0; dimension];
    for row in (0..dimension).rev() {
        let remainder = (row + 1..dimension)
            .map(|column| matrix[row * dimension + column] * solution[column])
            .sum::<f64>();
        solution[row] = (rhs[row] - remainder) / matrix[row * dimension + row];
    }
    if solution.iter().all(|value| value.is_finite()) {
        Ok(solution)
    } else {
        Err(solve_failed(
            "dense test solver produced a non-finite solution",
        ))
    }
}

fn dfg_source_bound_nonzero_positive_executes_before_mutants() {
    use crate::simplicial_navier_stokes::element::with_dfg_viscous_pair_probe;

    let source = exact_source();
    let accepted = owner(&source, 1.0e-4, 50, 1.0e-5);
    let program = geometry_program(&source, &dfg_source(), BOUNDARY_NAMES);
    let binding = TransientNavierStokesGeometryBinding2d::new_dfg(&program, accepted.clone())
        .expect("the exact private DFG source binds");
    let resolved = resolve(&program, &binding);
    let initial = independent_nonzero_initial(&accepted);
    let initial_vertices = initial.velocity().vertex_values();
    assert!(initial_vertices.iter().flatten().any(|value| *value > 0.0));
    let advance = |initial| {
        binding.advance_dfg_with_assembly(
            &program,
            &resolved,
            initial,
            NonZeroStepCount::new(NonZeroUsize::MIN),
            &REFERENCE_ASSEMBLY_BACKEND,
            &DenseGeneralSolver,
        )
    };
    let seen = AtomicU8::new(0);
    let observe =
        |basis: [usize; 2], component: [usize; 2], gradient: [[f64; 2]; 2], mu, actual| {
            let direct = if component[0] == component[1] {
                mu * (gradient[0][0] * gradient[1][0] + gradient[0][1] * gradient[1][1])
            } else {
                0.0
            };
            let crossed = mu * gradient[0][component[1]] * gradient[1][component[0]];
            assert_eq!(actual, direct);
            if basis.iter().all(|index| *index < 3) && crossed != 0.0 {
                assert_ne!(direct + crossed, actual);
                assert_ne!(direct - crossed, actual);
                assert_ne!(direct + 2.0 * crossed, actual);
                seen.fetch_or(1, Ordering::Relaxed);
            }
            if basis == [3, 3] && direct != 0.0 {
                seen.fetch_or(2, Ordering::Relaxed);
            }
            actual
        };
    let trajectory = with_dfg_viscous_pair_probe(&observe, || advance(initial.clone()))
        .expect("one nonzero source-bound DFG step executes");
    assert_eq!(trajectory.states().len(), 2);
    assert_eq!(trajectory.steps().len(), 1);
    assert!(trajectory.states().iter().all(|state| {
        state.pressure_reference() == SimplicialMiniStokesPressureReference2d::BoundaryTraction
            && state.pressure_reference().gauge_multiplier().is_none()
            && state
                .velocity()
                .vertex_values()
                .iter()
                .chain(state.velocity().cell_bubble_values())
                .flatten()
                .copied()
                .chain(state.pressure().vertex_values().iter().copied())
                .all(f64::is_finite)
    }));
    assert!(trajectory.states()[1].time() > trajectory.states()[0].time());
    let step = &trajectory.steps()[0];
    assert!(step.assembly_report().packet_count() > 0);
    assert!(step.jacobian_residual_assembly_count() > 0);
    assert_eq!(seen.load(Ordering::Relaxed), 3);
    with_dfg_viscous_pair_probe(&|_, _, _, _, _| f64::NAN, || advance(initial))
        .expect_err("a poisoned actual DFG pair must stop the source-bound advance");
}

fn dfg_semantic_pair_and_inlet_fail_closed() {
    let source = exact_source();
    let accepted = owner(&source, 1.0e-4, 50, 1.0e-5);
    let exact = dfg_source();
    let dfg = "dynamic_viscosity * grad(velocity)";
    let symmetric = "2 * dynamic_viscosity * symmetric_part(grad(velocity))";
    let both_symmetric = exact.replacen(dfg, symmetric, 2);
    for wrong in [
        SOURCE.to_owned(),
        both_symmetric.replacen(symmetric, dfg, 1),
        exact.replacen(dfg, symmetric, 1),
        exact.replace(
            "trace(velocity) + normal(isotropic_lift(inlet_profile)) = 0;",
            "trace(velocity) - normal(isotropic_lift(inlet_profile)) = 0;",
        ),
        exact.replace(
            "parameter inlet_speed: m / s = 0.3;",
            "parameter inlet_speed: m / s = 0.2;",
        ),
    ] {
        let program = geometry_program(&source, &wrong, BOUNDARY_NAMES);
        TransientNavierStokesGeometryBinding2d::new_dfg(&program, accepted.clone())
            .expect_err("DFG volume/outlet/inlet identity must be exact");
    }
}

#[test]
fn registered_dfg_nonsymmetric_transient_mini_oracle_executes_all_falsifiers() {
    dfg_source_bound_nonzero_positive_executes_before_mutants();
    dfg_semantic_pair_and_inlet_fail_closed();
}

fn dfg_source() -> String {
    SOURCE
        .replace(
            "  field force_potential on body as space: kg / (m * s ^ 2) = 0;",
            "  field force_potential on body as space: kg / (m * s ^ 2) = 0;\n  field inlet_profile on body as space: m / s = 0;",
        )
        .replace(
            "  parameter dynamic_viscosity: kg / (m * s) = 0.05;",
            "  parameter dynamic_viscosity: kg / (m * s) = 0.001;\n  parameter inlet_speed: m / s = 0.3;\n  parameter channel_height: m = 0.41;",
        )
        .replace(
            "  relation momentum continuous on body {",
            "  relation inlet_profile_definition continuous on body {\n    inlet_profile - 4 * inlet_speed * coordinate(1) * (channel_height - coordinate(1)) / channel_height ^ 2 = 0;\n  }\n  relation momentum continuous on body {",
        )
        .replace(
            "2 * dynamic_viscosity * symmetric_part(grad(velocity))",
            "dynamic_viscosity * grad(velocity)",
        )
        .replace(
            "relation inlet_velocity continuous on x_lower { trace(velocity) = 0; }",
            "relation inlet_velocity continuous on x_lower {\n    trace(velocity) + normal(isotropic_lift(inlet_profile)) = 0;\n  }",
        )
}

fn independent_nonzero_initial(
    accepted: &AcceptedCircularHoleChordalRealizationV1,
) -> SimplicialMiniNavierStokesState2d {
    let mesh = accepted.mesh().mesh();
    let correspondence = accepted.correspondence();
    let geometry = accepted.realized_geometry();
    let mut fixed = vec![None; mesh.vertices().len()];
    let mut facets = Vec::new();
    for name in ["inlet", "walls", "cylinder", "outlet"] {
        let essential = name != "outlet";
        let condition = if essential {
            SimplicialMiniStokesBoundaryCondition2d::EssentialVelocity
        } else {
            SimplicialMiniStokesBoundaryCondition2d::ConstantTraction { value: [0.0; 2] }
        };
        for facet in correspondence
            .region_entity_set_entities(geometry, name)
            .unwrap()
        {
            facets.push(SimplicialMiniStokesBoundaryFacet2d::new(facet, condition));
            if essential {
                for vertex in mesh.entity_vertices(facet).unwrap() {
                    let y = mesh.vertices()[vertex.index()][1];
                    let value = if name == "inlet" {
                        [4.0 * 0.3 * y * (0.41 - y) / 0.41_f64.powi(2), 0.0]
                    } else {
                        [0.0; 2]
                    };
                    assert!(fixed[vertex.index()].is_none_or(|prior| prior == value));
                    fixed[vertex.index()] = Some(value);
                }
            }
        }
    }
    let boundary = SimplicialMiniStokesBoundary2d::new(mesh, facets).unwrap();
    let essential = |coordinate: [f64; 2]| {
        mesh.vertices()
            .iter()
            .position(|point| *point == coordinate)
            .and_then(|index| fixed[index])
            .ok_or_else(|| {
                Diagnostic::error(codes::INVALID_DISCRETIZATION, "unknown essential vertex")
            })
    };
    let cell_quadrature = eqiora_meshing::triangle_duffy_gauss_legendre(5).unwrap();
    let facet_quadrature = eqiora_meshing::simplex_duffy_gauss_legendre(1, 2).unwrap();
    let iterations = NonZeroUsize::new(2000).unwrap();
    let steady_solver =
        SolverPlan::new(LinearSolver::MinimumResidual, 1.0e-11, 1.0e-12, iterations).unwrap();
    let solution = solve_simplicial_mini_stokes_2d_with_boundary(
        mesh,
        0.001,
        &|_| Ok([0.0; 2]),
        &boundary,
        &essential,
        &cell_quadrature,
        &facet_quadrature,
        LinearSolveRequest::new(&REFERENCE_LINEAR_SOLVER, steady_solver),
    )
    .expect("accepted steady MINI path supplies independent nonzero initial data");
    let pressure_reference = solution.pressure_reference();
    assert_eq!(
        pressure_reference,
        SimplicialMiniStokesPressureReference2d::BoundaryTraction
    );
    SimplicialMiniNavierStokesState2d::from_stokes_solution(0.0, &solution).unwrap()
}
