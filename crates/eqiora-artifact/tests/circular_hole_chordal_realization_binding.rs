use std::path::Path;
use std::process::Command;

use eqiora_artifact::{
    AcceptedCircularHoleChordalRealizationV1, ArtifactDigest,
    CircularHoleChordalRealizationEnvelopeV1, GeometryDefinitionV1,
    GeometryMeshCorrespondenceEnvelopeV1, JsonDecoderLimits, SimplicialMeshEnvelopeV1,
};
use eqiora_geometry::{
    CanonicalGeometryV1, EDGE_DIMENSION, FACE_DIMENSION, NamedEntitySet, PlanarRegion,
};
use eqiora_meshing::{MeshQualityGate, SimplicialMesh};
use sha2::{Digest, Sha256};

const SCHEMA: &str = "eqiora.circular-hole-chordal-realization-envelope/v1";
const BOUNDS: [[f64; 2]; 2] = [[0.0, 2.2], [0.0, 0.41]];
const CENTER: [f64; 2] = [0.2, 0.2];
const RADIUS_M: f64 = 0.05;
const SOURCE_TOLERANCE_M: f64 = 1.0e-12;
const MAX_BOUNDARY_ERROR_M: f64 = 1.0e-4;
const MAX_SEGMENTS: usize = 50;
const REQUIRED_MINIMUM_MEAN_RATIO: f64 = 1.0e-5;
const ORACLE_SHA256: &str = "0351c223c8100a96f4d11babcf46200737554f996653e9e58ab3378fa6240a41";

const FIELD_ORDER: [&str; 13] = [
    "schema",
    "encoding",
    "source_geometry_sha256",
    "realized_geometry_sha256",
    "mesh_sha256",
    "correspondence_sha256",
    "requested_max_boundary_error_m",
    "boundary_evaluation_allowance_m",
    "boundary_error_bound_m",
    "circle_segments",
    "circle_area_deficit_m2",
    "circle_perimeter_deficit_m",
    "required_minimum_mean_ratio",
];

struct Resources {
    source: CanonicalGeometryV1,
    owner: AcceptedCircularHoleChordalRealizationV1,
    geometry: GeometryDefinitionV1,
    mesh: SimplicialMeshEnvelopeV1,
    correspondence: GeometryMeshCorrespondenceEnvelopeV1,
}

fn source(center: [f64; 2]) -> CanonicalGeometryV1 {
    CanonicalGeometryV1::from_circular_hole(
        BOUNDS,
        center,
        RADIUS_M,
        vec![
            NamedEntitySet::new("inlet", EDGE_DIMENSION, vec![0]),
            NamedEntitySet::new("outlet", EDGE_DIMENSION, vec![1]),
            NamedEntitySet::new("walls", EDGE_DIMENSION, vec![2, 3]),
            NamedEntitySet::new("cylinder", EDGE_DIMENSION, vec![4]),
            NamedEntitySet::new("fluid", FACE_DIMENSION, vec![0]),
        ],
        SOURCE_TOLERANCE_M,
    )
    .expect("exact circular-hole source")
}

fn resources(required_minimum_mean_ratio: f64) -> Resources {
    let source = source(CENTER);
    let owner = AcceptedCircularHoleChordalRealizationV1::from_reference(
        &source,
        MAX_BOUNDARY_ERROR_M,
        MAX_SEGMENTS,
        MeshQualityGate::new(required_minimum_mean_ratio).unwrap(),
    )
    .expect("source-owned chordal reference realization");
    let geometry = owner.realized_geometry().clone();
    let mesh = owner.mesh().clone();
    let correspondence = GeometryMeshCorrespondenceEnvelopeV1::from_region(&geometry, &mesh)
        .expect("Model-free authored-region correspondence");
    Resources {
        source,
        owner,
        geometry,
        mesh,
        correspondence,
    }
}

fn capture(resources: &Resources) -> CircularHoleChordalRealizationEnvelopeV1 {
    resources.owner.envelope().clone()
}

fn replay(
    binding: &CircularHoleChordalRealizationEnvelopeV1,
    resources: &Resources,
) -> AcceptedCircularHoleChordalRealizationV1 {
    binding
        .replay_against(
            &resources.source,
            &resources.geometry,
            &resources.mesh,
            &resources.correspondence,
        )
        .expect("exact resources replay")
}

fn oracle_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../verify/geometry/circular-hole-chordal-realization-binding/oracle/binding_oracle.py",
    )
}

fn expected_path(file: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../verify/geometry/circular-hole-chordal-realization-binding/expected/{file}"
    ))
}

fn independent_encoding_witness() -> (Vec<u8>, String) {
    let readme = std::fs::read_to_string(expected_path("README.md"))
        .expect("read independent expected values");
    let after_fence = readme
        .split_once("```json\n")
        .expect("expected values contain the witness JSON")
        .1;
    let witness = after_fence
        .split_once("\n```")
        .expect("witness JSON fence is closed")
        .0
        .as_bytes()
        .to_vec();

    let contract: serde_json::Value = serde_json::from_slice(
        &std::fs::read(expected_path("binding-contract.json"))
            .expect("read independent binding contract"),
    )
    .expect("independent binding contract is JSON");
    let artificial = &contract["artificial_encoding_witness"];
    assert_eq!(
        witness.len() as u64,
        artificial["canonical_bytes"]
            .as_u64()
            .expect("canonical byte count")
    );
    let digest = artificial["sha256"]
        .as_str()
        .expect("witness digest")
        .to_owned();
    (witness, digest)
}

fn hex(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn replace_once(bytes: &[u8], from: &str, to: &str) -> Vec<u8> {
    let text = String::from_utf8(bytes.to_vec()).expect("canonical JSON is UTF-8");
    assert_eq!(
        text.matches(from).count(),
        1,
        "mutation needle must be unique: {from}"
    );
    text.replacen(from, to, 1).into_bytes()
}

fn replace_f64(bytes: &[u8], field: &str, from: f64, to: f64) -> Vec<u8> {
    replace_once(
        bytes,
        &format!(
            "\"{field}\":{}",
            serde_json::to_string(&from).expect("finite source value")
        ),
        &format!(
            "\"{field}\":{}",
            serde_json::to_string(&to).expect("finite replacement value")
        ),
    )
}

fn decode(bytes: &[u8]) -> CircularHoleChordalRealizationEnvelopeV1 {
    CircularHoleChordalRealizationEnvelopeV1::from_json(bytes, JsonDecoderLimits::default())
        .expect("locally valid canonical binding")
}

fn locally_admitted_cartesian_correspondence() -> GeometryMeshCorrespondenceEnvelopeV1 {
    let wire = serde_json::json!({
        "schema": "eqiora.geometry-mesh-correspondence-envelope/v1",
        "encoding": "eqiora.canonical-json/v1",
        "geometry_sha256": "11".repeat(32),
        "mesh_sha256": "22".repeat(32),
        "dimension": 2,
        "bodies": [{
            "domain_ulid": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "geometry_entity": {"dimension": 2, "index": 0},
            "cell_indices": [0],
        }],
        "boundaries": [{
            "domain_ulid": "01ARZ3NDEKTSV4RRFFQ69G5FAW",
            "parent_ulid": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "geometry_entity": {"dimension": 1, "index": 0},
            "axis": 0,
            "side": "lower",
            "orientation": "parent-outward",
            "facet_indices": [0],
        }],
    });
    GeometryMeshCorrespondenceEnvelopeV1::from_json(
        &serde_json::to_vec(&wire).unwrap(),
        Default::default(),
    )
    .expect("locally admitted Model-bound Cartesian correspondence variant")
}

#[test]
fn independent_oracle_canonical_wire_and_exact_resources_agree() {
    let oracle_bytes = std::fs::read(oracle_path()).expect("read independent oracle");
    assert_eq!(hex(Sha256::digest(&oracle_bytes)), ORACLE_SHA256);
    let output = Command::new("python3")
        .arg(oracle_path())
        .output()
        .expect("run independent oracle");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    for expected in [
        "coverage.admission_rows=11",
        "coverage.substitution_rows=16",
        "coverage.total_rows=39",
        "checks.total=60",
        "checks.failed=0",
        "oracle.result=pass",
    ] {
        assert!(stdout.contains(expected), "oracle omitted {expected}");
    }

    let (witness_bytes, witness_digest) = independent_encoding_witness();
    let witness = decode(&witness_bytes);
    assert_eq!(witness.canonical_json().unwrap(), witness_bytes);
    assert_eq!(witness.digest().unwrap().as_str(), witness_digest);

    let resources = resources(REQUIRED_MINIMUM_MEAN_RATIO);
    let binding = capture(&resources);
    let bytes = binding.canonical_json().unwrap();
    let text = String::from_utf8(bytes.clone()).unwrap();
    let positions = FIELD_ORDER.map(|field| {
        text.find(&format!("\"{field}\":"))
            .expect("field is present")
    });
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(text.matches("\":").count(), FIELD_ORDER.len());

    let mut digest = Sha256::new();
    digest.update(SCHEMA.as_bytes());
    digest.update([0]);
    digest.update(&bytes);
    assert_eq!(binding.digest().unwrap().as_str(), hex(digest.finalize()));
    assert_eq!(
        binding.source_geometry_artifact(),
        ArtifactDigest::from_sha256(resources.source.digest_bytes())
    );
    assert_eq!(
        binding.realized_geometry_artifact(),
        resources.geometry.digest().unwrap()
    );
    assert_eq!(binding.mesh_artifact(), resources.mesh.digest().unwrap());
    assert_eq!(
        binding.correspondence_artifact(),
        resources.correspondence.digest().unwrap()
    );

    let decoded = decode(&bytes);
    assert_eq!(decoded.canonical_json().unwrap(), bytes);
    assert_eq!(decoded.digest().unwrap(), binding.digest().unwrap());
    let regenerated = replay(&decoded, &resources);
    assert_eq!(
        regenerated.source().digest_bytes(),
        resources.source.digest_bytes()
    );
    assert_eq!(
        regenerated.boundary_error_bound_m().to_bits(),
        resources.owner.boundary_error_bound_m().to_bits()
    );
}

#[test]
fn canonical_admission_rejects_every_pre_replay_fault_class() {
    let binding = capture(&resources(REQUIRED_MINIMUM_MEAN_RATIO));
    let bytes = binding.canonical_json().unwrap();
    let prefix = format!("{{\"schema\":\"{SCHEMA}\",\"encoding\":\"eqiora.canonical-json/v1\",");
    let reordered_prefix =
        format!("{{\"encoding\":\"eqiora.canonical-json/v1\",\"schema\":\"{SCHEMA}\",");

    let source_digest = binding.source_geometry_artifact().to_string();
    let malformed_digest = replace_once(&bytes, &source_digest, "not-a-lowercase-sha256");
    let zero_request = replace_f64(
        &bytes,
        "requested_max_boundary_error_m",
        binding.requested_max_boundary_error_m(),
        0.0,
    );
    let negative_request = replace_f64(
        &bytes,
        "requested_max_boundary_error_m",
        binding.requested_max_boundary_error_m(),
        -binding.requested_max_boundary_error_m(),
    );
    let nonfinite_request = replace_once(
        &bytes,
        &format!(
            "\"requested_max_boundary_error_m\":{}",
            serde_json::to_string(&binding.requested_max_boundary_error_m()).unwrap()
        ),
        "\"requested_max_boundary_error_m\":NaN",
    );
    let unknown_schema = replace_once(&bytes, SCHEMA, "eqiora.unknown/v1");
    let unknown_encoding =
        replace_once(&bytes, "eqiora.canonical-json/v1", "eqiora.unknown-json/v1");
    let reordered = replace_once(&bytes, &prefix, &reordered_prefix);
    let noncanonical = [b" ".as_slice(), bytes.as_slice()].concat();
    let extra = replace_once(&bytes, "{", "{\"extra\":0,");
    let required_minimum_mean_ratio =
        serde_json::to_string(&binding.required_minimum_mean_ratio()).unwrap();
    let missing = replace_once(
        &bytes,
        &format!(",\"required_minimum_mean_ratio\":{required_minimum_mean_ratio}"),
        "",
    );
    let bound_below_allowance = replace_f64(
        &bytes,
        "boundary_error_bound_m",
        binding.boundary_error_bound_m(),
        binding.boundary_evaluation_allowance_m() / 2.0,
    );

    for invalid in [
        malformed_digest,
        zero_request,
        negative_request,
        nonfinite_request,
        unknown_schema,
        unknown_encoding,
        reordered,
        noncanonical,
        extra,
        missing,
        bound_below_allowance,
    ] {
        assert!(
            CircularHoleChordalRealizationEnvelopeV1::from_json(
                &invalid,
                JsonDecoderLimits::default()
            )
            .is_err()
        );
    }

    assert!(
        CircularHoleChordalRealizationEnvelopeV1::from_json(
            &bytes,
            JsonDecoderLimits {
                max_bytes: bytes.len() - 1,
                max_nesting_depth: 64,
            },
        )
        .is_err()
    );
    assert!(
        CircularHoleChordalRealizationEnvelopeV1::from_json(
            &bytes,
            JsonDecoderLimits {
                max_bytes: bytes.len(),
                max_nesting_depth: 0,
            },
        )
        .is_err()
    );
}

#[test]
fn replay_rejects_mutated_fields_and_substituted_resources() {
    let resources = resources(REQUIRED_MINIMUM_MEAN_RATIO);
    let binding = capture(&resources);
    let bytes = binding.canonical_json().unwrap();

    let mut changed_digest = binding.mesh_artifact().to_string();
    changed_digest.replace_range(
        ..1,
        if &changed_digest[..1] == "0" {
            "1"
        } else {
            "0"
        },
    );
    let digest_mutant = decode(&replace_once(
        &bytes,
        binding.mesh_artifact().as_str(),
        &changed_digest,
    ));
    assert!(
        digest_mutant
            .replay_against(
                &resources.source,
                &resources.geometry,
                &resources.mesh,
                &resources.correspondence,
            )
            .is_err()
    );

    let bound_mutant = decode(&replace_f64(
        &bytes,
        "boundary_error_bound_m",
        binding.boundary_error_bound_m(),
        binding.boundary_error_bound_m() / 2.0,
    ));
    assert!(
        bound_mutant
            .replay_against(
                &resources.source,
                &resources.geometry,
                &resources.mesh,
                &resources.correspondence,
            )
            .is_err()
    );

    let perturbed_source = source([CENTER[0] + 1.0e-6, CENTER[1]]);
    assert!(
        binding
            .replay_against(
                &perturbed_source,
                &resources.geometry,
                &resources.mesh,
                &resources.correspondence,
            )
            .is_err()
    );
    let perturbed_owner = AcceptedCircularHoleChordalRealizationV1::from_reference(
        &perturbed_source,
        MAX_BOUNDARY_ERROR_M,
        MAX_SEGMENTS,
        MeshQualityGate::new(REQUIRED_MINIMUM_MEAN_RATIO).unwrap(),
    )
    .unwrap();
    assert!(
        perturbed_owner
            .bind_conforming_mesh(&resources.mesh, &resources.correspondence)
            .is_err()
    );

    let reference_region = resources.geometry.region().unwrap();
    let changed_region = PlanarRegion::new(
        reference_region.vertices().to_vec(),
        reference_region.faces().to_vec(),
        reference_region.entity_sets().to_vec(),
        reference_region.tolerance_m() * 2.0,
    )
    .unwrap();
    let changed_geometry = GeometryDefinitionV1::from_region(&changed_region);
    assert!(
        binding
            .replay_against(
                &resources.source,
                &changed_geometry,
                &resources.mesh,
                &resources.correspondence,
            )
            .is_err()
    );

    let model_bound_correspondence = locally_admitted_cartesian_correspondence();
    let error = binding
        .replay_against(
            &resources.source,
            &resources.geometry,
            &resources.mesh,
            &model_bound_correspondence,
        )
        .expect_err("the durable binding accepts only the Model-free authored-region variant");
    assert!(
        error
            .message()
            .contains("was not derived from an authored planar region")
    );
}

#[test]
fn conforming_renumbering_and_policy_plateau_are_separately_bound() {
    let resources = resources(REQUIRED_MINIMUM_MEAN_RATIO);
    let original = capture(&resources);

    let reference_mesh = resources.mesh.mesh();
    let mut reversed_cells = reference_mesh.cells().to_vec();
    reversed_cells.reverse();
    let renumbered = SimplicialMesh::new(
        2,
        reference_mesh.vertices().to_vec(),
        reversed_cells,
        reference_mesh.quality_gate(),
    )
    .expect("renumbered cells remain a conforming accepted mesh");
    let renumbered_mesh = SimplicialMeshEnvelopeV1::from_mesh(&renumbered).unwrap();
    let renumbered_correspondence =
        GeometryMeshCorrespondenceEnvelopeV1::from_region(&resources.geometry, &renumbered_mesh)
            .expect("renumbered conforming mesh has its own correspondence");

    assert!(
        original
            .replay_against(
                &resources.source,
                &resources.geometry,
                &renumbered_mesh,
                &renumbered_correspondence,
            )
            .is_err()
    );
    let separately_bound = resources
        .owner
        .bind_conforming_mesh(&renumbered_mesh, &renumbered_correspondence)
        .expect("a conforming renumbering is valid only under its own binding");
    separately_bound.revalidate().unwrap();
    assert_ne!(
        separately_bound.envelope().digest().unwrap(),
        original.digest().unwrap()
    );
    assert_ne!(
        separately_bound.envelope().mesh_artifact(),
        original.mesh_artifact()
    );

    let relaxed_owner = AcceptedCircularHoleChordalRealizationV1::from_reference(
        &resources.source,
        MAX_BOUNDARY_ERROR_M,
        MAX_SEGMENTS,
        MeshQualityGate::new(REQUIRED_MINIMUM_MEAN_RATIO / 2.0).unwrap(),
    )
    .expect("the lower required threshold remains on the same policy plateau");
    assert_eq!(
        relaxed_owner.realized_geometry(),
        resources.owner.realized_geometry()
    );
    assert_eq!(
        relaxed_owner.boundary_error_bound_m().to_bits(),
        resources.owner.boundary_error_bound_m().to_bits()
    );
    let plateau = relaxed_owner
        .bind_conforming_mesh(&resources.mesh, &resources.correspondence)
        .expect("a plateau-preserving policy defines a distinct valid binding");
    plateau.revalidate().unwrap();
    assert_ne!(
        plateau.envelope().digest().unwrap(),
        original.digest().unwrap()
    );
    assert_eq!(plateau.envelope().mesh_artifact(), original.mesh_artifact());
    assert_eq!(
        plateau.envelope().required_minimum_mean_ratio(),
        REQUIRED_MINIMUM_MEAN_RATIO / 2.0
    );

    let strict_policy = decode(&replace_f64(
        &original.canonical_json().unwrap(),
        "required_minimum_mean_ratio",
        original.required_minimum_mean_ratio(),
        1.0,
    ));
    assert!(
        strict_policy
            .replay_against(
                &resources.source,
                &resources.geometry,
                &resources.mesh,
                &resources.correspondence,
            )
            .is_err()
    );
}
