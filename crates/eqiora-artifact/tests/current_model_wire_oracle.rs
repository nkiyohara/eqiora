//! Independent full-byte oracle for the one current Model artifact epoch.
//!
//! Every literal below was derived without the Rust producer serializer: the
//! canonical JSON was re-encoded by hand from the frozen public fixture and the
//! wire contract, and both hashes were computed with Python `hashlib`. See
//! `verify/artifacts/current-model-canonical-identity/references/` for the exact
//! route. Production encoding is compared *to* those literals; it never defines
//! them.
//!
//! The `historical/` corpus is a **negative** corpus only. It exists to prove
//! that the current decoder refuses historical bytes, never to claim that any
//! historical generation is still callable.

use eqiora_artifact::{
    CanonicalModelArtifact, ModelDecoderLimits, ModelEnvelopeV8, ModelTransactionEnvelopeV8,
    ReplayableCanonicalModelArtifact,
};
use eqiora_core::entity::kinds;
use eqiora_core::{DimExponents, DynQuantity, Entity, Id, OntologyId};
use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora_schema::kernel::{
    ActivationDef, CartesianAxisDefinition, CartesianCoordinateSource, DomainDef, ExprDagBuilder,
    KernelNode, ParameterDef, RelationDef,
};
use eqiora_schema::{Model, ModelView};
use eqiora_sem::KernelProgram;
use serde_json::Value;
use sha2::{Digest, Sha256};
use ulid::Ulid;

/// Every oracle byte lives under the registered case, never inside this crate.
macro_rules! oracle {
    ($name:literal) => {
        include_bytes!(concat!(
            "../../../verify/artifacts/current-model-canonical-identity/expected/",
            $name
        ))
    };
}

const MODEL_JSON: &[u8] = oracle!("model-v8.json");
const TRANSACTION_JSON: &[u8] = oracle!("model-transaction-v8.json");
const CYLINDER_JSON: &[u8] =
    include_bytes!("../../../examples/steady-flow-past-cylinder.model.json");
/// Retained only to prove that the re-encoded resource carries the identical
/// semantic Model. The historical resource itself is not decoded here.
const CYLINDER_V7_JSON: &[u8] =
    include_bytes!("../../../examples/steady-flow-past-cylinder.model-v7.json");

const MODEL_BYTES: usize = 2_347;
const MODEL_RAW_SHA256: &str = "7e179d0d90f8789b9818eae7b5696e10c33a9350a34205d2e7cfd56b938aa427";
const MODEL_DIGEST: &str = "e410295337a3a51a271f272e03ae7d7a4b8e7df1b04faf76645bb1e18567e4b3";
const TRANSACTION_BYTES: usize = 2_646;
const TRANSACTION_RAW_SHA256: &str =
    "5ceeef06b286e3edba6f7978c42c227a3c78db2f1d80de7d9b917ec23f8afc47";
const TRANSACTION_DIGEST: &str = "132168803ac8882f0f35187215d3f2ce44817d03921d6ad95b73a9cac62aa102";

const CYLINDER_BYTES: usize = 16_797;
const CYLINDER_RAW_SHA256: &str =
    "672016cb80683fb1448adab79d7c8f6a2fdda22f92c6df2d82b684bd5e65e099";
const CYLINDER_DIGEST: &str = "8bc5155bc1b64ed37f7a2ac010a966e1619091a118e6cf7806dbdf9621977146";
/// The schema-domain identity the same semantic Model carried as v7. It must
/// not survive the epoch change.
const CYLINDER_V7_DIGEST: &str = "668fa55e5ab1a46d0b7523e4e3162442ccd7698697c4308604cf4fe9269249de";
const CYLINDER_MODEL_ULID: &str = "01KYQFNFX85DKM2SE5FR6H4WPJ";
const CYLINDER_SOURCE_REVISION: u64 = 1;

const MODEL_SCHEMA: &[u8] = b"\"schema\":\"eqiora.model-envelope/v8\"";
const TRANSACTION_SCHEMA: &[u8] = b"\"schema\":\"eqiora.model-transaction-envelope/v8\"";

const HISTORICAL_MODELS: [(u8, &[u8]); 7] = [
    (1, oracle!("historical/model-v1.json")),
    (2, oracle!("historical/model-v2.json")),
    (3, oracle!("historical/model-v3.json")),
    (4, oracle!("historical/model-v4.json")),
    (5, oracle!("historical/model-v5.json")),
    (6, oracle!("historical/model-v6.json")),
    (7, oracle!("historical/model-v7.json")),
];

const HISTORICAL_TRANSACTIONS: [(u8, &[u8]); 7] = [
    (1, oracle!("historical/model-transaction-v1.json")),
    (2, oracle!("historical/model-transaction-v2.json")),
    (3, oracle!("historical/model-transaction-v3.json")),
    (4, oracle!("historical/model-transaction-v4.json")),
    (5, oracle!("historical/model-transaction-v5.json")),
    (6, oracle!("historical/model-transaction-v6.json")),
    (7, oracle!("historical/model-transaction-v7.json")),
];

const FIXED_TIMESTAMP_MILLIS: u64 = 1_700_000_008_000;

fn frozen(bytes: &[u8]) -> &[u8] {
    bytes.strip_suffix(b"\n").unwrap_or(bytes)
}

fn raw_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn replace_once(bytes: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let start = bytes
        .windows(from.len())
        .position(|window| window == from)
        .expect("specimen carries its own schema identifier exactly once");
    assert!(
        bytes[start + 1..]
            .windows(from.len())
            .all(|window| window != from),
        "schema identifier must be unique so substitution is unambiguous"
    );
    let mut result = bytes[..start].to_vec();
    result.extend_from_slice(to);
    result.extend_from_slice(&bytes[start + from.len()..]);
    result
}

fn historical_schema(family: &str, version: u8) -> Vec<u8> {
    format!("\"schema\":\"eqiora.{family}-envelope/v{version}\"").into_bytes()
}

fn fixed_id<E: Entity>(random: u128) -> Id<E> {
    Id::from_ulid(Ulid::from_parts(FIXED_TIMESTAMP_MILLIS, random))
}

fn length() -> DimExponents {
    DimExponents {
        length: 1,
        ..DimExponents::DIMENSIONLESS
    }
}

fn metres(value: f64) -> DynQuantity {
    DynQuantity::new(value, length())
}

struct Fixture {
    program: KernelProgram,
    transaction: Transaction,
}

/// The frozen public fixture: one root length Parameter driving two endpoints
/// of one 3D Cartesian Domain, recorded by exactly one
/// `Domain --DependsOn--> Parameter` edge.
fn fixture() -> Fixture {
    let parameter = fixed_id::<kinds::Parameter>(1);
    let body = fixed_id::<kinds::Domain>(2);
    let retain = fixed_id::<kinds::Relation>(3);
    let activation = fixed_id::<kinds::Activation>(4);
    let model = OntologyId::<Model>::from_ulid(Ulid::from_parts(FIXED_TIMESTAMP_MILLIS, 5));

    let driven = CartesianCoordinateSource::parameter(parameter);
    let coordinates = vec![
        CartesianAxisDefinition::new(
            CartesianCoordinateSource::fixed(metres(-1.0)).unwrap(),
            driven,
        ),
        CartesianAxisDefinition::new(
            driven,
            CartesianCoordinateSource::fixed(metres(6.0)).unwrap(),
        ),
        CartesianAxisDefinition::new(
            CartesianCoordinateSource::fixed(metres(0.5)).unwrap(),
            CartesianCoordinateSource::fixed(metres(5.5)).unwrap(),
        ),
    ];
    let mut residual = ExprDagBuilder::new();
    let coordinate = residual.spatial_coordinate(0).unwrap();
    let zero = residual.sub(coordinate, coordinate).unwrap();

    let nodes = vec![
        KernelNode::from(ParameterDef::new(parameter, metres(2.0))),
        KernelNode::from(DomainDef::cartesian_box_from_sources(body, coordinates).unwrap()),
        KernelNode::from(RelationDef::new(retain, residual.finish([zero]).unwrap())),
        KernelNode::from(ActivationDef::continuous(activation)),
    ];

    let mut transaction = Transaction::new("parameter-driven Cartesian model v8 fixture");
    for node in &nodes {
        transaction.push(Op::DefineKernelNode { node: node.clone() });
    }
    transaction
        .push(Op::Connect {
            from: body.erase(),
            to: parameter.erase(),
            edge: EdgeKind::DependsOn,
        })
        .push(Op::Connect {
            from: retain.erase(),
            to: body.erase(),
            edge: EdgeKind::AppliesOn,
        })
        .push(Op::Connect {
            from: activation.erase(),
            to: retain.erase(),
            edge: EdgeKind::Activates,
        })
        .push(Op::DefineOntologyView {
            view: ModelView::new(model, nodes.iter().map(KernelNode::id), [])
                .unwrap()
                .into(),
        });

    // The exact authored operation order is the transaction artifact's meaning,
    // so it is retained before the commit consumes the transaction.
    let mut retained = Transaction::new(transaction.label());
    for op in transaction.ops() {
        retained.push(op.clone());
    }
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    let program = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
    Fixture {
        program,
        transaction: retained,
    }
}

fn reordered(transaction: &Transaction) -> Transaction {
    let mut reversed = Transaction::new(transaction.label());
    for op in transaction.ops().iter().rev() {
        reversed.push(op.clone());
    }
    reversed
}

fn coordinates_of(wire: &mut Value) -> &mut Vec<Value> {
    wire["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find_map(|node| node["definition"]["domain"]["coordinates"].as_array_mut())
        .unwrap()
}

fn decode_model(wire: &Value) -> ModelEnvelopeV8 {
    ModelEnvelopeV8::from_json(
        &serde_json::to_vec(wire).unwrap(),
        ModelDecoderLimits::default(),
    )
    .unwrap()
}

#[test]
fn current_model_bytes_equal_the_independently_frozen_oracle() {
    let frozen_bytes = frozen(MODEL_JSON);
    assert_eq!(frozen_bytes.len(), MODEL_BYTES);
    assert_eq!(raw_sha256(frozen_bytes), MODEL_RAW_SHA256);

    let envelope = ModelEnvelopeV8::from_program(&fixture().program).unwrap();
    let produced = envelope.canonical_json().unwrap();
    assert_eq!(
        produced, frozen_bytes,
        "production encoding must reproduce the pre-committed oracle bytes exactly"
    );
    assert_eq!(envelope.digest().unwrap().as_str(), MODEL_DIGEST);

    let decoded = ModelEnvelopeV8::from_json(frozen_bytes, ModelDecoderLimits::default()).unwrap();
    assert_eq!(decoded.canonical_json().unwrap(), frozen_bytes);
    assert_eq!(decoded.digest().unwrap().as_str(), MODEL_DIGEST);
    assert_eq!(decoded.to_program().unwrap(), fixture().program);
    decoded
        .artifact_reference()
        .unwrap()
        .validate_artifact(&decoded)
        .unwrap();
    assert_eq!(
        decoded.replay_model().unwrap().program(),
        &fixture().program
    );
}

#[test]
fn current_model_canonicalizes_input_permutations() {
    let frozen_bytes = frozen(MODEL_JSON);
    let mut permuted: Value = serde_json::from_slice(frozen_bytes).unwrap();
    permuted["nodes"].as_array_mut().unwrap().reverse();
    permuted["edges"].as_array_mut().unwrap().reverse();
    permuted["values"].as_array_mut().unwrap().reverse();

    let decoded = decode_model(&permuted);
    assert_eq!(decoded.canonical_json().unwrap(), frozen_bytes);
    assert_eq!(decoded.digest().unwrap().as_str(), MODEL_DIGEST);
}

#[test]
fn current_model_axis_and_role_permutations_change_identity() {
    let frozen_bytes = frozen(MODEL_JSON);

    // Axis order is semantic: no set canonicalization may absorb it.
    let mut axes: Value = serde_json::from_slice(frozen_bytes).unwrap();
    coordinates_of(&mut axes).reverse();
    let decoded = decode_model(&axes);
    assert_ne!(decoded.canonical_json().unwrap(), frozen_bytes);
    assert_ne!(decoded.digest().unwrap().as_str(), MODEL_DIGEST);

    // The lower/upper roles inside one axis are semantic for the same reason.
    let mut decoded_swaps = 0;
    for axis in 0..3 {
        let mut roles: Value = serde_json::from_slice(frozen_bytes).unwrap();
        let coordinates = coordinates_of(&mut roles);
        let lower = coordinates[axis]["lower"].clone();
        let upper = coordinates[axis]["upper"].clone();
        coordinates[axis]["lower"] = upper;
        coordinates[axis]["upper"] = lower;
        let swapped = serde_json::to_vec(&roles).unwrap();
        // A fail-closed rejection is equally a role-sensitive outcome, so only
        // an accepted swap has an identity left to compare.
        if let Ok(decoded) = ModelEnvelopeV8::from_json(&swapped, ModelDecoderLimits::default()) {
            decoded_swaps += 1;
            assert_ne!(
                decoded.canonical_json().unwrap(),
                frozen_bytes,
                "axis {axis} role swap must not canonicalize back"
            );
            assert_ne!(decoded.digest().unwrap().as_str(), MODEL_DIGEST);
        }
    }
    assert!(
        decoded_swaps > 0,
        "at least one role swap must reach the identity comparison, or this \
         falsifier passes vacuously through rejection alone"
    );
}

#[test]
fn current_transaction_bytes_equal_the_independently_frozen_oracle() {
    let frozen_bytes = frozen(TRANSACTION_JSON);
    assert_eq!(frozen_bytes.len(), TRANSACTION_BYTES);
    assert_eq!(raw_sha256(frozen_bytes), TRANSACTION_RAW_SHA256);

    let fixture = fixture();
    let envelope = ModelTransactionEnvelopeV8::from_transaction(&fixture.transaction).unwrap();
    assert_eq!(
        envelope.canonical_json().unwrap(),
        frozen_bytes,
        "production encoding must reproduce the pre-committed oracle bytes exactly"
    );
    assert_eq!(envelope.digest().unwrap().as_str(), TRANSACTION_DIGEST);

    let decoded =
        ModelTransactionEnvelopeV8::from_json(frozen_bytes, ModelDecoderLimits::default()).unwrap();
    assert_eq!(decoded.canonical_json().unwrap(), frozen_bytes);
    assert_eq!(decoded.digest().unwrap().as_str(), TRANSACTION_DIGEST);

    let replayed = decoded.to_transaction().unwrap();
    assert_eq!(replayed.label(), fixture.transaction.label());
    assert_eq!(
        replayed.preconditions(),
        fixture.transaction.preconditions()
    );
    assert_eq!(replayed.ops(), fixture.transaction.ops());
}

#[test]
fn current_transaction_operation_reversal_produces_a_distinct_artifact() {
    let fixture = fixture();
    let forward = ModelTransactionEnvelopeV8::from_transaction(&fixture.transaction).unwrap();
    let reverse =
        ModelTransactionEnvelopeV8::from_transaction(&reordered(&fixture.transaction)).unwrap();
    assert_ne!(
        forward.canonical_json().unwrap(),
        reverse.canonical_json().unwrap()
    );
    assert_ne!(forward.digest().unwrap(), reverse.digest().unwrap());
    assert_eq!(
        reverse.to_transaction().unwrap().ops(),
        reordered(&fixture.transaction).ops()
    );
}

#[test]
fn historical_model_and_transaction_specimens_reject_through_the_current_decoder() {
    for (version, specimen) in HISTORICAL_MODELS {
        let specimen = frozen(specimen);
        let error = ModelEnvelopeV8::from_json(specimen, ModelDecoderLimits::default())
            .expect_err("historical Model bytes must not enter the current decoder");
        assert!(
            error.message().contains("eqiora.model-envelope/v8"),
            "v{version} rejection must name the one supported schema: {}",
            error.message()
        );
    }
    for (version, specimen) in HISTORICAL_TRANSACTIONS {
        let specimen = frozen(specimen);
        let error = ModelTransactionEnvelopeV8::from_json(specimen, ModelDecoderLimits::default())
            .expect_err("historical Transaction bytes must not enter the current decoder");
        assert!(
            error
                .message()
                .contains("eqiora.model-transaction-envelope/v8"),
            "v{version} rejection must name the one supported schema: {}",
            error.message()
        );
    }
}

#[test]
fn schema_substitution_alone_cannot_admit_a_historical_specimen() {
    for (version, specimen) in HISTORICAL_MODELS {
        let forged = replace_once(
            frozen(specimen),
            &historical_schema("model", version),
            MODEL_SCHEMA,
        );
        let error = ModelEnvelopeV8::from_json(&forged, ModelDecoderLimits::default())
            .expect_err("relabelling a historical Model must not make its meaning current");
        assert!(
            error.message().contains("model wire v8"),
            "v{version} must be refused on meaning, not only on its schema string: {}",
            error.message()
        );
    }
    for (version, specimen) in HISTORICAL_TRANSACTIONS {
        let forged = replace_once(
            frozen(specimen),
            &historical_schema("model-transaction", version),
            TRANSACTION_SCHEMA,
        );
        let error = ModelTransactionEnvelopeV8::from_json(&forged, ModelDecoderLimits::default())
            .expect_err("relabelling a historical Transaction must not make its meaning current");
        assert!(
            error.message().contains("model wire v8"),
            "v{version} must be refused on meaning, not only on its schema string: {}",
            error.message()
        );
    }
}

#[test]
fn current_cylinder_resource_round_trips_with_a_new_artifact_identity() {
    let frozen_bytes = frozen(CYLINDER_JSON);
    assert_eq!(frozen_bytes.len(), CYLINDER_BYTES);
    assert_eq!(raw_sha256(frozen_bytes), CYLINDER_RAW_SHA256);

    let decoded = ModelEnvelopeV8::from_json(frozen_bytes, ModelDecoderLimits::default()).unwrap();
    assert_eq!(decoded.canonical_json().unwrap(), frozen_bytes);
    assert_eq!(decoded.digest().unwrap().as_str(), CYLINDER_DIGEST);
    assert_ne!(
        decoded.digest().unwrap().as_str(),
        CYLINDER_V7_DIGEST,
        "the schema-domain artifact identity must change with the epoch"
    );

    // Semantic Model identity and provenance revision survive the epoch change.
    assert_eq!(
        decoded.model().unwrap().ulid().to_string(),
        CYLINDER_MODEL_ULID
    );
    assert_eq!(decoded.source_revision(), CYLINDER_SOURCE_REVISION);

    decoded
        .artifact_reference()
        .unwrap()
        .validate_artifact(&decoded)
        .unwrap();

    // Semantic content equals the historical resource's. The old resource is
    // read as data and relabelled; no historical decoder is reintroduced.
    //
    // Replay stops at the typed reconstruction on purpose: this Model's Domains
    // are geometry-bound, so whole-model admission needs the geometry bundle and
    // belongs to the geometry-admitting layer. The registered cylinder cases own
    // that claim and are untouched by this slice.
    let relabelled = replace_once(
        frozen(CYLINDER_V7_JSON),
        &historical_schema("model", 7),
        MODEL_SCHEMA,
    );
    let historical_content =
        ModelEnvelopeV8::from_json(&relabelled, ModelDecoderLimits::default()).unwrap();
    assert_eq!(historical_content.canonical_json().unwrap(), frozen_bytes);

    let (replayed, replayed_model) = decoded.to_transaction().unwrap();
    let (historical_replay, historical_model) = historical_content.to_transaction().unwrap();
    assert_eq!(replayed.ops(), historical_replay.ops());
    assert_eq!(replayed.preconditions(), historical_replay.preconditions());
    assert_eq!(replayed_model, historical_model);
    assert_eq!(replayed_model.ulid().to_string(), CYLINDER_MODEL_ULID);
}

#[test]
fn the_frozen_historical_corpus_is_labelled_and_complete() {
    // A silently shrunken corpus would weaken every rejection claim above.
    assert_eq!(HISTORICAL_MODELS.len(), 7);
    assert_eq!(HISTORICAL_TRANSACTIONS.len(), 7);
    for (version, specimen) in HISTORICAL_MODELS {
        let wire: Value = serde_json::from_slice(frozen(specimen)).unwrap();
        assert_eq!(wire["schema"], format!("eqiora.model-envelope/v{version}"));
    }
    for (version, specimen) in HISTORICAL_TRANSACTIONS {
        let wire: Value = serde_json::from_slice(frozen(specimen)).unwrap();
        assert_eq!(
            wire["schema"],
            format!("eqiora.model-transaction-envelope/v{version}")
        );
    }
}
