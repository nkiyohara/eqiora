use std::sync::Mutex;

use eqiora_artifact::{CartesianMeshDecoderLimits, CartesianMeshEnvelopeV1, MeshDecoderLimits};
use eqiora_core::diagnostic::codes;
use eqiora_meshing::{CartesianMesh, MeshTopology};
use serde_json::json;

const CARTESIAN_17_GOLDEN: &[u8] = br#"{"schema":"eqiora.cartesian-mesh-envelope/v1","encoding":"eqiora.canonical-json/v1","dimension":2,"scalar":"f64","cell_family":"hypercube","axes":[[0.0,0.0625,0.125,0.1875,0.25,0.3125,0.375,0.4375,0.5,0.5625,0.625,0.6875,0.75,0.8125,0.875,0.9375,1.0],[0.0,0.0625,0.125,0.1875,0.25,0.3125,0.375,0.4375,0.5,0.5625,0.625,0.6875,0.75,0.8125,0.875,0.9375,1.0]],"vertex_order":"last-axis-fastest","cell_order":"last-axis-fastest","local_node_order":"tensor-product-z"}"#;
const CARTESIAN_17_DIGEST: &str =
    "bed27889112148bf1934de2634b2251011fa92d8ea6ed13447b6fb4c9aeaca09";

static HEAVY_WITNESS_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn defaults_exact_limits_and_nonuniform_literals_are_frozen() {
    let defaults = CartesianMeshDecoderLimits::default();
    assert_eq!(defaults.mesh, MeshDecoderLimits::default());
    assert_eq!(defaults.max_cartesian_entities, 1_000_000);
    assert_eq!(defaults.max_cartesian_entity_vertex_references, 8_000_000);

    let three_by_three = wire_for_axis_counts(&[3, 3]);
    let exact = CartesianMeshEnvelopeV1::from_json_with_limits(&three_by_three, limits(25, 49))
        .expect("the inclusive E=25 and R=49 limits must admit [3,3]");
    assert_eq!(exact.mesh().entity_count(0), Some(9));
    assert_eq!(exact.mesh().entity_count(2), Some(4));

    assert_explicit_error(
        &three_by_three,
        limits(24, 49),
        "Cartesian mesh entity count 25 exceeds decoder limit 24",
    );
    assert_explicit_error(
        &three_by_three,
        limits(25, 48),
        "Cartesian mesh entity-closure vertex-reference count 49 exceeds decoder limit 48",
    );

    let nonuniform = wire_for_axis_counts(&[2, 3, 5]);
    let exact = CartesianMeshEnvelopeV1::from_json_with_limits(&nonuniform, limits(135, 364))
        .expect("the literal nonuniform E=135 and R=364 limits must admit [2,3,5]");
    assert_eq!(exact.mesh().entity_count(0), Some(30));
    assert_eq!(exact.mesh().entity_count(3), Some(8));
    assert_explicit_error(
        &nonuniform,
        limits(134, 364),
        "Cartesian mesh entity count 135 exceeds decoder limit 134",
    );
    assert_explicit_error(
        &nonuniform,
        limits(135, 363),
        "Cartesian mesh entity-closure vertex-reference count 364 exceeds decoder limit 363",
    );
}

#[test]
fn default_policy_uses_all_strata_and_all_entity_closures() {
    let _heavy_witness = HEAVY_WITNESS_LOCK.lock().unwrap();

    let entity_boundary = wire_for_axis_counts(&[16, 16, 16, 16]);
    let accepted =
        CartesianMeshEnvelopeV1::from_json(&entity_boundary, MeshDecoderLimits::default())
            .expect("E=923521 and R=4477456 must remain inside the inclusive defaults");
    assert_eq!(accepted.mesh().entity_count(0), Some(65_536));
    assert_eq!(accepted.mesh().entity_count(4), Some(50_625));
    drop(accepted);

    assert_default_error(
        &wire_for_axis_counts(&[17, 17, 17, 17]),
        "Cartesian mesh entity count 1185921 exceeds decoder limit 1000000",
    );

    let reference_boundary = wire_for_axis_counts(&[4, 4, 4, 4, 4, 4, 3]);
    let accepted =
        CartesianMeshEnvelopeV1::from_json(&reference_boundary, MeshDecoderLimits::default())
            .expect("E=588245 and R=7000000 must remain inside the inclusive defaults");
    assert_eq!(accepted.mesh().entity_count(0), Some(12_288));
    assert_eq!(accepted.mesh().entity_count(7), Some(1_458));
    drop(accepted);

    assert_default_error(
        &wire_for_axis_counts(&[4, 4, 4, 4, 4, 4, 4]),
        "Cartesian mesh entity-closure vertex-reference count 10000000 exceeds decoder limit 8000000",
    );

    assert_default_one_of(
        &wire_for_axis_counts(&[1_000, 1_000]),
        &[
            "Cartesian mesh entity count 3996001 exceeds decoder limit 1000000",
            "Cartesian mesh entity-closure vertex-reference count 8988004 exceeds decoder limit 8000000",
        ],
    );
    assert_default_one_of(
        &wire_for_axis_counts(&[100, 100, 100]),
        &[
            "Cartesian mesh entity count 7880599 exceeds decoder limit 1000000",
            "Cartesian mesh entity-closure vertex-reference count 26463592 exceeds decoder limit 8000000",
        ],
    );
}

#[test]
fn overflow_and_preconstruction_diagnostic_ownership_are_frozen() {
    assert_default_error(
        &wire_for_axis_counts(&[2; 12]),
        "Cartesian mesh entity-closure vertex-reference count 16777216 exceeds decoder limit 8000000",
    );

    assert_explicit_error(
        &wire_for_axis_counts(&[2; 32]),
        overflow_probe_limits(),
        "Cartesian mesh entity-closure vertex-reference count overflows usize",
    );

    let error = CartesianMeshEnvelopeV1::from_json_with_limits(
        &wire_for_axis_counts(&[2; 64]),
        overflow_probe_limits(),
    )
    .expect_err("64 two-point axes must fail a checked Cartesian product");
    assert_eq!(error.code(), codes::INVALID_ARTIFACT);
    assert!(
        [
            "Cartesian mesh entity count overflows usize",
            "Cartesian mesh entity-closure vertex-reference count overflows usize",
        ]
        .contains(&error.message()),
        "overflow escaped the Cartesian-specific preflight: {}",
        error.message(),
    );
}

#[test]
fn trusted_501_capture_requires_explicit_replay_policy() {
    let _heavy_witness = HEAVY_WITNESS_LOCK.lock().unwrap();

    let axis = axis(501);
    let source = CartesianMesh::from_axes(vec![axis.clone(), axis])
        .expect("the trusted 501x501 source remains below meshing hard caps");
    assert_eq!(source.entity_count(0), Some(251_001));
    assert_eq!(source.entity_count(2), Some(250_000));

    let captured = CartesianMeshEnvelopeV1::from_mesh(&source)
        .expect("trusted capture must not apply the untrusted E/R defaults");
    drop(source);
    let canonical = captured.canonical_json().unwrap();
    let digest = captured.digest().unwrap();

    assert_default_error(
        &canonical,
        "Cartesian mesh entity count 1002001 exceeds decoder limit 1000000",
    );

    let replay =
        CartesianMeshEnvelopeV1::from_json_with_limits(&canonical, limits(1_002_001, 2_253_001))
            .expect("an exact admitting policy must replay the trusted capture");
    assert_eq!(replay.canonical_json().unwrap(), canonical);
    assert_eq!(replay.digest().unwrap(), digest);
    assert_eq!(replay.mesh(), captured.mesh());
}

#[test]
fn existing_17_by_17_bytes_digest_and_order_remain_exact() {
    let decoded =
        CartesianMeshEnvelopeV1::from_json(CARTESIAN_17_GOLDEN, MeshDecoderLimits::default())
            .expect("the accepted 17x17 artifact must remain inside default policy");
    assert_eq!(decoded.canonical_json().unwrap(), CARTESIAN_17_GOLDEN);
    assert_eq!(decoded.digest().unwrap().as_str(), CARTESIAN_17_DIGEST);

    let captured = CartesianMeshEnvelopeV1::from_mesh(
        &CartesianMesh::uniform(&[[0.0, 1.0], [0.0, 1.0]], &[16, 16]).unwrap(),
    )
    .unwrap();
    assert_eq!(captured.canonical_json().unwrap(), CARTESIAN_17_GOLDEN);
    assert_eq!(captured.digest().unwrap().as_str(), CARTESIAN_17_DIGEST);
    assert_eq!(captured.mesh(), decoded.mesh());
}

fn limits(
    max_cartesian_entities: usize,
    max_cartesian_entity_vertex_references: usize,
) -> CartesianMeshDecoderLimits {
    CartesianMeshDecoderLimits {
        mesh: MeshDecoderLimits::default(),
        max_cartesian_entities,
        max_cartesian_entity_vertex_references,
    }
}

fn overflow_probe_limits() -> CartesianMeshDecoderLimits {
    CartesianMeshDecoderLimits {
        mesh: MeshDecoderLimits {
            max_mesh_vertices: usize::MAX,
            max_mesh_cells: usize::MAX,
            max_mesh_coordinate_values: usize::MAX,
            max_mesh_connectivity_indices: usize::MAX,
            ..MeshDecoderLimits::default()
        },
        max_cartesian_entities: usize::MAX,
        max_cartesian_entity_vertex_references: usize::MAX,
    }
}

fn assert_default_error(bytes: &[u8], expected_message: &str) {
    let error = CartesianMeshEnvelopeV1::from_json(bytes, MeshDecoderLimits::default())
        .expect_err("the frozen default Cartesian budget must reject this witness");
    assert_eq!(error.code(), codes::INVALID_ARTIFACT);
    assert_eq!(error.message(), expected_message);
}

fn assert_default_one_of(bytes: &[u8], expected_messages: &[&str]) {
    let error = CartesianMeshEnvelopeV1::from_json(bytes, MeshDecoderLimits::default())
        .expect_err("the frozen default Cartesian budget must reject this witness");
    assert_eq!(error.code(), codes::INVALID_ARTIFACT);
    assert!(
        expected_messages.contains(&error.message()),
        "the default rejection was not owned by an exact Cartesian budget: {}",
        error.message(),
    );
}

fn assert_explicit_error(bytes: &[u8], limits: CartesianMeshDecoderLimits, expected_message: &str) {
    let error = CartesianMeshEnvelopeV1::from_json_with_limits(bytes, limits)
        .expect_err("the frozen explicit Cartesian budget must reject this witness");
    assert_eq!(error.code(), codes::INVALID_ARTIFACT);
    assert_eq!(error.message(), expected_message);
}

fn wire_for_axis_counts(axis_counts: &[usize]) -> Vec<u8> {
    let axes = axis_counts.iter().copied().map(axis).collect::<Vec<_>>();
    serde_json::to_vec(&json!({
        "schema": "eqiora.cartesian-mesh-envelope/v1",
        "encoding": "eqiora.canonical-json/v1",
        "dimension": axis_counts.len() as u64,
        "scalar": "f64",
        "cell_family": "hypercube",
        "axes": axes,
        "vertex_order": "last-axis-fastest",
        "cell_order": "last-axis-fastest",
        "local_node_order": "tensor-product-z",
    }))
    .unwrap()
}

fn axis(vertex_count: usize) -> Vec<f64> {
    (0..vertex_count).map(|index| index as f64).collect()
}
