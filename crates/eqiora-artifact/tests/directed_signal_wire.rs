use eqiora_artifact::{ModelEnvelope, StructuralSemanticFingerprint};
use eqiora_core::{
    DimExponents, DynQuantity, Id, OntologyId, ScalarDomain, ValueType, entity::kinds,
};
use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora_schema::{
    Model, ModelView,
    kernel::{
        ActivationDef, ConnectionDef, ConnectionSemantics, ExprDagBuilder, KernelNode, PortDef,
        RelationDef, SignalDirection,
    },
};
use eqiora_sem::KernelProgram;

fn program(reverse: bool) -> KernelProgram {
    let ports = [Id::<kinds::Port>::new(), Id::new()];
    let connection = Id::<kinds::Connection>::new();
    let relation = Id::<kinds::Relation>::new();
    let activation = Id::<kinds::Activation>::new();
    let model = OntologyId::<Model>::new();
    let mut dag = ExprDagBuilder::new();
    let zero = dag
        .constant(DynQuantity::new(0.0, DimExponents::DIMENSIONLESS))
        .unwrap();
    let mut nodes = vec![
        KernelNode::from(ConnectionDef::new(
            connection,
            ConnectionSemantics::Signal {
                driver: ports[usize::from(reverse)],
            },
        )),
        RelationDef::new(relation, dag.finish([zero]).unwrap()).into(),
        ActivationDef::continuous(activation).into(),
    ];
    nodes.extend(ports.map(|port| {
        PortDef::signal(
            port,
            SignalDirection::Output,
            ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS),
        )
        .into()
    }));
    let members = nodes.iter().map(KernelNode::id).collect::<Vec<_>>();
    let mut transaction = Transaction::new("directed wire fixture");
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
    for port in ports {
        transaction.push(Op::Connect {
            from: connection.erase(),
            to: port.erase(),
            edge: EdgeKind::Connects,
        });
    }
    transaction.push(Op::Connect {
        from: activation.erase(),
        to: relation.erase(),
        edge: EdgeKind::Activates,
    });
    // Boundary membership distinguishes the two Output endpoints structurally.
    transaction.push(Op::DefineOntologyView {
        view: ModelView::new(model, members, [ports[0].erase()])
            .unwrap()
            .into(),
    });
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    KernelProgram::from_snapshot(&store.snapshot(), model).unwrap()
}

#[test]
fn signal_driver_replays_exactly_and_is_nominal_fingerprint_meaning() {
    let original = program(false);
    let envelope = ModelEnvelope::from_program(&original).unwrap();
    let bytes = envelope.canonical_json().unwrap();
    let replay = ModelEnvelope::from_json(&bytes, Default::default())
        .unwrap()
        .to_program()
        .unwrap();
    assert_eq!(original, replay);
    let fingerprint = StructuralSemanticFingerprint::from_program(&original).unwrap();
    assert_eq!(
        fingerprint,
        StructuralSemanticFingerprint::from_program(&program(false)).unwrap()
    );
    assert_ne!(
        fingerprint,
        StructuralSemanticFingerprint::from_program(&program(true)).unwrap()
    );
    let wire: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    for change in 0..3 {
        let mut invalid = wire.clone();
        let semantics = &mut invalid["nodes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|node| node["definition"]["kind"] == "connection")
            .unwrap()["definition"]["connection"];
        match change {
            0 => {
                semantics.as_object_mut().unwrap().remove("driver");
            }
            1 => {
                semantics["driver"]["kind"] = serde_json::json!("field");
            }
            _ => {
                semantics["driver"]["ulid"] =
                    serde_json::json!(Id::<kinds::Port>::new().erase().ulid().to_string());
            }
        }
        assert!(
            ModelEnvelope::from_json(&serde_json::to_vec(&invalid).unwrap(), Default::default())
                .map(|envelope| envelope.to_program().is_err())
                .unwrap_or(true)
        );
    }
}
