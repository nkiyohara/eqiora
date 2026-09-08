//! Ordered eager selectors retain both operands, including equal-valued branches.

use eqiora_artifact::{
    ModelDecoderLimits, ModelEnvelope, ModelTransactionEnvelope, StructuralSemanticFingerprint,
};
use eqiora_core::{DimExponents, DynQuantity, Id, OntologyId, ScalarDomain, ValueType};
use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora_schema::{
    ModelView,
    kernel::{
        ActivationDef, ExprDagBuilder, FieldDef, FieldRole, KernelNode, RelationDef, SymbolRef,
    },
};
use eqiora_sem::KernelProgram;
use serde_json::{Value, json};

fn program(maximum: bool, reverse: bool, shared: bool) -> KernelProgram {
    let field = Id::new();
    let relation = Id::new();
    let activation = Id::new();
    let model = OntologyId::new();
    let mut dag = ExprDagBuilder::new();
    let output = dag.symbol(SymbolRef::Field(field)).unwrap();
    let two = dag
        .constant(DynQuantity::new(2.0, DimExponents::DIMENSIONLESS))
        .unwrap();
    let three = dag
        .constant(DynQuantity::new(3.0, DimExponents::DIMENSIONLESS))
        .unwrap();
    let (left, right) = if reverse { (three, two) } else { (two, three) };
    let mut select = || {
        if maximum {
            dag.max(left, right).unwrap()
        } else {
            dag.min(left, right).unwrap()
        }
    };
    let first = select();
    let second = if shared { first } else { select() };
    // Both operands have equal values; their ordered identity and sharing remain.
    let selected = dag.max(first, second).unwrap();
    let nodes: Vec<KernelNode> = vec![
        FieldDef::new(
            field,
            ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS),
            FieldRole::Variable,
        )
        .into(),
        RelationDef::new(relation, dag.finish([output, selected]).unwrap())
            .unwrap()
            .into(),
        ActivationDef::continuous(activation).into(),
    ];
    let view = ModelView::new(model, nodes.iter().map(KernelNode::id), []).unwrap();
    let mut transaction = Transaction::new("ordered selector fixture");
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
    for (from, to, edge) in [
        (activation.erase(), relation.erase(), EdgeKind::Activates),
        (relation.erase(), field.erase(), EdgeKind::DependsOn),
    ] {
        transaction.push(Op::Connect { from, to, edge });
    }
    transaction.push(Op::DefineOntologyView { view: view.into() });
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    KernelProgram::from_snapshot(&store.snapshot(), model).unwrap()
}

fn relation_expression(json: &mut Value) -> &mut Value {
    &mut json["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|node| node["definition"]["kind"] == "relation")
        .unwrap()["definition"]["expression"]
}

#[test]
fn selector_model_and_transaction_replay_preserve_ordered_shared_edges() {
    let original = program(false, false, true);
    let envelope = ModelEnvelope::from_program(&original).unwrap();
    let bytes = envelope.canonical_json().unwrap();
    let mut json: Value = serde_json::from_slice(&bytes).unwrap();
    let expression = relation_expression(&mut json);
    // x, 2, 3, 2<=3, select(2<=3,2,3), shared>=shared, select(...,shared,shared).
    assert_eq!(
        expression["nodes"][3],
        json!({"op":"compare","comparison":"less-equal","left":1,"right":2})
    );
    assert_eq!(
        expression["nodes"][4],
        json!({"op":"select","condition":3,"then_value":1,"else_value":2})
    );
    assert_eq!(
        expression["nodes"][5],
        json!({"op":"compare","comparison":"greater-equal","left":4,"right":4})
    );
    assert_eq!(
        expression["nodes"][6],
        json!({"op":"select","condition":5,"then_value":4,"else_value":4})
    );
    assert_eq!(expression["roots"], json!([0, 6]));
    let replay = ModelEnvelope::from_json(&bytes, ModelDecoderLimits::default()).unwrap();
    assert_eq!(replay.to_program().unwrap(), original);
    assert_eq!(replay.canonical_json().unwrap(), bytes);
    let (seed, model) = replay.to_transaction().unwrap();
    let transaction = ModelTransactionEnvelope::from_transaction(&seed).unwrap();
    let transaction_bytes = transaction.canonical_json().unwrap();
    let restored =
        ModelTransactionEnvelope::from_json(&transaction_bytes, ModelDecoderLimits::default())
            .unwrap()
            .to_transaction()
            .unwrap();
    assert_eq!(restored.ops(), seed.ops());
    let mut store = InMemoryGraphStore::new();
    store.commit(restored).unwrap();
    assert_eq!(
        KernelProgram::from_snapshot(&store.snapshot(), model).unwrap(),
        original
    );
}

#[test]
fn selector_fingerprint_distinguishes_operation_order_and_dag_sharing() {
    let mut fingerprints = std::collections::BTreeSet::new();
    for maximum in [false, true] {
        for reverse in [false, true] {
            for shared in [false, true] {
                let value = program(maximum, reverse, shared);
                assert!(
                    fingerprints
                        .insert(StructuralSemanticFingerprint::from_program(&value).unwrap())
                );
            }
        }
    }
    assert_eq!(fingerprints.len(), 8);
}

#[test]
fn selector_wire_rejects_bad_references_and_nonordered_operand_types() {
    let bytes = ModelEnvelope::from_program(&program(false, false, true))
        .unwrap()
        .canonical_json()
        .unwrap();
    let original: Value = serde_json::from_slice(&bytes).unwrap();
    let rejects = |json: Value| match ModelEnvelope::from_json(
        &serde_json::to_vec(&json).unwrap(),
        ModelDecoderLimits::default(),
    ) {
        Err(_) => true,
        Ok(envelope) => envelope.to_program().is_err(),
    };
    for removed in ["min", "max"] {
        let mut invalid = original.clone();
        relation_expression(&mut invalid)["nodes"][3] = json!({"op":removed,"left":1,"right":2});
        assert!(
            rejects(invalid),
            "removed persisted selector must be rejected"
        );
    }
    for index in [3, 99] {
        let mut invalid = original.clone();
        relation_expression(&mut invalid)["nodes"][3]["right"] = json!(index);
        assert!(
            rejects(invalid),
            "operand must refer to a topologically prior node"
        );
    }
    for (domain, components) in [
        ("boolean", json!({"kind":"boolean","value":true})),
        ("complex", json!({"kind":"dense","values":[[3.0,1.0]]})),
        ("integer", json!({"kind":"integer","values":[3]})),
    ] {
        let mut invalid = original.clone();
        let literal = &mut relation_expression(&mut invalid)["nodes"][2]["value"];
        literal["value_type"]["domain"] = json!(domain);
        literal["components"] = components;
        assert!(
            rejects(invalid),
            "no Boolean/complex ordering or mixed Integer/Real selection"
        );
    }
}
