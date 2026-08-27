use eqiora::DimExponents;
use eqiora::api::ModelDocument;
use eqiora::language::{DraftExpression, DraftField, DraftParameter, DraftRelation, ModelDraft};

const SOURCE: &str = include_str!("../../../verify/language/native-modeling/models/decay.eqi");

#[test]
fn native_and_source_models_share_structure_and_artifacts() {
    let state = DraftField::new("x", DimExponents::DIMENSIONLESS, 1.0);
    let rate = DraftParameter::new(
        "rate",
        DimExponents {
            time: -1,
            ..DimExponents::DIMENSIONLESS
        },
        1.0,
    );
    let flow = DraftRelation::continuous(
        "flow",
        [DraftExpression::derivative(&state) + rate.expression() * state.expression()],
    );
    // Independent declaration order is presentation, not symbol resolution.
    let draft = ModelDraft::new("decay", [rate.into(), state.into(), flow.into()]).unwrap();

    let source = ModelDocument::compile("decay.eqi", SOURCE).unwrap();
    let native = ModelDocument::define(&draft).unwrap();
    assert!(native.structurally_equivalent(&source).unwrap());
    assert_eq!(
        native.structural_fingerprint().unwrap(),
        source.structural_fingerprint().unwrap()
    );
    assert_ne!(
        native.artifact_reference().unwrap(),
        source.artifact_reference().unwrap()
    );

    let bytes = native.canonical_json().unwrap();
    let reconstructed = eqiora::api::ModelDocument::replay(&bytes).unwrap();
    assert_eq!(reconstructed.canonical_json().unwrap(), bytes);
    assert_eq!(reconstructed.digest().unwrap(), native.digest().unwrap());
}

#[test]
fn native_modeling_failures_have_paths_and_never_return_a_model() {
    let included = DraftField::new("x", DimExponents::DIMENSIONLESS, 1.0);
    let foreign = DraftField::new("x", DimExponents::DIMENSIONLESS, 1.0);
    let relation = DraftRelation::continuous("flow", [foreign.expression()]);
    let diagnostic = ModelDraft::new("decay", [included.into(), relation.into()]).unwrap_err();
    assert_eq!(
        diagnostic[0].graph_path().unwrap().to_string(),
        "decay.flow"
    );

    let temperature = DraftField::new(
        "temperature",
        DimExponents {
            temperature: 1,
            ..DimExponents::DIMENSIONLESS
        },
        293.0,
    );
    let duration = DraftParameter::new(
        "duration",
        DimExponents {
            time: 1,
            ..DimExponents::DIMENSIONLESS
        },
        1.0,
    );
    let invalid = DraftRelation::continuous(
        "invalid",
        [temperature.expression() + duration.expression()],
    );
    let draft = ModelDraft::new(
        "thermal",
        [temperature.into(), duration.into(), invalid.into()],
    )
    .unwrap();
    let diagnostics = ModelDocument::define(&draft).unwrap_err();
    assert_eq!(
        diagnostics[0].graph_path().unwrap().to_string(),
        "thermal.invalid"
    );
    assert!(diagnostics[0].source_span().is_none());
}
