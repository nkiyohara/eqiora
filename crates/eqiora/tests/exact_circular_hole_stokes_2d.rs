use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::num::NonZeroUsize;

use eqiora::api::UnstructuredP1ScalarFieldProjection2d;
use eqiora::artifact::{
    DiscreteFieldEnvelopeV1, ExecutionProvenanceV1, ExecutionTopologyV1, FieldSnapshotEnvelopeV1,
    GeometryDefinitionV1, GeometryMeshCorrespondenceEnvelopeV1, LayoutArtifacts, ModelEnvelopeV7,
    RealizationEnvelopeV2, RunManifestV2, SimplicialMeshEnvelopeV1,
};
use eqiora::compatibility::ExactModelCodec;
use eqiora::geometry::{
    CanonicalCircularHoleGeometryV1, CanonicalGeometryLimits, CanonicalGeometryRef,
    CircularHoleChordalMeshV1, EDGE_DIMENSION, FACE_DIMENSION, NamedEntitySet,
};
use eqiora::graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora::kernel::{BoundarySide, DomainDef, DomainKind, KernelNode};
use eqiora::meshing::{
    DiscreteFieldAssociation, DiscreteFieldPayload, DiscreteFieldShape, MeshEntity,
    MeshQualityGate, MeshTopology, SimplicialMesh,
};
use eqiora::numerics::{
    IncompressibleFlowScaleProfile2d, SteadyStokesGeometryBinding2d, SteadyStokesMiniSolution2d,
    solve_resolved_steady_stokes_geometry_mini_2d,
};
use eqiora::ontology::ModelView;
use eqiora::realization::{
    DiscretizationMethod, FieldwiseRealizationRequest, MeshKind, RealizationCapabilities,
    RealizationRevision, SemanticRevision, SpatialDimensionSupport, TargetCapabilities,
    VectorLayoutKind, resolve_fieldwise,
};
use eqiora::sem::KernelProgram;
use eqiora::solver::{
    LinearSolver, LinearSolverBackend, PreconditionerPolicy, ReductionPolicy,
    SERIAL_EXECUTION_PROVIDER, SolverPlan,
};
use eqiora::{Diagnostic, DimExponents, DynQuantity};
use eqiora_backend_faer::FaerLinearSolver;
use eqiora_numerics::fluid::SteadyStokesPressureReference2d;
use serde::Deserialize;

const SOURCE: &str = include_str!("../../../examples/steady-flow-past-cylinder.eqi");
const EXAMPLE_GEOMETRY: &[u8] =
    include_bytes!("../../../examples/steady-flow-past-cylinder.geometry.json");
const EXAMPLE_MODEL: &[u8] =
    include_bytes!("../../../examples/steady-flow-past-cylinder.model-v7.json");

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

#[test]
fn exact_geometry_model_executes_the_frozen_reference_tuple() {
    let witness = execute_witness(1.0e-6).expect("frozen reference tuple executes");
    let authored = execute_source_witness(exact_source(), SOURCE, 1.0e-6)
        .expect("readable example source executes");
    assert_same_example_solution(&witness.solution, &authored.solution);
    let ExecutedWitness {
        solution,
        model,
        realization,
        source,
        owner,
        geometry,
        correspondence,
        mesh,
        inlet_facets,
        outlet_facets,
        wall_facets,
        cylinder_facets,
    } = witness;
    let physical_mesh = solution.velocity().mesh();
    let oracle = frozen_oracle();
    assert_frozen_boundary_partition(
        physical_mesh,
        &inlet_facets,
        &outlet_facets,
        &wall_facets,
        &cylinder_facets,
    );

    for probe in &oracle.observations.velocity_probes {
        let cell = select_cell_by_barycentre(physical_mesh, probe.target_m);
        let actual = velocity_at_barycentre(&solution, cell);
        assert_vector_close(actual, probe.velocity_m_s, velocity_tolerance());
    }

    let cylinder_vertices = facet_vertices(physical_mesh, &cylinder_facets);
    let exterior_vertices = facet_vertices(
        physical_mesh,
        &inlet_facets
            .iter()
            .chain(&outlet_facets)
            .chain(&wall_facets)
            .copied()
            .collect::<Vec<_>>(),
    );
    let pressure = solution.pressure().vertex_values();
    for probe in &oracle.observations.pressure_probes {
        let vertex = match probe.name.as_str() {
            "cylinder_min_x" => select_extreme_vertex(physical_mesh, &cylinder_vertices, 0, false),
            "cylinder_max_x" => select_extreme_vertex(physical_mesh, &cylinder_vertices, 0, true),
            "cylinder_min_y" => select_extreme_vertex(physical_mesh, &cylinder_vertices, 1, false),
            "cylinder_max_y" => select_extreme_vertex(physical_mesh, &cylinder_vertices, 1, true),
            "outer_nearest_x_low_mid" => {
                select_nearest_vertex(physical_mesh, &exterior_vertices, [0.0, 0.20])
            }
            "outer_nearest_x_high_mid" => {
                select_nearest_vertex(physical_mesh, &exterior_vertices, [2.2, 0.20])
            }
            name => panic!("unknown frozen pressure selector {name}"),
        };
        assert_close(pressure[vertex], probe.pressure_pa, pressure_tolerance());
    }

    let inlet_flux = solution
        .named_boundary_flux("inlet")
        .expect("retained inlet flux");
    let outlet_flux = solution
        .named_boundary_flux("outlet")
        .expect("retained outlet flux");
    assert_eq!(solution.named_boundary_flux("walls"), None);
    assert_eq!(solution.named_boundary_flux("cylinder"), None);
    assert_close(
        inlet_flux,
        oracle.observations.signed_flux_m2_s.inlet,
        flux_tolerance(),
    );
    assert_close(
        outlet_flux,
        oracle.observations.signed_flux_m2_s.outlet,
        flux_tolerance(),
    );
    assert!((inlet_flux + outlet_flux).abs() <= 1.0e-8);

    assert_vector_close(
        solution
            .named_boundary_reaction("cylinder")
            .expect("named cylinder reaction"),
        oracle
            .observations
            .cylinder_reaction_n_m
            .constraint_force_on_fluid,
        reaction_tolerance(),
    );
    assert_vector_close(
        solution.boundary_reaction(),
        oracle.observations.global_balance_n_m.constrained_reaction,
        reaction_tolerance(),
    );
    assert_vector_close(
        solution.integrated_body_force(),
        oracle.observations.global_balance_n_m.integrated_body_force,
        reaction_tolerance(),
    );
    assert_vector_close(
        solution.integrated_boundary_traction(),
        oracle.observations.global_balance_n_m.integrated_traction,
        reaction_tolerance(),
    );
    for component in 0..2 {
        let balance = solution.boundary_reaction()[component]
            + solution.integrated_body_force()[component]
            + solution.integrated_boundary_traction()[component];
        assert!(balance.abs() <= 1.0e-10);
    }

    assert_eq!(
        solution.pressure_reference(),
        SteadyStokesPressureReference2d::BoundaryTraction
    );
    assert_eq!(solution.gauge_multiplier(), None);
    let dimensionless = solution.dimensionless_solution();
    assert!(dimensionless.continuity_residual_norm().is_finite());
    let report = dimensionless.solve_report();
    assert_eq!(report.algorithm(), LinearSolver::SparseLu);
    assert_eq!(report.preconditioner(), PreconditionerPolicy::Identity);
    assert_eq!(report.reduction(), ReductionPolicy::Fast);
    assert_eq!(
        report.residual_target(),
        oracle.observations.residuals.solver_selected_target
    );
    assert!(report.true_residual_norm().is_finite());
    assert!(report.true_residual_norm() <= report.residual_target());
    let weak_bound = report.residual_target()
        + 4096.0
            * f64::EPSILON
            * (1.0 + dimensionless.continuity_residual_norm() + report.residual_target());
    assert!(dimensionless.continuity_residual_norm() <= weak_bound);

    assert_studio_pressure_projection(
        &solution,
        &model,
        &realization,
        &source,
        &owner,
        &geometry,
        &correspondence,
        &mesh,
    );
}

#[path = "exact_circular_hole_stokes_2d/falsifiers.rs"]
mod falsifiers;

struct ExecutedWitness {
    solution: SteadyStokesMiniSolution2d,
    model: ModelEnvelopeV7,
    realization: RealizationEnvelopeV2,
    source: CanonicalCircularHoleGeometryV1,
    owner: CircularHoleChordalMeshV1,
    geometry: GeometryDefinitionV1,
    correspondence: GeometryMeshCorrespondenceEnvelopeV1,
    mesh: SimplicialMeshEnvelopeV1,
    inlet_facets: Vec<MeshEntity>,
    outlet_facets: Vec<MeshEntity>,
    wall_facets: Vec<MeshEntity>,
    cylinder_facets: Vec<MeshEntity>,
}

fn execute_witness(relative_tolerance: f64) -> Result<ExecutedWitness, Diagnostic> {
    let source = CanonicalCircularHoleGeometryV1::decode_canonical(
        embedded_json(EXAMPLE_GEOMETRY),
        CanonicalGeometryLimits::default(),
    )?;
    assert_eq!(source, exact_source());
    let model = ModelEnvelopeV7::from_json(embedded_json(EXAMPLE_MODEL), Default::default())?;
    let program = replay_example_program(&model, &source)?;
    execute_program_witness(source, program, model, relative_tolerance)
}

fn embedded_json(bytes: &[u8]) -> &[u8] {
    bytes.strip_suffix(b"\n").unwrap_or(bytes)
}

fn execute_source_witness(
    source: CanonicalCircularHoleGeometryV1,
    model_source: &str,
    relative_tolerance: f64,
) -> Result<ExecutedWitness, Diagnostic> {
    let program = geometry_program_from_text(&source, model_source);
    let model = ModelEnvelopeV7::from_program(&program)?;
    execute_program_witness(source, program, model, relative_tolerance)
}

fn execute_program_witness(
    source: CanonicalCircularHoleGeometryV1,
    program: KernelProgram,
    model: ModelEnvelopeV7,
    relative_tolerance: f64,
) -> Result<ExecutedWitness, Diagnostic> {
    let owner = frozen_owner(&source);
    assert_frozen_mesh_inventory(&owner);
    let geometry = GeometryDefinitionV1::from_region(owner.region());
    let mesh = SimplicialMeshEnvelopeV1::from_mesh(owner.mesh()).expect("mesh artifact");
    let mesh_reference = mesh.artifact_reference().expect("mesh identity");
    let correspondence = GeometryMeshCorrespondenceEnvelopeV1::from_region(&geometry, &mesh)
        .expect("correspondence");
    let inlet_facets = correspondence
        .region_entity_set_entities(&geometry, "inlet")
        .expect("inlet facets");
    let outlet_facets = correspondence
        .region_entity_set_entities(&geometry, "outlet")
        .expect("outlet facets");
    let wall_facets = correspondence
        .region_entity_set_entities(&geometry, "walls")
        .expect("wall facets");
    let cylinder_facets = correspondence
        .region_entity_set_entities(&geometry, "cylinder")
        .expect("cylinder facets");
    let binding = SteadyStokesGeometryBinding2d::new(
        &program,
        source.clone(),
        owner.clone(),
        geometry.clone(),
        mesh.clone(),
        correspondence.clone(),
    )?;
    let scales = IncompressibleFlowScaleProfile2d::new(
        DynQuantity::new(0.41, LENGTH),
        DynQuantity::new(0.3, VELOCITY),
        DynQuantity::new(0.001 * 0.3 / 0.41, PRESSURE),
    )
    .expect("frozen coherent-SI scale profile");
    let solver = SolverPlan::new(
        LinearSolver::SparseLu,
        relative_tolerance,
        1.0e-13,
        NonZeroUsize::new(10_000).unwrap(),
    )
    .expect("frozen solver tuple")
    .with_reduction(ReductionPolicy::Fast);
    let plan = binding
        .mini_plan(mesh_reference, scales, solver)
        .expect("method-neutral MINI plan");
    let resolved = resolve_fieldwise(
        &FieldwiseRealizationRequest::explicit(
            program.model(),
            SemanticRevision::new(program.revision().0),
            RealizationRevision::new(131),
            plan,
        ),
        binding.fieldwise_requirements(),
        &RealizationCapabilities::cartesian_product(
            [DiscretizationMethod::ContinuousGalerkin],
            [(
                MeshKind::ImportedAffineSimplicial,
                SpatialDimensionSupport::exact(NonZeroUsize::new(2).unwrap()),
            )],
            [VectorLayoutKind::Replicated],
            FaerLinearSolver.capabilities(),
            TargetCapabilities::none().with_host_cpu(NonZeroUsize::MIN),
        )
        .expect("faer symmetric mixed simplex capability"),
    )
    .expect("reference capability resolves");
    let realization =
        RealizationEnvelopeV2::from_resolved(&model, &resolved, LayoutArtifacts::Replicated)?;
    let solution = solve_resolved_steady_stokes_geometry_mini_2d(
        &program,
        &resolved,
        &binding,
        &FaerLinearSolver,
    )?;
    Ok(ExecutedWitness {
        solution,
        model,
        realization,
        source,
        owner,
        geometry,
        correspondence,
        mesh,
        inlet_facets,
        outlet_facets,
        wall_facets,
        cylinder_facets,
    })
}

fn replay_example_program(
    model: &ModelEnvelopeV7,
    source: &CanonicalCircularHoleGeometryV1,
) -> Result<KernelProgram, Diagnostic> {
    let (transaction, model_id) = model.to_transaction().map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .next()
            .expect("Model replay diagnostic")
    })?;
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .next()
            .expect("Model commit diagnostic")
    })?;
    KernelProgram::from_snapshot_with_geometry(
        &store.snapshot(),
        model_id,
        &[CanonicalGeometryRef::from(source)],
    )
    .map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .next()
            .expect("Semantic replay diagnostic")
    })
}

fn assert_same_example_solution(
    embedded: &SteadyStokesMiniSolution2d,
    authored: &SteadyStokesMiniSolution2d,
) {
    assert_eq!(
        embedded.velocity().vertex_values().len(),
        authored.velocity().vertex_values().len()
    );
    assert_eq!(
        embedded.pressure().vertex_values().len(),
        authored.pressure().vertex_values().len()
    );
    for (embedded, authored) in embedded
        .velocity()
        .vertex_values()
        .iter()
        .flatten()
        .zip(authored.velocity().vertex_values().iter().flatten())
    {
        assert_close(*embedded, *authored, velocity_tolerance());
    }
    for (embedded, authored) in embedded
        .pressure()
        .vertex_values()
        .iter()
        .zip(authored.pressure().vertex_values())
    {
        assert_close(*embedded, *authored, pressure_tolerance());
    }
    for boundary in ["inlet", "outlet"] {
        assert_close(
            embedded
                .named_boundary_flux(boundary)
                .expect("embedded named flux"),
            authored
                .named_boundary_flux(boundary)
                .expect("authored named flux"),
            flux_tolerance(),
        );
    }
    assert_vector_close(
        embedded
            .named_boundary_reaction("cylinder")
            .expect("embedded cylinder reaction"),
        authored
            .named_boundary_reaction("cylinder")
            .expect("authored cylinder reaction"),
        reaction_tolerance(),
    );
}

#[allow(clippy::too_many_arguments)]
fn assert_studio_pressure_projection(
    solution: &SteadyStokesMiniSolution2d,
    model: &ModelEnvelopeV7,
    realization: &RealizationEnvelopeV2,
    source: &CanonicalCircularHoleGeometryV1,
    owner: &CircularHoleChordalMeshV1,
    geometry: &GeometryDefinitionV1,
    correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
    mesh: &SimplicialMeshEnvelopeV1,
) {
    let payload = DiscreteFieldPayload::new(
        mesh.mesh(),
        DiscreteFieldAssociation::Vertex,
        DiscreteFieldShape::Scalar,
        solution.pressure().vertex_values().to_vec(),
    )
    .expect("accepted pressure payload");
    let block =
        DiscreteFieldEnvelopeV1::from_payload(mesh, &payload).expect("accepted pressure block");
    let snapshot = FieldSnapshotEnvelopeV1::new_authored_fieldwise(
        model,
        realization,
        source,
        owner,
        geometry,
        correspondence,
        mesh,
        solution.pressure_field(),
        std::slice::from_ref(&block),
    )
    .expect("accepted authored pressure snapshot");
    let execution = ExecutionProvenanceV1::from_provider_releases(
        FaerLinearSolver.provider(),
        SERIAL_EXECUTION_PROVIDER,
        ExecutionTopologyV1::Host {
            workers: NonZeroUsize::MIN,
        },
        ReductionPolicy::Fast,
        std::iter::empty::<(&str, &str)>(),
    )
    .expect("accepted execution provenance");
    let run = RunManifestV2::new(realization, execution)
        .expect("accepted Run")
        .with_output(snapshot.digest().expect("snapshot identity"));
    let projection = UnstructuredP1ScalarFieldProjection2d::from_authored_fieldwise_snapshot(
        model,
        realization,
        source,
        owner,
        geometry,
        correspondence,
        mesh,
        &run,
        &snapshot,
        &block,
    )
    .expect("accepted Studio pressure projection");

    assert_eq!(projection.model_artifact(), &model.digest().unwrap());
    assert_eq!(
        projection.realization_artifact(),
        &realization.digest().unwrap()
    );
    assert_eq!(projection.run_artifact(), &run.digest().unwrap());
    assert_eq!(projection.snapshot_artifact(), &snapshot.digest().unwrap());
    assert_eq!(projection.mesh_artifact(), &mesh.digest().unwrap());
    assert_eq!(projection.field(), solution.pressure_field());
    assert_eq!(projection.values(), solution.pressure().vertex_values());
}

#[derive(Debug, Deserialize)]
struct FrozenOracle {
    observations: FrozenObservations,
}

#[derive(Debug, Deserialize)]
struct FrozenObservations {
    velocity_probes: Vec<FrozenVelocityProbe>,
    pressure_probes: Vec<FrozenPressureProbe>,
    signed_flux_m2_s: FrozenFlux,
    #[serde(rename = "cylinder_reaction_N_m")]
    cylinder_reaction_n_m: FrozenCylinderReaction,
    #[serde(rename = "global_balance_N_m")]
    global_balance_n_m: FrozenGlobalBalance,
    residuals: FrozenResiduals,
}

#[derive(Debug, Deserialize)]
struct FrozenVelocityProbe {
    target_m: [f64; 2],
    velocity_m_s: [f64; 2],
}

#[derive(Debug, Deserialize)]
struct FrozenPressureProbe {
    name: String,
    #[serde(rename = "pressure_Pa")]
    pressure_pa: f64,
}

#[derive(Debug, Deserialize)]
struct FrozenFlux {
    inlet: f64,
    outlet: f64,
}

#[derive(Debug, Deserialize)]
struct FrozenCylinderReaction {
    constraint_force_on_fluid: [f64; 2],
}

#[derive(Debug, Deserialize)]
struct FrozenGlobalBalance {
    constrained_reaction: [f64; 2],
    integrated_body_force: [f64; 2],
    integrated_traction: [f64; 2],
}

#[derive(Debug, Deserialize)]
struct FrozenResiduals {
    solver_selected_target: f64,
}

fn frozen_oracle() -> FrozenOracle {
    serde_json::from_str(include_str!(
        "../../../verify/fluid/exact-circular-hole-stokes-2d/routes/python/result.json"
    ))
    .expect("frozen physical oracle parses")
}

#[derive(Debug, Deserialize)]
struct FrozenMesh {
    vertices_m: Vec<[f64; 2]>,
    cells: Vec<[usize; 3]>,
    boundary_facets: Vec<FrozenBoundaryFacet>,
}

#[derive(Debug, Deserialize)]
struct FrozenBoundaryFacet {
    vertices: [usize; 2],
    cell: usize,
}

fn assert_frozen_boundary_partition(
    mesh: &SimplicialMesh,
    inlet: &[MeshEntity],
    outlet: &[MeshEntity],
    walls: &[MeshEntity],
    cylinder: &[MeshEntity],
) {
    assert_eq!(
        [inlet.len(), outlet.len(), walls.len(), cylinder.len()],
        [14, 2, 38, 50]
    );
    let mut partition = BTreeSet::new();
    for facet in inlet.iter().chain(outlet).chain(walls).chain(cylinder) {
        assert_eq!(mesh.is_boundary_entity(*facet), Some(true));
        assert!(
            partition.insert(*facet),
            "named boundary partition repeats {facet:?}"
        );
    }
    assert_eq!(partition.len(), 104);
}

fn assert_frozen_mesh_inventory(owner: &CircularHoleChordalMeshV1) {
    let frozen: FrozenMesh = serde_json::from_str(include_str!(
        "../../../verify/fluid/exact-circular-hole-stokes-2d/mesh/mesh.json"
    ))
    .expect("frozen oracle mesh parses");
    let mesh = owner.mesh();
    assert_eq!(mesh.vertices().len(), frozen.vertices_m.len());
    assert_eq!(mesh.cells().len(), frozen.cells.len());
    let coordinate_tolerance = owner.boundary_evaluation_allowance_m();
    let vertex_map = mesh
        .vertices()
        .iter()
        .map(|actual| {
            let candidates = frozen
                .vertices_m
                .iter()
                .enumerate()
                .filter_map(|(index, expected)| {
                    ((actual[0] - expected[0]).abs() <= coordinate_tolerance
                        && (actual[1] - expected[1]).abs() <= coordinate_tolerance)
                        .then_some(index)
                })
                .collect::<Vec<_>>();
            assert_eq!(
                candidates.len(),
                1,
                "production vertex {actual:?} must match one frozen vertex"
            );
            candidates[0]
        })
        .collect::<Vec<_>>();
    for (actual, &frozen_index) in mesh.vertices().iter().zip(&vertex_map) {
        let expected = frozen.vertices_m[frozen_index];
        assert_eq!(actual.len(), 2);
        assert_close(actual[0], expected[0], coordinate_tolerance);
        assert_close(actual[1], expected[1], coordinate_tolerance);
    }
    let actual_cells = mesh
        .cells()
        .iter()
        .map(|cell| sorted_triple(cell.map_indices(&vertex_map)))
        .collect::<BTreeSet<_>>();
    let frozen_cells = frozen
        .cells
        .iter()
        .copied()
        .map(sorted_triple)
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_cells, frozen_cells);

    let frozen_facets = frozen
        .boundary_facets
        .iter()
        .map(|facet| {
            let [first, second] = facet.vertices;
            (
                first.min(second),
                first.max(second),
                sorted_triple(frozen.cells[facet.cell]),
            )
        })
        .collect::<BTreeSet<_>>();
    let actual_facets = (0..mesh.entity_count(1).expect("edge count"))
        .filter_map(|index| {
            let facet = MeshEntity::new(1, index);
            (mesh.is_boundary_entity(facet) == Some(true)).then(|| {
                let vertices = entity_vertex_indices(mesh, facet);
                assert_eq!(vertices.len(), 2);
                let cell = adjacent_cell(mesh, vertices[0], vertices[1]);
                let first = vertex_map[vertices[0]];
                let second = vertex_map[vertices[1]];
                (
                    first.min(second),
                    first.max(second),
                    sorted_triple(mesh.cells()[cell].map_indices(&vertex_map)),
                )
            })
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_facets, frozen_facets);
}

trait MapVertexIndices {
    fn map_indices(&self, vertex_map: &[usize]) -> [usize; 3];
}

impl MapVertexIndices for Vec<usize> {
    fn map_indices(&self, vertex_map: &[usize]) -> [usize; 3] {
        assert_eq!(self.len(), 3);
        [
            vertex_map[self[0]],
            vertex_map[self[1]],
            vertex_map[self[2]],
        ]
    }
}

fn sorted_triple(mut values: [usize; 3]) -> [usize; 3] {
    values.sort_unstable();
    values
}

fn select_cell_by_barycentre(mesh: &SimplicialMesh, target: [f64; 2]) -> usize {
    (0..mesh.cells().len())
        .min_by(|&left, &right| {
            let left_barycentre = cell_barycentre(mesh, left);
            let right_barycentre = cell_barycentre(mesh, right);
            squared_distance(left_barycentre, target)
                .total_cmp(&squared_distance(right_barycentre, target))
                .then_with(|| {
                    cell_coordinate_key(mesh, left).cmp_by_total(&cell_coordinate_key(mesh, right))
                })
        })
        .expect("nonempty frozen mesh")
}

trait TotalCoordinateOrder {
    fn cmp_by_total(&self, other: &Self) -> Ordering;
}

impl TotalCoordinateOrder for Vec<[f64; 2]> {
    fn cmp_by_total(&self, other: &Self) -> Ordering {
        self.iter()
            .zip(other)
            .find_map(|(left, right)| {
                let order = coordinate_cmp(*left, *right);
                (order != Ordering::Equal).then_some(order)
            })
            .unwrap_or_else(|| self.len().cmp(&other.len()))
    }
}

fn cell_coordinate_key(mesh: &SimplicialMesh, cell: usize) -> Vec<[f64; 2]> {
    let mut key = mesh.cells()[cell]
        .iter()
        .map(|&vertex| [mesh.vertices()[vertex][0], mesh.vertices()[vertex][1]])
        .collect::<Vec<_>>();
    key.sort_by(|left, right| coordinate_cmp(*left, *right));
    key
}

fn coordinate_cmp(left: [f64; 2], right: [f64; 2]) -> Ordering {
    left[0]
        .total_cmp(&right[0])
        .then_with(|| left[1].total_cmp(&right[1]))
}

fn cell_barycentre(mesh: &SimplicialMesh, cell: usize) -> [f64; 2] {
    let mut barycentre = [0.0; 2];
    for &vertex in &mesh.cells()[cell] {
        barycentre[0] += mesh.vertices()[vertex][0] / 3.0;
        barycentre[1] += mesh.vertices()[vertex][1] / 3.0;
    }
    barycentre
}

fn velocity_at_barycentre(solution: &SteadyStokesMiniSolution2d, cell: usize) -> [f64; 2] {
    let velocity = solution.velocity();
    let mut value = velocity.cell_bubble_values()[cell];
    for &vertex in &velocity.mesh().cells()[cell] {
        for (value_component, vertex_component) in value
            .iter_mut()
            .zip(velocity.vertex_values()[vertex].iter())
        {
            *value_component += vertex_component / 3.0;
        }
    }
    value
}

fn facet_vertices(mesh: &SimplicialMesh, facets: &[MeshEntity]) -> BTreeSet<usize> {
    facets
        .iter()
        .flat_map(|&facet| entity_vertex_indices(mesh, facet))
        .collect()
}

fn entity_vertex_indices(mesh: &SimplicialMesh, entity: MeshEntity) -> Vec<usize> {
    mesh.entity_vertices(entity)
        .expect("accepted mesh entity")
        .iter()
        .map(|vertex| vertex.index())
        .collect()
}

fn select_extreme_vertex(
    mesh: &SimplicialMesh,
    vertices: &BTreeSet<usize>,
    axis: usize,
    maximum: bool,
) -> usize {
    vertices
        .iter()
        .copied()
        .min_by(|&left, &right| {
            let order = mesh.vertices()[left][axis].total_cmp(&mesh.vertices()[right][axis]);
            (if maximum { order.reverse() } else { order }).then_with(|| {
                coordinate_cmp(
                    [mesh.vertices()[left][0], mesh.vertices()[left][1]],
                    [mesh.vertices()[right][0], mesh.vertices()[right][1]],
                )
            })
        })
        .expect("nonempty selector set")
}

fn select_nearest_vertex(
    mesh: &SimplicialMesh,
    vertices: &BTreeSet<usize>,
    target: [f64; 2],
) -> usize {
    vertices
        .iter()
        .copied()
        .min_by(|&left, &right| {
            let left_coordinate = [mesh.vertices()[left][0], mesh.vertices()[left][1]];
            let right_coordinate = [mesh.vertices()[right][0], mesh.vertices()[right][1]];
            squared_distance(left_coordinate, target)
                .total_cmp(&squared_distance(right_coordinate, target))
                .then_with(|| coordinate_cmp(left_coordinate, right_coordinate))
        })
        .expect("nonempty selector set")
}

fn adjacent_cell(mesh: &SimplicialMesh, first: usize, second: usize) -> usize {
    let mut cells = mesh.cells().iter().enumerate().filter_map(|(index, cell)| {
        (cell.contains(&first) && cell.contains(&second)).then_some(index)
    });
    let cell = cells.next().expect("facet has an adjacent cell");
    assert!(
        cells.next().is_none(),
        "boundary facet has two adjacent cells"
    );
    cell
}

fn squared_distance(left: [f64; 2], right: [f64; 2]) -> f64 {
    (left[0] - right[0]).powi(2) + (left[1] - right[1]).powi(2)
}

fn vertex_coordinate(mesh: &SimplicialMesh, vertex: usize) -> [f64; 2] {
    [mesh.vertices()[vertex][0], mesh.vertices()[vertex][1]]
}

fn velocity_tolerance() -> f64 {
    2.0e-12 + 5.0e-7 * 0.3
}

fn pressure_tolerance() -> f64 {
    2.0e-14 + 5.0e-7 * (0.001 * 0.3 / 0.41)
}

fn flux_tolerance() -> f64 {
    2.0e-13 + 5.0e-7 * (0.3 * 0.41)
}

fn reaction_tolerance() -> f64 {
    2.0e-14 + 5.0e-7 * (0.001 * 0.3)
}

fn assert_vector_close(actual: [f64; 2], expected: [f64; 2], tolerance: f64) {
    assert_close(actual[0], expected[0], tolerance);
    assert_close(actual[1], expected[1], tolerance);
}

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        actual.is_finite() && (actual - expected).abs() <= tolerance,
        "{actual:e} differs from {expected:e} by more than {tolerance:e}"
    );
}

fn exact_source() -> CanonicalCircularHoleGeometryV1 {
    circular_source([0.2, 0.2], [0, 1], vec![2, 3], 4)
}

fn circular_source(
    circle_center: [f64; 2],
    [inlet, outlet]: [usize; 2],
    walls: Vec<usize>,
    cylinder: usize,
) -> CanonicalCircularHoleGeometryV1 {
    CanonicalCircularHoleGeometryV1::new(
        [[0.0, 2.2], [0.0, 0.41]],
        circle_center,
        0.05,
        vec![
            NamedEntitySet::new("fluid", FACE_DIMENSION, vec![0]),
            NamedEntitySet::new("walls", EDGE_DIMENSION, walls),
            NamedEntitySet::new("inlet", EDGE_DIMENSION, vec![inlet]),
            NamedEntitySet::new("cylinder", EDGE_DIMENSION, vec![cylinder]),
            NamedEntitySet::new("outlet", EDGE_DIMENSION, vec![outlet]),
        ],
        1.0e-12,
    )
    .expect("frozen exact source")
}

fn frozen_owner(source: &CanonicalCircularHoleGeometryV1) -> CircularHoleChordalMeshV1 {
    CircularHoleChordalMeshV1::from_exact(
        source,
        1.0e-4,
        50,
        MeshQualityGate::new(1.0e-5).expect("frozen quality gate"),
    )
    .expect("frozen chordal owner")
}

fn geometry_program_from_text(
    source: &CanonicalCircularHoleGeometryV1,
    model_source: &str,
) -> KernelProgram {
    let cartesian = ExactModelCodec::V5
        .compile("exact-circular-hole-stokes-2d.eqi", model_source)
        .expect("Cartesian authoring scaffold compiles");
    let program = cartesian.program();
    let body = program
        .nodes()
        .find_map(|node| match node {
            KernelNode::Domain(domain)
                if matches!(domain.kind(), DomainKind::CartesianBox { .. }) =>
            {
                Some(domain.id())
            }
            _ => None,
        })
        .expect("one body");
    let mut nodes = Vec::new();
    for node in program.nodes() {
        let replacement = match node {
            KernelNode::Domain(domain) if domain.id() == body => KernelNode::from(
                DomainDef::geometry_region(
                    domain.id(),
                    eqiora::kernel::GeometryDigest::new(source.digest_bytes()),
                    "fluid",
                )
                .unwrap(),
            ),
            KernelNode::Domain(domain) => match domain.kind() {
                DomainKind::CartesianBoundary { axis, side } => {
                    let name = match (*axis, *side) {
                        (0, BoundarySide::Lower) => "inlet",
                        (0, BoundarySide::Upper) => "outlet",
                        (1, BoundarySide::Lower) => "walls",
                        (1, BoundarySide::Upper) => "cylinder",
                        _ => panic!("unexpected Cartesian scaffold boundary"),
                    };
                    KernelNode::from(DomainDef::geometry_boundary(domain.id(), name).unwrap())
                }
                _ => node.clone(),
            },
            _ => node.clone(),
        };
        nodes.push(replacement);
    }
    let members = nodes.iter().map(KernelNode::id).collect::<Vec<_>>();
    let mut transaction = Transaction::new("graph-authored exact circular-hole Stokes witness");
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
    for node in program.nodes() {
        if let Some(value) = program.value(node.id()) {
            transaction.push(Op::SetValue {
                target: node.id(),
                value,
            });
        }
    }
    for edge in program.edges() {
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
        view: ModelView::new(program.model(), members, None)
            .expect("closed geometry witness")
            .into(),
    });
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).expect("geometry witness commits");
    KernelProgram::from_snapshot_with_geometry(
        &store.snapshot(),
        program.model(),
        &[CanonicalGeometryRef::from(source)],
    )
    .expect("exact geometry admission")
}
