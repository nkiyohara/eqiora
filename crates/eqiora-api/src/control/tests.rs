use serde_json::Value;

use super::*;

const SOURCE: &str = r#"
model decay {
  field x: 1 = 1;
  parameter rate: 1 / s = 1;
  relation flow continuous {
    derivative(x) + rate * x = 0;
  }
}
"#;

#[test]
fn unordered_duplicate_features_normalize_before_round_trip() {
    let wire = format!(
        r#"{{"protocol":"{CONTROL_PROTOCOL_V1}","command":"{COMPILE_COMMAND_V1}","requestId":"compile-1","requiredFeatures":["model-wire/v4","{COMPILE_FEATURE_V1}","model-wire/v4"],"modelWire":"v4","filename":"model.eqi","source":{}}}"#,
        serde_json::to_string(SOURCE).unwrap()
    );
    let request = CompileRequestV1::from_json(wire.as_bytes()).unwrap();
    assert_eq!(
        request.required_features(),
        [
            CompileFeatureV1::CompileCheck,
            CompileFeatureV1::ModelSchemaGeneration4
        ]
    );
    let replay = CompileRequestV1::from_json(&request.canonical_json().unwrap()).unwrap();
    assert_eq!(replay, request);
}

#[test]
fn mismatched_or_unknown_features_fail_closed() {
    for (features, code) in [
        (r#"["model.compile-check/v1","model-wire/v2"]"#, "EQ0001"),
        (
            r#"["model.compile-check/v1","future-feature/v9"]"#,
            "EQ0001",
        ),
    ] {
        let wire = format!(
            r#"{{"protocol":"{CONTROL_PROTOCOL_V1}","command":"{COMPILE_COMMAND_V1}","requestId":"compile-1","requiredFeatures":{features},"modelWire":"v1","filename":"model.eqi","source":""}}"#
        );
        let diagnostic = CompileRequestV1::from_json(wire.as_bytes()).unwrap_err();
        assert_eq!(diagnostic.code(), code);
    }
}

#[test]
fn unknown_fields_and_protocols_fail_before_compiler_diagnostics() {
    let unknown_field = format!(
        r#"{{"protocol":"{CONTROL_PROTOCOL_V1}","command":"{COMPILE_COMMAND_V1}","requestId":"compile-1","requiredFeatures":["{COMPILE_FEATURE_V1}","model-wire/v1"],"modelWire":"v1","filename":"bad.eqi","source":"not valid source","guessWire":true}}"#
    );
    let diagnostic = CompileRequestV1::from_json(unknown_field.as_bytes()).unwrap_err();
    assert_eq!(diagnostic.code(), "EQ0901");

    let wrong_protocol = unknown_field
        .replace(CONTROL_PROTOCOL_V1, "eqiora.control/v2")
        .replace(",\"guessWire\":true", "");
    let diagnostic = CompileRequestV1::from_json(wrong_protocol.as_bytes()).unwrap_err();
    assert_eq!(diagnostic.code(), "EQ0001");
}

#[test]
fn accepted_execution_echoes_identity_and_owns_the_existing_document() {
    let request =
        CompileRequestV1::new_exact("compile-1", ExactModelCodec::V1, "decay.eqi", SOURCE).unwrap();
    let execution = execute_compile_v1(&request);
    let document = execution.document().expect("accepted document");
    let CompileOutcomeV1::Accepted { model } = execution.response().outcome() else {
        panic!("valid source must be accepted")
    };
    assert_eq!(execution.response().request_id(), "compile-1");
    assert_eq!(model.exact_codec(), ExactModelCodec::V1);
    assert_eq!(model.schema(), "eqiora.model-envelope/v1");
    assert_eq!(model.digest(), document.digest().unwrap());
    assert_eq!(model.semantic_revision(), document.program().revision().0);
}

#[test]
fn rejected_execution_has_structured_diagnostics_and_no_document() {
    let request =
        CompileRequestV1::new_exact("compile-1", ExactModelCodec::V1, "bad.eqi", "not a model")
            .unwrap();
    let execution = execute_compile_v1(&request);
    assert!(execution.document().is_none());
    let CompileOutcomeV1::Rejected { diagnostics } = execution.response().outcome() else {
        panic!("invalid source must be rejected")
    };
    assert!(!diagnostics.is_empty());
    assert_eq!(diagnostics[0].source(), ControlDiagnosticSourceV1::Kernel);
    assert!(diagnostics[0].code().starts_with("EQ"));
}

#[test]
fn committed_schema_is_exactly_the_deterministic_generation() {
    assert_eq!(
        generated_compile_v1_schema_json().unwrap(),
        COMPILE_V1_SCHEMA_JSON
    );
}

#[test]
#[ignore = "explicit schema regeneration command"]
fn regenerate_committed_schema() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/control/compile-v1.schema.json");
    std::fs::write(path, generated_compile_v1_schema_json().unwrap()).unwrap();
}

#[test]
fn committed_schema_is_draft_2020_12_and_closed_at_both_boundaries() {
    let schema: Value = serde_json::from_str(COMPILE_V1_SCHEMA_JSON).unwrap();
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(schema["$defs"]["request"]["additionalProperties"], false);
    assert_eq!(schema["$defs"]["response"]["additionalProperties"], false);
    assert_eq!(schema["$defs"]["diagnostic"]["additionalProperties"], false);
    assert_eq!(
        schema["$defs"]["request"]["x-eqiora-maxEncodedUtf8Bytes"],
        MAX_COMPILE_REQUEST_BYTES_V1
    );
    assert_eq!(
        schema["$defs"]["request"]["properties"]["filename"]["x-eqiora-maxUtf8Bytes"],
        MAX_COMPILE_FILENAME_BYTES_V1
    );
    assert_eq!(
        schema["$defs"]["request"]["properties"]["source"]["x-eqiora-maxUtf8Bytes"],
        MAX_COMPILE_SOURCE_BYTES_V1
    );
}
