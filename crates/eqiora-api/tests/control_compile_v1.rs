use eqiora_api::control::{
    COMPILE_COMMAND_V1, COMPILE_FEATURE_V1, CONTROL_PROTOCOL_V1, CompileFeatureV1,
    CompileOutcomeV1, CompileRequestV1, ControlDiagnosticSourceV1, ExactModelCodec,
    execute_compile_v1,
};

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
fn public_compile_control_path_uses_every_explicit_immutable_model_wire() {
    for wire in [
        ExactModelCodec::V1,
        ExactModelCodec::V2,
        ExactModelCodec::V3,
        ExactModelCodec::V4,
        ExactModelCodec::V5,
        ExactModelCodec::V6,
    ] {
        let request = CompileRequestV1::new_exact(
            format!("compile-{}", wire.as_str()),
            wire,
            "decay.eqi",
            SOURCE,
        )
        .unwrap();
        let execution = execute_compile_v1(&request);
        let document = execution.document().expect("accepted immutable Model");
        let CompileOutcomeV1::Accepted { model } = execution.response().outcome() else {
            panic!("wire-compatible scalar source must compile")
        };

        assert_eq!(model.exact_codec(), wire);
        assert_eq!(model.schema(), wire.model_schema());
        assert_eq!(model.digest(), document.digest().unwrap());
        assert_eq!(
            execution.response().required_features(),
            [CompileFeatureV1::CompileCheck, wire_feature(wire)]
        );
    }
}

#[test]
fn public_json_boundary_is_closed_and_never_auto_detects() {
    let wrong_protocol = format!(
        r#"{{"protocol":"eqiora.control/v2","command":"{COMPILE_COMMAND_V1}","requestId":"compile-1","requiredFeatures":["{COMPILE_FEATURE_V1}","model-wire/v1"],"modelWire":"v1","filename":"decay.eqi","source":""}}"#
    );
    let diagnostic = CompileRequestV1::from_json(wrong_protocol.as_bytes()).unwrap_err();
    assert_eq!(diagnostic.source(), ControlDiagnosticSourceV1::Control);
    assert_eq!(diagnostic.code(), "EQ0001");

    let unknown_field = format!(
        r#"{{"protocol":"{CONTROL_PROTOCOL_V1}","command":"{COMPILE_COMMAND_V1}","requestId":"compile-1","requiredFeatures":["{COMPILE_FEATURE_V1}","model-wire/v1"],"modelWire":"v1","filename":"decay.eqi","source":"","model":null}}"#
    );
    let diagnostic = CompileRequestV1::from_json(unknown_field.as_bytes()).unwrap_err();
    assert_eq!(diagnostic.source(), ControlDiagnosticSourceV1::Control);
    assert_eq!(diagnostic.code(), "EQ0901");
}

#[test]
fn response_is_deterministic_small_control_metadata_not_scientific_data() {
    let request =
        CompileRequestV1::new_exact("compile-1", ExactModelCodec::V1, "decay.eqi", SOURCE).unwrap();
    let execution = execute_compile_v1(&request);
    let first_json = execution.response().canonical_json().unwrap();
    assert_eq!(first_json, execution.response().canonical_json().unwrap());
    assert_eq!(
        eqiora_api::control::CompileResponseV1::from_json(&first_json).unwrap(),
        *execution.response()
    );

    let response: serde_json::Value = serde_json::from_slice(&first_json).unwrap();
    assert_eq!(response["protocol"], CONTROL_PROTOCOL_V1);
    assert_eq!(response["command"], COMPILE_COMMAND_V1);
    assert_eq!(response["requestId"], "compile-1");
    assert!(response.get("source").is_none());
    assert!(response.get("mesh").is_none());
    assert!(response.get("fields").is_none());
    assert!(response.get("trajectory").is_none());

    let mut forged = response;
    forged["artifactBytes"] = serde_json::json!([]);
    let diagnostic =
        eqiora_api::control::CompileResponseV1::from_json(&serde_json::to_vec(&forged).unwrap())
            .unwrap_err();
    assert_eq!(diagnostic.code(), "EQ0901");
}

fn wire_feature(wire: ExactModelCodec) -> CompileFeatureV1 {
    match wire {
        ExactModelCodec::V1 => CompileFeatureV1::ModelSchemaGeneration1,
        ExactModelCodec::V2 => CompileFeatureV1::ModelSchemaGeneration2,
        ExactModelCodec::V3 => CompileFeatureV1::ModelSchemaGeneration3,
        ExactModelCodec::V4 => CompileFeatureV1::ModelSchemaGeneration4,
        ExactModelCodec::V5 => CompileFeatureV1::ModelSchemaGeneration5,
        ExactModelCodec::V6 => CompileFeatureV1::ModelSchemaGeneration6,
        _ => panic!("test fixture selected an unsupported future Model codec"),
    }
}
