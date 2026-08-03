use eqiora::api::{MixedBoundaryElasticityResult2d, ModelDocument};
use eqiora::artifact::{
    CartesianMeshEnvelopeV1, CartesianQ1FieldSnapshotEnvelopeV1, GeometryIdentityEnvelopeV1,
    GeometryMeshCorrespondenceEnvelopeV1,
};
use eqiora::kernel::BoundarySide;
use eqiora::meshing::{MeshEntity, MeshTopology};
use eqiora::solver::REFERENCE_LINEAR_SOLVER;
use serde_json::{Value, json};

const SOURCE: &str =
    include_str!("../../../verify/solid/mixed-boundary-elasticity-2d/models/direct.eqi");
const CELLS_PER_AXIS: usize = 16;
const VERTICES_PER_AXIS: usize = CELLS_PER_AXIS + 1;

fn accepted() -> (ModelDocument, MixedBoundaryElasticityResult2d) {
    let document = ModelDocument::compile("mixed-boundary-elasticity.eqi", SOURCE)
        .expect("accepted direct Model");
    let result =
        MixedBoundaryElasticityResult2d::solve_reference(&document, &REFERENCE_LINEAR_SOLVER)
            .expect("accepted mixed-boundary result");
    (document, result)
}

#[test]
fn generated_cartesian_q1_output_closes_exact_spatial_lineage() {
    let (document, result) = accepted();
    let model = result.model();
    let realization = result.realization();
    let geometry = result.geometry();
    let mesh_artifact = result.mesh_artifact();
    let correspondence = result.correspondence();
    let snapshot = result.displacement_snapshot();
    let mesh = mesh_artifact.mesh();

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
                    .expect("Q1 cell")
                    .into_iter()
                    .map(|vertex| vertex.index())
                    .collect::<Vec<_>>(),
                [lower, lower + 17, lower + 1, lower + 18],
                "cell({i},{j}) must retain tensor-product/Z local order",
            );
        }
    }

    assert_eq!(geometry.model_artifact(), model.digest().unwrap());
    geometry.validate_against(model).unwrap();
    assert_eq!(geometry.bodies().len(), 1);
    assert_eq!(geometry.boundaries().len(), 4);
    let body = geometry.bodies()[0].domain();
    assert_eq!(
        correspondence.body_cells(body).unwrap(),
        (0..256).collect::<Vec<_>>(),
    );
    for boundary in geometry.boundaries() {
        let expected = match (boundary.axis(), boundary.side()) {
            (0, BoundarySide::Lower) => (0..16).map(|t| 272 + t).collect(),
            (0, BoundarySide::Upper) => (0..16).map(|t| 528 + t).collect(),
            (1, BoundarySide::Lower) => (0..16).map(|t| 17 * t).collect(),
            (1, BoundarySide::Upper) => (0..16).map(|t| 17 * t + 16).collect(),
            other => panic!("unexpected Cartesian boundary role {other:?}"),
        };
        assert_eq!(
            correspondence.boundary_facets(boundary.domain()).unwrap(),
            expected
        );
    }

    assert_eq!(
        correspondence.geometry_artifact(),
        geometry.digest().unwrap()
    );
    assert_eq!(
        correspondence.mesh_artifact(),
        mesh_artifact.digest().unwrap()
    );
    assert_eq!(snapshot.model_artifact(), model.digest().unwrap());
    assert_eq!(
        snapshot.realization_artifact(),
        realization.digest().unwrap()
    );
    assert_eq!(snapshot.geometry_artifact(), geometry.digest().unwrap());
    assert_eq!(
        snapshot.correspondence_artifact(),
        correspondence.digest().unwrap(),
    );
    assert_eq!(snapshot.mesh_artifact(), mesh_artifact.digest().unwrap());
    assert_eq!(
        snapshot.field(),
        document.field_ref("displacement").unwrap().id()
    );
    assert_eq!(snapshot.support_domain(), body);
    correspondence
        .validate_against_cartesian(geometry, model, mesh_artifact)
        .unwrap();

    let coefficients = snapshot.coefficients();
    assert_eq!(coefficients.len(), 578);
    assert_eq!(result.displacements_m().len(), 289);
    assert!(
        coefficients
            .iter()
            .filter(|value| **value == 0.0)
            .all(|value| value.to_bits() == 0),
        "every serialized mathematical zero is canonical positive zero",
    );
    let projected = result
        .displacements_m()
        .iter()
        .flat_map(|value| value.iter());
    assert!(
        coefficients
            .iter()
            .zip(projected)
            .all(|(artifact, solver)| artifact.to_bits() == solver.to_bits()),
        "snapshot coefficients are the exact entity-major solver projection",
    );

    let snapshot_digest = snapshot.digest().unwrap();
    assert_eq!(result.run().outputs(), [snapshot_digest]);
}

#[test]
fn cartesian_mesh_and_q1_snapshot_round_trip_canonically_and_reject_mutants() {
    let (_, result) = accepted();
    let geometry = result.geometry();
    let mesh = result.mesh_artifact();
    let correspondence = result.correspondence();
    let snapshot = result.displacement_snapshot();

    let mesh_bytes = mesh.canonical_json().unwrap();
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
    assert_eq!(mesh_json.as_object().unwrap().len(), 9);
    assert_eq!(mesh_json["schema"], "eqiora.cartesian-mesh-envelope/v1");
    assert_eq!(mesh_json["encoding"], "eqiora.canonical-json/v1");
    assert_eq!(mesh_json["dimension"], 2);
    assert_eq!(mesh_json["scalar"], "f64");
    assert_eq!(mesh_json["cell_family"], "hypercube");
    assert_eq!(mesh_json["axes"], json!([axis(), axis()]));
    assert_eq!(mesh_json["vertex_order"], "last-axis-fastest");
    assert_eq!(mesh_json["cell_order"], "last-axis-fastest");
    assert_eq!(mesh_json["local_node_order"], "tensor-product-z");
    let decoded_mesh = CartesianMeshEnvelopeV1::from_json(&mesh_bytes, Default::default()).unwrap();
    assert_eq!(decoded_mesh.canonical_json().unwrap(), mesh_bytes);
    assert_eq!(decoded_mesh.digest().unwrap(), mesh.digest().unwrap());

    let mut wrong_axis = mesh_json.clone();
    wrong_axis["axes"][0].as_array_mut().unwrap().reverse();
    assert!(
        CartesianMeshEnvelopeV1::from_json(
            &serde_json::to_vec(&wrong_axis).unwrap(),
            Default::default()
        )
        .is_err()
    );

    let mut transposed_order = mesh_json;
    transposed_order["vertex_order"] = json!("first-axis-fastest");
    assert!(
        CartesianMeshEnvelopeV1::from_json(
            &serde_json::to_vec(&transposed_order).unwrap(),
            Default::default()
        )
        .is_err()
    );

    let geometry_bytes = geometry.canonical_json().unwrap();
    let decoded_geometry =
        GeometryIdentityEnvelopeV1::from_json(&geometry_bytes, Default::default()).unwrap();
    assert_eq!(decoded_geometry.canonical_json().unwrap(), geometry_bytes);
    let correspondence_bytes = correspondence.canonical_json().unwrap();
    let decoded_correspondence =
        GeometryMeshCorrespondenceEnvelopeV1::from_json(&correspondence_bytes, Default::default())
            .unwrap();
    assert_eq!(
        decoded_correspondence.canonical_json().unwrap(),
        correspondence_bytes,
    );
    decoded_correspondence
        .validate_against_cartesian(&decoded_geometry, result.model(), &decoded_mesh)
        .unwrap();

    let snapshot_bytes = snapshot.canonical_json().unwrap();
    assert_top_level_key_order(
        &snapshot_bytes,
        &[
            "schema",
            "encoding",
            "model_sha256",
            "model_ulid",
            "semantic_revision",
            "realization_sha256",
            "geometry_sha256",
            "correspondence_sha256",
            "mesh_sha256",
            "field_ulid",
            "support_domain_ulid",
            "association",
            "space",
            "scalar",
            "value_shape",
            "dimension",
            "frame",
            "ordering",
            "coefficients",
        ],
    );
    let snapshot_json: Value = serde_json::from_slice(&snapshot_bytes).unwrap();
    assert_eq!(snapshot_json.as_object().unwrap().len(), 19);
    assert_eq!(
        snapshot_json["schema"],
        "eqiora.cartesian-q1-field-snapshot-envelope/v1"
    );
    assert_eq!(snapshot_json["association"], "vertex");
    assert_eq!(
        snapshot_json["space"],
        json!({"family": "continuous-lagrange", "order": 1})
    );
    assert_eq!(snapshot_json["scalar"], "f64");
    assert_eq!(snapshot_json["value_shape"], json!([2]));
    assert_eq!(
        snapshot_json["dimension"],
        json!({
            "mass": 0, "length": 1, "time": 0, "current": 0,
            "temperature": 0, "amount": 0, "luminous_intensity": 0,
        })
    );
    assert_eq!(snapshot_json["frame"], "spatial-cartesian");
    assert_eq!(snapshot_json["ordering"], "entity-major-component-last");
    assert_eq!(snapshot_json["coefficients"].as_array().unwrap().len(), 578);
    let decoded_snapshot =
        CartesianQ1FieldSnapshotEnvelopeV1::from_json(&snapshot_bytes, Default::default()).unwrap();
    assert_eq!(decoded_snapshot.canonical_json().unwrap(), snapshot_bytes);
    assert_eq!(
        decoded_snapshot.digest().unwrap(),
        snapshot.digest().unwrap()
    );
    decoded_snapshot
        .validate_against(
            result.model(),
            result.realization(),
            geometry,
            correspondence,
            mesh,
        )
        .unwrap();

    let mut negative_zero = snapshot_json.clone();
    negative_zero["coefficients"][1] = json!(-0.0);
    assert!(
        CartesianQ1FieldSnapshotEnvelopeV1::from_json(
            &serde_json::to_vec(&negative_zero).unwrap(),
            Default::default(),
        )
        .is_err()
    );

    for (key, value) in [
        ("association", json!("cell")),
        ("space", json!({"family": "simplex-lagrange", "order": 1})),
    ] {
        let mut wrong_metadata = snapshot_json.clone();
        wrong_metadata[key] = value;
        assert!(
            CartesianQ1FieldSnapshotEnvelopeV1::from_json(
                &serde_json::to_vec(&wrong_metadata).unwrap(),
                Default::default(),
            )
            .is_err(),
            "specialized Cartesian Q1 snapshot admitted wrong {key}",
        );
    }

    let mut scalar = snapshot_json.clone();
    scalar["value_shape"] = json!([]);
    scalar["coefficients"] = Value::Array(
        snapshot_json["coefficients"]
            .as_array()
            .unwrap()
            .iter()
            .step_by(2)
            .cloned()
            .collect(),
    );
    assert_locally_valid_but_wrong_for_displacement(&scalar, &result);

    for (key, value) in [
        (
            "dimension",
            json!({
                "mass": 0, "length": 0, "time": 0, "current": 0,
                "temperature": 0, "amount": 0, "luminous_intensity": 0,
            }),
        ),
        ("frame", json!("invariant")),
    ] {
        let mut different_physical_tuple = snapshot_json.clone();
        different_physical_tuple[key] = value;
        assert_locally_valid_but_wrong_for_displacement(&different_physical_tuple, &result);
    }

    for (key, digest) in [
        ("model_sha256", result.realization().digest().unwrap()),
        ("realization_sha256", result.model().digest().unwrap()),
        ("geometry_sha256", result.model().digest().unwrap()),
        ("correspondence_sha256", result.model().digest().unwrap()),
        ("mesh_sha256", result.model().digest().unwrap()),
    ] {
        let mut stale_json = snapshot_json.clone();
        stale_json[key] = json!(digest.to_string());
        let stale = CartesianQ1FieldSnapshotEnvelopeV1::from_json(
            &serde_json::to_vec(&stale_json).unwrap(),
            Default::default(),
        )
        .expect("a locally well-formed stale digest remains untrusted until linked replay");
        assert!(
            stale
                .validate_against(
                    result.model(),
                    result.realization(),
                    geometry,
                    correspondence,
                    mesh,
                )
                .is_err(),
            "linked replay admitted stale {key}",
        );
    }
}

fn assert_locally_valid_but_wrong_for_displacement(
    json: &Value,
    result: &MixedBoundaryElasticityResult2d,
) {
    let decoded = CartesianQ1FieldSnapshotEnvelopeV1::from_json(
        &serde_json::to_vec(json).unwrap(),
        Default::default(),
    )
    .expect("scalar or different physical metadata is valid low-level Q1 snapshot grammar");
    assert!(
        decoded
            .validate_against(
                result.model(),
                result.realization(),
                result.geometry(),
                result.correspondence(),
                result.mesh_artifact(),
            )
            .is_err(),
        "foreign physical tuple matched the exact displacement Field",
    );
}

fn axis() -> Vec<f64> {
    (0..=CELLS_PER_AXIS)
        .map(|index| index as f64 / CELLS_PER_AXIS as f64)
        .collect()
}

fn assert_top_level_key_order(bytes: &[u8], keys: &[&str]) {
    let text = std::str::from_utf8(bytes).expect("canonical JSON is UTF-8");
    let mut cursor = 0;
    for key in keys {
        let needle = format!("\"{key}\":");
        let offset = text[cursor..]
            .find(&needle)
            .unwrap_or_else(|| panic!("canonical JSON omitted ordered key {key}"));
        cursor += offset + needle.len();
    }
}
