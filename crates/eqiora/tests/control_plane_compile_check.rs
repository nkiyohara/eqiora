use eqiora::compatibility::ExactModelCodec;
use eqiora::control::{
    CompileOutcomeV1, CompileRequestV1, ControlDiagnosticSourceV1, execute_compile_v1,
};
use serde::Deserialize;
use serde_json::Value;

const ACCEPTED_REQUEST: &[u8] = include_bytes!(
    "../../../verify/interfaces/control-plane-compile-check/models/accepted-v1.json"
);
const REJECTED_SOURCE_REQUEST: &[u8] = include_bytes!(
    "../../../verify/interfaces/control-plane-compile-check/models/rejected-source-v1.json"
);
const UNSUPPORTED_PROTOCOL_REQUEST: &[u8] = include_bytes!(
    "../../../verify/interfaces/control-plane-compile-check/models/unsupported-protocol-v1.json"
);
const EXPECTED: &str =
    include_str!("../../../verify/interfaces/control-plane-compile-check/expected/contract.json");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExpectedContract {
    schema: String,
    accepted: ExpectedAccepted,
    rejected_source: ExpectedRejected,
    unsupported_protocol: ExpectedControlRejection,
    forbidden_response_fields: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExpectedAccepted {
    request: String,
    request_id: String,
    outcome: String,
    model_wire: String,
    model_schema: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExpectedRejected {
    request: String,
    request_id: String,
    outcome: String,
    diagnostic_source: String,
    diagnostic_code: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExpectedControlRejection {
    request: String,
    diagnostic_source: String,
    diagnostic_code: String,
}

#[test]
fn shared_compile_control_fixture_reaches_one_authoritative_path() {
    let expected: ExpectedContract = serde_json::from_str(EXPECTED).unwrap();
    assert_eq!(
        expected.schema,
        "eqiora.verify.control-plane-compile-check/v1"
    );
    assert_eq!(expected.accepted.request, "accepted-v1.json");

    let request = CompileRequestV1::from_json(ACCEPTED_REQUEST).unwrap();
    let execution = execute_compile_v1(&request);
    assert!(execution.document().is_some());
    let CompileOutcomeV1::Accepted { model } = execution.response().outcome() else {
        panic!("the shared accepted source must produce a Model descriptor")
    };
    assert_eq!(
        execution.response().request_id(),
        expected.accepted.request_id
    );
    assert_eq!("accepted", expected.accepted.outcome);
    assert_eq!(model.exact_codec().as_str(), expected.accepted.model_wire);
    assert_eq!(model.schema(), expected.accepted.model_schema);

    let response: Value =
        serde_json::from_slice(&execution.response().canonical_json().unwrap()).unwrap();
    for field in expected.forbidden_response_fields {
        assert!(
            response.get(&field).is_none(),
            "scientific data field `{field}` entered the control response"
        );
    }
}

#[test]
fn every_immutable_model_wire_is_selected_explicitly_without_fallback() {
    let fixture: Value = serde_json::from_slice(ACCEPTED_REQUEST).unwrap();
    let filename = fixture["filename"].as_str().unwrap();
    let source = fixture["source"].as_str().unwrap();

    for wire in [
        ExactModelCodec::V1,
        ExactModelCodec::V2,
        ExactModelCodec::V3,
        ExactModelCodec::V4,
        ExactModelCodec::V5,
        ExactModelCodec::V6,
        ExactModelCodec::V7,
        ExactModelCodec::V8,
    ] {
        let request = CompileRequestV1::new_exact(
            format!("registered-{}", wire.as_str()),
            wire,
            filename,
            source,
        )
        .unwrap();
        let execution = execute_compile_v1(&request);
        let CompileOutcomeV1::Accepted { model } = execution.response().outcome() else {
            panic!("the scalar fixture must compile through every explicit compatible wire")
        };
        assert_eq!(model.exact_codec(), wire);
        assert_eq!(model.schema(), wire.model_schema());
    }
}

#[test]
fn shared_rejection_fixtures_distinguish_kernel_and_control_failures() {
    let expected: ExpectedContract = serde_json::from_str(EXPECTED).unwrap();
    assert_eq!(expected.rejected_source.request, "rejected-source-v1.json");
    let request = CompileRequestV1::from_json(REJECTED_SOURCE_REQUEST).unwrap();
    let execution = execute_compile_v1(&request);
    assert!(execution.document().is_none());
    let CompileOutcomeV1::Rejected { diagnostics } = execution.response().outcome() else {
        panic!("the empty shared source must be rejected")
    };
    assert_eq!(
        execution.response().request_id(),
        expected.rejected_source.request_id
    );
    assert_eq!("rejected", expected.rejected_source.outcome);
    assert_eq!(diagnostics[0].source(), ControlDiagnosticSourceV1::Kernel);
    assert_eq!("kernel", expected.rejected_source.diagnostic_source);
    assert_eq!(
        diagnostics[0].code(),
        expected.rejected_source.diagnostic_code
    );

    assert_eq!(
        expected.unsupported_protocol.request,
        "unsupported-protocol-v1.json"
    );
    let diagnostic = CompileRequestV1::from_json(UNSUPPORTED_PROTOCOL_REQUEST).unwrap_err();
    assert_eq!(diagnostic.source(), ControlDiagnosticSourceV1::Control);
    assert_eq!("control", expected.unsupported_protocol.diagnostic_source);
    assert_eq!(
        diagnostic.code(),
        expected.unsupported_protocol.diagnostic_code
    );
}
