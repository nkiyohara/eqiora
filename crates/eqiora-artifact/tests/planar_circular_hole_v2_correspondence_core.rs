use std::collections::{BTreeMap, BTreeSet};

use eqiora_artifact::{
    AcceptedCircularHoleChordalRealizationV1, GeometryDecoderLimits, GeometryDefinitionV1,
    GeometryMeshCorrespondenceEnvelopeV1, SimplicialMeshEnvelopeV1,
};
use eqiora_geometry::{
    CadAuthoredGraph, CanonicalGeometryV1, ConstrainedRectangleV1, EDGE_DIMENSION, FACE_DIMENSION,
    NamedEntitySet, PlanarFace, PlanarRegion,
};
use eqiora_meshing::{MeshEntity, MeshQualityGate, MeshTopology, SimplicialMesh};
use serde_json::Value;

const MAX_BOUNDARY_ERROR_M: f64 = 1.0e-4;
const MAX_SEGMENTS: usize = 50;
const MINIMUM_MEAN_RATIO: f64 = 1.0e-5;

fn v2_geometry(center: [f64; 2]) -> CanonicalGeometryV1 {
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
        .circular_through_cut(center, 0.05, 1.0e-10)
        .unwrap();
    let result = graph.planar_result().unwrap();
    let cut_wall = graph.face_handle("cut-wall").unwrap();
    let named = BTreeMap::from([
        ("fluid".to_owned(), vec![end_cap]),
        ("inlet".to_owned(), vec![x_lower]),
        ("outlet".to_owned(), vec![x_upper]),
        ("walls".to_owned(), vec![y_lower, y_upper]),
        ("cylinder".to_owned(), vec![cut_wall]),
    ]);
    result.with_named_topology(&named).unwrap()
}

fn reference_mesh() -> eqiora_artifact::SimplicialMeshEnvelopeV1 {
    let source = CanonicalGeometryV1::from_circular_hole(
        [[0.0, 2.2], [0.0, 0.41]],
        [0.2, 0.2],
        0.05,
        vec![
            NamedEntitySet::new("inlet", EDGE_DIMENSION, vec![0]),
            NamedEntitySet::new("outlet", EDGE_DIMENSION, vec![1]),
            NamedEntitySet::new("walls", EDGE_DIMENSION, vec![2, 3]),
            NamedEntitySet::new("cylinder", EDGE_DIMENSION, vec![4]),
            NamedEntitySet::new("fluid", FACE_DIMENSION, vec![0]),
        ],
        1.0e-12,
    )
    .unwrap();
    AcceptedCircularHoleChordalRealizationV1::from_reference(
        &source,
        MAX_BOUNDARY_ERROR_M,
        MAX_SEGMENTS,
        MeshQualityGate::new(MINIMUM_MEAN_RATIO).unwrap(),
    )
    .unwrap()
    .mesh()
    .clone()
}

fn authored_triangle_correspondence() -> (
    GeometryDefinitionV1,
    SimplicialMeshEnvelopeV1,
    GeometryMeshCorrespondenceEnvelopeV1,
) {
    let region = PlanarRegion::new(
        vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
        vec![PlanarFace::new(vec![0, 1, 2], Vec::new())],
        Vec::new(),
        1.0e-12,
    )
    .unwrap();
    let geometry = GeometryDefinitionV1::from_region(&region);
    let mesh = SimplicialMeshEnvelopeV1::from_mesh(
        &SimplicialMesh::new(
            2,
            vec![vec![0.0, 0.0], vec![1.0, 0.0], vec![0.0, 1.0]],
            vec![vec![0, 1, 2]],
            MeshQualityGate::new(0.1).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let correspondence = GeometryMeshCorrespondenceEnvelopeV1::from_region(&geometry, &mesh)
        .expect("the pre-existing authored-region variant remains constructible");
    (geometry, mesh, correspondence)
}

#[test]
fn persisted_authored_region_variant_round_trips_and_unknown_sources_remain_closed() {
    let (geometry, mesh, correspondence) = authored_triangle_correspondence();
    let bytes = correspondence.canonical_json().unwrap();
    let decoded =
        GeometryMeshCorrespondenceEnvelopeV1::from_json(&bytes, GeometryDecoderLimits::default())
            .expect("the pre-existing authored-region wire variant remains decodable");
    assert_eq!(decoded, correspondence);
    assert_eq!(decoded.canonical_json().unwrap(), bytes);
    decoded.validate_against_region(&geometry, &mesh).unwrap();

    let mut unknown: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(unknown["source"], "authored-planar-region-v1");
    unknown["source"] = Value::String("unknown-correspondence-source".to_owned());
    GeometryMeshCorrespondenceEnvelopeV1::from_json(
        &serde_json::to_vec(&unknown).unwrap(),
        GeometryDecoderLimits::default(),
    )
    .expect_err("the persisted envelope must reject unknown source discriminators");
}

#[test]
fn direct_v2_source_lineage_replays_and_resolves_the_complete_reference_topology() {
    let geometry = v2_geometry([0.2, 0.2]);
    let independently_constructed_mesh = reference_mesh();
    let (mesh, correspondence) =
        GeometryMeshCorrespondenceEnvelopeV1::from_planar_circular_hole_v2_reference(
            &geometry,
            MAX_BOUNDARY_ERROR_M,
            MAX_SEGMENTS,
            MeshQualityGate::new(MINIMUM_MEAN_RATIO).unwrap(),
        )
        .unwrap();
    assert_eq!(mesh, independently_constructed_mesh);
    assert_eq!(
        correspondence,
        GeometryMeshCorrespondenceEnvelopeV1::from_planar_circular_hole_v2_reference_mesh(
            &geometry,
            &mesh,
            MAX_BOUNDARY_ERROR_M,
            MAX_SEGMENTS,
        )
        .unwrap()
    );
    correspondence
        .validate_against_planar_circular_hole_v2_reference(
            &geometry,
            &mesh,
            MAX_BOUNDARY_ERROR_M,
            MAX_SEGMENTS,
        )
        .unwrap();

    let cells = correspondence
        .planar_circular_hole_v2_entity_set_entities(&geometry, "fluid")
        .unwrap();
    assert_eq!(
        cells,
        (0..mesh.mesh().entity_count(FACE_DIMENSION).unwrap())
            .map(|index| MeshEntity::new(FACE_DIMENSION, index))
            .collect::<Vec<_>>()
    );

    let mut named_boundary = BTreeSet::new();
    for name in ["inlet", "outlet", "walls", "cylinder"] {
        for entity in correspondence
            .planar_circular_hole_v2_entity_set_entities(&geometry, name)
            .unwrap()
        {
            assert!(named_boundary.insert(entity));
        }
    }
    let exact_boundary = (0..mesh.mesh().entity_count(EDGE_DIMENSION).unwrap())
        .map(|index| MeshEntity::new(EDGE_DIMENSION, index))
        .filter(|&entity| mesh.mesh().is_boundary_entity(entity) == Some(true))
        .collect::<BTreeSet<_>>();
    assert_eq!(named_boundary, exact_boundary);

    let bytes = correspondence.canonical_json().unwrap();
    let decoded =
        GeometryMeshCorrespondenceEnvelopeV1::from_json(&bytes, GeometryDecoderLimits::default())
            .unwrap();
    assert_eq!(decoded, correspondence);
}

#[test]
fn v2_source_replay_rejects_foreign_geometry_and_structural_mutants() {
    let geometry = v2_geometry([0.2, 0.2]);
    let (mesh, correspondence) =
        GeometryMeshCorrespondenceEnvelopeV1::from_planar_circular_hole_v2_reference(
            &geometry,
            MAX_BOUNDARY_ERROR_M,
            MAX_SEGMENTS,
            MeshQualityGate::new(MINIMUM_MEAN_RATIO).unwrap(),
        )
        .unwrap();
    assert!(
        correspondence
            .validate_against_planar_circular_hole_v2_reference(
                &v2_geometry([0.21, 0.2]),
                &mesh,
                MAX_BOUNDARY_ERROR_M,
                MAX_SEGMENTS,
            )
            .is_err()
    );

    let mut wire: serde_json::Value =
        serde_json::from_slice(&correspondence.canonical_json().unwrap()).unwrap();
    let first_facet = wire["frontiers"][0]["facet_indices"][0].clone();
    wire["frontiers"][1]["facet_indices"]
        .as_array_mut()
        .unwrap()
        .insert(0, first_facet);
    wire["frontiers"][1]["parent_outward"]
        .as_array_mut()
        .unwrap()
        .insert(
            0,
            serde_json::Value::String("left-of-canonical-facet".to_owned()),
        );
    assert!(
        GeometryMeshCorrespondenceEnvelopeV1::from_json(
            &serde_json::to_vec(&wire).unwrap(),
            GeometryDecoderLimits::default(),
        )
        .is_err()
    );
}
