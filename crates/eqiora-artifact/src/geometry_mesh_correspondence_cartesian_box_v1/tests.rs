use std::collections::BTreeMap;

use eqiora_geometry::{GeometryGraph, PlanarTopologyHandle};
use eqiora_meshing::{MeshEntity, MeshTopology, OrientationCode};

use super::*;
use crate::{CartesianMeshCellsV2, MeshProductionLineageEnvelopeV1};

fn interval(lower: f64, upper: f64) -> CanonicalGeometryV1 {
    let graph = GeometryGraph::new();
    let operation = graph.interval([lower, upper]).unwrap();
    let [left, right]: [_; 2] = operation.boundaries().try_into().unwrap();
    graph
        .build(
            &operation,
            &BTreeMap::from([
                (
                    "body".to_owned(),
                    vec![PlanarTopologyHandle::from(operation.region())],
                ),
                ("left".to_owned(), vec![PlanarTopologyHandle::from(left)]),
                ("right".to_owned(), vec![PlanarTopologyHandle::from(right)]),
            ]),
        )
        .unwrap()
}

#[test]
fn registered_interval_cartesian_common_mesh_evidence() {
    let geometry = interval(-1.0, 2.0);
    let policy = CartesianMeshCellsV2::new([3]).unwrap();
    let (mesh, correspondence) =
        GeometryMeshCorrespondenceEnvelopeV1::from_cartesian_box_v1(&geometry, policy.cells())
            .unwrap();
    let production = MeshProductionLineageEnvelopeV1::from_structured_cartesian_v2_resources(
        &policy,
        &geometry,
        &mesh,
        &correspondence,
    )
    .unwrap();

    assert_eq!(mesh.dimension(), 1);
    assert_eq!(mesh.mesh().entity_count(0), Some(4));
    assert_eq!(mesh.mesh().entity_count(1), Some(3));
    assert_eq!(
        (0..4)
            .map(|index| mesh
                .mesh()
                .vertex_coordinates(MeshEntity::new(0, index))
                .unwrap())
            .collect::<Vec<_>>(),
        vec![vec![-1.0], vec![0.0], vec![1.0], vec![2.0]],
    );
    assert_eq!(
        (0..3)
            .map(|index| {
                mesh.mesh()
                    .entity_vertices(MeshEntity::new(1, index))
                    .unwrap()
                    .into_iter()
                    .map(MeshEntity::index)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>(),
        vec![vec![0, 1], vec![1, 2], vec![2, 3]],
    );
    let left = correspondence
        .cartesian_box_v1_entity_set_entities(&geometry, "left")
        .unwrap();
    let right = correspondence
        .cartesian_box_v1_entity_set_entities(&geometry, "right")
        .unwrap();
    assert_eq!(left, vec![MeshEntity::new(0, 0)]);
    assert_eq!(right, vec![MeshEntity::new(0, 3)]);
    for (facet, parent, ordinal) in [(left[0], 0, 0), (right[0], 2, 1)] {
        let incidence = mesh.mesh().incidence(facet, 1).unwrap();
        assert_eq!(incidence.len(), 1);
        assert_eq!(incidence[0].entity, MeshEntity::new(1, parent));
        assert_eq!(incidence[0].local_ordinal, ordinal);
        assert_eq!(incidence[0].orientation, OrientationCode::identity());
    }

    correspondence
        .validate_against_cartesian_box_v1(&geometry, &mesh, policy.cells())
        .unwrap();
    production
        .validate_against_structured_cartesian_v2_resources(
            &policy,
            &geometry,
            &mesh,
            &correspondence,
        )
        .unwrap();
    let decoded = GeometryMeshCorrespondenceEnvelopeV1::from_json(
        &correspondence.canonical_json().unwrap(),
        GeometryDecoderLimits::default(),
    )
    .unwrap();
    assert_eq!(decoded, correspondence);

    assert!(CartesianMeshCellsV2::new(Vec::<usize>::new()).is_err());
    assert!(CartesianMeshCellsV2::new([0]).is_err());
    assert!(CartesianMeshCellsV2::new([1, 1, 1, 1]).is_err());
    assert!(
        correspondence
            .validate_against_cartesian_box_v1(&interval(-1.0, 3.0), &mesh, policy.cells())
            .is_err()
    );
    assert!(
        correspondence
            .validate_against_cartesian_box_v1(&geometry, &mesh, &[4])
            .is_err()
    );

    let mut mutated = correspondence.clone();
    let WireCorrespondenceV1::CartesianBoxV1(wire) = &mut mutated.wire else {
        unreachable!()
    };
    wire.sides[0].facet_indices[0] = 1;
    assert!(
        mutated
            .validate_against_cartesian_box_v1(&geometry, &mesh, policy.cells())
            .is_err()
    );
}
