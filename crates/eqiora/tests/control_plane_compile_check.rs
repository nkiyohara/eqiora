use eqiora::api::ModelDocument;
use eqiora::control::{
    CompileOutcomeV2, CompileRequestV2, ControlDiagnosticSourceV2, execute_compile_v2,
};
use serde_json::Value;

macro_rules! fixture {
    ($name:literal) => {
        include_bytes!(concat!(
            "../../../verify/interfaces/control-plane-compile-check/models/",
            $name
        ))
    };
}

const ACCEPTED: &[u8] = fixture!("accepted-v2.json");
const REJECTED_SOURCE: &[u8] = fixture!("rejected-source-v2.json");
const RETIRED: &[u8] = fixture!("retired-v1.json");
const UNKNOWN_PROTOCOL: &[u8] = fixture!("unknown-protocol-v2.json");
const UNKNOWN_COMMAND: &[u8] = fixture!("unknown-command-v2.json");
const FORBIDDEN_MODEL_SELECTION: &[u8] = fixture!("forbidden-model-wire-v2.json");
const FORBIDDEN_FEATURE_LIST: &[u8] = fixture!("forbidden-required-features-v2.json");
const EXPECTED: &str =
    include_str!("../../../verify/interfaces/control-plane-compile-check/expected/contract.json");

fn expected() -> Value {
    serde_json::from_str(EXPECTED).unwrap()
}

#[test]
fn accepted_fixture_links_one_execution_and_preserves_structural_meaning() {
    let expected = expected();
    assert_eq!(
        expected["schema"],
        "eqiora.verify.control-plane-compile-check/v2"
    );
    let request = CompileRequestV2::from_json(ACCEPTED).unwrap();
    let first = execute_compile_v2(&request);
    let document = first.document().expect("accepted document");
    let CompileOutcomeV2::Accepted { model } = first.response().outcome() else {
        panic!("accepted fixture must produce a Model descriptor")
    };
    let reference = document.artifact_reference().unwrap();
    assert_eq!(first.response().request_id(), "shared-accepted-v2");
    assert_eq!(model.schema(), "eqiora.model-envelope/v8");
    assert_eq!(
        model.transaction_schema(),
        "eqiora.model-transaction-envelope/v8"
    );
    assert_eq!(model.digest(), reference.artifact().as_str());
    assert_eq!(model.model_id(), reference.model().to_string());
    assert_eq!(model.semantic_revision(), 1);

    let second = execute_compile_v2(&request);
    let second_document = second.document().unwrap();
    let ordinary = ModelDocument::compile(request.filename(), request.source()).unwrap();
    let identities = [document, second_document, &ordinary]
        .map(|value| (value.digest().unwrap(), value.program().model()));
    assert!(identities[0].0 != identities[1].0 && identities[1].0 != identities[2].0);
    assert!(identities[0].1 != identities[1].1 && identities[1].1 != identities[2].1);
    assert_eq!(
        document.structural_fingerprint().unwrap(),
        second_document.structural_fingerprint().unwrap()
    );
    assert_eq!(
        document.structural_fingerprint().unwrap(),
        ordinary.structural_fingerprint().unwrap()
    );

    let response: Value =
        serde_json::from_slice(&first.response().canonical_json().unwrap()).unwrap();
    for field in expected["forbiddenResponseFields"].as_array().unwrap() {
        let field = field.as_str().unwrap();
        assert!(
            response.get(field).is_none(),
            "`{field}` entered control data"
        );
    }
}

#[test]
fn admitted_source_failure_is_a_closed_response_without_a_document() {
    let request = CompileRequestV2::from_json(REJECTED_SOURCE).unwrap();
    let execution = execute_compile_v2(&request);
    assert!(execution.document().is_none());
    let CompileOutcomeV2::Rejected { diagnostics } = execution.response().outcome() else {
        panic!("empty source must be rejected")
    };
    assert_eq!(
        execution.response().request_id(),
        "shared-rejected-source-v2"
    );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].source(), ControlDiagnosticSourceV2::Kernel);
    assert_eq!(diagnostics[0].code(), "EQ0602");
}

#[test]
fn dispatch_rejections_precede_closed_dto_admission() {
    for (bytes, message) in [
        (
            RETIRED,
            "unsupported control protocol `eqiora.control/v1`; expected `eqiora.control/v2`",
        ),
        (
            UNKNOWN_PROTOCOL,
            "unsupported control protocol `eqiora.control/unknown-test`; expected `eqiora.control/v2`",
        ),
        (
            UNKNOWN_COMMAND,
            "unsupported control command `model.unknown-test`; expected `model.compile-check/v1`",
        ),
    ] {
        let diagnostic = CompileRequestV2::from_json(bytes).unwrap_err();
        assert_eq!(diagnostic.source(), ControlDiagnosticSourceV2::Control);
        assert_eq!(diagnostic.code(), "EQ0001");
        assert_eq!(diagnostic.message(), message);
        assert!(diagnostic.graph_path().is_none());
        assert!(diagnostic.span().is_none());
        assert!(diagnostic.patch().is_none());
    }
    for bytes in [FORBIDDEN_MODEL_SELECTION, FORBIDDEN_FEATURE_LIST] {
        let diagnostic = CompileRequestV2::from_json(bytes).unwrap_err();
        assert_eq!(diagnostic.source(), ControlDiagnosticSourceV2::Control);
        assert_eq!(diagnostic.code(), "EQ0901");
        assert!(!diagnostic.message().is_empty());
    }
}

#[test]
fn generated_request_resource_falsifiers_fail_closed() {
    let accepted: Value = serde_json::from_slice(ACCEPTED).unwrap();
    for (member, value) in [
        ("source", "x".repeat(8 * 1_024 * 1_024 + 1)),
        ("filename", "x".repeat(4_097)),
        ("requestId", "x".repeat(129)),
    ] {
        let mut mutant = accepted.clone();
        mutant[member] = Value::String(value);
        assert_eq!(
            CompileRequestV2::from_json(&serde_json::to_vec(&mutant).unwrap())
                .unwrap_err()
                .code(),
            "EQ0901"
        );
    }

    let mut padded = serde_json::to_vec(&accepted).unwrap();
    padded.resize(8 * 1_024 * 1_024 + 16 * 1_024 + 1, b' ');
    assert_eq!(
        CompileRequestV2::from_json(&padded).unwrap_err().code(),
        "EQ0901"
    );
}
