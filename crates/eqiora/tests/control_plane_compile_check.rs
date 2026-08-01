use eqiora::api::ModelDocument;
use eqiora::control::{
    COMPILE_COMMAND_V1, CONTROL_PROTOCOL_V2, CompileOutcomeV2, CompileRequestV2, CompileResponseV2,
    ControlDiagnosticSourceV2, execute_compile_v2,
};
use serde_json::Value;

#[path = "control_plane_compile_check/contract.rs"]
mod contract;

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

fn rejection<'a>(expected: &'a Value, name: &str) -> &'a Value {
    expected["rejections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["name"] == name)
        .unwrap_or_else(|| panic!("the frozen contract omits rejection `{name}`"))
}

#[test]
fn accepted_fixture_links_one_execution_and_preserves_structural_meaning() {
    let expected = expected();
    let accepted = &expected["accepted"];
    let linkage = &expected["sameExecutionLinkage"];
    let relation = &expected["structuralRelation"];
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
    assert_eq!(first.response().request_id(), accepted["requestId"]);
    assert_eq!(model.schema(), accepted["modelSchema"]);
    assert_eq!(
        model.transaction_schema(),
        accepted["modelTransactionSchema"]
    );
    assert_eq!(model.digest(), reference.artifact().as_str());
    assert_eq!(model.model_id(), reference.model().to_string());
    assert_eq!(model.semantic_revision(), accepted["semanticRevision"]);
    assert_eq!(linkage["request"], accepted["request"]);
    assert_eq!(linkage["documentPresent"], first.document().is_some());
    assert_eq!(linkage["echoesRequestId"], true);

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
    assert_eq!(relation["request"], accepted["request"]);
    assert_eq!(relation["semanticRevision"], model.semantic_revision());
    assert_eq!(
        relation["pairwiseDistinctFields"],
        serde_json::json!(["modelId", "digest"])
    );
    assert_eq!(
        relation["equalFields"],
        serde_json::json!(["structuralSemanticFingerprint"])
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
    let expected = expected();
    let frozen = rejection(&expected, "rejected-source");
    let request = CompileRequestV2::from_json(REJECTED_SOURCE).unwrap();
    let execution = execute_compile_v2(&request);
    assert!(execution.document().is_none());
    let CompileOutcomeV2::Rejected { diagnostics } = execution.response().outcome() else {
        panic!("empty source must be rejected")
    };
    assert_eq!(execution.response().request_id(), frozen["requestId"]);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(frozen["stage"], "compilation");
    assert_eq!(frozen["documentPresent"], execution.document().is_some());
    assert_eq!(frozen["diagnosticSource"], "kernel");
    assert_eq!(diagnostics[0].source(), ControlDiagnosticSourceV2::Kernel);
    assert_eq!(diagnostics[0].code(), frozen["diagnosticCode"]);
    assert_eq!(
        frozen["messageNonempty"],
        !diagnostics[0].message().is_empty()
    );
}

#[test]
fn dispatch_rejections_precede_closed_dto_admission() {
    let expected = expected();
    for (name, bytes) in [
        ("retired-protocol", RETIRED),
        ("unknown-protocol", UNKNOWN_PROTOCOL),
        ("unknown-command", UNKNOWN_COMMAND),
    ] {
        let frozen = rejection(&expected, name);
        let diagnostic = CompileRequestV2::from_json(bytes).unwrap_err();
        assert_eq!(frozen["stage"], "dispatch-prelude");
        assert_eq!(frozen["standaloneDiagnostic"], true);
        assert_eq!(diagnostic.source(), ControlDiagnosticSourceV2::Control);
        assert_eq!(diagnostic.code(), frozen["diagnosticCode"]);
        assert_eq!(diagnostic.message(), frozen["message"]);
        let value = serde_json::to_value(&diagnostic).unwrap();
        for field in expected["dispatcherNullDiagnosticFields"]
            .as_array()
            .unwrap()
        {
            assert!(value[field.as_str().unwrap()].is_null());
        }
    }
    for (name, bytes) in [
        ("forbidden-model-selection", FORBIDDEN_MODEL_SELECTION),
        ("forbidden-feature-list", FORBIDDEN_FEATURE_LIST),
    ] {
        let frozen = rejection(&expected, name);
        let diagnostic = CompileRequestV2::from_json(bytes).unwrap_err();
        assert_eq!(frozen["stage"], "dto-admission");
        assert_eq!(diagnostic.source(), ControlDiagnosticSourceV2::Control);
        assert_eq!(diagnostic.code(), frozen["diagnosticCode"]);
        assert!(!diagnostic.message().is_empty());
    }
    for witness in expected["precedenceWitnesses"].as_array().unwrap() {
        let diagnostic = CompileRequestV2::from_json(contract::named_fixture(
            witness["request"].as_str().unwrap(),
        ))
        .unwrap_err();
        assert_eq!(witness["expectedStage"], "dispatch-prelude");
        assert_eq!(diagnostic.code(), witness["expectedCode"]);
    }
}

#[test]
fn generated_request_resource_falsifiers_fail_closed() {
    let expected = expected();
    let accepted: Value = serde_json::from_slice(ACCEPTED).unwrap();
    for boundary in expected["generatedResourceBoundaries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|boundary| boundary["member"] != "request")
    {
        let member = boundary["member"].as_str().unwrap();
        let mut mutant = accepted.clone();
        mutant[member] =
            Value::String("x".repeat(boundary["falsifier"].as_u64().unwrap() as usize));
        assert_eq!(
            CompileRequestV2::from_json(&serde_json::to_vec(&mutant).unwrap())
                .unwrap_err()
                .code(),
            boundary["diagnosticCode"]
        );
    }

    let encoded = expected["generatedResourceBoundaries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|boundary| boundary["member"] == "request")
        .unwrap();
    let mut padded = serde_json::to_vec(&accepted).unwrap();
    padded.resize(encoded["falsifier"].as_u64().unwrap() as usize, b' ');
    assert_eq!(
        CompileRequestV2::from_json(&padded).unwrap_err().code(),
        encoded["diagnosticCode"]
    );
}

#[test]
fn dispatch_prelude_boundaries_are_generated_from_the_frozen_contract() {
    let expected = expected();
    let prelude = &expected["dispatchPrelude"];
    let marker = prelude["contentMarker"].as_str().unwrap();
    for falsifier in prelude["overflow"]["falsifiers"].as_array().unwrap() {
        let characters = falsifier["characters"].as_u64().unwrap() as usize;
        let utf8_bytes = falsifier["utf8Bytes"].as_u64().unwrap() as usize;
        let marker_characters = marker.chars().count();
        let mut value = if utf8_bytes == characters {
            "x".repeat(characters - marker_characters)
        } else {
            let mut value = "é".to_owned();
            value.push_str(&"x".repeat(characters - marker_characters - 1));
            value
        };
        value.push_str(marker);
        assert_eq!(value.chars().count(), characters);
        assert_eq!(value.len(), utf8_bytes);

        let mut request = serde_json::json!({
            "protocol": CONTROL_PROTOCOL_V2,
            "command": COMPILE_COMMAND_V1,
        });
        request[falsifier["member"].as_str().unwrap()] = Value::String(value);
        let diagnostic =
            CompileRequestV2::from_json(&serde_json::to_vec(&request).unwrap()).unwrap_err();
        assert_eq!(diagnostic.code(), prelude["overflow"]["diagnosticCode"]);
        assert!(!diagnostic.message().contains(marker));
    }

    let at_bound = &prelude["atBound"];
    let value = "x".repeat(at_bound["characters"].as_u64().unwrap() as usize);
    let request = serde_json::json!({
        "protocol": value,
        "command": COMPILE_COMMAND_V1,
    });
    let diagnostic =
        CompileRequestV2::from_json(&serde_json::to_vec(&request).unwrap()).unwrap_err();
    assert_eq!(diagnostic.code(), at_bound["diagnosticCode"]);
}

#[test]
fn frozen_diagnostic_overflow_is_one_closed_decodable_response() {
    let expected = expected();
    let overflow = &expected["diagnosticOverflow"];
    let request_id = "overflow-contract";
    let bytes = serde_json::to_vec(&serde_json::json!({
        "protocol": CONTROL_PROTOCOL_V2,
        "command": COMPILE_COMMAND_V1,
        "requestId": request_id,
        "outcome": {
            "status": overflow["outcome"],
            "diagnostics": [{
                "source": overflow["diagnosticSource"],
                "severity": overflow["diagnosticSeverity"],
                "code": overflow["diagnosticCode"],
                "message": overflow["message"],
                "graphPath": null,
                "span": null,
                "patch": null,
            }],
        },
    }))
    .unwrap();
    let response = CompileResponseV2::from_json(&bytes).unwrap();
    assert_eq!(response.request_id(), request_id);
    let CompileOutcomeV2::Rejected { diagnostics } = response.outcome() else {
        panic!("the frozen overflow cannot expose a Model descriptor")
    };
    assert_eq!(diagnostics.len(), overflow["diagnosticCount"]);
    assert_eq!(diagnostics[0].code(), overflow["diagnosticCode"]);
    assert_eq!(diagnostics[0].message(), overflow["message"]);
    let value = serde_json::to_value(&diagnostics[0]).unwrap();
    for field in expected["dispatcherNullDiagnosticFields"]
        .as_array()
        .unwrap()
    {
        assert!(value[field.as_str().unwrap()].is_null());
    }
    assert_eq!(overflow["partialKernelDiagnosticSerialized"], false);
    assert_eq!(overflow["truncated"], false);
    assert_eq!(overflow["echoesRequestId"], true);
    assert_eq!(overflow["modelDescriptorPresent"], false);
    assert_eq!(overflow["documentPresent"], false);
    assert_eq!(
        overflow["triggers"],
        serde_json::json!([
            "message",
            "graphPath",
            "patchSummary",
            "diagnosticCount",
            "response"
        ])
    );
}
