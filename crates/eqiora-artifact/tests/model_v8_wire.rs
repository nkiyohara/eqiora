use eqiora_artifact::{
    CanonicalModelArtifact, ModelEnvelopeV1, ModelEnvelopeV2, ModelEnvelopeV3, ModelEnvelopeV4,
    ModelEnvelopeV5, ModelEnvelopeV6, ModelEnvelopeV7, ModelEnvelopeV8, ModelTransactionEnvelopeV1,
    ModelTransactionEnvelopeV2, ModelTransactionEnvelopeV3, ModelTransactionEnvelopeV4,
    ModelTransactionEnvelopeV5, ModelTransactionEnvelopeV6, ModelTransactionEnvelopeV7,
    ModelTransactionEnvelopeV8, ReplayableCanonicalModelArtifact,
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
use ulid::Ulid;

const FIXED_TIMESTAMP_MILLIS: u64 = 1_700_000_008_000;
// Fixed ULIDs and one fixed timestamp make this writer output stable. As in the
// v6 golden, the exact byte count fixes framing while the digest commits every
// canonical byte without copying the production JSON structure into the test.
const MODEL_V8_BYTES: usize = 2_347;
const MODEL_V8_DIGEST: &str = "e410295337a3a51a271f272e03ae7d7a4b8e7df1b04faf76645bb1e18567e4b3";
const TRANSACTION_V8_BYTES: usize = 2_646;
const TRANSACTION_V8_DIGEST: &str =
    "132168803ac8882f0f35187215d3f2ce44817d03921d6ad95b73a9cac62aa102";

fn fixed<E: Entity>(random: u128) -> Id<E> {
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
    parameter: Id<kinds::Parameter>,
    body: Id<kinds::Domain>,
}

/// One root length Parameter driving two endpoints of one 3D Cartesian Domain,
/// recorded by exactly one `Domain --DependsOn--> Parameter` edge.
fn fixture() -> Fixture {
    let parameter = fixed::<kinds::Parameter>(1);
    let body = fixed::<kinds::Domain>(2);
    let retain = fixed::<kinds::Relation>(3);
    let activation = fixed::<kinds::Activation>(4);
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
        parameter,
        body,
    }
}

fn ulid_of<E: Entity>(id: Id<E>) -> String {
    id.ulid().to_string()
}

fn coordinate_endpoints(wire: &Value) -> Vec<&Value> {
    wire["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|node| node["definition"]["domain"]["coordinates"].as_array())
        .flatten()
        .flat_map(|axis| [&axis["lower"], &axis["upper"]])
        .collect()
}

fn parameter_driven_endpoints(wire: &Value, parameter: Id<kinds::Parameter>) -> usize {
    let ulid = ulid_of(parameter);
    coordinate_endpoints(wire)
        .into_iter()
        .filter(|endpoint| {
            endpoint["source"] == "parameter"
                && endpoint["parameter"]["kind"] == "parameter"
                && endpoint["parameter"]["ulid"] == ulid.as_str()
        })
        .count()
}

fn wire_dependency_edges(wire: &Value) -> usize {
    wire["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|edge| edge["kind"] == "depends-on")
        .count()
}

fn graph_dependency_edges(
    program: &KernelProgram,
    body: Id<kinds::Domain>,
    parameter: Id<kinds::Parameter>,
) -> usize {
    program
        .edges()
        .iter()
        .filter(|edge| {
            edge.from() == body.erase()
                && edge.to() == parameter.erase()
                && edge.kind() == EdgeKind::DependsOn
        })
        .count()
}

fn reordered(transaction: &Transaction) -> Transaction {
    let mut reversed = Transaction::new(transaction.label());
    for op in transaction.ops().iter().rev() {
        reversed.push(op.clone());
    }
    reversed
}

#[test]
fn v8_round_trips_one_parameter_driven_cartesian_model() {
    let fixture = fixture();
    assert_eq!(
        graph_dependency_edges(&fixture.program, fixture.body, fixture.parameter),
        1
    );
    for rejected in [
        ModelEnvelopeV1::from_program(&fixture.program).is_err(),
        ModelEnvelopeV2::from_program(&fixture.program).is_err(),
        ModelEnvelopeV3::from_program(&fixture.program).is_err(),
        ModelEnvelopeV4::from_program(&fixture.program).is_err(),
        ModelEnvelopeV5::from_program(&fixture.program).is_err(),
        ModelEnvelopeV6::from_program(&fixture.program).is_err(),
        ModelEnvelopeV7::from_program(&fixture.program).is_err(),
    ] {
        assert!(rejected);
    }

    let envelope = ModelEnvelopeV8::from_program(&fixture.program).unwrap();
    let bytes = envelope.canonical_json().unwrap();
    assert_eq!(bytes.len(), MODEL_V8_BYTES);
    assert_eq!(envelope.digest().unwrap().as_str(), MODEL_V8_DIGEST);

    // The persisted meaning is the coordinate recipe plus its dependency, not
    // evaluated bounds: one Parameter is visible at exactly two endpoints and
    // is recorded by exactly one dependency edge.
    let wire: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(wire["schema"], "eqiora.model-envelope/v8");
    assert_eq!(coordinate_endpoints(&wire).len(), 6);
    assert_eq!(parameter_driven_endpoints(&wire, fixture.parameter), 2);
    assert_eq!(wire_dependency_edges(&wire), 1);
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("eqiora.model-envelope/v8"));
    assert!(text.contains("cartesian-box-sources"));

    let decoded = ModelEnvelopeV8::from_json(&bytes, Default::default()).unwrap();
    assert_eq!(decoded.canonical_json().unwrap(), bytes);
    assert_eq!(decoded.digest().unwrap(), envelope.digest().unwrap());
    assert_eq!(decoded.to_program().unwrap(), fixture.program);
    let reference = decoded.artifact_reference().unwrap();
    reference.validate_artifact(&decoded).unwrap();
    assert_eq!(decoded.replay_model().unwrap().program(), &fixture.program);

    for rejected in [
        ModelEnvelopeV1::from_json(&bytes, Default::default()).is_err(),
        ModelEnvelopeV2::from_json(&bytes, Default::default()).is_err(),
        ModelEnvelopeV3::from_json(&bytes, Default::default()).is_err(),
        ModelEnvelopeV4::from_json(&bytes, Default::default()).is_err(),
        ModelEnvelopeV5::from_json(&bytes, Default::default()).is_err(),
        ModelEnvelopeV6::from_json(&bytes, Default::default()).is_err(),
        ModelEnvelopeV7::from_json(&bytes, Default::default()).is_err(),
    ] {
        assert!(rejected);
    }
    let mut forged_v7 = wire.clone();
    forged_v7["schema"] = Value::String("eqiora.model-envelope/v7".to_owned());
    let error =
        ModelEnvelopeV7::from_json(&serde_json::to_vec(&forged_v7).unwrap(), Default::default())
            .unwrap_err();
    assert!(
        error
            .message()
            .contains("Cartesian coordinate sources require model wire v8"),
        "{}",
        error.message()
    );
}

#[test]
fn v8_model_canonicalizes_input_permutations_but_not_coordinate_order() {
    let fixture = fixture();
    let envelope = ModelEnvelopeV8::from_program(&fixture.program).unwrap();
    let canonical = envelope.canonical_json().unwrap();
    let mut permuted: Value = serde_json::from_slice(&canonical).unwrap();
    permuted["nodes"].as_array_mut().unwrap().reverse();
    permuted["edges"].as_array_mut().unwrap().reverse();
    permuted["values"].as_array_mut().unwrap().reverse();

    let decoded =
        ModelEnvelopeV8::from_json(&serde_json::to_vec(&permuted).unwrap(), Default::default())
            .unwrap();
    assert_eq!(decoded.canonical_json().unwrap(), canonical);
    assert_eq!(decoded.digest().unwrap(), envelope.digest().unwrap());

    // Axis order and the lower/upper roles are semantic, so no set-order
    // canonicalization may absorb a permutation of the coordinate list.
    let mut reordered_axes: Value = serde_json::from_slice(&canonical).unwrap();
    let coordinates = reordered_axes["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find_map(|node| node["definition"]["domain"]["coordinates"].as_array_mut())
        .unwrap();
    coordinates.reverse();
    let decoded = ModelEnvelopeV8::from_json(
        &serde_json::to_vec(&reordered_axes).unwrap(),
        Default::default(),
    )
    .unwrap();
    assert_ne!(decoded.canonical_json().unwrap(), canonical);
    assert_ne!(decoded.digest().unwrap(), envelope.digest().unwrap());
}

#[test]
fn transaction_v8_preserves_order_and_rejects_version_fallback() {
    let fixture = fixture();
    for rejected in [
        ModelTransactionEnvelopeV1::from_transaction(&fixture.transaction).is_err(),
        ModelTransactionEnvelopeV2::from_transaction(&fixture.transaction).is_err(),
        ModelTransactionEnvelopeV3::from_transaction(&fixture.transaction).is_err(),
        ModelTransactionEnvelopeV4::from_transaction(&fixture.transaction).is_err(),
        ModelTransactionEnvelopeV5::from_transaction(&fixture.transaction).is_err(),
        ModelTransactionEnvelopeV6::from_transaction(&fixture.transaction).is_err(),
        ModelTransactionEnvelopeV7::from_transaction(&fixture.transaction).is_err(),
    ] {
        assert!(rejected);
    }

    let envelope = ModelTransactionEnvelopeV8::from_transaction(&fixture.transaction).unwrap();
    let bytes = envelope.canonical_json().unwrap();
    assert_eq!(bytes.len(), TRANSACTION_V8_BYTES);
    assert_eq!(envelope.digest().unwrap().as_str(), TRANSACTION_V8_DIGEST);
    let wire: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(wire["schema"], "eqiora.model-transaction-envelope/v8");
    assert_eq!(
        wire["ops"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|op| op["op"] == "connect" && op["edge"] == "depends-on")
            .count(),
        1
    );

    let decoded = ModelTransactionEnvelopeV8::from_json(&bytes, Default::default()).unwrap();
    assert_eq!(decoded.canonical_json().unwrap(), bytes);
    assert_eq!(decoded.digest().unwrap(), envelope.digest().unwrap());
    let replayed = decoded.to_transaction().unwrap();
    assert_eq!(replayed.label(), fixture.transaction.label());
    assert_eq!(
        replayed.preconditions(),
        fixture.transaction.preconditions()
    );
    assert_eq!(replayed.ops(), fixture.transaction.ops());

    for rejected in [
        ModelTransactionEnvelopeV1::from_json(&bytes, Default::default()).is_err(),
        ModelTransactionEnvelopeV2::from_json(&bytes, Default::default()).is_err(),
        ModelTransactionEnvelopeV3::from_json(&bytes, Default::default()).is_err(),
        ModelTransactionEnvelopeV4::from_json(&bytes, Default::default()).is_err(),
        ModelTransactionEnvelopeV5::from_json(&bytes, Default::default()).is_err(),
        ModelTransactionEnvelopeV6::from_json(&bytes, Default::default()).is_err(),
        ModelTransactionEnvelopeV7::from_json(&bytes, Default::default()).is_err(),
    ] {
        assert!(rejected);
    }
    let mut forged_v7 = wire;
    forged_v7["schema"] = Value::String("eqiora.model-transaction-envelope/v7".to_owned());
    let error = ModelTransactionEnvelopeV7::from_json(
        &serde_json::to_vec(&forged_v7).unwrap(),
        Default::default(),
    )
    .unwrap_err();
    assert!(
        error
            .message()
            .contains("Cartesian coordinate sources require model wire v8"),
        "{}",
        error.message()
    );
}

#[test]
fn transaction_v8_operation_reversal_produces_a_distinct_artifact() {
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
