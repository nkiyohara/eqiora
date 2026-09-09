//! Closed heterogeneous products preserve ordered nominal membership on replay.
use eqiora_artifact::{
    ModelDecoderLimits, ModelEnvelope, ModelTransactionEnvelope, StructuralSemanticFingerprint,
};
use eqiora_core::{Id, OntologyId, ValueLiteral, ValueType};
use eqiora_graph::{GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora_schema::{
    Model, ModelView,
    kernel::{
        ActivationDef, EnumDef, ExprDagBuilder, KernelNode, ParameterDef, RecordDef,
        RecordInstanceDef, RelationDef,
    },
};
use eqiora_sem::KernelProgram;
use serde_json::{Value, json};

fn fixture(reverse: bool, shared: bool) -> KernelProgram {
    let mode = EnumDef::new(Id::new(), ["Off".into(), "On".into()]).unwrap();
    let members = vec![
        ("enabled".into(), ValueType::boolean()),
        ("ready".into(), ValueType::boolean()),
        ("mode".into(), mode.value_type()),
    ];
    let first = RecordDef::new(Id::new(), members.clone()).unwrap();
    let second = RecordDef::new(Id::new(), members).unwrap();
    let enabled = ParameterDef::new(Id::new(), ValueLiteral::boolean(true));
    let ready = ParameterDef::new(Id::new(), ValueLiteral::boolean(false));
    let selected = ParameterDef::new(Id::new(), mode.value(1).unwrap());
    let mut leaves = vec![
        enabled.id().erase(),
        ready.id().erase(),
        selected.id().erase(),
    ];
    if reverse {
        leaves.swap(0, 1);
    }
    let a = RecordInstanceDef::new(Id::new(), first.id(), leaves.clone()).unwrap();
    let b = RecordInstanceDef::new(
        Id::new(),
        if shared { first.id() } else { second.id() },
        leaves,
    )
    .unwrap();
    let activation = Id::new();
    let relation = Id::new();
    let mut dag = ExprDagBuilder::new();
    let zero = dag
        .constant(eqiora_core::DynQuantity::new(
            0.0,
            eqiora_core::DimExponents::DIMENSIONLESS,
        ))
        .unwrap();
    let nodes: Vec<KernelNode> = vec![
        mode.into(),
        first.into(),
        second.into(),
        enabled.into(),
        ready.into(),
        selected.into(),
        a.into(),
        b.into(),
        ActivationDef::continuous(activation).into(),
        RelationDef::new(relation, dag.finish([zero, zero]).unwrap())
            .unwrap()
            .into(),
    ];
    let model = OntologyId::<Model>::new();
    let view = ModelView::new(model, nodes.iter().map(KernelNode::id), []).unwrap();
    let mut transaction = Transaction::new("closed record wire fixture");
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
    transaction.push(Op::Connect {
        from: activation.erase(),
        to: relation.erase(),
        edge: eqiora_graph::EdgeKind::Activates,
    });
    transaction.push(Op::DefineOntologyView { view: view.into() });
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    KernelProgram::from_snapshot(&store.snapshot(), model).unwrap()
}

fn wire(program: &KernelProgram) -> Value {
    serde_json::from_slice(
        &ModelEnvelope::from_program(program)
            .unwrap()
            .canonical_json()
            .unwrap(),
    )
    .unwrap()
}
fn reopen(value: &Value) -> Result<KernelProgram, eqiora_core::Diagnostic> {
    ModelEnvelope::from_json(
        &serde_json::to_vec(value).unwrap(),
        ModelDecoderLimits::default(),
    )?
    .to_program()
}
fn definition<'a>(wire: &'a mut Value, kind: &str) -> &'a mut Value {
    &mut wire["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|node| node["definition"]["kind"] == kind)
        .unwrap()["definition"]
}

#[test]
fn closed_record_replay_retains_exact_order_types_and_membership() {
    let program = fixture(false, true);
    let envelope = ModelEnvelope::from_program(&program).unwrap();
    let (transaction, model) = envelope.to_transaction().unwrap();
    let transaction_wire = ModelTransactionEnvelope::from_transaction(&transaction)
        .unwrap()
        .canonical_json()
        .unwrap();
    let restored =
        ModelTransactionEnvelope::from_json(&transaction_wire, ModelDecoderLimits::default())
            .unwrap()
            .to_transaction()
            .unwrap();
    let mut store = InMemoryGraphStore::new();
    store.commit(restored).unwrap();
    assert_eq!(
        KernelProgram::from_snapshot(&store.snapshot(), model).unwrap(),
        program
    );
    let original = wire(&program);
    assert_eq!(reopen(&original).unwrap(), program);
    let declaration = original["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["definition"]["kind"] == "record")
        .unwrap();
    assert_eq!(declaration["definition"]["members"][0][0], "enabled");
    assert_eq!(declaration["definition"]["members"][2][1]["domain"], "enum");
    assert_eq!(wire(&reopen(&original).unwrap()), original);
}

#[test]
fn structural_record_identity_binds_order_and_nominal_sharing() {
    let mut fingerprints = std::collections::BTreeSet::new();
    for reverse in [false, true] {
        for shared in [false, true] {
            let fingerprint =
                StructuralSemanticFingerprint::from_program(&fixture(reverse, shared)).unwrap();
            assert_eq!(
                fingerprint,
                StructuralSemanticFingerprint::from_program(&fixture(reverse, shared)).unwrap(),
                "fresh exact IDs preserve structural meaning"
            );
            assert!(fingerprints.insert(fingerprint));
        }
    }
}

#[test]
fn malformed_record_members_and_enum_tags_fail_closed() {
    let original = wire(&fixture(false, true));
    for mutation in 0..9 {
        let mut invalid = original.clone();
        match mutation {
            0 => {
                definition(&mut invalid, "record-instance")["members"]
                    .as_array_mut()
                    .unwrap()
                    .pop();
            }
            1 => {
                let instance = definition(&mut invalid, "record-instance");
                instance["members"][0] = instance["members"][1].clone();
            }
            2 => {
                definition(&mut invalid, "record-instance")["members"][0]["ulid"] = json!(
                    Id::<eqiora_core::entity::kinds::Parameter>::new()
                        .ulid()
                        .to_string()
                );
            }
            3 => {
                definition(&mut invalid, "record-instance")["definition"]["kind"] = json!("enum");
            }
            4 => {
                definition(&mut invalid, "record")["members"][1][0] = json!("enabled");
            }
            5 => {
                definition(&mut invalid, "record-instance")["members"]
                    .as_array_mut()
                    .unwrap()
                    .swap(0, 2);
            }
            6 => {
                definition(&mut invalid, "record-instance")["definition"]["ulid"] = json!(
                    Id::<eqiora_core::entity::kinds::Record>::new()
                        .ulid()
                        .to_string()
                );
            }
            7 => {
                let node = invalid["nodes"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|node| node["definition"]["value"]["components"]["kind"] == "enum")
                    .unwrap();
                node["definition"]["value"]["components"]["tag"] = json!(2);
            }
            _ => {
                definition(&mut invalid, "record-instance")["members"][0]["kind"] = json!("record");
            }
        }
        assert!(reopen(&invalid).is_err(), "mutation {mutation}");
    }
    // Swapping same-typed members is a different, valid record value, not corruption.
    let mut valid = original;
    definition(&mut valid, "record-instance")["members"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    assert!(reopen(&valid).is_ok());
}
