use eqiora_artifact::{
    DecoderLimits, ModelEnvelopeV1, ModelEnvelopeV2, ModelTransactionEnvelopeV1,
    ModelTransactionEnvelopeV2,
};
use eqiora_compiler::compile;
use eqiora_core::entity::kinds;
use eqiora_core::{DimExponents, DynQuantity, Id, OntologyId};
use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora_schema::kernel::{
    ActivationDef, ConnectionDef, ConnectionSemantics, DomainDef, ExprDagBuilder, KernelNode,
    ParameterDef, PortDef, RelationDef, SymbolRef,
};
use eqiora_schema::{Model, ModelView};
use eqiora_sem::KernelProgram;

const POISSON: &str = include_str!("../../../verify/numerics/poisson-fem-fvm/models/poisson.eqi");

#[derive(Clone, Copy)]
struct PhysicalIds {
    domain: Id<kinds::Domain>,
    parameter: Id<kinds::Parameter>,
    connection: Id<kinds::Connection>,
    activation: Id<kinds::Activation>,
    ports: [Id<kinds::Port>; 2],
    relations: [Id<kinds::Relation>; 2],
    model: OntologyId<Model>,
}

fn ids() -> PhysicalIds {
    PhysicalIds {
        domain: Id::new(),
        parameter: Id::new(),
        connection: Id::new(),
        activation: Id::new(),
        ports: [Id::new(), Id::new()],
        relations: [Id::new(), Id::new()],
        model: OntologyId::new(),
    }
}

fn dimensions() -> (DimExponents, DimExponents) {
    (
        DimExponents {
            mass: 1,
            length: 2,
            time: -3,
            current: -1,
            ..DimExponents::DIMENSIONLESS
        },
        DimExponents {
            current: 1,
            ..DimExponents::DIMENSIONLESS
        },
    )
}

fn physical_transaction(ids: PhysicalIds, reversed: bool) -> Transaction {
    let (across_dimension, through_dimension) = dimensions();
    let mut first = ExprDagBuilder::new();
    let across = first.symbol(SymbolRef::Across(ids.ports[0])).unwrap();
    let parameter = first.symbol(SymbolRef::Parameter(ids.parameter)).unwrap();
    let first_root = first.sub(across, parameter).unwrap();
    let mut second = ExprDagBuilder::new();
    let through = second.symbol(SymbolRef::Through(ids.ports[1])).unwrap();

    let mut nodes = vec![
        DomainDef::scalar_physical(ids.domain, across_dimension, through_dimension).into(),
        ParameterDef::new(ids.parameter, DynQuantity::new(12.0, across_dimension)).into(),
        PortDef::scalar_physical(ids.ports[0], ids.domain).into(),
        PortDef::scalar_physical(ids.ports[1], ids.domain).into(),
        RelationDef::new(ids.relations[0], first.finish([first_root]).unwrap()).into(),
        RelationDef::new(ids.relations[1], second.finish([through]).unwrap()).into(),
        ActivationDef::continuous(ids.activation).into(),
        ConnectionDef::new(ids.connection, ConnectionSemantics::Conserving).into(),
    ];
    if reversed {
        nodes.reverse();
    }
    let view = ModelView::new(ids.model, nodes.iter().map(KernelNode::id), []).unwrap();
    let mut transaction = Transaction::new("physical v2 fixture");
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
    let mut edges = vec![
        (
            ids.relations[0].erase(),
            ids.ports[0].erase(),
            EdgeKind::HasPort,
        ),
        (
            ids.relations[0].erase(),
            ids.ports[0].erase(),
            EdgeKind::DependsOn,
        ),
        (
            ids.relations[0].erase(),
            ids.parameter.erase(),
            EdgeKind::DependsOn,
        ),
        (
            ids.relations[1].erase(),
            ids.ports[1].erase(),
            EdgeKind::HasPort,
        ),
        (
            ids.relations[1].erase(),
            ids.ports[1].erase(),
            EdgeKind::DependsOn,
        ),
        (
            ids.activation.erase(),
            ids.relations[0].erase(),
            EdgeKind::Activates,
        ),
        (
            ids.activation.erase(),
            ids.relations[1].erase(),
            EdgeKind::Activates,
        ),
        (
            ids.connection.erase(),
            ids.ports[0].erase(),
            EdgeKind::Connects,
        ),
        (
            ids.connection.erase(),
            ids.ports[1].erase(),
            EdgeKind::Connects,
        ),
    ];
    if reversed {
        edges.reverse();
    }
    for (from, to, edge) in edges {
        transaction.push(Op::Connect { from, to, edge });
    }
    transaction.push(Op::DefineOntologyView { view: view.into() });
    transaction
}

fn physical_program(ids: PhysicalIds, reversed: bool) -> KernelProgram {
    let mut store = InMemoryGraphStore::new();
    store.commit(physical_transaction(ids, reversed)).unwrap();
    KernelProgram::from_snapshot(&store.snapshot(), ids.model).unwrap()
}

#[test]
fn physical_model_and_transaction_round_trip_only_through_explicit_v2() {
    let ids = ids();
    let transaction = physical_transaction(ids, false);
    assert!(ModelTransactionEnvelopeV1::from_transaction(&transaction).is_err());

    let transaction_v2 = ModelTransactionEnvelopeV2::from_transaction(&transaction).unwrap();
    let transaction_bytes = transaction_v2.canonical_json().unwrap();
    let decoded_transaction =
        ModelTransactionEnvelopeV2::from_json(&transaction_bytes, DecoderLimits::default())
            .unwrap();
    assert_eq!(
        decoded_transaction.canonical_json().unwrap(),
        transaction_bytes
    );
    assert_eq!(
        decoded_transaction.to_transaction().unwrap().ops(),
        transaction.ops()
    );

    let program = physical_program(ids, false);
    assert!(ModelEnvelopeV1::from_program(&program).is_err());
    let model_v2 = ModelEnvelopeV2::from_program(&program).unwrap();
    let bytes = model_v2.canonical_json().unwrap();
    let digest = model_v2.digest().unwrap();
    let decoded = ModelEnvelopeV2::from_json(&bytes, DecoderLimits::default()).unwrap();
    let round_trip_program = decoded.to_program().unwrap();
    let round_trip = ModelEnvelopeV2::from_program(&round_trip_program).unwrap();
    assert_eq!(round_trip.canonical_json().unwrap(), bytes);
    assert_eq!(round_trip.digest().unwrap(), digest);
    assert_eq!(
        round_trip_program
            .compose_scalar_physical_subsystem(ids.connection)
            .unwrap(),
        program
            .compose_scalar_physical_subsystem(ids.connection)
            .unwrap()
    );

    assert!(ModelEnvelopeV1::from_json(&bytes, DecoderLimits::default()).is_err());
    assert!(
        ModelTransactionEnvelopeV1::from_json(&transaction_bytes, DecoderLimits::default())
            .is_err()
    );

    let mut forged_model_v1: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    forged_model_v1["schema"] = serde_json::Value::String("eqiora.model-envelope/v1".to_owned());
    assert!(
        ModelEnvelopeV1::from_json(
            &serde_json::to_vec(&forged_model_v1).unwrap(),
            DecoderLimits::default(),
        )
        .is_err()
    );
    let mut forged_transaction_v1: serde_json::Value =
        serde_json::from_slice(&transaction_bytes).unwrap();
    forged_transaction_v1["schema"] =
        serde_json::Value::String("eqiora.model-transaction-envelope/v1".to_owned());
    assert!(
        ModelTransactionEnvelopeV1::from_json(
            &serde_json::to_vec(&forged_transaction_v1).unwrap(),
            DecoderLimits::default(),
        )
        .is_err()
    );
}

#[test]
fn model_v2_canonicalizes_graph_insertion_without_hiding_transaction_order() {
    let ids = ids();
    let forward = ModelEnvelopeV2::from_program(&physical_program(ids, false)).unwrap();
    let reversed = ModelEnvelopeV2::from_program(&physical_program(ids, true)).unwrap();
    assert_eq!(
        forward.canonical_json().unwrap(),
        reversed.canonical_json().unwrap()
    );
    assert_eq!(forward.digest().unwrap(), reversed.digest().unwrap());

    let forward_transaction =
        ModelTransactionEnvelopeV2::from_transaction(&physical_transaction(ids, false)).unwrap();
    let reversed_transaction =
        ModelTransactionEnvelopeV2::from_transaction(&physical_transaction(ids, true)).unwrap();
    assert_ne!(
        forward_transaction.canonical_json().unwrap(),
        reversed_transaction.canonical_json().unwrap()
    );
}

#[test]
fn model_v2_rejects_wrong_kind_and_dangling_physical_domain_ids_before_commit() {
    let ids = ids();
    let envelope = ModelEnvelopeV2::from_program(&physical_program(ids, false)).unwrap();
    let mut wrong_kind: serde_json::Value =
        serde_json::from_slice(&envelope.canonical_json().unwrap()).unwrap();
    let port_id = wrong_kind["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["definition"]["kind"] == "scalar-physical-port")
        .unwrap()["id"]
        .clone();
    let physical_port = wrong_kind["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|node| node["definition"]["kind"] == "scalar-physical-port")
        .unwrap();
    physical_port["definition"]["domain"] = port_id;
    assert!(
        ModelEnvelopeV2::from_json(
            &serde_json::to_vec(&wrong_kind).unwrap(),
            DecoderLimits::default(),
        )
        .is_err()
    );

    let mut dangling: serde_json::Value =
        serde_json::from_slice(&envelope.canonical_json().unwrap()).unwrap();
    let physical_port = dangling["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|node| node["definition"]["kind"] == "scalar-physical-port")
        .unwrap();
    physical_port["definition"]["domain"]["ulid"] =
        serde_json::Value::String(Id::<kinds::Domain>::new().ulid().to_string());
    assert!(
        ModelEnvelopeV2::from_json(
            &serde_json::to_vec(&dangling).unwrap(),
            DecoderLimits::default(),
        )
        .is_err()
    );
}

#[test]
fn transaction_v2_admits_local_edits_but_complete_model_admission_rejects_dangling_domain() {
    let ids = ids();
    let envelope =
        ModelTransactionEnvelopeV2::from_transaction(&physical_transaction(ids, false)).unwrap();
    let mut wire: serde_json::Value =
        serde_json::from_slice(&envelope.canonical_json().unwrap()).unwrap();
    let physical_port = wire["ops"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|op| {
            op["op"] == "define-kernel-node"
                && op["node"]["definition"]["kind"] == "scalar-physical-port"
        })
        .unwrap();
    physical_port["node"]["definition"]["domain"]["ulid"] =
        serde_json::Value::String(Id::<kinds::Domain>::new().ulid().to_string());

    let decoded = ModelTransactionEnvelopeV2::from_json(
        &serde_json::to_vec(&wire).unwrap(),
        DecoderLimits::default(),
    )
    .unwrap();
    let mut store = InMemoryGraphStore::new();
    store.commit(decoded.to_transaction().unwrap()).unwrap();

    let diagnostics = KernelProgram::from_snapshot(&store.snapshot(), ids.model).unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("scalar physical Port must name a scalar physical Domain")
    }));
}

#[test]
fn v2_model_and_transaction_enforce_independent_root_member_and_boundary_budgets() {
    let ids = ids();
    let transaction =
        ModelTransactionEnvelopeV2::from_transaction(&physical_transaction(ids, false)).unwrap();
    let transaction_bytes = transaction.canonical_json().unwrap();
    let model = ModelEnvelopeV2::from_program(&physical_program(ids, false)).unwrap();
    let model_bytes = model.canonical_json().unwrap();

    for limits in [
        DecoderLimits {
            max_expression_roots: 0,
            ..DecoderLimits::default()
        },
        DecoderLimits {
            max_model_view_members: 0,
            ..DecoderLimits::default()
        },
    ] {
        assert!(ModelEnvelopeV2::from_json(&model_bytes, limits).is_err());
        assert!(ModelTransactionEnvelopeV2::from_json(&transaction_bytes, limits).is_err());
    }

    let mut model_wire: serde_json::Value = serde_json::from_slice(&model_bytes).unwrap();
    let port = model_wire["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["definition"]["kind"] == "scalar-physical-port")
        .unwrap()["id"]
        .clone();
    model_wire["boundary"] = serde_json::Value::Array(vec![port]);
    assert!(
        ModelEnvelopeV2::from_json(
            &serde_json::to_vec(&model_wire).unwrap(),
            DecoderLimits {
                max_model_boundary: 0,
                ..DecoderLimits::default()
            },
        )
        .is_err()
    );

    let mut transaction_wire: serde_json::Value =
        serde_json::from_slice(&transaction_bytes).unwrap();
    let view = transaction_wire["ops"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|op| op["op"] == "define-model-view")
        .unwrap()["view"]
        .as_object_mut()
        .unwrap();
    let port = view["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|member| member["kind"] == "port")
        .unwrap()
        .clone();
    view.insert("boundary".to_owned(), serde_json::Value::Array(vec![port]));
    assert!(
        ModelTransactionEnvelopeV2::from_json(
            &serde_json::to_vec(&transaction_wire).unwrap(),
            DecoderLimits {
                max_model_boundary: 0,
                ..DecoderLimits::default()
            },
        )
        .is_err()
    );
}

#[test]
fn v2_explicitly_reenvelops_v1_values_under_a_distinct_identity() {
    let mut compiled = compile("poisson.eqi", POISSON).unwrap();
    let compiled = compiled.remove(0);
    let transaction_v1 =
        ModelTransactionEnvelopeV1::from_transaction(compiled.transaction()).unwrap();
    let transaction_v2 =
        ModelTransactionEnvelopeV2::from_transaction(compiled.transaction()).unwrap();
    assert_ne!(
        transaction_v1.digest().unwrap(),
        transaction_v2.digest().unwrap()
    );

    let (transaction, model, _) = compiled.into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    let program = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
    let model_v1 = ModelEnvelopeV1::from_program(&program).unwrap();
    let model_v2 = ModelEnvelopeV2::from_program(&program).unwrap();
    assert_ne!(model_v1.digest().unwrap(), model_v2.digest().unwrap());
    assert!(
        ModelEnvelopeV2::from_json(
            &model_v1.canonical_json().unwrap(),
            DecoderLimits::default()
        )
        .is_err()
    );
}
