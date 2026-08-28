use std::collections::BTreeMap;

use eqiora_geometry::{PlanarOperationGraph, PlanarTopologyHandle};

use super::*;
use crate::MeshProductionLineageEnvelopeV1;

fn geometry() -> CanonicalGeometryV1 {
    let graph = PlanarOperationGraph::new();
    let left = graph.rectangle([0.0, 1.0], [0.0, 1.0]).unwrap();
    let right = graph.rectangle([1.0, 2.0], [0.0, 1.0]).unwrap();
    let left_edges = left.boundaries();
    let right_edges = right.boundaries();
    let partition = graph
        .partition(&left, &right, [left_edges[1], right_edges[0]])
        .unwrap();
    graph
        .build(
            &partition,
            &BTreeMap::from([
                (
                    "fluid".to_owned(),
                    vec![PlanarTopologyHandle::from(left.region())],
                ),
                ("solid".to_owned(), vec![right.region().into()]),
                (
                    "interface".to_owned(),
                    vec![left_edges[1].into(), right_edges[0].into()],
                ),
                ("inlet".to_owned(), vec![left_edges[0].into()]),
                ("outlet".to_owned(), vec![right_edges[1].into()]),
                (
                    "walls".to_owned(),
                    vec![
                        left_edges[2].into(),
                        left_edges[3].into(),
                        right_edges[2].into(),
                        right_edges[3].into(),
                    ],
                ),
            ]),
        )
        .unwrap()
}

#[test]
fn exact_partition_replays_mesh_correspondence_and_parent_relative_interface() {
    let geometry = geometry();
    let (mesh, correspondence) =
        GeometryMeshCorrespondenceEnvelopeV1::from_adjacent_rectangle_partition_affine_triangles(
            &geometry,
            [2, 2],
        )
        .unwrap();
    assert_eq!(
        mesh.mesh().vertices(),
        &[
            vec![0.0, 0.0],
            vec![0.0, 0.5],
            vec![0.0, 1.0],
            vec![1.0, 0.0],
            vec![1.0, 0.5],
            vec![1.0, 1.0],
            vec![2.0, 0.0],
            vec![2.0, 0.5],
            vec![2.0, 1.0],
        ]
    );
    assert_eq!(
        mesh.mesh().cells(),
        &[
            vec![0, 3, 4],
            vec![0, 4, 1],
            vec![1, 4, 5],
            vec![1, 5, 2],
            vec![3, 6, 7],
            vec![3, 7, 4],
            vec![4, 7, 8],
            vec![4, 8, 5],
        ]
    );
    assert_eq!(
        correspondence
            .adjacent_rectangle_partition_entity_set_entities(&geometry, "fluid")
            .unwrap()
            .len(),
        4
    );
    let wire: serde_json::Value =
        serde_json::from_slice(&correspondence.canonical_json().unwrap()).unwrap();
    assert_eq!(wire["source"], ADJACENT_PARTITION_SOURCE);
    assert_eq!(
        wire["frontiers"][1]["facet_indices"],
        serde_json::json!([7, 10])
    );
    assert_eq!(
        wire["frontiers"][7]["facet_indices"],
        serde_json::json!([7, 10])
    );
    assert_eq!(
        wire["frontiers"][1]["parent_outward"],
        serde_json::json!(["right-of-canonical-facet", "right-of-canonical-facet"])
    );
    assert_eq!(
        wire["frontiers"][7]["parent_outward"],
        serde_json::json!(["left-of-canonical-facet", "left-of-canonical-facet"])
    );
    assert_eq!(
        correspondence
            .adjacent_rectangle_partition_entity_set_entities(&geometry, "solid")
            .unwrap()
            .len(),
        4
    );
    assert_eq!(
        correspondence
            .adjacent_rectangle_partition_entity_set_entities(&geometry, "interface")
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        correspondence
            .adjacent_rectangle_partition_entity_set_entities(&geometry, "walls")
            .unwrap()
            .len(),
        4
    );
    correspondence
        .validate_against_adjacent_rectangle_partition_affine_triangles(&geometry, &mesh, [2, 2])
        .unwrap();
    let policy = AffineTriangleMeshCellsV1::new([2, 2]).unwrap();
    let production = MeshProductionLineageEnvelopeV1::from_affine_triangle_rectangle_v1_resources(
        policy,
        &geometry,
        &mesh,
        &correspondence,
    )
    .unwrap();
    production
        .validate_against_affine_triangle_rectangle_v1_resources(
            policy,
            &geometry,
            &mesh,
            &correspondence,
        )
        .unwrap();
    assert!(
        GeometryMeshCorrespondenceEnvelopeV1::from_adjacent_rectangle_partition_affine_triangles(
            &geometry,
            [1, 2]
        )
        .is_err()
    );

    let mut changed = mesh.canonical_json().unwrap();
    let position = changed
        .windows(3)
        .position(|window| window == b"0.5")
        .unwrap();
    changed.splice(position..position + 3, b"0.6".iter().copied());
    let changed = SimplicialMeshEnvelopeV1::from_json(&changed, Default::default());
    assert!(changed.is_err());

    let mut changed_correspondence = correspondence.clone();
    let WireCorrespondenceV1::AuthoredRegion(wire) = &mut changed_correspondence.wire else {
        panic!("partition correspondence must use the authored-region wire")
    };
    wire.mesh_sha256 = "00".repeat(32);
    assert!(
        changed_correspondence
            .validate_against_adjacent_rectangle_partition_affine_triangles(
                &geometry,
                &mesh,
                [2, 2],
            )
            .is_err()
    );
    assert!(
        production
            .validate_against_affine_triangle_rectangle_v1_resources(
                policy,
                &geometry,
                &mesh,
                &changed_correspondence,
            )
            .is_err()
    );
}
