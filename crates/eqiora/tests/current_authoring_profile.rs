use eqiora::DimExponents;
use eqiora::api::ModelDocument;
use eqiora::compatibility::ExactModelCodec;
use eqiora::control::{CompileRequestV1, execute_compile_v1};
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
    model_wire: String,
    model_schema: String,
    exact_codecs: Vec<String>,
}

#[test]
fn rust_authoring_and_control_select_the_registered_current_profile() {
    let expected: ExpectedProfile = serde_json::from_slice(PROFILE).unwrap();
    assert_eq!(
        expected.schema,
        "eqiora.verify.current-authoring-profile/v1"
    );
    assert_eq!(expected.profile, "current");
    assert_eq!(ExactModelCodec::CURRENT.as_str(), expected.model_wire);
    assert_eq!(
        ExactModelCodec::CURRENT.model_schema(),
        expected.model_schema
    );

    let source = ModelDocument::compile("elastic-relation.eqi", CURRENT_ONLY_SOURCE).unwrap();
    assert_eq!(source.exact_codec(), ExactModelCodec::CURRENT);
    let edit = source
        .preview_value_edit(source.aliases()["mu"], 4.0)
        .unwrap();
    assert_eq!(edit.exact_codec(), ExactModelCodec::CURRENT);
    assert!(
        String::from_utf8(edit.transaction_json().unwrap())
            .unwrap()
            .contains("eqiora.model-transaction-envelope/v7")
    );
    assert_eq!(
        source
            .commit_value_edit(edit)
            .unwrap()
            .document()
            .exact_codec(),
        ExactModelCodec::CURRENT
    );

    let state = DraftField::new("x", DimExponents::DIMENSIONLESS, 1.0);
    let hold = DraftRelation::continuous("hold", [DraftExpression::derivative(&state)]);
    let draft = ModelDraft::new("decay", [state.into(), hold.into()]).unwrap();
    let native = ModelDocument::define(&draft).unwrap();
    assert_eq!(native.exact_codec(), ExactModelCodec::CURRENT);

    let request =
        CompileRequestV1::new_current("rust.current-profile", "decay.eqi", SCALAR_SOURCE).unwrap();
    assert_eq!(request.model_codec(), ExactModelCodec::CURRENT);
    let execution = execute_compile_v1(&request);
    assert_eq!(
        execution.document().unwrap().exact_codec(),
        ExactModelCodec::CURRENT
    );
}

#[test]
fn exact_replay_remains_explicit_and_unknown_or_mismatched_codecs_fail_closed() {
    let expected: ExpectedProfile = serde_json::from_slice(PROFILE).unwrap();
    let codecs = [
        ExactModelCodec::V1,
        ExactModelCodec::V2,
        ExactModelCodec::V3,
        ExactModelCodec::V4,
        ExactModelCodec::V5,
        ExactModelCodec::V6,
        ExactModelCodec::V7,
    ];
    assert_eq!(
        codecs.map(ExactModelCodec::as_str).as_slice(),
        expected.exact_codecs
    );

    for codec in codecs {
        let document = codec.compile("decay.eqi", SCALAR_SOURCE).unwrap();
        let edit = document
            .preview_value_edit(document.aliases()["x"], 2.0)
            .unwrap();
        assert_eq!(edit.exact_codec(), codec);
        assert_eq!(
            document
                .commit_value_edit(edit)
                .unwrap()
                .document()
                .exact_codec(),
            codec
        );
        let bytes = document.canonical_json().unwrap();
        let digest = document.digest().unwrap();
        let replay = codec.replay(&bytes).unwrap();
        assert_eq!(replay.canonical_json().unwrap(), bytes);
        assert_eq!(replay.digest().unwrap(), digest);
        assert_eq!(replay.exact_codec(), codec);
    }

    let v1 = ExactModelCodec::V1
        .compile("decay.eqi", SCALAR_SOURCE)
        .unwrap()
        .canonical_json()
        .unwrap();
    assert!(ExactModelCodec::CURRENT.replay(&v1).is_err());

    assert!(
        ExactModelCodec::V4
            .compile("current-authoring.eqi", CURRENT_ONLY_SOURCE)
            .is_err(),
        "exact v4 must reject the current pure-operator vocabulary"
    );

    let unknown = br#"{"protocol":"eqiora.control/v1","command":"model.compile-check/v1","requestId":"unknown-codec","requiredFeatures":["model.compile-check/v1","model-wire/v9"],"modelWire":"v9","filename":"decay.eqi","source":""}"#;
    assert!(CompileRequestV1::from_json(unknown).is_err());
}
