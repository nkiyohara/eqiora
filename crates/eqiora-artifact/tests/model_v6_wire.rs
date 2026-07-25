use eqiora_artifact::{
    CanonicalModelArtifact, ModelEnvelopeV1, ModelEnvelopeV2, ModelEnvelopeV3, ModelEnvelopeV4,
    ModelEnvelopeV5, ModelEnvelopeV6, ModelTransactionEnvelopeV1, ModelTransactionEnvelopeV2,
    ModelTransactionEnvelopeV3, ModelTransactionEnvelopeV4, ModelTransactionEnvelopeV5,
    ModelTransactionEnvelopeV6, ReplayableCanonicalModelArtifact,
};
use eqiora_core::entity::kinds;
use eqiora_core::{DimExponents, DynQuantity, Entity, Id, OntologyId, ValueShape};
use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora_schema::kernel::{
    ActivationDef, AxisBounds, BoundaryPairing, BoundaryPhysicalConnector, BoundarySide,
    ConnectionDef, ConnectionSemantics, DomainDef, ExprDagBuilder, FieldDef, KernelNode, PortDef,
    RelationDef, SymbolRef, ValueFrame,
};
use eqiora_schema::{Model, ModelView};
use eqiora_sem::KernelProgram;
use serde_json::Value;
use ulid::Ulid;

const FIXED_TIMESTAMP_MILLIS: u64 = 1_700_000_006_000;
// Fixed ULIDs make these writer outputs stable. As in the legacy v2/v3
// goldens, the exact byte count fixes framing while the digest commits every
// canonical byte without copying the production JSON structure into the test.
const MODEL_V6_BYTES: usize = 4_966;
const MODEL_V6_DIGEST: &str = "e7ac4d5b5140b91accf67fd06321d512fb3a17aa5c7712f00aa3476a495b0d2c";
const TRANSACTION_V6_BYTES: usize = 6_210;
const TRANSACTION_V6_DIGEST: &str =
    "0480b8dde44523f0433bf05904b78368b70d80a5d3c0ba1c245790323a71df9d";

fn fixed<E: Entity>(random: u128) -> Id<E> {
    Id::from_ulid(Ulid::from_parts(FIXED_TIMESTAMP_MILLIS, random))
}

struct Fixture {
    program: KernelProgram,
    transaction: Transaction,
}

fn fixture() -> Fixture {
    let body = fixed::<kinds::Domain>(1);
    let lower = fixed::<kinds::Domain>(2);
    let upper = fixed::<kinds::Domain>(3);
    let connector = fixed::<kinds::Domain>(4);
    let lower_port = fixed::<kinds::Port>(5);
    let upper_port = fixed::<kinds::Port>(6);
    let lower_relation = fixed::<kinds::Relation>(7);
    let upper_relation = fixed::<kinds::Relation>(8);
    let lower_activation = fixed::<kinds::Activation>(9);
    let upper_activation = fixed::<kinds::Activation>(10);
    let connection = fixed::<kinds::Connection>(11);
    let model = OntologyId::<Model>::from_ulid(Ulid::from_parts(FIXED_TIMESTAMP_MILLIS, 12));
    let length = DimExponents {
        length: 1,
        ..DimExponents::DIMENSIONLESS
    };
    let bounds = [
        AxisBounds::new(DynQuantity::new(0.0, length), DynQuantity::new(2.0, length)).unwrap(),
        AxisBounds::new(
            DynQuantity::new(-1.0, length),
            DynQuantity::new(1.0, length),
        )
        .unwrap(),
    ];
    let connector_contract = BoundaryPhysicalConnector::new(
        DimExponents::DIMENSIONLESS,
        DimExponents::DIMENSIONLESS,
        ValueShape::scalar(),
        ValueFrame::Invariant,
        BoundaryPairing::EuclideanBoundaryDuality,
    )
    .unwrap();
    let carrier = |port| {
        let mut expression = ExprDagBuilder::new();
        let trace = expression.symbol(SymbolRef::PortTrace(port)).unwrap();
        let flux = expression.symbol(SymbolRef::PortFlux(port)).unwrap();
        let trace_zero = expression.sub(trace, trace).unwrap();
        let flux_zero = expression.sub(flux, flux).unwrap();
        expression.finish([trace_zero, flux_zero]).unwrap()
    };
    let nodes = vec![
        KernelNode::from(DomainDef::cartesian_box(body, bounds.to_vec()).unwrap()),
        KernelNode::from(DomainDef::cartesian_boundary(lower, 0, BoundarySide::Lower)),
        KernelNode::from(DomainDef::cartesian_boundary(upper, 0, BoundarySide::Upper)),
        KernelNode::from(DomainDef::boundary_physical(connector, connector_contract)),
        KernelNode::from(PortDef::boundary_physical(lower_port, connector, lower)),
        KernelNode::from(PortDef::boundary_physical(upper_port, connector, upper)),
        KernelNode::from(RelationDef::new(lower_relation, carrier(lower_port))),
        KernelNode::from(RelationDef::new(upper_relation, carrier(upper_port))),
        KernelNode::from(ActivationDef::continuous(lower_activation)),
        KernelNode::from(ActivationDef::continuous(upper_activation)),
        KernelNode::from(ConnectionDef::new(
            connection,
            ConnectionSemantics::SpatialPeriodic,
        )),
    ];
    let mut transaction = Transaction::new("spatial-periodic model v6 fixture");
    for node in &nodes {
        transaction.push(Op::DefineKernelNode { node: node.clone() });
    }
    for boundary in [lower, upper] {
        transaction.push(Op::Connect {
            from: boundary.erase(),
            to: body.erase(),
            edge: EdgeKind::BoundaryOf,
        });
    }
    for (relation, port, boundary, activation) in [
        (lower_relation, lower_port, lower, lower_activation),
        (upper_relation, upper_port, upper, upper_activation),
    ] {
        transaction
            .push(Op::Connect {
                from: relation.erase(),
                to: port.erase(),
                edge: EdgeKind::HasPort,
            })
            .push(Op::Connect {
                from: relation.erase(),
                to: port.erase(),
                edge: EdgeKind::DependsOn,
            })
            .push(Op::Connect {
                from: relation.erase(),
                to: boundary.erase(),
                edge: EdgeKind::AppliesOn,
            })
            .push(Op::Connect {
                from: activation.erase(),
                to: relation.erase(),
                edge: EdgeKind::Activates,
            })
            .push(Op::Connect {
                from: connection.erase(),
                to: port.erase(),
                edge: EdgeKind::Connects,
            });
    }
    transaction.push(Op::DefineOntologyView {
        view: ModelView::new(model, nodes.iter().map(KernelNode::id), [])
            .unwrap()
            .into(),
    });

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

fn connection_transaction(semantics: ConnectionSemantics) -> Transaction {
    let mut transaction = Transaction::new("one spatial Connection");
    transaction.push(Op::DefineKernelNode {
        node: ConnectionDef::new(Id::new(), semantics).into(),
    });
    transaction
}

fn nonperiodic_fixture() -> Fixture {
    let field = fixed::<kinds::Field>(101);
    let relation = fixed::<kinds::Relation>(102);
    let activation = fixed::<kinds::Activation>(103);
    let model = OntologyId::<Model>::from_ulid(Ulid::from_parts(FIXED_TIMESTAMP_MILLIS, 104));
    let mut expression = ExprDagBuilder::new();
    let value = expression.symbol(SymbolRef::Field(field)).unwrap();
    let residual = expression.sub(value, value).unwrap();
    let nodes = [
        KernelNode::from(FieldDef::new(field, DimExponents::DIMENSIONLESS)),
        KernelNode::from(RelationDef::new(
            relation,
            expression.finish([residual]).unwrap(),
        )),
        KernelNode::from(ActivationDef::continuous(activation)),
    ];
    let mut transaction = Transaction::new("nonperiodic v5-v6 identity fixture");
    for node in &nodes {
        transaction.push(Op::DefineKernelNode { node: node.clone() });
    }
    transaction
        .push(Op::Connect {
            from: relation.erase(),
            to: field.erase(),
            edge: EdgeKind::DependsOn,
        })
        .push(Op::Connect {
            from: activation.erase(),
            to: relation.erase(),
            edge: EdgeKind::Activates,
        })
        .push(Op::DefineOntologyView {
            view: ModelView::new(model, nodes.iter().map(KernelNode::id), [])
                .unwrap()
                .into(),
        });

    let mut retained = Transaction::new(transaction.label());
    for op in transaction.ops() {
        retained.push(op.clone());
    }
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    Fixture {
        program: KernelProgram::from_snapshot(&store.snapshot(), model).unwrap(),
        transaction: retained,
    }
}

#[test]
fn v6_round_trips_one_valid_spatial_periodic_model() {
    let fixture = fixture();
    assert!(ModelEnvelopeV1::from_program(&fixture.program).is_err());
    assert!(ModelEnvelopeV2::from_program(&fixture.program).is_err());
    assert!(ModelEnvelopeV3::from_program(&fixture.program).is_err());
    assert!(ModelEnvelopeV4::from_program(&fixture.program).is_err());
    assert!(ModelEnvelopeV5::from_program(&fixture.program).is_err());

    let envelope = ModelEnvelopeV6::from_program(&fixture.program).unwrap();
    let bytes = envelope.canonical_json().unwrap();
    assert_eq!(bytes.len(), MODEL_V6_BYTES);
    assert_eq!(envelope.digest().unwrap().as_str(), MODEL_V6_DIGEST);
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("eqiora.model-envelope/v6"));
    assert!(text.contains("spatial-periodic"));

    let decoded = ModelEnvelopeV6::from_json(&bytes, Default::default()).unwrap();
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
    ] {
        assert!(rejected);
    }
    let mut forged_v5: Value = serde_json::from_slice(&bytes).unwrap();
    forged_v5["schema"] = Value::String("eqiora.model-envelope/v5".to_owned());
    let error =
        ModelEnvelopeV5::from_json(&serde_json::to_vec(&forged_v5).unwrap(), Default::default())
            .unwrap_err();
    assert!(error.message().contains("require model wire v6"));
}

#[test]
fn v6_model_canonicalizes_set_order_before_computing_identity() {
    let fixture = fixture();
    let envelope = ModelEnvelopeV6::from_program(&fixture.program).unwrap();
    let canonical = envelope.canonical_json().unwrap();
    let mut permuted: Value = serde_json::from_slice(&canonical).unwrap();
    permuted["nodes"].as_array_mut().unwrap().reverse();
    permuted["edges"].as_array_mut().unwrap().reverse();

    let decoded =
        ModelEnvelopeV6::from_json(&serde_json::to_vec(&permuted).unwrap(), Default::default())
            .unwrap();
    assert_eq!(decoded.canonical_json().unwrap(), canonical);
    assert_eq!(decoded.digest().unwrap(), envelope.digest().unwrap());
}

#[test]
fn transaction_v6_preserves_order_and_rejects_version_fallback() {
    let fixture = fixture();
    for rejected in [
        ModelTransactionEnvelopeV1::from_transaction(&fixture.transaction).is_err(),
        ModelTransactionEnvelopeV2::from_transaction(&fixture.transaction).is_err(),
        ModelTransactionEnvelopeV3::from_transaction(&fixture.transaction).is_err(),
        ModelTransactionEnvelopeV4::from_transaction(&fixture.transaction).is_err(),
        ModelTransactionEnvelopeV5::from_transaction(&fixture.transaction).is_err(),
    ] {
        assert!(rejected);
    }
    let spatial_only = connection_transaction(ConnectionSemantics::SpatialPeriodic);
    for rejected in [
        ModelTransactionEnvelopeV1::from_transaction(&spatial_only).is_err(),
        ModelTransactionEnvelopeV2::from_transaction(&spatial_only).is_err(),
        ModelTransactionEnvelopeV3::from_transaction(&spatial_only).is_err(),
        ModelTransactionEnvelopeV4::from_transaction(&spatial_only).is_err(),
        ModelTransactionEnvelopeV5::from_transaction(&spatial_only).is_err(),
    ] {
        assert!(rejected);
    }

    let envelope = ModelTransactionEnvelopeV6::from_transaction(&fixture.transaction).unwrap();
    let bytes = envelope.canonical_json().unwrap();
    assert_eq!(bytes.len(), TRANSACTION_V6_BYTES);
    assert_eq!(envelope.digest().unwrap().as_str(), TRANSACTION_V6_DIGEST);
    let decoded = ModelTransactionEnvelopeV6::from_json(&bytes, Default::default()).unwrap();
    assert_eq!(decoded.canonical_json().unwrap(), bytes);
    assert_eq!(decoded.digest().unwrap(), envelope.digest().unwrap());
    assert_eq!(
        decoded.to_transaction().unwrap().ops(),
        fixture.transaction.ops()
    );

    for rejected in [
        ModelTransactionEnvelopeV1::from_json(&bytes, Default::default()).is_err(),
        ModelTransactionEnvelopeV2::from_json(&bytes, Default::default()).is_err(),
        ModelTransactionEnvelopeV3::from_json(&bytes, Default::default()).is_err(),
        ModelTransactionEnvelopeV4::from_json(&bytes, Default::default()).is_err(),
        ModelTransactionEnvelopeV5::from_json(&bytes, Default::default()).is_err(),
    ] {
        assert!(rejected);
    }
    let mut forged_v5: Value = serde_json::from_slice(&bytes).unwrap();
    forged_v5["schema"] = Value::String("eqiora.model-transaction-envelope/v5".to_owned());
    let error = ModelTransactionEnvelopeV5::from_json(
        &serde_json::to_vec(&forged_v5).unwrap(),
        Default::default(),
    )
    .unwrap_err();
    assert!(error.message().contains("require model wire v6"));
}

#[test]
fn transaction_v6_digest_includes_semantics_and_operation_order() {
    let periodic = connection_transaction(ConnectionSemantics::SpatialPeriodic);
    let conserving = connection_transaction(ConnectionSemantics::Conserving);
    let periodic = ModelTransactionEnvelopeV6::from_transaction(&periodic).unwrap();
    let conserving = ModelTransactionEnvelopeV6::from_transaction(&conserving).unwrap();
    assert_ne!(periodic.digest().unwrap(), conserving.digest().unwrap());

    let first = ConnectionDef::new(Id::new(), ConnectionSemantics::SpatialPeriodic);
    let second = ConnectionDef::new(Id::new(), ConnectionSemantics::Conserving);
    let ordered = |nodes: [&ConnectionDef; 2]| {
        let mut transaction = Transaction::new("ordered Connections");
        for node in nodes {
            transaction.push(Op::DefineKernelNode {
                node: node.clone().into(),
            });
        }
        ModelTransactionEnvelopeV6::from_transaction(&transaction).unwrap()
    };
    let forward = ordered([&first, &second]);
    let reverse = ordered([&second, &first]);
    assert_ne!(
        forward.canonical_json().unwrap(),
        reverse.canonical_json().unwrap()
    );
    assert_ne!(forward.digest().unwrap(), reverse.digest().unwrap());
}

#[test]
fn v5_and_v6_identities_are_domain_separated_for_the_same_nonperiodic_meaning() {
    let fixture = nonperiodic_fixture();
    let model_v5 = ModelEnvelopeV5::from_program(&fixture.program).unwrap();
    let model_v6 = ModelEnvelopeV6::from_program(&fixture.program).unwrap();
    assert_ne!(model_v5.digest().unwrap(), model_v6.digest().unwrap());
    assert_eq!(
        ModelEnvelopeV5::from_json(&model_v5.canonical_json().unwrap(), Default::default())
            .unwrap()
            .to_program()
            .unwrap(),
        ModelEnvelopeV6::from_json(&model_v6.canonical_json().unwrap(), Default::default())
            .unwrap()
            .to_program()
            .unwrap()
    );

    let transaction_v5 =
        ModelTransactionEnvelopeV5::from_transaction(&fixture.transaction).unwrap();
    let transaction_v6 =
        ModelTransactionEnvelopeV6::from_transaction(&fixture.transaction).unwrap();
    assert_ne!(
        transaction_v5.digest().unwrap(),
        transaction_v6.digest().unwrap()
    );
    let decoded_v5 = transaction_v5.to_transaction().unwrap();
    let decoded_v6 = transaction_v6.to_transaction().unwrap();
    assert_eq!(decoded_v5.label(), decoded_v6.label());
    assert_eq!(decoded_v5.ops(), decoded_v6.ops());
    assert_eq!(decoded_v5.preconditions(), decoded_v6.preconditions());
}
