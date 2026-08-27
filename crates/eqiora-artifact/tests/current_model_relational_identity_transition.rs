//! Independent oracle for RFC 0083's relational identity transition.
//!
//! The sealed transition tree remains the byte-exact alpha.1 Model-epoch
//! observation. Four live expected artifacts may later differ from that history
//! only at eight fixed release-owned compilation, Run, and binding pointers;
//! this case owns every other raw byte but not those pointers' current values.
//! Historical identities remain derived from committed canonical artifacts.
//! Bridges, retained goldens, moving-spatial lineage, and the complete path
//! classification remain checked below. The repository sweep stays in the
//! exact-path private support module included at the end of this test target.

use eqiora_artifact::{
    CanonicalModelArtifact, ModelDecoderLimits, ModelEnvelope, RealizationEnvelopeV4,
    ReplayableCanonicalModelArtifact, SemanticFingerprintGeneration, SpatialStateEnvelopeV2,
    SpatialTrajectoryEnvelopeV2, SpatialTrajectorySegmentEnvelopeV2, StructuralSemanticFingerprint,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[path = "current_model_relational_identity_transition/transition_contract.rs"]
mod transition_contract;

macro_rules! oracle {
    ($name:literal) => {
        include_bytes!(concat!(
            "../../../verify/artifacts/current-model-relational-identity-transition/",
            $name
        ))
    };
}

const TRANSITION: &[u8] = oracle!("expected/transition.json");
const CLASSIFICATION: &[u8] = oracle!("expected/classification.json");
const CLASSIFICATION_INVENTORY: &str = include_str!(concat!(
    "../../../verify/artifacts/current-model-relational-identity-transition/",
    "expected/classification-inventory.txt"
));
const RETAINED_REALIZATION_V4: &[u8] = oracle!("expected/retained/realization-v4.json");
const CURRENT_MODEL_SCHEMA: &str = "eqiora.model-envelope/v8";
const CANONICAL_ENCODING: &str = "eqiora.canonical-json/v1";
const BRIDGE_GENERATION: SemanticFingerprintGeneration = SemanticFingerprintGeneration::V2;
const REVISION_KEY: &[u8] = b"\"source_revision\":";

struct Deterministic {
    name: &'static str,
    model: &'static [u8],
    historical: &'static [u8],
    live: &'static [u8],
    artifacts: &'static [(&'static str, &'static [u8])],
}

const DETERMINISTIC: [Deterministic; 4] = [
    Deterministic {
        name: "packaged-dc-motor-controller",
        model: oracle!("expected/deterministic/packaged-dc-motor-controller/model.json"),
        historical: oracle!("expected/deterministic/packaged-dc-motor-controller/identities.json"),
        live: include_bytes!(
            "../../../verify/hybrid/packaged-dc-motor-controller/expected/identities.json"
        ),
        artifacts: &[
            (
                "compilation.json",
                oracle!("expected/deterministic/packaged-dc-motor-controller/compilation.json"),
            ),
            (
                "run.json",
                oracle!("expected/deterministic/packaged-dc-motor-controller/run.json"),
            ),
            (
                "run-binding.json",
                oracle!("expected/deterministic/packaged-dc-motor-controller/run-binding.json"),
            ),
        ],
    },
    Deterministic {
        name: "composed-model-package",
        model: oracle!("expected/deterministic/composed-model-package/model.json"),
        historical: oracle!("expected/deterministic/composed-model-package/identities.json"),
        live: include_bytes!(
            "../../../verify/packages/composed-model-package/expected/identities.json"
        ),
        artifacts: &[(
            "compilation.json",
            oracle!("expected/deterministic/composed-model-package/compilation.json"),
        )],
    },
    Deterministic {
        name: "offline-model-package",
        model: oracle!("expected/deterministic/offline-model-package/model.json"),
        historical: oracle!("expected/deterministic/offline-model-package/identities.json"),
        live: include_bytes!(
            "../../../verify/packages/offline-model-package/expected/identities.json"
        ),
        artifacts: &[
            (
                "compilation.json",
                oracle!("expected/deterministic/offline-model-package/compilation.json"),
            ),
            (
                "run.json",
                oracle!("expected/deterministic/offline-model-package/run.json"),
            ),
            (
                "run-binding.json",
                oracle!("expected/deterministic/offline-model-package/run-binding.json"),
            ),
        ],
    },
    Deterministic {
        name: "fixed-topology-ale-monolithic-3d",
        model: oracle!("expected/deterministic/fixed-topology-ale-monolithic-3d/model.json"),
        historical: oracle!(
            "expected/deterministic/fixed-topology-ale-monolithic-3d/accepted-trajectory.json"
        ),
        live: include_bytes!(
            "../../../verify/fsi/fixed-topology-ale-monolithic-3d/expected/accepted-trajectory.json"
        ),
        artifacts: &[
            (
                "geometry-identity.json",
                oracle!(
                    "expected/deterministic/fixed-topology-ale-monolithic-3d/geometry-identity.json"
                ),
            ),
            (
                "correspondence.json",
                oracle!(
                    "expected/deterministic/fixed-topology-ale-monolithic-3d/correspondence.json"
                ),
            ),
            (
                "realization.json",
                oracle!("expected/deterministic/fixed-topology-ale-monolithic-3d/realization.json"),
            ),
            (
                "run.json",
                oracle!("expected/deterministic/fixed-topology-ale-monolithic-3d/run.json"),
            ),
            (
                "trajectory.json",
                oracle!("expected/deterministic/fixed-topology-ale-monolithic-3d/trajectory.json"),
            ),
            (
                "trajectory-segment-0.json",
                oracle!(
                    "expected/deterministic/fixed-topology-ale-monolithic-3d/trajectory-segment-0.json"
                ),
            ),
            (
                "trajectory-segment-1.json",
                oracle!(
                    "expected/deterministic/fixed-topology-ale-monolithic-3d/trajectory-segment-1.json"
                ),
            ),
            (
                "trajectory-root-0.json",
                oracle!(
                    "expected/deterministic/fixed-topology-ale-monolithic-3d/trajectory-root-0.json"
                ),
            ),
            (
                "geometry-state-0.json",
                oracle!(
                    "expected/deterministic/fixed-topology-ale-monolithic-3d/geometry-state-0.json"
                ),
            ),
            (
                "geometry-state-1.json",
                oracle!(
                    "expected/deterministic/fixed-topology-ale-monolithic-3d/geometry-state-1.json"
                ),
            ),
            (
                "geometry-state-2.json",
                oracle!(
                    "expected/deterministic/fixed-topology-ale-monolithic-3d/geometry-state-2.json"
                ),
            ),
            (
                "spatial-state-0.json",
                oracle!(
                    "expected/deterministic/fixed-topology-ale-monolithic-3d/spatial-state-0.json"
                ),
            ),
            (
                "spatial-state-1.json",
                oracle!(
                    "expected/deterministic/fixed-topology-ale-monolithic-3d/spatial-state-1.json"
                ),
            ),
            (
                "spatial-state-2.json",
                oracle!(
                    "expected/deterministic/fixed-topology-ale-monolithic-3d/spatial-state-2.json"
                ),
            ),
        ],
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReleaseProjection {
    fixture: &'static str,
    path: &'static str,
    bytes: usize,
    pointers: &'static [&'static str],
}

const RELEASE_PROJECTION: [ReleaseProjection; 4] = [
    ReleaseProjection {
        fixture: "composed-model-package",
        path: "verify/packages/composed-model-package/expected/identities.json",
        bytes: 1016,
        pointers: &["/compilation_digest"],
    },
    ReleaseProjection {
        fixture: "offline-model-package",
        path: "verify/packages/offline-model-package/expected/identities.json",
        bytes: 944,
        pointers: &["/compilation_digest", "/run_digest", "/run_binding_digest"],
    },
    ReleaseProjection {
        fixture: "packaged-dc-motor-controller",
        path: "verify/hybrid/packaged-dc-motor-controller/expected/identities.json",
        bytes: 1200,
        pointers: &["/compilation_digest", "/run_digest", "/run_binding_digest"],
    },
    ReleaseProjection {
        fixture: "fixed-topology-ale-monolithic-3d",
        path: "verify/fsi/fixed-topology-ale-monolithic-3d/expected/accepted-trajectory.json",
        bytes: 5131,
        pointers: &["/provenance/run_sha256"],
    },
];

struct Bridge {
    name: &'static str,
    historical: &'static [u8],
    current: &'static [u8],
    bundle: &'static [&'static [u8]],
}

const BRIDGES: [Bridge; 2] = [
    Bridge {
        name: "canonical-cartesian-poisson-cuda",
        historical: include_bytes!(
            "../../../verify/numerics/canonical-cartesian-poisson-cuda/artifacts/model.json"
        ),
        current: oracle!("expected/bridge/canonical-cartesian-poisson-cuda/current-model.json"),
        bundle: &[
            include_bytes!(
                "../../../verify/numerics/canonical-cartesian-poisson-cuda/artifacts/q1-fem-realization.json"
            ),
            include_bytes!(
                "../../../verify/numerics/canonical-cartesian-poisson-cuda/artifacts/q1-fem-run.json"
            ),
            include_bytes!(
                "../../../verify/numerics/canonical-cartesian-poisson-cuda/artifacts/cell-centered-tpfa-realization.json"
            ),
            include_bytes!(
                "../../../verify/numerics/canonical-cartesian-poisson-cuda/artifacts/cell-centered-tpfa-run.json"
            ),
        ],
    },
    Bridge {
        name: "fixed-reference-cuda-solve-2d",
        historical: include_bytes!(
            "../../../verify/fsi/fixed-reference-cuda-solve-2d/artifacts/model.json"
        ),
        current: oracle!("expected/bridge/fixed-reference-cuda-solve-2d/current-model.json"),
        bundle: &[
            include_bytes!(
                "../../../verify/fsi/fixed-reference-cuda-solve-2d/artifacts/cuda-realization.json"
            ),
            include_bytes!(
                "../../../verify/fsi/fixed-reference-cuda-solve-2d/artifacts/cuda-run.json"
            ),
        ],
    },
];

struct ModelInputConsumer {
    name: &'static str,
    artifacts: &'static [(&'static str, &'static [u8], &'static [u8])],
}

const MODEL_INPUT_CONSUMERS: [ModelInputConsumer; 1] = [ModelInputConsumer {
    name: "moving-spatial-v2-wire",
    artifacts: &[
        (
            "spatial-state-1",
            oracle!("expected/consumer/moving-spatial-v2-wire/pre-reset/spatial-state-1.json"),
            oracle!("expected/consumer/moving-spatial-v2-wire/replacement/spatial-state-1.json"),
        ),
        (
            "trajectory-segment-1",
            oracle!("expected/consumer/moving-spatial-v2-wire/pre-reset/trajectory-segment-1.json"),
            oracle!(
                "expected/consumer/moving-spatial-v2-wire/replacement/trajectory-segment-1.json"
            ),
        ),
        (
            "trajectory-root-1",
            oracle!("expected/consumer/moving-spatial-v2-wire/pre-reset/trajectory-root-1.json"),
            oracle!("expected/consumer/moving-spatial-v2-wire/replacement/trajectory-root-1.json"),
        ),
    ],
}];

fn frozen(bytes: &[u8]) -> &[u8] {
    bytes.strip_suffix(b"\n").unwrap_or(bytes)
}

fn raw_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn frozen_inventory() -> BTreeSet<String> {
    CLASSIFICATION_INVENTORY
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

fn replace_exact_once(bytes: &mut [u8], old: &str, new: &str) {
    assert_eq!(
        old.len(),
        new.len(),
        "identity substitutions preserve byte length"
    );
    let matches = bytes
        .windows(old.len())
        .enumerate()
        .filter_map(|(index, window)| (window == old.as_bytes()).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "superseded identity {old} must occur exactly once in the original bytes"
    );
    let start = matches[0];
    bytes[start..start + old.len()].copy_from_slice(new.as_bytes());
}

fn identity_substitutions(expected: &Value) -> BTreeMap<String, String> {
    let table = expected["identity_substitutions"].as_object().unwrap();
    let map = table
        .iter()
        .map(|(old, new)| (old.clone(), new.as_str().unwrap().to_owned()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        map.len(),
        expected["identity_substitution_count"].as_u64().unwrap() as usize
    );
    for (old, new) in &map {
        for identity in [old, new] {
            assert_eq!(identity.len(), 64);
            assert!(
                identity
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
                "an identity literal is 64 lowercase hex characters: {identity}"
            );
        }
        assert_ne!(old, new, "a substitution that changes nothing is not one");
        assert!(
            !map.contains_key(new),
            "{new} is both a replacement and a superseded identity, so the table is order-dependent"
        );
    }
    map
}

fn substitute_identities(source: &[u8], table: &BTreeMap<String, String>) -> Vec<u8> {
    let mut out = source.to_vec();
    for (old, new) in table {
        let positions = out
            .windows(old.len())
            .enumerate()
            .filter_map(|(index, window)| (window == old.as_bytes()).then_some(index))
            .collect::<Vec<_>>();
        for start in positions {
            out[start..start + new.len()].copy_from_slice(new.as_bytes());
        }
    }
    assert_eq!(out.len(), source.len());
    out
}

fn domain_digest(domain: &str, content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update([0]);
    hasher.update(content);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn model_content(bytes: &[u8]) -> Vec<u8> {
    let start = bytes
        .windows(REVISION_KEY.len())
        .position(|window| window == REVISION_KEY)
        .expect("a canonical Model names its source revision exactly once");
    assert!(
        bytes[start + 1..]
            .windows(REVISION_KEY.len())
            .all(|window| window != REVISION_KEY),
        "the source revision key must be unique so the projection is unambiguous"
    );
    let mut end = start + REVISION_KEY.len();
    while bytes[end].is_ascii_digit() {
        end += 1;
    }
    assert_eq!(bytes[end], b',', "source revision is not the final member");
    let mut content = bytes[..start].to_vec();
    content.extend_from_slice(&bytes[end + 1..]);
    content
}

fn model_digest_from_bytes(bytes: &[u8]) -> String {
    let wire: Value = serde_json::from_slice(bytes).unwrap();
    domain_digest(wire["schema"].as_str().unwrap(), &model_content(bytes))
}

fn transition() -> Value {
    serde_json::from_slice(frozen(TRANSITION)).unwrap()
}

fn entry<'a>(document: &'a Value, group: &str, name: &str) -> &'a Value {
    document[group]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["name"] == name)
        .unwrap_or_else(|| panic!("{group} entry `{name}` must be classified"))
}

fn artifact<'a>(fixture: &'a Deterministic, name: &str) -> &'a [u8] {
    frozen(
        fixture
            .artifacts
            .iter()
            .find(|(candidate, _)| *candidate == name)
            .unwrap_or_else(|| panic!("{} must commit `{name}`", fixture.name))
            .1,
    )
}

fn identity_artifacts(expected: &Value) -> Vec<&Value> {
    let mut artifacts = expected["supporting_artifacts"]
        .as_array()
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    artifacts.extend(expected["edges"].as_array().unwrap());
    artifacts
}

fn resolve<'a>(document: &'a Value, path: &str) -> &'a Value {
    let mut node = document;
    for token in path.trim_start_matches('/').split('/') {
        let (name, rest) = token.split_once('[').unwrap_or((token, ""));
        node = &node[name];
        for index in rest
            .trim_end_matches(']')
            .split("][")
            .filter(|s| !s.is_empty())
        {
            node = &node[index.parse::<usize>().unwrap()];
        }
    }
    node
}

fn leaves(document: &Value, prefix: String, out: &mut Vec<(String, Value)>) {
    match document {
        Value::Object(map) => {
            for (key, value) in map {
                leaves(value, format!("{prefix}/{key}"), out);
            }
        }
        Value::Array(items) => {
            for (index, value) in items.iter().enumerate() {
                leaves(value, format!("{prefix}[{index}]"), out);
            }
        }
        scalar => out.push((prefix, scalar.clone())),
    }
}

fn flatten(document: &Value) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    leaves(document, String::new(), &mut out);
    out
}

fn release_fixture(projection: &ReleaseProjection) -> &'static Deterministic {
    DETERMINISTIC
        .iter()
        .find(|fixture| fixture.name == projection.fixture)
        .unwrap()
}

fn mask_release_scalar(
    bytes: &mut [u8],
    document: &Value,
    pointer: &str,
    position: usize,
) -> String {
    let value = resolve(document, pointer)
        .as_str()
        .unwrap_or_else(|| panic!("{pointer} must be a JSON string"));
    assert!(
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "{pointer} must be 64-character lowercase hex"
    );
    let quoted = format!("\"{value}\"");
    let matches = bytes
        .windows(quoted.len())
        .enumerate()
        .filter_map(|(index, window)| (window == quoted.as_bytes()).then_some(index + 1))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "{pointer} token must occur exactly once");
    bytes[matches[0]..matches[0] + 64].fill(b'G' + position as u8);
    value.to_owned()
}

fn assert_release_projection(
    classification: &[ReleaseProjection],
    overrides: &BTreeMap<&str, Vec<u8>>,
) {
    assert_eq!(
        classification, RELEASE_PROJECTION,
        "release projection is exact"
    );
    let mut stale = Vec::new();
    for projection in classification {
        let fixture = release_fixture(projection);
        let historical = fixture.historical;
        let live = overrides
            .get(projection.path)
            .map(Vec::as_slice)
            .unwrap_or(fixture.live);
        for (state, bytes) in [("historical", historical), ("live", live)] {
            assert_eq!(
                bytes.len(),
                projection.bytes,
                "{} {state} length",
                projection.path
            );
            assert!(
                bytes.ends_with(b"\n") && !bytes.ends_with(b"\n\n"),
                "{} {state} terminal LF",
                projection.path
            );
            assert!(
                serde_json::from_slice::<Value>(bytes).unwrap().is_object(),
                "{} {state} must be one JSON object",
                projection.path
            );
        }
        let historical_document: Value = serde_json::from_slice(historical).unwrap();
        let live_document: Value = serde_json::from_slice(live).unwrap();
        let (mut historical_masked, mut live_masked) = (historical.to_vec(), live.to_vec());
        for (position, pointer) in projection.pointers.iter().enumerate() {
            let old = mask_release_scalar(
                &mut historical_masked,
                &historical_document,
                pointer,
                position,
            );
            let new = mask_release_scalar(&mut live_masked, &live_document, pointer, position);
            if old == new {
                stale.push(format!("{} {pointer}: stale-alpha.1", projection.path));
            }
        }
        assert_eq!(
            historical_masked, live_masked,
            "{} may differ only at its exact release pointers",
            projection.path
        );
    }
    let paths = stale
        .iter()
        .filter_map(|entry| entry.split_once(' ').map(|(path, _)| path))
        .collect::<BTreeSet<_>>();
    assert!(
        stale.is_empty(),
        "current release projection has stale-alpha.1 at exactly {} paths and {} pointers:\n{}",
        paths.len(),
        stale.len(),
        stale.join("\n")
    );
}

#[test]
fn every_precommitted_current_model_reproduces_its_frozen_identity() {
    let transition = transition();
    assert_eq!(transition["current_model_schema"], CURRENT_MODEL_SCHEMA);
    assert_eq!(transition["canonical_encoding"], CANONICAL_ENCODING);

    for fixture in &DETERMINISTIC {
        let expected = entry(&transition, "deterministic", fixture.name);
        let bytes = frozen(fixture.model);
        assert_eq!(
            bytes.len(),
            expected["model_canonical_bytes"].as_u64().unwrap() as usize,
            "{} canonical byte length",
            fixture.name
        );
        assert_eq!(raw_sha256(bytes), expected["model_raw_sha256"]);

        let digest = expected["model_digest"].as_str().unwrap();
        assert_eq!(model_digest_from_bytes(bytes), digest);

        let decoded = ModelEnvelope::from_json(bytes, ModelDecoderLimits::default())
            .unwrap_or_else(|error| panic!("{} must decode: {}", fixture.name, error.message()));
        assert_eq!(decoded.canonical_json().unwrap(), bytes);
        assert_eq!(decoded.digest().unwrap().as_str(), digest);
        assert_eq!(decoded.source_revision(), expected["source_revision"]);
        assert_eq!(
            decoded.model().unwrap().ulid().to_string(),
            expected["model_ulid"]
        );
        let reference = decoded.artifact_reference().unwrap();
        reference.validate_artifact(&decoded).unwrap();
        assert_eq!(
            decoded.replay_model().unwrap().program(),
            &decoded.to_program().unwrap(),
            "{} replay and typed reconstruction must agree",
            fixture.name
        );
    }
}

#[test]
fn every_downstream_identity_and_reference_edge_derives_from_bytes() {
    let transition = transition();
    let mut checked_edges = 0;

    for fixture in &DETERMINISTIC {
        let expected = entry(&transition, "deterministic", fixture.name);
        let mut identities = vec![(
            expected["model_pointer"].as_str().unwrap().to_owned(),
            expected["model_digest"].as_str().unwrap().to_owned(),
        )];

        let artifacts = identity_artifacts(expected);
        for edge in &artifacts {
            let name = edge["artifact"].as_str().unwrap();
            let bytes = artifact(fixture, name);
            let wire: Value = serde_json::from_slice(bytes).unwrap();
            assert_eq!(
                wire["schema"], edge["schema"],
                "{} {name} schema",
                fixture.name
            );
            assert_eq!(raw_sha256(bytes), edge["raw_sha256"]);
            assert_eq!(
                domain_digest(edge["digest_domain"].as_str().unwrap(), bytes),
                edge["digest"].as_str().unwrap(),
                "{} {name} identity must derive from its own canonical bytes",
                fixture.name
            );
            identities.push((
                edge["pointer"].as_str().unwrap().to_owned(),
                edge["digest"].as_str().unwrap().to_owned(),
            ));
        }

        for edge in &artifacts {
            let name = edge["artifact"].as_str().unwrap();
            let wire: Value = serde_json::from_slice(artifact(fixture, name)).unwrap();
            for reference in edge["references"].as_array().unwrap() {
                let target = reference["target"].as_str().unwrap();
                let digest = &identities
                    .iter()
                    .find(|(pointer, _)| pointer == target)
                    .unwrap_or_else(|| panic!("{name} names unclassified target {target}"))
                    .1;
                assert_eq!(
                    resolve(&wire, reference["path"].as_str().unwrap()),
                    digest.as_str(),
                    "{} {name} must embed {target} at {}",
                    fixture.name,
                    reference["path"]
                );
                checked_edges += 1;
            }
        }
    }

    assert!(
        checked_edges >= 47,
        "the classified reference DAG has at least 47 edges, found {checked_edges}"
    );
}

fn assert_historical_consumers_carry_the_precommitted_model_epoch() {
    let transition = transition();

    for fixture in &DETERMINISTIC {
        let expected = entry(&transition, "deterministic", fixture.name);
        let historical = frozen(fixture.historical);
        let document: Value = serde_json::from_slice(historical).unwrap();

        let mut identities = vec![(
            expected["model_pointer"].as_str().unwrap().to_owned(),
            expected["model_digest"].as_str().unwrap().to_owned(),
        )];
        identities.extend(expected["edges"].as_array().unwrap().iter().map(|edge| {
            (
                edge["pointer"].as_str().unwrap().to_owned(),
                edge["digest"].as_str().unwrap().to_owned(),
            )
        }));
        assert_eq!(
            identities
                .iter()
                .map(|(pointer, _)| pointer.clone())
                .collect::<BTreeSet<_>>(),
            expected["superseded"]
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>(),
            "{} must record one superseded identity for exactly its identity pointers",
            fixture.name
        );

        let leaves = flatten(&document);
        for (pointer, digest) in &identities {
            assert_eq!(
                resolve(&document, pointer),
                digest.as_str(),
                "{} {pointer} must carry the identity derived from its own bytes",
                fixture.name
            );

            let superseded = expected["superseded"][pointer].as_str().unwrap();
            assert_eq!(
                superseded.len(),
                digest.len(),
                "{} {pointer} must move to a same-length identity",
                fixture.name
            );
            assert_ne!(
                resolve(&document, pointer),
                superseded,
                "{} {pointer} must not keep its superseded identity",
                fixture.name
            );
            assert!(
                leaves.iter().all(|(_, value)| value != superseded),
                "{} must not retain superseded identity {superseded}",
                fixture.name
            );
        }
    }
}

fn assert_historical_then_live_projection(
    classification: &[ReleaseProjection],
    overrides: &BTreeMap<&str, Vec<u8>>,
) {
    every_precommitted_current_model_reproduces_its_frozen_identity();
    every_downstream_identity_and_reference_edge_derives_from_bytes();
    assert_historical_consumers_carry_the_precommitted_model_epoch();
    assert_release_projection(classification, overrides);
}

#[test]
fn alpha1_history_precedes_the_exact_live_release_projection() {
    assert_historical_then_live_projection(&RELEASE_PROJECTION, &BTreeMap::new());
}

fn scalar_mutant(source: &[u8], pointer: &str, replacement: &str) -> Vec<u8> {
    let document: Value = serde_json::from_slice(source).unwrap();
    let original = resolve(&document, pointer).as_str().unwrap();
    let mut bytes = source.to_vec();
    replace_exact_once(
        &mut bytes,
        &format!("\"{original}\""),
        &format!("\"{replacement}\""),
    );
    bytes
}

fn must_refuse(classification: &[ReleaseProjection], changed: Option<(&str, Vec<u8>)>) {
    let mut overrides = BTreeMap::new();
    if let Some((path, bytes)) = changed {
        overrides.insert(path, bytes);
    }
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            assert_release_projection(classification, &overrides);
        }))
        .is_err()
    );
}

#[test]
fn live_release_projection_refuses_the_complete_mutant_matrix() {
    assert_historical_then_live_projection(&RELEASE_PROJECTION, &BTreeMap::new());

    for projection in &RELEASE_PROJECTION {
        let fixture = release_fixture(projection);
        let historical: Value = serde_json::from_slice(fixture.historical).unwrap();
        for pointer in projection.pointers {
            let stale = resolve(&historical, pointer).as_str().unwrap();
            must_refuse(
                &RELEASE_PROJECTION,
                Some((projection.path, scalar_mutant(fixture.live, pointer, stale))),
            );
        }
    }

    let mut missing = RELEASE_PROJECTION;
    missing[0].pointers = &[];
    must_refuse(&missing, None);
    let mut widened = RELEASE_PROJECTION;
    widened[0].pointers = &["/compilation_digest", "/model_digest"];
    must_refuse(&widened, None);
    let mut reordered = RELEASE_PROJECTION;
    reordered.swap(0, 1);
    must_refuse(&reordered, None);

    let projection = &RELEASE_PROJECTION[0];
    let fixture = release_fixture(projection);
    let document: Value = serde_json::from_slice(fixture.live).unwrap();
    let current = resolve(&document, projection.pointers[0]).as_str().unwrap();
    let quoted = format!("\"{current}\"");
    let mut non_string = fixture.live.to_vec();
    replace_exact_once(&mut non_string, &quoted, &format!("null{}", " ".repeat(62)));
    must_refuse(&RELEASE_PROJECTION, Some((projection.path, non_string)));
    for replacement in [format!("g{}", &current[1..]), format!("A{}", &current[1..])] {
        must_refuse(
            &RELEASE_PROJECTION,
            Some((
                projection.path,
                scalar_mutant(fixture.live, projection.pointers[0], &replacement),
            )),
        );
    }
    let mut wrong_length = fixture.live.to_vec();
    replace_exact_once(
        &mut wrong_length,
        &quoted,
        &format!("\"{}\" ", &current[..63]),
    );
    must_refuse(&RELEASE_PROJECTION, Some((projection.path, wrong_length)));
    let escaped = format!("\"\\u00{:02x}{}\"", current.as_bytes()[0], &current[1..]);
    let zero_occurrence = std::str::from_utf8(fixture.live)
        .unwrap()
        .replacen(&quoted, &escaped, 1)
        .replacen("Eqiora.Electrical.Basic", "Eqiora.Electrical.", 1)
        .into_bytes();
    must_refuse(
        &RELEASE_PROJECTION,
        Some((projection.path, zero_occurrence)),
    );
    let duplicate = scalar_mutant(fixture.live, "/basic/semantic_digest", current);
    must_refuse(&RELEASE_PROJECTION, Some((projection.path, duplicate)));

    for (index, pointer, exact) in [
        (
            0,
            "/model_digest",
            Some("b7de1eb8e21f9989cb1da97b41c59c6f3e0084d36ae44a3f29337c221338d91b"),
        ),
        (3, "/provenance/trajectory_sha256", None),
        (3, "/provenance/geometry_identity_sha256", None),
        (3, "/provenance/correspondence_sha256", None),
    ] {
        let projection = &RELEASE_PROJECTION[index];
        let fixture = release_fixture(projection);
        let document: Value = serde_json::from_slice(fixture.live).unwrap();
        let value = resolve(&document, pointer).as_str().unwrap();
        let changed = exact.map(str::to_owned).unwrap_or_else(|| {
            format!(
                "{}{}",
                if &value[..1] == "0" { "1" } else { "0" },
                &value[1..]
            )
        });
        must_refuse(
            &RELEASE_PROJECTION,
            Some((
                projection.path,
                scalar_mutant(fixture.live, pointer, &changed),
            )),
        );
    }

    let ale = release_fixture(&RELEASE_PROJECTION[3]);
    for (old, new) in [
        ("0.5002500000003046", "0.5002500000003047"),
        ("0.02126735054320236", "0.02126735054320237"),
        ("\"time_s\":0.01", "\"time_s\":1e-2"),
    ] {
        let mut bytes = ale.live.to_vec();
        replace_exact_once(&mut bytes, old, new);
        must_refuse(
            &RELEASE_PROJECTION,
            Some((RELEASE_PROJECTION[3].path, bytes)),
        );
    }
    let order_old = "\"name\":\"Eqiora.Electrical.Basic\",\"version\":\"0.1.0\"";
    let order_new = "\"version\":\"0.1.0\",\"name\":\"Eqiora.Electrical.Basic\"";
    let mut order = fixture.live.to_vec();
    replace_exact_once(&mut order, order_old, order_new);
    must_refuse(
        &RELEASE_PROJECTION,
        Some((RELEASE_PROJECTION[0].path, order)),
    );
    for bytes in [
        fixture.live[..fixture.live.len() - 1].to_vec(),
        [fixture.live, b"\n"].concat(),
        [fixture.live, b" "].concat(),
    ] {
        must_refuse(
            &RELEASE_PROJECTION,
            Some((RELEASE_PROJECTION[0].path, bytes)),
        );
    }
}

#[test]
fn recorded_accelerator_bundles_are_bridged_and_never_relabelled() {
    let transition = transition();

    for bridge in &BRIDGES {
        let expected = entry(&transition, "bridge", bridge.name);
        let historical = frozen(bridge.historical);
        assert_eq!(
            historical.len(),
            expected["historical_bytes"].as_u64().unwrap() as usize
        );
        assert_eq!(raw_sha256(historical), expected["historical_raw_sha256"]);

        let historical_wire: Value = serde_json::from_slice(historical).unwrap();
        assert_eq!(historical_wire["schema"], expected["historical_schema"]);
        assert_ne!(
            historical_wire["schema"], CURRENT_MODEL_SCHEMA,
            "{} historical bytes must stay on their own schema",
            bridge.name
        );
        let historical_digest = expected["historical_artifact_digest"].as_str().unwrap();
        assert_eq!(model_digest_from_bytes(historical), historical_digest);

        let current = frozen(bridge.current);
        assert_eq!(raw_sha256(current), expected["current_raw_sha256"]);
        let current_digest = expected["current_artifact_digest"].as_str().unwrap();
        assert_eq!(model_digest_from_bytes(current), current_digest);
        let decoded = ModelEnvelope::from_json(current, ModelDecoderLimits::default()).unwrap();
        assert_eq!(decoded.canonical_json().unwrap(), current);
        assert_eq!(decoded.digest().unwrap().as_str(), current_digest);
        assert_ne!(
            current_digest, historical_digest,
            "{} schema domain is part of identity",
            bridge.name
        );

        assert_eq!(
            decoded.model().unwrap().ulid().to_string(),
            historical_wire["model_ulid"]
        );
        assert_eq!(
            decoded.source_revision(),
            historical_wire["source_revision"]
        );

        let fingerprint =
            StructuralSemanticFingerprint::from_program(&decoded.to_program().unwrap()).unwrap();
        assert_eq!(fingerprint.generation(), BRIDGE_GENERATION);
        assert_eq!(
            fingerprint.to_string(),
            expected["current_fingerprint"].as_str().unwrap()
        );
        assert_eq!(
            expected["current_fingerprint"], expected["historical_fingerprint"],
            "{} must bridge the same semantic program",
            bridge.name
        );

        let members = expected["historical_bundle"].as_array().unwrap();
        assert_eq!(members.len(), bridge.bundle.len());
        for (member, bytes) in members.iter().zip(bridge.bundle) {
            let bytes = frozen(bytes);
            let wire: Value = serde_json::from_slice(bytes).unwrap();
            assert_eq!(wire["schema"], member["schema"]);
            assert_eq!(raw_sha256(bytes), member["raw_sha256"]);
            assert_eq!(
                domain_digest(wire["schema"].as_str().unwrap(), bytes),
                member["artifact_digest"].as_str().unwrap()
            );
            assert_eq!(
                wire["model_sha256"], historical_digest,
                "{} bundle must keep observing the historical Model",
                bridge.name
            );
        }
    }
}

#[test]
fn retained_realization_v4_is_an_opaque_exact_golden() {
    let bytes = RETAINED_REALIZATION_V4;
    assert_eq!(bytes.len(), 8_333);
    assert_eq!(
        raw_sha256(bytes),
        "ba9efbdbca265dea0fdf9b1476ea2cae876eb2c97b4ac6f332f3755d866b5d9e"
    );
    assert_eq!(
        domain_digest("eqiora.realization-envelope/v4", bytes),
        "b5bbe49235f75163bf764f37cb2a1168c4471271cd85c5b09f5d5e411ce52c7f"
    );
    let wire: Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(wire["schema"], "eqiora.realization-envelope/v4");
    assert_eq!(wire["encoding"], CANONICAL_ENCODING);
    assert_eq!(
        wire["model_sha256"],
        "16d7bfb39746ccfda33c07ac3f054b42827ee5dd65380c553b93f7c3751d26ba"
    );
    assert_ne!(
        wire["model_sha256"],
        transition()["bridge"][1]["current_artifact_digest"]
    );
}

#[test]
fn the_retained_realization_v4_golden_is_accepted_without_any_model_decoder() {
    let transition = transition();
    let expected = entry(&transition, "retained_family_goldens", "realization-v4");
    let bytes = frozen(RETAINED_REALIZATION_V4);
    assert_eq!(
        bytes.len(),
        expected["canonical_bytes"].as_u64().unwrap() as usize
    );
    assert_eq!(raw_sha256(bytes), expected["raw_sha256"]);
    assert_eq!(expected["post_reset_acceptance"]["model_decoder_calls"], 0);

    let golden = RealizationEnvelopeV4::from_json(bytes, Default::default())
        .expect("the retained Realization family still decodes its own golden");
    assert_eq!(golden.canonical_json().unwrap(), bytes);
    assert_eq!(
        golden.digest().unwrap().as_str(),
        expected["artifact_digest"].as_str().unwrap()
    );
    assert_eq!(
        domain_digest(expected["family"].as_str().unwrap(), bytes),
        expected["artifact_digest"].as_str().unwrap()
    );

    let opaque = &expected["opaque_model_reference"];
    assert_eq!(golden.model_artifact().as_str(), opaque["value"]);
    assert_eq!(
        golden.model().unwrap().ulid().to_string(),
        expected["model_ulid"]
    );
    assert_eq!(
        golden.semantic_revision().get(),
        expected["semantic_revision"].as_u64().unwrap()
    );

    let historical = frozen(BRIDGES[1].historical);
    assert_eq!(raw_sha256(historical), opaque["raw_sha256"]);
    assert_eq!(
        serde_json::from_slice::<Value>(historical).unwrap()["schema"],
        opaque["schema"]
    );
    assert!(
        ModelEnvelope::from_json(historical, ModelDecoderLimits::default()).is_err(),
        "the current Model owner must reject the opaque historical bytes: {}",
        expected["forbidden"]["current_model_decoder_on_opaque_bytes"]
    );

    let current =
        ModelEnvelope::from_json(frozen(BRIDGES[1].current), ModelDecoderLimits::default())
            .unwrap();
    assert!(
        golden.validate_model_artifact(&current).is_err(),
        "the schema domain is part of identity, so the current Model cannot claim this golden"
    );
}

#[test]
fn relabelling_the_retained_realization_v4_golden_to_a_current_model_is_refused() {
    let transition = transition();
    let expected = entry(&transition, "retained_family_goldens", "realization-v4");
    let forbidden = &expected["forbidden"];
    let bytes = frozen(RETAINED_REALIZATION_V4);

    let mut relabelled = bytes.to_vec();
    replace_exact_once(
        &mut relabelled,
        expected["opaque_model_reference"]["value"]
            .as_str()
            .unwrap(),
        forbidden["relabelled_model_reference"].as_str().unwrap(),
    );
    assert_eq!(relabelled.len(), bytes.len());
    assert_ne!(relabelled, bytes);
    assert_eq!(
        raw_sha256(&relabelled),
        forbidden["relabelled_raw_sha256"],
        "the exact bytes a relabelled golden would carry are frozen, not recomputed by the reset"
    );
    assert_ne!(raw_sha256(&relabelled), expected["raw_sha256"]);
    assert_ne!(
        domain_digest(expected["family"].as_str().unwrap(), &relabelled),
        expected["artifact_digest"].as_str().unwrap()
    );

    let current =
        ModelEnvelope::from_json(frozen(BRIDGES[1].current), ModelDecoderLimits::default())
            .unwrap();
    let decoded = RealizationEnvelopeV4::from_json(&relabelled, Default::default()).unwrap();
    assert!(
        decoded.validate_model_artifact(&current).is_ok(),
        "a relabelled golden is internally consistent, which is why its bytes are frozen"
    );
    assert_ne!(
        decoded.digest().unwrap().as_str(),
        expected["artifact_digest"]
    );
}

#[test]
fn the_moving_spatial_consumer_replacement_is_an_identity_only_substitution() {
    let transition = transition();
    for consumer in &MODEL_INPUT_CONSUMERS {
        let expected = entry(&transition, "model_input_consumers", consumer.name);
        let table = identity_substitutions(expected);
        assert_eq!(
            table
                .get(expected["pre_reset_model_digest"].as_str().unwrap())
                .map(String::as_str),
            expected["current_model_digest"].as_str(),
            "the Model edge of the table is the precommitted current Model, not a chosen one"
        );

        let artifacts = expected["artifacts"].as_array().unwrap();
        assert_eq!(artifacts.len(), consumer.artifacts.len());
        for (frozen_entry, (name, pre, replacement)) in artifacts.iter().zip(consumer.artifacts) {
            assert_eq!(frozen_entry["name"], *name);
            let (pre, replacement) = (frozen(pre), frozen(replacement));
            let bytes = frozen_entry["canonical_bytes"].as_u64().unwrap() as usize;
            assert_eq!(pre.len(), bytes);
            assert_eq!(
                replacement.len(),
                bytes,
                "{name} replacement must be byte-length identical"
            );
            assert_eq!(raw_sha256(pre), frozen_entry["pre_reset_raw_sha256"]);
            assert_eq!(
                raw_sha256(replacement),
                frozen_entry["replacement_raw_sha256"]
            );

            let schema = frozen_entry["schema"].as_str().unwrap();
            assert_eq!(
                domain_digest(schema, pre),
                frozen_entry["pre_reset_digest"].as_str().unwrap()
            );
            assert_eq!(
                domain_digest(schema, replacement),
                frozen_entry["replacement_digest"].as_str().unwrap()
            );

            // Leaf-by-leaf containment, then the byte reconstruction.
            let before = flatten(&serde_json::from_slice(pre).unwrap());
            let after = flatten(&serde_json::from_slice(replacement).unwrap());
            assert_eq!(
                before.iter().map(|(path, _)| path).collect::<Vec<_>>(),
                after.iter().map(|(path, _)| path).collect::<Vec<_>>(),
                "{name} keeps its exact leaf set and order"
            );
            let changed = before
                .iter()
                .zip(&after)
                .filter(|((_, old), (_, new))| old != new)
                .map(|((path, old), (_, new))| {
                    (
                        path.clone(),
                        old.as_str().unwrap().to_owned(),
                        new.as_str().unwrap().to_owned(),
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(
                changed
                    .iter()
                    .map(|(path, ..)| path.clone())
                    .collect::<BTreeSet<_>>(),
                frozen_entry["substituted_pointers"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|path| path.as_str().unwrap().to_owned())
                    .collect::<BTreeSet<_>>(),
                "{name} substitutes exactly its frozen pointer set"
            );
            for (path, old, new) in &changed {
                assert_eq!(
                    table.get(old).map(String::as_str),
                    Some(new.as_str()),
                    "{name}{path} moved outside the frozen identity table"
                );
            }
            assert_eq!(
                before.len() - changed.len(),
                frozen_entry["unchanged_leaves"].as_u64().unwrap() as usize,
                "{name} must leave every non-identity leaf alone"
            );
            assert_eq!(
                substitute_identities(pre, &table),
                replacement,
                "{name} replacement must be the committed fixture with only its identities moved"
            );
        }
    }
}

#[test]
fn the_moving_spatial_replacement_replays_through_the_retained_spatial_wires() {
    let transition = transition();
    for consumer in &MODEL_INPUT_CONSUMERS {
        let expected = entry(&transition, "model_input_consumers", consumer.name);
        let table = identity_substitutions(expected);
        let current_model = expected["current_model_digest"].as_str().unwrap();

        // The Model input the reset moves to, through the current owner only.
        let model =
            ModelEnvelope::from_json(frozen(BRIDGES[1].current), ModelDecoderLimits::default())
                .expect("the precommitted current Model input decodes through the current owner");
        assert_eq!(model.digest().unwrap().as_str(), current_model);
        assert_eq!(
            model.model().unwrap().ulid().to_string(),
            expected["model_ulid"]
        );
        assert!(
            ModelEnvelope::from_json(frozen(BRIDGES[1].historical), ModelDecoderLimits::default())
                .is_err(),
            "the pre-reset Model input is rejected by the current owner, which is why it moves"
        );

        let mut replayed = BTreeMap::new();
        for (frozen_entry, (name, _, replacement)) in expected["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .zip(consumer.artifacts)
        {
            let replacement = frozen(replacement);
            let schema = frozen_entry["schema"].as_str().unwrap();
            let digest = match schema {
                "eqiora.spatial-state-envelope/v2" => {
                    let decoded =
                        SpatialStateEnvelopeV2::from_json(replacement, Default::default()).unwrap();
                    assert_eq!(decoded.canonical_json().unwrap(), replacement);
                    decoded.digest().unwrap().as_str().to_owned()
                }
                "eqiora.spatial-trajectory-segment/v2" => {
                    let decoded = SpatialTrajectorySegmentEnvelopeV2::from_json(
                        replacement,
                        Default::default(),
                    )
                    .unwrap();
                    assert_eq!(decoded.canonical_json().unwrap(), replacement);
                    decoded.digest().unwrap().as_str().to_owned()
                }
                "eqiora.spatial-trajectory/v2" => {
                    let decoded =
                        SpatialTrajectoryEnvelopeV2::from_json(replacement, Default::default())
                            .unwrap();
                    assert_eq!(decoded.canonical_json().unwrap(), replacement);
                    decoded.digest().unwrap().as_str().to_owned()
                }
                other => panic!("{name} names an unclassified retained spatial schema {other}"),
            };
            assert_eq!(digest, frozen_entry["replacement_digest"].as_str().unwrap());

            // No superseded identity may survive anywhere in a replacement.
            let text = std::str::from_utf8(replacement).unwrap();
            for superseded in table.keys() {
                assert!(
                    !text.contains(superseded.as_str()),
                    "{name} still carries the superseded identity {superseded}"
                );
            }
            replayed.insert((*name).to_owned(), digest);
        }

        // Every reference edge, read out of the replacement bytes alone.
        for (frozen_entry, (name, _, replacement)) in expected["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .zip(consumer.artifacts)
        {
            let wire: Value = serde_json::from_slice(frozen(replacement)).unwrap();
            for edge in frozen_entry["references"].as_array().unwrap() {
                let target = edge["target"].as_str().unwrap();
                let value = match target.strip_prefix("artifact:") {
                    Some(artifact) => replayed[artifact].clone(),
                    None if target == "current_model_digest" => current_model.to_owned(),
                    None => expected["downstream_current_identities"][target]
                        .as_str()
                        .unwrap()
                        .to_owned(),
                };
                assert_eq!(
                    resolve(&wire, edge["path"].as_str().unwrap()),
                    &Value::String(value),
                    "{name}{} must name {target}",
                    edge["path"]
                );
            }
        }
    }
}

#[test]
fn an_omitted_partial_or_regenerated_moving_spatial_substitution_is_refused() {
    let transition = transition();
    for consumer in &MODEL_INPUT_CONSUMERS {
        let expected = entry(&transition, "model_input_consumers", consumer.name);
        let table = identity_substitutions(expected);
        let artifacts = expected["artifacts"].as_array().unwrap();

        // Omission: the frozen literals the consumer carries today are exactly
        // the pre-reset digests, and not one of them survives the migration.
        let literals = expected["frozen_literals"].as_array().unwrap();
        assert_eq!(literals.len(), artifacts.len());
        for (literal, frozen_entry) in literals.iter().zip(artifacts) {
            assert_eq!(literal["artifact"], frozen_entry["name"]);
            assert_eq!(literal["pre_reset"], frozen_entry["pre_reset_digest"]);
            assert_eq!(literal["replacement"], frozen_entry["replacement_digest"]);
            assert_ne!(literal["pre_reset"], literal["replacement"]);
        }

        for (frozen_entry, (name, pre, replacement)) in artifacts.iter().zip(consumer.artifacts) {
            let (pre, replacement) = (frozen(pre), frozen(replacement));
            let schema = frozen_entry["schema"].as_str().unwrap();
            let target = frozen_entry["replacement_digest"].as_str().unwrap();
            assert_ne!(
                domain_digest(schema, pre),
                target,
                "{name} left wholly unmigrated must not reach its replacement identity"
            );

            // Partial substitution: omitting any one identity this artifact
            // actually carries is a failure, so no proper subset of the table
            // can be mistaken for the whole of it. An identity the artifact does
            // not carry — a sibling's own digest, say — is not a partial
            // migration of *this* artifact and is checked through the sibling.
            let mut carried = 0;
            for omitted in table.keys() {
                if !pre
                    .windows(omitted.len())
                    .any(|window| window == omitted.as_bytes())
                {
                    continue;
                }
                carried += 1;
                let mut partial = table.clone();
                partial.remove(omitted);
                let rebuilt = substitute_identities(pre, &partial);
                assert_ne!(
                    raw_sha256(&rebuilt),
                    raw_sha256(replacement),
                    "{name} without {omitted} must not equal the replacement"
                );
                assert_ne!(
                    domain_digest(schema, &rebuilt),
                    target,
                    "{name} without {omitted} must not reach the replacement identity"
                );
            }
            assert!(
                carried > 0,
                "{name} must carry at least one identity for a partial migration to be possible"
            );

            // Regenerated but unregistered: a current Model that is not the
            // precommitted one is a different Model, however current it is.
            let foreign = transition["bridge"][0]["current_artifact_digest"]
                .as_str()
                .unwrap();
            assert!(!table.values().any(|value| value == foreign));
            let mut regenerated = replacement.to_vec();
            replace_exact_once(
                &mut regenerated,
                expected["current_model_digest"].as_str().unwrap(),
                foreign,
            );
            assert_ne!(regenerated, replacement);
            assert_ne!(
                domain_digest(schema, &regenerated),
                target,
                "{name} built over an unregistered current Model must not reach the replacement"
            );
        }
    }
}

#[test]
fn a_mutated_oracle_byte_or_severed_edge_is_refused() {
    let fixture = &DETERMINISTIC[0];
    let bytes = frozen(fixture.model);

    let mut mutated = bytes.to_vec();
    let marker = b"\"model_ulid\":\"";
    let position = mutated
        .windows(marker.len())
        .position(|window| window == marker)
        .expect("the canonical Model carries one Model ULID")
        + marker.len();
    mutated[position] = if mutated[position] == b'0' {
        b'1'
    } else {
        b'0'
    };
    assert_ne!(raw_sha256(&mutated), raw_sha256(bytes));
    let original_digest = model_digest_from_bytes(bytes);
    let mutated_digest = model_digest_from_bytes(&mutated);
    assert_ne!(mutated_digest, original_digest);
    let decoded = ModelEnvelope::from_json(&mutated, ModelDecoderLimits::default())
        .expect("the valid-JSON mutant must remain admissible to exercise identity drift");
    assert_eq!(decoded.canonical_json().unwrap(), mutated);
    assert_eq!(decoded.digest().unwrap().as_str(), mutated_digest);
    assert_ne!(decoded.digest().unwrap().as_str(), original_digest);

    let transition = transition();
    let expected = entry(&transition, "deterministic", fixture.name);
    let compilation = artifact(fixture, "compilation.json");
    let current = expected["model_digest"].as_str().unwrap();
    let superseded = expected["superseded"][expected["model_pointer"].as_str().unwrap()]
        .as_str()
        .unwrap();
    let severed = String::from_utf8(compilation.to_vec())
        .unwrap()
        .replace(current, superseded);
    assert_ne!(severed.as_bytes(), compilation);
    let edge = &expected["edges"].as_array().unwrap()[0];
    assert_ne!(
        domain_digest(edge["digest_domain"].as_str().unwrap(), severed.as_bytes()),
        edge["digest"].as_str().unwrap(),
        "severing the Model edge must change the compilation identity"
    );
}

#[test]
fn the_classification_is_complete_and_labelled() {
    let classification: Value = serde_json::from_slice(frozen(CLASSIFICATION)).unwrap();
    assert_eq!(
        classification["schema"],
        "eqiora.verify.current-model-relational-identity-classification/v1"
    );
    assert_eq!(
        frozen_inventory().len(),
        classification["search"]["candidate_path_count"]
            .as_u64()
            .unwrap() as usize
    );
    let classes = classification["classes"].as_object().unwrap();
    for required in [
        "deterministic-current-model-bytes",
        "flat-fresh-occurrence",
        "historical-recorded-execution",
        "retained-separate-family-golden",
        "delegated-current-owner-evidence",
        "negative-historical-rejection-corpus",
        "current-owner-assertion",
        "mixed-claim-surface",
        "version-named-current-owner",
    ] {
        assert!(classes.contains_key(required), "missing class {required}");
    }

    let entries = classification["entries"].as_array().unwrap();
    for entry in entries {
        let class = entry["class"].as_str().expect("every entry is classified");
        assert!(classes.contains_key(class), "undeclared class {class}");
        if class == "mixed-claim-surface" {
            let claims = entry["claims"]
                .as_array()
                .expect("a mixed surface classifies each claim separately");
            assert!(!claims.is_empty());
            for claim in claims {
                let claim_class = claim["class"].as_str().unwrap();
                assert!(classes.contains_key(claim_class));
                assert!(!claim["selector"].as_str().unwrap().is_empty());
                assert!(
                    ["delete", "migrate", "delegate"]
                        .contains(&claim["disposition"].as_str().unwrap())
                );
            }
        }
    }
    let compatibility_paths = entries
        .iter()
        .filter(|entry| entry["class"] == "compatibility-only")
        .flat_map(|entry| entry["paths"].as_array().unwrap())
        .map(|path| path.as_str().unwrap())
        .collect::<BTreeSet<_>>();
    for mixed in [
        "crates/eqiora-artifact/tests/model_v8_wire.rs",
        "verify/interfaces/current-authoring-profile/expected/profile.json",
        "crates/eqiora-python/tests/python_control_plane.rs",
        "bindings/python/tests/test_control_plane.py",
        // RFC 0083: "the current implementation hosts v8 encoding in
        // `model_v2.rs`". The version in the file name is the generation the
        // module was born for, not the only generation it still serves, so
        // deleting either file whole would delete the current encoder.
        "crates/eqiora-artifact/src/model_v2.rs",
        "crates/eqiora-artifact/src/model_transaction_v2.rs",
    ] {
        assert!(
            !compatibility_paths.contains(mixed),
            "mixed current/compatibility evidence must not be deleted wholesale: {mixed}"
        );
        assert!(entries.iter().any(|entry| {
            entry["class"] == "mixed-claim-surface"
                && entry["paths"]
                    .as_array()
                    .is_some_and(|paths| paths.iter().any(|path| path == mixed))
        }));
    }
    let historical_negative = entries
        .iter()
        .find(|entry| entry["class"] == "negative-historical-rejection-corpus")
        .unwrap();
    assert_eq!(historical_negative["paths"].as_array().unwrap().len(), 14);
    let canonical_owner_paths = entries
        .iter()
        .filter(|entry| entry["case"] == "artifacts.current-model-canonical-identity")
        .flat_map(|entry| entry["paths"].as_array().unwrap())
        .map(|path| path.as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        canonical_owner_paths.len(),
        26,
        "22 owner files, its Rust oracle, the current cylinder, and the historical cylinder at \
         both its pre-reset and post-reset paths are delegated"
    );
    for required in [
        "verify/artifacts/current-model-canonical-identity/case.toml",
        "verify/artifacts/current-model-canonical-identity/expected/model-v8.json",
        "verify/artifacts/current-model-canonical-identity/expected/model-transaction-v8.json",
        "crates/eqiora-artifact/tests/current_model_wire_oracle.rs",
        "examples/steady-flow-past-cylinder.model.json",
        "examples/steady-flow-past-cylinder.model-v7.json",
        "verify/artifacts/current-model-canonical-identity/expected/historical/\
         steady-flow-past-cylinder.model-v7.json",
    ] {
        assert!(canonical_owner_paths.contains(required));
    }
    let named = entries
        .iter()
        .flat_map(|entry| {
            entry["case"]
                .as_str()
                .map(|case| vec![case.to_owned()])
                .unwrap_or_else(|| {
                    entry["cases"]
                        .as_array()
                        .map(|cases| {
                            cases
                                .iter()
                                .map(|case| case.as_str().unwrap().to_owned())
                                .collect()
                        })
                        .unwrap_or_default()
                })
        })
        .collect::<Vec<_>>();
    for required in [
        "fsi.fixed-topology-ale-monolithic-3d",
        "hybrid.packaged-dc-motor-controller",
        "packages.composed-model-package",
        "packages.offline-model-package",
        "artifacts.realization-run-wire",
        "numerics.canonical-cartesian-poisson-cuda",
        "numerics.canonical-cartesian-poisson-mpi",
        "fsi.fixed-reference-cuda-solve-2d",
        "fsi.fixed-reference-distributed-solve-mpi-2d",
        "fsi.fixed-reference-distributed-cuda-solve-mpi-2d",
        "fsi.fixed-reference-distributed-assembly-mpi-2d",
        "artifacts.fixed-reference-fsi-spatial-trajectory",
        "geometry.model-references-a-geometry",
        "artifacts.model-reference-lineage",
        "interfaces.agent-authored-model-change",
        "artifacts.current-model-canonical-identity",
        "interfaces.control-plane-compile-check",
    ] {
        assert!(
            named.iter().any(|case| case == required),
            "RFC 0083 requires {required} to be classified"
        );
    }

    let owned = entries
        .iter()
        .filter(|entry| {
            entry["class"] == "deterministic-current-model-bytes" && entry["owned_here"] == true
        })
        .count();
    assert_eq!(owned, DETERMINISTIC.len());
    let literals: u64 = entries
        .iter()
        .filter(|entry| {
            entry["class"] == "deterministic-current-model-bytes" && entry["owned_here"] == true
        })
        .map(|entry| entry["identity_literals"].as_u64().unwrap())
        .sum();
    let transition = transition();
    let frozen_literals: usize = DETERMINISTIC
        .iter()
        .map(|fixture| {
            entry(&transition, "deterministic", fixture.name)["edges"]
                .as_array()
                .unwrap()
                .len()
                + 1
        })
        .sum();
    assert_eq!(literals as usize, frozen_literals);
}

#[test]
fn every_inventory_path_carries_exactly_one_disposition() {
    let classification: Value = serde_json::from_slice(frozen(CLASSIFICATION)).unwrap();
    let search = &classification["search"];
    let vocabulary = classification["dispositions"]
        .as_object()
        .expect("the classification must declare the fates it assigns");
    assert_eq!(
        vocabulary.keys().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "decompose-by-claim".to_owned(),
            "delegate".to_owned(),
            "delete".to_owned(),
            "migrate".to_owned(),
            "migrate-in-place".to_owned(),
            "preserve-bytes".to_owned(),
            "rename-source".to_owned(),
        ]),
        "the vocabulary is frozen so a new fate cannot appear without review"
    );

    let entries = classification["entries"].as_array().unwrap();
    let inventory = frozen_inventory();
    let retired = search["transition"]["retired"]
        .as_array()
        .unwrap()
        .iter()
        .map(|path| path.as_str().unwrap().to_owned())
        .collect::<BTreeSet<_>>();

    let mut fate: BTreeMap<String, String> = BTreeMap::new();
    for entry in entries {
        let paths = entry["paths"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        if paths.is_empty() {
            continue;
        }
        let disposition = entry["disposition"].as_str().unwrap_or_else(|| {
            panic!(
                "the {} entry names {} paths and must declare their fate",
                entry["class"],
                paths.len()
            )
        });
        assert!(
            vocabulary.contains_key(disposition),
            "undeclared disposition {disposition}"
        );
        if entry["class"] == "mixed-claim-surface" {
            assert_eq!(
                disposition, "decompose-by-claim",
                "a mixed surface's fate is the per-claim list, never a single verb"
            );
            for claim in entry["claims"].as_array().unwrap() {
                assert!(vocabulary.contains_key(claim["disposition"].as_str().unwrap()));
            }
        }
        for path in paths {
            let path = path.as_str().unwrap().to_owned();
            assert!(
                fate.insert(path.clone(), disposition.to_owned()).is_none(),
                "`{path}` is classified by more than one entry, so it has no single fate"
            );
        }
    }

    let remainder_entry = entries
        .iter()
        .find(|entry| entry["inventory_remainder"] == true)
        .expect("the classification must declare its inventory remainder");
    assert_eq!(remainder_entry["disposition"], "migrate-in-place");
    assert_eq!(remainder_entry["excludes_retired"], true);
    assert!(
        remainder_entry["paths"].is_null(),
        "the remainder is defined by exclusion; a path list here would be a second inventory"
    );

    let remainder = inventory
        .iter()
        .filter(|path| !fate.contains_key(*path))
        .cloned()
        .collect::<BTreeSet<_>>();
    let classified = inventory.len() - remainder.len();
    assert_eq!(
        remainder.len(),
        remainder_entry["path_count"].as_u64().unwrap() as usize,
        "the remainder count is frozen beside the rule that defines it"
    );
    assert_eq!(
        classified,
        search["classified_inventory_path_count"].as_u64().unwrap() as usize
    );
    assert_eq!(
        classified + remainder.len(),
        search["candidate_path_count"].as_u64().unwrap() as usize,
        "named paths and the remainder partition the inventory exactly"
    );

    let retired_in_remainder = remainder.intersection(&retired).collect::<Vec<_>>();
    assert!(
        retired_in_remainder.is_empty(),
        "the reset removes {retired_in_remainder:?}; a removed path cannot migrate in place"
    );
    for path in retired.intersection(&inventory) {
        let disposition = fate
            .get(path)
            .unwrap_or_else(|| panic!("retired inventory path `{path}` must be classified"));
        assert!(
            ["delete", "rename-source", "delegate", "decompose-by-claim"]
                .contains(&disposition.as_str()),
            "`{path}` is retired; `{disposition}` is not a fate a removed path can have"
        );
    }

    for path in [
        "bindings/python/python/eqiora/compatibility.py",
        "bindings/python/python/eqiora/compatibility.pyi",
        "crates/eqiora-api/src/codec.rs",
        "crates/eqiora-api/tests/control_compile_v1.rs",
        "crates/eqiora-api/tests/versioned_model_document.rs",
        "crates/eqiora-artifact/src/model_v3.rs",
        "crates/eqiora-artifact/src/model_v4.rs",
        "crates/eqiora-artifact/src/model_v5.rs",
        "crates/eqiora-artifact/src/model_v6.rs",
        "crates/eqiora-artifact/src/model_v7.rs",
        "crates/eqiora-artifact/src/model_transaction_v3.rs",
        "crates/eqiora-artifact/src/model_transaction_v4.rs",
        "crates/eqiora-artifact/src/model_transaction_v5.rs",
        "crates/eqiora-artifact/src/model_transaction_v6.rs",
        "crates/eqiora-artifact/src/model_transaction_v7.rs",
    ] {
        assert_eq!(fate[path], "delete", "`{path}` is deleted by the reset");
    }

    for (path, owner) in [
        (
            "crates/eqiora-artifact/src/model_v2.rs",
            "crates/eqiora-artifact/src/model_wire.rs",
        ),
        (
            "crates/eqiora-artifact/src/model_transaction_v2.rs",
            "crates/eqiora-artifact/src/model_transaction_wire.rs",
        ),
    ] {
        assert_eq!(
            fate[path], "decompose-by-claim",
            "`{path}` hosts both a removed generation and the current encoder"
        );
        let entry = entries
            .iter()
            .find(|entry| {
                entry["paths"]
                    .as_array()
                    .is_some_and(|paths| paths.iter().any(|candidate| candidate == path))
            })
            .unwrap();
        let claims = entry["claims"].as_array().unwrap();
        let dispositions = claims
            .iter()
            .map(|claim| claim["disposition"].as_str().unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            dispositions,
            BTreeSet::from(["delete", "migrate"]),
            "`{path}` loses its historical branch and keeps its current implementation"
        );
        let migrating = claims
            .iter()
            .find(|claim| claim["disposition"] == "migrate")
            .unwrap();
        assert_eq!(
            migrating["class"], "current-owner-assertion",
            "the surviving claim is a current assertion, not compatibility"
        );
        assert_eq!(
            migrating["owner"], owner,
            "`{path}` migrates its current v8 implementation to `{owner}`, not to the other owner"
        );
    }

    let renamed = [
        (
            "crates/eqiora-artifact/src/model_v8.rs",
            "crates/eqiora-artifact/src/model_wire.rs",
        ),
        (
            "crates/eqiora-artifact/src/model_transaction_v8.rs",
            "crates/eqiora-artifact/src/model_transaction_wire.rs",
        ),
    ];
    for (path, _) in renamed {
        assert_eq!(
            fate[path], "rename-source",
            "`{path}` hosts the current encoding and is renamed, not deleted"
        );
    }

    let rename_entry = entries
        .iter()
        .find(|entry| entry["disposition"] == "rename-source")
        .expect("the version-named current owners must be classified together");
    let sources = rename_entry["paths"]
        .as_array()
        .unwrap()
        .iter()
        .map(|path| path.as_str().unwrap())
        .collect::<Vec<_>>();
    let targets = rename_entry["renames_to"]
        .as_array()
        .expect("a rename names its target owner")
        .iter()
        .map(|path| path.as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(
        rename_entry["rename_pairing"]
            .as_str()
            .expect("parallel arrays are only readable once their pairing is declared")
            .contains("positional"),
        "`renames_to` is paired with `paths` by position; nothing may leave that implicit"
    );
    assert_eq!(sources.len(), targets.len());
    assert_eq!(
        rename_entry["renames"]
            .as_array()
            .expect("the pairing is frozen as explicit from/to pairs")
            .iter()
            .map(|rename| (
                rename["from"].as_str().unwrap(),
                rename["to"].as_str().unwrap()
            ))
            .collect::<Vec<_>>(),
        renamed.to_vec(),
        "Model renames to the Model wire owner and Transaction to the Transaction one"
    );
    assert_eq!(
        sources
            .iter()
            .copied()
            .zip(targets.iter().copied())
            .collect::<Vec<_>>(),
        renamed.to_vec(),
        "the declared positional zip of `paths` and `renames_to` is the same pairing"
    );
    assert_eq!(
        targets
            .iter()
            .map(|path| (*path).to_owned())
            .collect::<BTreeSet<_>>(),
        search["transition"]["required_post_reset_without_frozen_bytes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|path| path.as_str().unwrap().to_owned())
            .collect::<BTreeSet<_>>()
    );
}
