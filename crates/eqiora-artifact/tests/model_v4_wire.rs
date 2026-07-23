use eqiora_artifact::{
    DecoderLimits, ModelEnvelopeV1, ModelEnvelopeV2, ModelEnvelopeV3, ModelEnvelopeV4,
    ModelTransactionEnvelopeV1, ModelTransactionEnvelopeV2, ModelTransactionEnvelopeV3,
    ModelTransactionEnvelopeV4,
};
use eqiora_compiler::compile;
use eqiora_graph::{GraphStore, InMemoryGraphStore};
use eqiora_sem::KernelProgram;

const ELASTIC_RELATION: &str = r#"
model elastic_relation {
  domain body = box(0, 1, 0, 1);
  representation space = continuum;
  field displacement on body as space: m shape spatial_vector;
  parameter mu: kg / (m * s ^ 2) = 2;
  parameter lambda: kg / (m * s ^ 2) = 3;
  relation balance continuous on body {
    -div(
      2 * mu * symmetric_part(grad(displacement))
      + lambda * isotropic_lift(div(displacement))
    ) = 0;
  }
}
"#;

fn program_fixture() -> KernelProgram {
    let compiled = compile("elastic-relation.eqi", ELASTIC_RELATION)
        .unwrap()
        .remove(0);
    let (transaction, model, _) = compiled.into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    KernelProgram::from_snapshot(&store.snapshot(), model).unwrap()
}

fn transaction_fixture() -> eqiora_graph::Transaction {
    compile("elastic-relation.eqi", ELASTIC_RELATION)
        .unwrap()
        .remove(0)
        .into_parts()
        .0
}

#[test]
fn tensor_operators_require_explicit_model_wire_v4() {
    let program = program_fixture();
    assert!(ModelEnvelopeV1::from_program(&program).is_err());
    assert!(ModelEnvelopeV2::from_program(&program).is_err());
    assert!(ModelEnvelopeV3::from_program(&program).is_err());

    let v4 = ModelEnvelopeV4::from_program(&program).unwrap();
    let bytes = v4.canonical_json().unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("eqiora.model-envelope/v4"));
    assert!(text.contains("symmetric-part"));
    assert!(text.contains("isotropic-lift"));

    let decoded = ModelEnvelopeV4::from_json(&bytes, DecoderLimits::default()).unwrap();
    assert_eq!(decoded.canonical_json().unwrap(), bytes);
    assert_eq!(decoded.digest().unwrap(), v4.digest().unwrap());
    assert_eq!(decoded.to_program().unwrap(), program);

    assert!(ModelEnvelopeV3::from_json(&bytes, DecoderLimits::default()).is_err());
    let mut forged_v3: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    forged_v3["schema"] = serde_json::Value::String("eqiora.model-envelope/v3".to_owned());
    assert!(
        ModelEnvelopeV3::from_json(
            &serde_json::to_vec(&forged_v3).unwrap(),
            DecoderLimits::default(),
        )
        .is_err()
    );
}

#[test]
fn tensor_operators_require_explicit_transaction_wire_v4() {
    let transaction = transaction_fixture();
    assert!(ModelTransactionEnvelopeV1::from_transaction(&transaction).is_err());
    assert!(ModelTransactionEnvelopeV2::from_transaction(&transaction).is_err());
    assert!(ModelTransactionEnvelopeV3::from_transaction(&transaction).is_err());

    let v4 = ModelTransactionEnvelopeV4::from_transaction(&transaction).unwrap();
    let bytes = v4.canonical_json().unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("eqiora.model-transaction-envelope/v4"));
    assert!(text.contains("symmetric-part"));
    assert!(text.contains("isotropic-lift"));

    let decoded = ModelTransactionEnvelopeV4::from_json(&bytes, DecoderLimits::default()).unwrap();
    assert_eq!(decoded.canonical_json().unwrap(), bytes);
    assert_eq!(decoded.digest().unwrap(), v4.digest().unwrap());
    let replay = decoded.to_transaction().unwrap();
    assert_eq!(replay.label(), transaction.label());
    assert_eq!(replay.ops(), transaction.ops());
    assert_eq!(replay.preconditions(), transaction.preconditions());

    assert!(ModelTransactionEnvelopeV3::from_json(&bytes, DecoderLimits::default()).is_err());
    let mut forged_v3: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    forged_v3["schema"] =
        serde_json::Value::String("eqiora.model-transaction-envelope/v3".to_owned());
    assert!(
        ModelTransactionEnvelopeV3::from_json(
            &serde_json::to_vec(&forged_v3).unwrap(),
            DecoderLimits::default(),
        )
        .is_err()
    );

    let mut malformed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    malformed["ops"] = serde_json::Value::Array(Vec::new());
    let diagnostic = ModelTransactionEnvelopeV4::from_json(
        &serde_json::to_vec(&malformed).unwrap(),
        DecoderLimits::default(),
    )
    .unwrap_err();
    assert!(diagnostic.message().contains("model transaction v4"));
    assert!(!diagnostic.message().contains("model transaction v2"));
}
