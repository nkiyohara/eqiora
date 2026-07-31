//! Independent oracle for the relational identity transition of RFC 0083.
//!
//! Changing the Model digest changes a downstream artifact only when that exact
//! artifact embeds the Model reference. This test owns the classification's
//! executable half: it proves that every precommitted current Model reproduces
//! its frozen identity through the current owner, that every downstream
//! identity is re-derivable from committed bytes alone, that each replacement
//! fixture differs from the fixture it replaces at exactly its identity
//! pointers, that the recorded accelerator bundles are bridged rather than
//! relabelled, and that every classified path carries exactly one fate.
//!
//! Every literal lives under the registered case, never inside this crate. The
//! values were observed by replaying the accepted deterministic producers
//! through their already-live current encoder and then re-derived from bytes;
//! see `verify/artifacts/current-model-relational-identity-transition/
//! references/` for the exact route. Production encoding is compared *to* those
//! literals; it never defines them.
//!
//! No historical decoder appears below. The historical side of each bridge is
//! hashed straight from its untouched bytes, exactly as RFC 0083 requires once
//! the historical decoders are gone.
//!
//! The case's second responsibility — the frozen two-state transition contract
//! and the repository sweep that feeds it — lives in the private support module
//! below, included with `#[path]` so this stays one Cargo integration-test
//! target. Both files are excluded from that sweep by exact path, because both
//! spell the tokens it searches for; see `ORACLE_FILES` there.

use eqiora_artifact::{
    CanonicalModelArtifact, ModelDecoderLimits, ModelEnvelopeV8, ReplayableCanonicalModelArtifact,
    SemanticFingerprintGeneration, StructuralSemanticFingerprint,
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

/// One deterministic fixture: its current Model plus every downstream artifact
/// whose identity it moves, and the complete replacement for its consumer.
struct Deterministic {
    name: &'static str,
    model: &'static [u8],
    replacement: &'static [u8],
    committed: &'static [u8],
    artifacts: &'static [(&'static str, &'static [u8])],
}

const DETERMINISTIC: [Deterministic; 5] = [
    Deterministic {
        name: "packaged-dc-motor-controller",
        model: oracle!("expected/deterministic/packaged-dc-motor-controller/model.json"),
        replacement: oracle!("expected/deterministic/packaged-dc-motor-controller/identities.json"),
        committed: include_bytes!(
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
        replacement: oracle!("expected/deterministic/composed-model-package/identities.json"),
        committed: include_bytes!(
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
        replacement: oracle!("expected/deterministic/offline-model-package/identities.json"),
        committed: include_bytes!(
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
        name: "typed-execution-lineage",
        model: oracle!("expected/deterministic/typed-execution-lineage/model.json"),
        replacement: oracle!("expected/deterministic/typed-execution-lineage/identities.json"),
        committed: include_bytes!(
            "../../../verify/packages/typed-execution-lineage/expected/identities.json"
        ),
        artifacts: &[
            (
                "compilation.json",
                oracle!("expected/deterministic/typed-execution-lineage/compilation.json"),
            ),
            (
                "realization.json",
                oracle!("expected/deterministic/typed-execution-lineage/realization.json"),
            ),
            (
                "run.json",
                oracle!("expected/deterministic/typed-execution-lineage/run.json"),
            ),
            (
                "binding.json",
                oracle!("expected/deterministic/typed-execution-lineage/binding.json"),
            ),
        ],
    },
    Deterministic {
        name: "fixed-topology-ale-monolithic-3d",
        model: oracle!("expected/deterministic/fixed-topology-ale-monolithic-3d/model.json"),
        replacement: oracle!(
            "expected/deterministic/fixed-topology-ale-monolithic-3d/accepted-trajectory.json"
        ),
        committed: include_bytes!(
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

/// One recorded accelerator bundle: its untouched historical Model and the
/// current Model built from the same decoded semantic program.
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

/// RFC 0008 content identity: schema-domain bytes, one NUL, canonical content.
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

/// The Model content projection omits `source_revision`: a graph revision is
/// provenance, not meaning.
///
/// The projection is cut out of the canonical bytes rather than rebuilt from a
/// parsed value, so no re-serializer can reorder a key or respell a float on
/// the way. That keeps this a second, independent derivation of the digest
/// rather than a second call into the producer.
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

/// Walk one `/key[index]...` path, matching the paths the route record emits.
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

        // Derivation one: the bytes themselves, through the RFC 0008 domain.
        let digest = expected["model_digest"].as_str().unwrap();
        assert_eq!(model_digest_from_bytes(bytes), digest);

        // Derivation two: the current owner, which must agree and must also
        // round-trip these exact bytes and replay the same program.
        let decoded = ModelEnvelopeV8::from_json(bytes, ModelDecoderLimits::default())
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

    // A silently emptied edge set would make the claim above vacuous.
    assert!(
        checked_edges >= 47,
        "the classified reference DAG has at least 47 edges, found {checked_edges}"
    );
}

#[test]
fn every_replacement_changes_exactly_its_identity_pointers() {
    let transition = transition();

    for fixture in &DETERMINISTIC {
        let expected = entry(&transition, "deterministic", fixture.name);
        let committed_bytes = frozen(fixture.committed);
        let replacement_bytes = frozen(fixture.replacement);
        let committed: Value = serde_json::from_slice(committed_bytes).unwrap();
        let replacement: Value = serde_json::from_slice(replacement_bytes).unwrap();

        let mut pointers = vec![expected["model_pointer"].as_str().unwrap().to_owned()];
        pointers.extend(
            expected["edges"]
                .as_array()
                .unwrap()
                .iter()
                .map(|edge| edge["pointer"].as_str().unwrap().to_owned()),
        );

        let before = flatten(&committed);
        let after = flatten(&replacement);
        assert_eq!(
            before.iter().map(|(path, _)| path).collect::<Vec<_>>(),
            after.iter().map(|(path, _)| path).collect::<Vec<_>>(),
            "{} replacement must not add, drop, or reorder a leaf",
            fixture.name
        );

        let mut changed = before
            .iter()
            .zip(&after)
            .filter(|((_, old), (_, new))| old != new)
            .map(|((path, _), _)| path.clone())
            .collect::<Vec<_>>();
        changed.sort();
        let mut permitted = pointers.clone();
        permitted.sort();
        assert_eq!(
            changed, permitted,
            "{} delta must be exactly its identity pointers: arrays, coordinates, \
             times, tolerances, source and package identities are immutable",
            fixture.name
        );

        // This is stronger than semantic JSON equality: the committed target
        // may change only by replacing each unique 64-byte identity literal
        // with its precommitted same-length successor. Key order, whitespace,
        // number spelling, and every non-identity byte must remain untouched.
        let mut exact_replacement = committed_bytes.to_vec();
        for pointer in &pointers {
            replace_exact_once(
                &mut exact_replacement,
                expected["superseded"][pointer].as_str().unwrap(),
                resolve(&replacement, pointer).as_str().unwrap(),
            );
        }
        assert_eq!(
            exact_replacement, replacement_bytes,
            "{} replacement must be a byte-exact identity substitution",
            fixture.name
        );

        // Every superseded identity must be retired, and none may reappear
        // anywhere else in the replacement.
        for pointer in &pointers {
            let superseded = expected["superseded"][pointer].as_str().unwrap();
            assert_eq!(resolve(&committed, pointer), superseded);
            assert_ne!(
                resolve(&replacement, pointer),
                superseded,
                "{} {pointer} must not keep its superseded identity",
                fixture.name
            );
            assert!(
                after.iter().all(|(_, value)| value != superseded),
                "{} must not retain superseded identity {superseded}",
                fixture.name
            );
        }
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

        // Hashed from the untouched bytes. No product decoder admits them, so
        // this survives the removal of the historical decoders unchanged.
        let historical_wire: Value = serde_json::from_slice(historical).unwrap();
        assert_eq!(historical_wire["schema"], expected["historical_schema"]);
        assert_ne!(
            historical_wire["schema"], CURRENT_MODEL_SCHEMA,
            "{} historical bytes must stay on their own schema",
            bridge.name
        );
        let historical_digest = expected["historical_artifact_digest"].as_str().unwrap();
        assert_eq!(model_digest_from_bytes(historical), historical_digest);

        // The current side goes through the current owner and nothing else.
        let current = frozen(bridge.current);
        assert_eq!(raw_sha256(current), expected["current_raw_sha256"]);
        let current_digest = expected["current_artifact_digest"].as_str().unwrap();
        assert_eq!(model_digest_from_bytes(current), current_digest);
        let decoded = ModelEnvelopeV8::from_json(current, ModelDecoderLimits::default()).unwrap();
        assert_eq!(decoded.canonical_json().unwrap(), current);
        assert_eq!(decoded.digest().unwrap().as_str(), current_digest);
        assert_ne!(
            current_digest, historical_digest,
            "{} schema domain is part of identity",
            bridge.name
        );

        // Semantic identity survives the epoch change; the artifact identity
        // does not. Model ULID and revision are read from the historical bytes.
        assert_eq!(
            decoded.model().unwrap().ulid().to_string(),
            historical_wire["model_ulid"]
        );
        assert_eq!(
            decoded.source_revision(),
            historical_wire["source_revision"]
        );

        // The bridge relation itself. The historical value was observed once,
        // while its decoder still existed, and is frozen in the case; the
        // current value is recomputed here by the RFC 0073 owner.
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

        // The recorded Run observed the historical artifact. Relabelling it as
        // a current Run would break exactly here.
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
fn a_mutated_oracle_byte_or_severed_edge_is_refused() {
    let fixture = &DETERMINISTIC[0];
    let bytes = frozen(fixture.model);

    // Mutate one valid Crockford Base32 character in the Model ULID while
    // preserving canonical JSON and the envelope's admissibility. This makes
    // parse failure unavailable as an escape hatch: both independent identity
    // derivations and the exact current-owner round-trip must observe the
    // changed meaning-bearing bytes.
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
    let decoded = ModelEnvelopeV8::from_json(&mutated, ModelDecoderLimits::default())
        .expect("the valid-JSON mutant must remain admissible to exercise identity drift");
    assert_eq!(decoded.canonical_json().unwrap(), mutated);
    assert_eq!(decoded.digest().unwrap().as_str(), mutated_digest);
    assert_ne!(decoded.digest().unwrap().as_str(), original_digest);

    // A downstream artifact re-pointed at the superseded Model must not keep
    // its identity, so a half-applied migration cannot pass.
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

    // Every entry belongs to exactly one declared class, and each required
    // target of RFC 0083 is present with a class.
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
        "packages.typed-execution-lineage",
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

    // The deterministic class this test executes must match the classification.
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

/// Every classified path has exactly one fate, and a path the reset removes
/// never inherits the remainder's in-place migration.
///
/// The remainder entry is what makes the classification complete without
/// listing 338 paths twice, and it is also the entry that can quietly say the
/// wrong thing: "everything else migrates in place" is false the moment a
/// retired path is left unnamed. So the remainder is defined to exclude retired
/// paths, every retired inventory path is named by an entry that says what
/// actually happens to it, and both halves are counted here.
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

    // One disposition per entry, one entry per path.
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

    // The remainder is an exclusion rule, not a list, and it excludes retired
    // paths explicitly rather than by hoping none is left over.
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

    // No retired path inherits `migrate-in-place`.
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

    // The nineteen retired inventory paths that no fixture entry names divide
    // three ways: fifteen whose sole claim is a removed generation, two whose
    // historical branch goes but whose current v8 implementation moves, and two
    // version-named current owners the reset renames rather than deletes.
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

    // The two v2-named modules are retired paths whose fate is not a single
    // verb: RFC 0083 puts the current v8 encoding inside them, so the historical
    // admission is deleted while the current implementation migrates to the
    // matching unversioned owner. Each must carry both claims, and the migrating
    // one must name that owner rather than a general direction.
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

    // A renamed source names where it goes, and it names it per file. Two sets
    // that happen to have the same members would let Model and Transaction swap
    // targets silently, so the pairing is frozen as ordered `from`/`to` pairs and
    // the parallel arrays are checked against them as a declared positional zip.
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
