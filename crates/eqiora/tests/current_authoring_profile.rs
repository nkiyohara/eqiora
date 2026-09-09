use eqiora::DimExponents;
use eqiora::api::ModelDocument;
use eqiora::control::{CompileRequestV2, execute_compile_v2};
use eqiora::language::{DraftExpression, DraftField, DraftRelation, Module};
use serde_json::Value;

const SCALAR_SOURCE: &str = r#"
model decay() {
  state x: 1; initial { x = 1; }
  relation hold {
    derivative(x) = 0;
  }
}
"#;

const CURRENT_ONLY_SOURCE: &str = include_str!(
    "../../../verify/interfaces/current-authoring-profile/models/current-authoring.eqi"
);

#[test]
fn rust_authoring_edit_replay_and_control_share_the_current_profile() {
    let public_schema: Value = serde_json::from_str(include_str!(
        "../../eqiora-api/schemas/compile-v2.schema.json"
    ))
    .unwrap();
    let schemas = &public_schema["$defs"]["model"]["properties"];

    let source = ModelDocument::compile("elastic-relation.eqi", CURRENT_ONLY_SOURCE).unwrap();
    let source_bytes = source.canonical_json().unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&source_bytes).unwrap()["schema"],
        schemas["schema"]["const"]
    );
    let edit = source
        .preview_value_edit(
            source.aliases()["mu"],
            eqiora_core::ValueLiteral::from_real(
                source
                    .program()
                    .typed_value(source.aliases()["mu"])
                    .unwrap()
                    .value_type()
                    .clone(),
                4.0,
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&edit.transaction_json().unwrap()).unwrap()["schema"],
        schemas["transactionSchema"]["const"]
    );
    let child = source.commit_value_edit(edit).unwrap().into_document();
    let child_bytes = child.canonical_json().unwrap();
    let replay = ModelDocument::replay(&child_bytes).unwrap();
    assert_eq!(replay.canonical_json().unwrap(), child_bytes);
    assert_eq!(replay.digest().unwrap(), child.digest().unwrap());

    let state = DraftField::new(
        "x",
        eqiora_core::ValueType::scalar(
            eqiora_core::ScalarDomain::Real,
            DimExponents::DIMENSIONLESS,
        )
        .expect("valid scalar type"),
        eqiora::language::FieldRoleSyntax::State,
    );
    let hold = DraftRelation::continuous(
        "hold",
        [(
            DraftExpression::derivative(&state),
            DraftExpression::constant(
                eqiora::language::DecimalLiteral::from_f64(0.0).expect("finite zero"),
            ),
        )],
    );
    let initial = eqiora::language::DraftDeclaration::Initial(vec![(
        state.expression(),
        DraftExpression::constant(
            eqiora::language::DecimalLiteral::from_f64(1.0).expect("finite fixture literal"),
        ),
    )]);
    let draft = Module::new("decay", [state.into(), hold.into(), initial]).unwrap();
    let native = ModelDocument::compile_module(&draft, None, &[]).unwrap();
    let source_scalar = ModelDocument::compile("decay.eqi", SCALAR_SOURCE).unwrap();
    assert!(native.structurally_equivalent(&source_scalar).unwrap());

    let request =
        CompileRequestV2::new("rust.current-profile", "decay.eqi", SCALAR_SOURCE).unwrap();
    let execution = execute_compile_v2(&request);
    assert!(execution.document().is_some());
}
