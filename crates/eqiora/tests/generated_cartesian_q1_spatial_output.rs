use eqiora::api::{MixedBoundaryElasticityResult2d, ModelDocument};
use eqiora::artifact::{
    CartesianMeshEnvelopeV1, CartesianQ1FieldSnapshotEnvelopeV1, GeometryIdentityEnvelopeV1,
    GeometryMeshCorrespondenceEnvelopeV1, RealizationEnvelopeV1,
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
        let expected: Vec<usize> = match (boundary.axis(), boundary.side()) {
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

#[test]
fn linked_replay_rejects_foreign_support_membership_drift_and_huge_realization() {
    let (_, result) = accepted();
    let geometry = result.geometry();
    let mesh = result.mesh_artifact();
    let correspondence = result.correspondence();
    let snapshot = result.displacement_snapshot();

    let snapshot_json: Value = serde_json::from_slice(&snapshot.canonical_json().unwrap()).unwrap();
    let mut wrong_support = snapshot_json.clone();
    wrong_support["support_domain_ulid"] =
        json!(geometry.boundaries()[0].domain().ulid().to_string());
    let wrong_support = CartesianQ1FieldSnapshotEnvelopeV1::from_json(
        &serde_json::to_vec(&wrong_support).unwrap(),
        Default::default(),
    )
    .expect("a different Model-owned Domain is locally valid snapshot grammar");
    assert!(
        wrong_support
            .validate_against(
                result.model(),
                result.realization(),
                geometry,
                correspondence,
                mesh,
            )
            .is_err(),
        "a boundary Domain must not replay as the displacement Field's body support",
    );

    let correspondence_json: Value =
        serde_json::from_slice(&correspondence.canonical_json().unwrap()).unwrap();
    let mut missing_body_cell = correspondence_json.clone();
    missing_body_cell["bodies"][0]["cell_indices"]
        .as_array_mut()
        .unwrap()
        .pop()
        .expect("accepted body membership is nonempty");
    assert_cartesian_correspondence_replay_rejects(
        &missing_body_cell,
        &result,
        "body membership omitted one Cartesian cell",
    );

    let mut missing_boundary_facet = correspondence_json;
    missing_boundary_facet["boundaries"][0]["facet_indices"]
        .as_array_mut()
        .unwrap()
        .pop()
        .expect("accepted boundary membership is nonempty");
    assert_cartesian_correspondence_replay_rejects(
        &missing_boundary_facet,
        &result,
        "boundary membership omitted one Cartesian facet",
    );

    let mut huge_realization: Value =
        serde_json::from_slice(&result.realization().canonical_json().unwrap()).unwrap();
    huge_realization["plan"]["discretization"]["mesh"]["cells_per_axis"] = json!(1_000_000_000_u64);
    let huge_realization = RealizationEnvelopeV1::from_json(
        &serde_json::to_vec(&huge_realization).unwrap(),
        Default::default(),
    )
    .expect("a large positive generated-uniform count is valid Realization grammar");
    assert_ne!(
        huge_realization.digest().unwrap(),
        result.realization().digest().unwrap(),
    );
    assert!(
        snapshot
            .validate_against(
                result.model(),
                &huge_realization,
                geometry,
                correspondence,
                mesh,
            )
            .is_err(),
        "linked replay must reject the foreign Realization identity before constructing its \
         billion-by-billion generated mesh",
    );
}

fn assert_cartesian_correspondence_replay_rejects(
    json: &Value,
    result: &MixedBoundaryElasticityResult2d,
    mutation: &str,
) {
    let decoded = GeometryMeshCorrespondenceEnvelopeV1::from_json(
        &serde_json::to_vec(json).unwrap(),
        Default::default(),
    )
    .expect("incomplete membership is locally valid correspondence grammar");
    assert!(
        decoded
            .validate_against_cartesian(result.geometry(), result.model(), result.mesh_artifact())
            .is_err(),
        "exact Cartesian replay admitted drift: {mutation}",
    );
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
    let mut depth = 0_usize;
    let mut string_start = None;
    let mut escaped = false;
    let mut observed = Vec::new();

    for (index, byte) in bytes.iter().copied().enumerate() {
        if let Some(start) = string_start {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                string_start = None;
                let next = bytes[index + 1..]
                    .iter()
                    .copied()
                    .find(|next| !next.is_ascii_whitespace());
                if depth == 1 && next == Some(b':') {
                    let encoded = &bytes[start..=index];
                    observed.push(
                        serde_json::from_slice::<String>(encoded)
                            .expect("canonical JSON object key is a string"),
                    );
                }
            }
            continue;
        }

        match byte {
            b'"' => string_start = Some(index),
            b'{' | b'[' => depth += 1,
            b'}' | b']' => depth = depth.checked_sub(1).expect("balanced canonical JSON"),
            _ => {}
        }
    }

    assert_eq!(depth, 0, "canonical JSON containers are balanced");
    assert!(
        string_start.is_none(),
        "canonical JSON string is terminated"
    );
    assert_eq!(
        observed, keys,
        "canonical JSON top-level key sequence changed",
    );
}
