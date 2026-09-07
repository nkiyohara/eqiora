use eqiora_api::ModelDocument;
use eqiora_core::{DimExponents, ScalarDomain, ValueLiteral, ValueType};

fn integer(value: i64) -> ValueLiteral {
    ValueLiteral::from_integer(
        ValueType::scalar(ScalarDomain::Integer, DimExponents::DIMENSIONLESS),
        value,
    )
    .unwrap()
}

#[test]
fn indexset_extent_parameters_reject_edits_before_and_after_replay() {
    let source = "model Extent() { parameter n: integer = 3; parameter probe: integer = 7; let count = n + 1; indexset Rows = range(count); }";
    let original = ModelDocument::compile("extent.eqi", source).unwrap();
    let original_bytes = original.canonical_json().unwrap();
    let replayed = ModelDocument::replay(&original_bytes).unwrap();
    for model in [original, replayed] {
        let n = model.aliases()["n"];
        let before = model.canonical_json().unwrap();
        let error = model.preview_value_edit(n, integer(4)).unwrap_err();
        assert!(error.message().contains("structural") || error.message().contains("index"));
        assert_eq!(model.canonical_json().unwrap(), before);
        let probe = model.aliases()["probe"];
        let plan = model.preview_value_edit(probe, integer(8)).unwrap();
        let changed = model.commit_value_edit(plan).unwrap();
        assert_eq!(
            changed
                .document()
                .program()
                .typed_value(probe)
                .unwrap()
                .integer_scalar_value(),
            Some(8)
        );
        assert_eq!(
            changed
                .document()
                .program()
                .typed_value(n)
                .unwrap()
                .integer_scalar_value(),
            Some(3)
        );
    }
}
