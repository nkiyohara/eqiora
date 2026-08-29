use std::collections::BTreeMap;
use std::num::NonZeroUsize;

use eqiora::api::ModelDocument;
use eqiora::artifact::{
    CartesianMeshCellsV1, CartesianMeshEnvelopeV1, GeometryMeshCorrespondenceEnvelopeV1,
    MeshProductionLineageEnvelopeV1, ModelEnvelope,
};
use eqiora::geometry::{CanonicalGeometryV1, PlanarOperationGraph, PlanarTopologyHandle};
use eqiora::meshing::{MeshEntity, MeshTopology};
use eqiora::solver::REFERENCE_LINEAR_SOLVER;
use eqiora_numerics::{
    AuthenticatedCommonMesh, CommonElasticityPlan, CommonElasticityRunOutput, CommonSolvePolicy,
    CommonSpatialPolicy, resolve_common_plan,
};
use serde_json::{Value, json};

const SOURCE: &str = r#"public component MixedBoundaryElasticity2d {
  public support body: volume(ambient_dimension = 2);
  public support x_lower: boundary(parent = body);
  public support x_upper: boundary(parent = body);
  public support y_lower: boundary(parent = body);
  public support y_upper: boundary(parent = body);
  representation space = continuum;
  field displacement on body as space: m shape spatial_vector;
  field load_potential on body as space: kg / (m * s ^ 2) = 0;
  public parameter mu: kg / (m * s ^ 2);
  public parameter lambda: kg / (m * s ^ 2);
  public parameter length_scale: m;
  relation load continuous on body {
    load_potential - 2 * mu * coordinate(0) / length_scale = 0;
  }
  relation balance continuous on body {
    -div(
      2 * mu * symmetric_part(grad(displacement))
      + lambda * isotropic_lift(div(displacement))
    ) - grad(load_potential) = 0;
  }
  relation x_lower_fixed continuous on x_lower { trace(displacement) = 0; }
  relation x_upper_free continuous on x_upper {
    normal(2 * mu * symmetric_part(grad(displacement))
      + lambda * isotropic_lift(div(displacement))) = 0;
  }
  relation y_lower_free continuous on y_lower {
    normal(2 * mu * symmetric_part(grad(displacement))
      + lambda * isotropic_lift(div(displacement))) = 0;
  }
  relation y_upper_free continuous on y_upper {
    normal(2 * mu * symmetric_part(grad(displacement))
      + lambda * isotropic_lift(div(displacement))) = 0;
  }
}"#;
const CELLS_PER_AXIS: usize = 16;
const VERTICES_PER_AXIS: usize = CELLS_PER_AXIS + 1;

struct Accepted {
    document: ModelDocument,
    geometry: CanonicalGeometryV1,
    mesh: CartesianMeshEnvelopeV1,
    correspondence: GeometryMeshCorrespondenceEnvelopeV1,
    plan: CommonElasticityPlan,
    output: CommonElasticityRunOutput,
}

fn accepted() -> Accepted {
    let graph = PlanarOperationGraph::new();
    let rectangle = graph.rectangle([0.0, 1.0], [0.0, 1.0]).unwrap();
    let edges = rectangle.boundaries();
    let geometry = graph
        .build(
            &rectangle,
            &BTreeMap::from([
                ("body".to_owned(), vec![rectangle.region().into()]),
                (
                    "x_lower".to_owned(),
                    vec![PlanarTopologyHandle::from(edges[0])],
                ),
                (
                    "x_upper".to_owned(),
                    vec![PlanarTopologyHandle::from(edges[1])],
                ),
                (
                    "y_lower".to_owned(),
                    vec![PlanarTopologyHandle::from(edges[2])],
                ),
                (
                    "y_upper".to_owned(),
                    vec![PlanarTopologyHandle::from(edges[3])],
                ),
            ]),
        )
        .unwrap();
    let document = ModelDocument::compile_with_geometry(
        "mixed-boundary-elasticity.eqi",
        SOURCE,
        &geometry,
        None,
        &[("mu", 3.0), ("lambda", 0.0), ("length_scale", 1.0)],
    )
    .unwrap();
    let cells = CartesianMeshCellsV1::new([CELLS_PER_AXIS; 2]).unwrap();
    let (mesh, correspondence) =
        GeometryMeshCorrespondenceEnvelopeV1::from_planar_rectangle_v2_cartesian(
            &geometry,
            cells.cells(),
        )
        .unwrap();
    let production = MeshProductionLineageEnvelopeV1::from_structured_cartesian_v1_resources(
        cells,
        &geometry,
        &mesh,
        &correspondence,
    )
    .unwrap();
    let owner = AuthenticatedCommonMesh::structured_cartesian(
        geometry.clone(),
        mesh.clone(),
        correspondence.clone(),
        production,
    )
    .unwrap();
    let solver =
        CommonSolvePolicy::linear(1.0e-10, 1.0e-12, NonZeroUsize::new(10_000).unwrap()).unwrap();
    let model = ModelEnvelope::from_program(document.program()).unwrap();
    let plan = resolve_common_plan(
        &model,
        owner,
        CommonSpatialPolicy::Q1,
        solver,
        None,
        None,
        &REFERENCE_LINEAR_SOLVER,
    )
    .unwrap()
    .project(
        |_| panic!("elasticity fixture resolved as ODE"),
        |_| panic!("elasticity fixture resolved as scalar"),
        |plan| plan,
        |_| panic!("elasticity fixture resolved as Stokes"),
        |_| panic!("elasticity fixture resolved as transient flow"),
        |_| panic!("elasticity fixture resolved as FSI"),
    );
    let output = plan.run_observed().unwrap();
    Accepted {
        document,
        geometry,
        mesh,
        correspondence,
        plan,
        output,
    }
}

#[test]
fn common_elasticity_output_closes_exact_plan_and_mesh_lineage() {
    let accepted = accepted();
    let mesh = accepted.mesh.mesh();
    assert_eq!(mesh.entity_count(0), Some(289));
    assert_eq!(mesh.entity_count(2), Some(256));
    assert_eq!(mesh.axis_coordinates(0), Some(axis().as_slice()));
    assert_eq!(mesh.axis_coordinates(1), Some(axis().as_slice()));

    for i in 0..VERTICES_PER_AXIS {
        for j in 0..VERTICES_PER_AXIS {
            let vertex = 17 * i + j;
            assert_eq!(
                mesh.vertex_coordinates(MeshEntity::new(0, vertex)),
                Some(vec![i as f64 / 16.0, j as f64 / 16.0]),
            );
        }
    }
    for i in 0..CELLS_PER_AXIS {
        for j in 0..CELLS_PER_AXIS {
            let cell = 16 * i + j;
            let lower = 17 * i + j;
            assert_eq!(
                mesh.entity_vertices(MeshEntity::new(2, cell))
                    .unwrap()
                    .into_iter()
                    .map(|vertex| vertex.index())
                    .collect::<Vec<_>>(),
                [lower, lower + 17, lower + 1, lower + 18],
            );
        }
    }

    assert_eq!(accepted.output.plan_identity(), accepted.plan.identity());
    assert_eq!(
        accepted.plan.model_digest(),
        accepted.document.digest().unwrap()
    );
    assert_eq!(accepted.plan.cells(), [CELLS_PER_AXIS; 2]);
    assert_eq!(
        accepted.plan.geometry_digest(),
        accepted
            .geometry
            .digest_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    accepted
        .correspondence
        .validate_against_planar_rectangle_v2_cartesian(
            &accepted.geometry,
            &accepted.mesh,
            [CELLS_PER_AXIS; 2],
        )
        .unwrap();

    let (solution, observation) = accepted.output.into_parts();
    assert_eq!(solution.displacement().values().len(), 578);
    assert!(
        solution
            .displacement()
            .values()
            .iter()
            .all(|value| value.is_finite())
    );
    assert_eq!(observation.exact_bounds(), [[0.0, 1.0], [0.0, 1.0]]);
    assert_eq!(
        observation.constrained_reaction(),
        solution.boundary_reaction()
    );
    assert_eq!(
        observation.integrated_body_force(),
        solution.integrated_body_force()
    );
    assert!(observation.solve().true_residual_norm() <= observation.solve().residual_target());
}

#[test]
fn cartesian_mesh_and_correspondence_round_trip_canonically_and_reject_mutants() {
    let accepted = accepted();
    let mesh_bytes = accepted.mesh.canonical_json().unwrap();
    assert_top_level_key_order(
        &mesh_bytes,
        &[
            "schema",
            "encoding",
            "dimension",
            "scalar",
            "cell_family",
            "axes",
            "vertex_order",
            "cell_order",
            "local_node_order",
        ],
    );
    let mesh_json: Value = serde_json::from_slice(&mesh_bytes).unwrap();
    assert_eq!(mesh_json["axes"], json!([axis(), axis()]));
    let decoded = CartesianMeshEnvelopeV1::from_json(&mesh_bytes, Default::default()).unwrap();
    assert_eq!(decoded, accepted.mesh);

    let mut wrong_axis = mesh_json.clone();
    wrong_axis["axes"][0].as_array_mut().unwrap().reverse();
    assert!(
        CartesianMeshEnvelopeV1::from_json(
            &serde_json::to_vec(&wrong_axis).unwrap(),
            Default::default(),
        )
        .is_err()
    );

    let correspondence_bytes = accepted.correspondence.canonical_json().unwrap();
    let decoded =
        GeometryMeshCorrespondenceEnvelopeV1::from_json(&correspondence_bytes, Default::default())
            .unwrap();
    assert_eq!(decoded, accepted.correspondence);
    decoded
        .validate_against_planar_rectangle_v2_cartesian(
            &accepted.geometry,
            &accepted.mesh,
            [CELLS_PER_AXIS; 2],
        )
        .unwrap();
}

fn axis() -> Vec<f64> {
    (0..=CELLS_PER_AXIS)
        .map(|index| index as f64 / CELLS_PER_AXIS as f64)
        .collect()
}

fn assert_top_level_key_order(bytes: &[u8], keys: &[&str]) {
    let text = std::str::from_utf8(bytes).unwrap();
    let positions = keys
        .iter()
        .map(|key| text.find(&format!("\"{key}\"")).unwrap())
        .collect::<Vec<_>>();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
}
