//! Select and Require retain condition, branch, and guard identity through artifact replay.

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

fn program(reverse: bool, guarded: bool) -> KernelProgram {
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
    let condition = dag
        .constant(eqiora_core::ValueLiteral::boolean(true))
        .unwrap();
    let (then_value, else_value) = if reverse { (three, two) } else { (two, three) };
    let selected = dag.select(condition, then_value, else_value).unwrap();
    let selected = if guarded {
        dag.require(condition, selected).unwrap()
    } else {
        selected
    };
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
fn conditional_model_transaction_replay_preserves_operand_roles() {
    let original = program(false, true);
    let bytes = ModelEnvelope::from_program(&original)
        .unwrap()
        .canonical_json()
        .unwrap();
    let mut json: Value = serde_json::from_slice(&bytes).unwrap();
    let expression = relation_expression(&mut json);
    // Independently enumerated: x, 2, 3, true, select(true,2,3), require(true,select).
    assert_eq!(
        expression["nodes"][4],
        json!({"op":"select","condition":3,"then_value":1,"else_value":2})
    );
    assert_eq!(
        expression["nodes"][5],
        json!({"op":"require","condition":3,"value":4})
    );
    let restored = ModelEnvelope::from_json(&bytes, ModelDecoderLimits::default()).unwrap();
    assert_eq!(restored.to_program().unwrap(), original);
    let (seed, model) = restored.to_transaction().unwrap();
    let bytes = ModelTransactionEnvelope::from_transaction(&seed)
        .unwrap()
        .canonical_json()
        .unwrap();
    let transaction = ModelTransactionEnvelope::from_json(&bytes, ModelDecoderLimits::default())
        .unwrap()
        .to_transaction()
        .unwrap();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    assert_eq!(
        KernelProgram::from_snapshot(&store.snapshot(), model).unwrap(),
        original
    );
}

#[test]
fn conditional_fingerprint_distinguishes_branches_and_domain_guard() {
    let mut fingerprints = std::collections::BTreeSet::new();
    for reverse in [false, true] {
        for guarded in [false, true] {
            assert!(fingerprints.insert(
                StructuralSemanticFingerprint::from_program(&program(reverse, guarded)).unwrap()
            ));
        }
    }
}

#[test]
fn conditional_wire_rejects_forward_references_and_numeric_conditions() {
    let original: Value = serde_json::from_slice(
        &ModelEnvelope::from_program(&program(false, true))
            .unwrap()
            .canonical_json()
            .unwrap(),
    )
    .unwrap();
    for (node, field, value) in [
        (4, "condition", 99),
        (4, "condition", 1),
        (4, "else_value", 5),
        (5, "condition", 1),
        (5, "value", 5),
    ] {
        let mut invalid = original.clone();
        relation_expression(&mut invalid)["nodes"][node][field] = json!(value);
        let rejected = match ModelEnvelope::from_json(
            &serde_json::to_vec(&invalid).unwrap(),
            ModelDecoderLimits::default(),
        ) {
            Err(_) => true,
            Ok(envelope) => envelope.to_program().is_err(),
        };
        assert!(rejected, "node {node} field {field}");
    }
}
