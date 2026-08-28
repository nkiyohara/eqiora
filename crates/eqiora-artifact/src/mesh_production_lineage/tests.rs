use std::collections::BTreeMap;

use eqiora_geometry::{PlanarOperationGraph, PlanarTopologyHandle};

use super::*;

fn resources() -> (
    CanonicalGeometryV1,
    SimplicialMeshEnvelopeV1,
    GeometryMeshCorrespondenceEnvelopeV1,
) {
    let graph = PlanarOperationGraph::new();
    let rectangle = graph.rectangle([0.0, 2.2], [0.0, 0.41]).unwrap();
    let circle = graph.circle([0.2, 0.2], 0.05).unwrap();
    let fluid = graph.subtract(&rectangle, &circle).unwrap();
    let outer = rectangle.boundaries();
    let cut = circle.boundaries();
    let geometry = graph
        .build(
            &fluid,
            &BTreeMap::from([
                (
                    "fluid".to_owned(),
                    vec![PlanarTopologyHandle::from(fluid.region())],
                ),
                ("inlet".to_owned(), vec![outer[0].into()]),
                ("outlet".to_owned(), vec![outer[1].into()]),
                ("walls".to_owned(), vec![outer[2].into(), outer[3].into()]),
                ("cylinder".to_owned(), vec![cut[0].into()]),
            ]),
        )
        .unwrap();
    let policy = PlanarMeshQualityV1::new(1.0e-4, 1.0e-5, 50).unwrap();
    let (mesh, correspondence) =
        GeometryMeshCorrespondenceEnvelopeV1::from_planar_circular_hole_v2_reference(
            &geometry,
            policy.maximum_boundary_error_m(),
            policy.maximum_boundary_facets(),
            MeshQualityGate::new(policy.minimum_mean_ratio()).unwrap(),
        )
        .unwrap();
    (geometry, mesh, correspondence)
}

fn cartesian_resources() -> (
    CanonicalGeometryV1,
    CartesianMeshEnvelopeV1,
    GeometryMeshCorrespondenceEnvelopeV1,
) {
    let graph = PlanarOperationGraph::new();
    let rectangle = graph.rectangle([0.0, 2.0], [-1.0, 2.0]).unwrap();
    let edges = rectangle.boundaries();
    let geometry = graph
        .build(
            &rectangle,
            &BTreeMap::from([
                ("region".to_owned(), vec![rectangle.region().into()]),
                ("left".to_owned(), vec![edges[0].into()]),
                ("right".to_owned(), vec![edges[1].into()]),
                ("bottom".to_owned(), vec![edges[2].into()]),
                ("top".to_owned(), vec![edges[3].into()]),
            ]),
        )
        .unwrap();
    let (mesh, correspondence) =
        GeometryMeshCorrespondenceEnvelopeV1::from_planar_rectangle_v2_cartesian(&geometry, [2, 3])
            .unwrap();
    (geometry, mesh, correspondence)
}

fn affine_triangle_resources() -> (
    CanonicalGeometryV1,
    SimplicialMeshEnvelopeV1,
    GeometryMeshCorrespondenceEnvelopeV1,
) {
    let graph = PlanarOperationGraph::new();
    let rectangle = graph.rectangle([0.0, 2.0], [-1.0, 2.0]).unwrap();
    let edges = rectangle.boundaries();
    let geometry = graph
        .build(
            &rectangle,
            &BTreeMap::from([
                ("region".to_owned(), vec![rectangle.region().into()]),
                ("left".to_owned(), vec![edges[0].into()]),
                ("right".to_owned(), vec![edges[1].into()]),
                ("bottom".to_owned(), vec![edges[2].into()]),
                ("top".to_owned(), vec![edges[3].into()]),
            ]),
        )
        .unwrap();
    let (mesh, correspondence) =
        GeometryMeshCorrespondenceEnvelopeV1::from_planar_rectangle_v2_affine_triangles(
            &geometry,
            [2, 3],
        )
        .unwrap();
    (geometry, mesh, correspondence)
}

#[test]
fn registered_mesh_production_lineage_replays_and_rejects_mutations() {
    let (geometry, mesh, correspondence) = resources();
    let policy = PlanarMeshQualityV1::new(1.0e-4, 1.0e-5, 50).unwrap();
    let lineage =
        MeshProductionLineageEnvelopeV1::from_planar_circular_hole_reference_v1_resources(
            policy,
            &geometry,
            &mesh,
            &correspondence,
        )
        .unwrap();
    let bytes = lineage.canonical_json().unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["effective_policy"]["kind"], "planar-mesh-quality");
    assert_eq!(
        MeshProductionLineageEnvelopeV1::from_json(&bytes).unwrap(),
        lineage
    );
    assert!(
        lineage
            .validate_against_resources(
                MeshProductionProvider::Gmsh4152,
                policy,
                &geometry,
                &mesh,
                &correspondence,
            )
            .is_err()
    );
    for changed_policy in [
        PlanarMeshQualityV1::new(2.0e-4, 1.0e-5, 50).unwrap(),
        PlanarMeshQualityV1::new(1.0e-4, 2.0e-5, 50).unwrap(),
        PlanarMeshQualityV1::new(1.0e-4, 1.0e-5, 51).unwrap(),
    ] {
        assert!(
            lineage
                .validate_against_resources(
                    MeshProductionProvider::PlanarCircularHoleReferenceV1,
                    changed_policy,
                    &geometry,
                    &mesh,
                    &correspondence,
                )
                .is_err()
        );
    }
    let mut mutated: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    mutated["provider"]["version"] = serde_json::Value::String("2".to_owned());
    assert!(
        MeshProductionLineageEnvelopeV1::from_json(&serde_json::to_vec(&mutated).unwrap()).is_err()
    );

    let mut unknown: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    unknown["unexpected"] = true.into();
    assert!(
        MeshProductionLineageEnvelopeV1::from_json(&serde_json::to_vec(&unknown).unwrap()).is_err()
    );
    assert!(MeshProductionLineageEnvelopeV1::from_json(b"not-json").is_err());
    let mut noncanonical = bytes.clone();
    noncanonical.push(b'\n');
    assert!(MeshProductionLineageEnvelopeV1::from_json(&noncanonical).is_err());

    let resource_digests = [
        ArtifactDigest::from_sha256(geometry.digest_bytes()).to_string(),
        mesh.digest().unwrap().to_string(),
        correspondence.digest().unwrap().to_string(),
    ];
    for digest in resource_digests {
        let resource_mutation = std::str::from_utf8(&bytes)
            .unwrap()
            .replacen(&digest, &"0".repeat(64), 1)
            .into_bytes();
        let mutated_lineage =
            MeshProductionLineageEnvelopeV1::from_json(&resource_mutation).unwrap();
        assert!(
            mutated_lineage
                .validate_against_resources(
                    MeshProductionProvider::PlanarCircularHoleReferenceV1,
                    policy,
                    &geometry,
                    &mesh,
                    &correspondence,
                )
                .is_err()
        );
    }

    let (rectangle, cartesian_mesh, cartesian_correspondence) = cartesian_resources();
    let cells = CartesianMeshCellsV1::new([2, 3]).unwrap();
    let cartesian = MeshProductionLineageEnvelopeV1::from_structured_cartesian_v1_resources(
        cells,
        &rectangle,
        &cartesian_mesh,
        &cartesian_correspondence,
    )
    .unwrap();
    let cartesian_bytes = cartesian.canonical_json().unwrap();
    let cartesian_value: serde_json::Value = serde_json::from_slice(&cartesian_bytes).unwrap();
    assert_eq!(
        cartesian_value["effective_policy"],
        serde_json::json!({
            "kind": "cartesian-cells", "cells": [2, 3]
        })
    );
    cartesian
        .validate_against_structured_cartesian_v1_resources(
            cells,
            &rectangle,
            &cartesian_mesh,
            &cartesian_correspondence,
        )
        .unwrap();
    for changed_cells in [[3, 3], [2, 4]] {
        assert!(
            cartesian
                .validate_against_structured_cartesian_v1_resources(
                    CartesianMeshCellsV1::new(changed_cells).unwrap(),
                    &rectangle,
                    &cartesian_mesh,
                    &cartesian_correspondence,
                )
                .is_err()
        );
    }
    for invalid_cells in [[0, 3], [2, 0], [usize::MAX, 3], [2, usize::MAX]] {
        assert!(CartesianMeshCellsV1::new(invalid_cells).is_err());
    }

    let (foreign_geometry, foreign_mesh, foreign_correspondence) = {
        let graph = PlanarOperationGraph::new();
        let rectangle = graph.rectangle([0.0, 3.0], [-1.0, 2.0]).unwrap();
        let edges = rectangle.boundaries();
        let geometry = graph
            .build(
                &rectangle,
                &BTreeMap::from([
                    ("region".to_owned(), vec![rectangle.region().into()]),
                    ("left".to_owned(), vec![edges[0].into()]),
                    ("right".to_owned(), vec![edges[1].into()]),
                    ("bottom".to_owned(), vec![edges[2].into()]),
                    ("top".to_owned(), vec![edges[3].into()]),
                ]),
            )
            .unwrap();
        let (mesh, correspondence) =
            GeometryMeshCorrespondenceEnvelopeV1::from_planar_rectangle_v2_cartesian(
                &geometry,
                [2, 3],
            )
            .unwrap();
        (geometry, mesh, correspondence)
    };
    for resources in [
        (
            &foreign_geometry,
            &cartesian_mesh,
            &cartesian_correspondence,
        ),
        (&rectangle, &foreign_mesh, &cartesian_correspondence),
        (&rectangle, &cartesian_mesh, &foreign_correspondence),
    ] {
        assert!(
            cartesian
                .validate_against_structured_cartesian_v1_resources(
                    cells,
                    resources.0,
                    resources.1,
                    resources.2,
                )
                .is_err()
        );
    }
    let cartesian_resource_digests = [
        ArtifactDigest::from_sha256(rectangle.digest_bytes()).to_string(),
        cartesian_mesh.digest().unwrap().to_string(),
        cartesian_correspondence.digest().unwrap().to_string(),
    ];
    for digest in cartesian_resource_digests {
        let resource_mutation = std::str::from_utf8(&cartesian_bytes)
            .unwrap()
            .replacen(&digest, &"0".repeat(64), 1)
            .into_bytes();
        let mutated_lineage =
            MeshProductionLineageEnvelopeV1::from_json(&resource_mutation).unwrap();
        assert!(
            mutated_lineage
                .validate_against_structured_cartesian_v1_resources(
                    cells,
                    &rectangle,
                    &cartesian_mesh,
                    &cartesian_correspondence,
                )
                .is_err()
        );
    }
    let foreign_lineage = MeshProductionLineageEnvelopeV1::from_structured_cartesian_v1_resources(
        cells,
        &foreign_geometry,
        &foreign_mesh,
        &foreign_correspondence,
    )
    .unwrap();
    assert!(
        foreign_lineage
            .validate_against_structured_cartesian_v1_resources(
                cells,
                &rectangle,
                &cartesian_mesh,
                &cartesian_correspondence,
            )
            .is_err()
    );
    let mut provider_mismatch = cartesian_value.clone();
    provider_mismatch["provider"] = serde_json::json!({
        "identity": REFERENCE_IDENTITY, "version": REFERENCE_VERSION
    });
    assert!(
        MeshProductionLineageEnvelopeV1::from_json(
            &serde_json::to_vec(&provider_mismatch).unwrap()
        )
        .is_err()
    );
}

#[test]
fn affine_triangle_lineage_replays_and_rejects_policy_provider_and_resource_mutation() {
    let (geometry, mesh, correspondence) = affine_triangle_resources();
    let policy = AffineTriangleMeshCellsV1::new([2, 3]).unwrap();
    let lineage = MeshProductionLineageEnvelopeV1::from_affine_triangle_rectangle_v1_resources(
        policy,
        &geometry,
        &mesh,
        &correspondence,
    )
    .unwrap();
    let bytes = lineage.canonical_json().unwrap();
    assert_eq!(
        MeshProductionLineageEnvelopeV1::from_json(&bytes).unwrap(),
        lineage
    );
    lineage
        .validate_against_affine_triangle_rectangle_v1_resources(
            policy,
            &geometry,
            &mesh,
            &correspondence,
        )
        .unwrap();

    let mut diagonal: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    diagonal["effective_policy"]["diagonal"] = "upper-left-to-lower-right".into();
    assert!(
        MeshProductionLineageEnvelopeV1::from_json(&serde_json::to_vec(&diagonal).unwrap())
            .is_err()
    );

    let mut provider: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    provider["provider"]["identity"] = CARTESIAN_IDENTITY.into();
    assert!(
        MeshProductionLineageEnvelopeV1::from_json(&serde_json::to_vec(&provider).unwrap())
            .is_err()
    );

    let counts_bytes = std::str::from_utf8(&bytes)
        .unwrap()
        .replace("\"cells\":[2,3]", "\"cells\":[3,2]")
        .into_bytes();
    let counts = MeshProductionLineageEnvelopeV1::from_json(&counts_bytes).unwrap();
    assert!(
        counts
            .validate_against_affine_triangle_rectangle_v1_resources(
                policy,
                &geometry,
                &mesh,
                &correspondence,
            )
            .is_err()
    );

    for digest in [
        ArtifactDigest::from_sha256(geometry.digest_bytes()).to_string(),
        mesh.digest().unwrap().to_string(),
        correspondence.digest().unwrap().to_string(),
    ] {
        let resource_bytes = std::str::from_utf8(&bytes)
            .unwrap()
            .replacen(&digest, &"0".repeat(64), 1)
            .into_bytes();
        let resource = MeshProductionLineageEnvelopeV1::from_json(&resource_bytes).unwrap();
        assert!(
            resource
                .validate_against_affine_triangle_rectangle_v1_resources(
                    policy,
                    &geometry,
                    &mesh,
                    &correspondence,
                )
                .is_err()
        );
    }
}
