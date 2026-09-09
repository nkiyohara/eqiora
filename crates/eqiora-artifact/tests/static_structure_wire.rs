//! Static specialization remains a guarded dependency after both current codecs.
use eqiora_artifact::{ModelEnvelope, ModelTransactionEnvelope, StructuralSemanticFingerprint};
use eqiora_compiler::compile;
use eqiora_core::ValueLiteral;
use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora_sem::KernelProgram;

#[test]
fn extent_and_folded_slice_dependencies_survive_model_and_transaction_replay() {
    let compiled = compile("static-wire.eqi", "model M(){parameter n:integer=3; parameter stop:integer=2; variable values:array<1,n>; relation zero{values[0:stop]=[0,0];}}")
        .unwrap().remove(0);
    let n = compiled.symbols().get("n").unwrap();
    let stop = compiled.symbols().get("stop").unwrap();
    let (transaction, model, _) = compiled.into_parts();
    let wire = ModelTransactionEnvelope::from_transaction(&transaction)
        .unwrap()
        .canonical_json()
        .unwrap();
    let transaction = ModelTransactionEnvelope::from_json(&wire, Default::default())
        .unwrap()
        .to_transaction()
        .unwrap();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    let program = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
    let dependencies = program
        .edges()
        .iter()
        .filter(|edge| edge.kind() == EdgeKind::StructurallyDependsOn)
        .collect::<Vec<_>>();
    assert_eq!(dependencies.len(), 2);
    assert!(dependencies.iter().any(|edge| edge.to() == n));
    assert!(dependencies.iter().any(|edge| edge.to() == stop));
    let bytes = ModelEnvelope::from_program(&program)
        .unwrap()
        .canonical_json()
        .unwrap();
    let replay = ModelEnvelope::from_json(&bytes, Default::default())
        .unwrap()
        .to_program()
        .unwrap();
    assert_eq!(program, replay);
    for target in [n, stop] {
        let value = program.typed_value(target).unwrap();
        let replacement = ValueLiteral::from_integer(value.value_type().clone(), 1).unwrap();
        let mut edit = Transaction::new("static shape edit after replay");
        edit.push(Op::SetValue {
            target,
            value: replacement,
        });
        assert!(store.commit(edit).is_err());
    }
    // The new projection is identity-bearing, not just an encoded decoration.
    let mut stripped: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    stripped["edges"]
        .as_array_mut()
        .unwrap()
        .retain(|edge| edge["kind"] != "structurally-depends-on");
    let stripped =
        ModelEnvelope::from_json(&serde_json::to_vec(&stripped).unwrap(), Default::default())
            .unwrap()
            .to_program()
            .unwrap();
    assert_ne!(
        StructuralSemanticFingerprint::from_program(&program).unwrap(),
        StructuralSemanticFingerprint::from_program(&stripped).unwrap()
    );
}
