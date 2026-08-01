use eqiora::DimExponents;
use eqiora::api::ModelDocument;
use eqiora::control::{CompileRequestV2, execute_compile_v2};
use eqiora::language::{DraftExpression, DraftField, DraftRelation, ModelDraft};
use serde::Deserialize;

const SCALAR_SOURCE: &str = r#"
model decay {
  field x: 1 = 1;
  relation hold continuous {
    derivative(x) = 0;
  }
}
"#;

const CURRENT_ONLY_SOURCE: &str = include_str!(
    "../../../verify/interfaces/current-authoring-profile/models/current-authoring.eqi"
);
const PROFILE: &[u8] =
    include_bytes!("../../../verify/interfaces/current-authoring-profile/expected/profile.json");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExpectedProfile {
    schema: String,
    profile: String,
    model_schema: String,
    transaction_schema: String,
}

#[test]
fn rust_authoring_edit_replay_and_control_share_the_current_profile() {
    let expected: ExpectedProfile = serde_json::from_slice(PROFILE).unwrap();
    assert_eq!(
        expected.schema,
        "eqiora.verify.current-authoring-profile/v1"
    );
    assert_eq!(expected.profile, "current");

    let source = ModelDocument::compile("elastic-relation.eqi", CURRENT_ONLY_SOURCE).unwrap();
    let source_bytes = source.canonical_json().unwrap();
    assert!(
        String::from_utf8_lossy(&source_bytes).contains(&expected.model_schema),
        "ordinary source authoring must emit the current Model schema"
    );
    let edit = source
        .preview_value_edit(source.aliases()["mu"], 4.0)
        .unwrap();
    assert!(
        String::from_utf8(edit.transaction_json().unwrap())
            .unwrap()
            .contains(&expected.transaction_schema)
    );
    let child = source.commit_value_edit(edit).unwrap().into_document();
    let child_bytes = child.canonical_json().unwrap();
    let replay = ModelDocument::replay(&child_bytes).unwrap();
    assert_eq!(replay.canonical_json().unwrap(), child_bytes);
    assert_eq!(replay.digest().unwrap(), child.digest().unwrap());

    let state = DraftField::new("x", DimExponents::DIMENSIONLESS, 1.0);
    let hold = DraftRelation::continuous("hold", [DraftExpression::derivative(&state)]);
    let draft = ModelDraft::new("decay", [state.into(), hold.into()]).unwrap();
    let native = ModelDocument::define(&draft).unwrap();
    let source_scalar = ModelDocument::compile("decay.eqi", SCALAR_SOURCE).unwrap();
    assert!(native.structurally_equivalent(&source_scalar).unwrap());

    let request =
        CompileRequestV2::new("rust.current-profile", "decay.eqi", SCALAR_SOURCE).unwrap();
    let execution = execute_compile_v2(&request);
    assert!(execution.document().is_some());
}
