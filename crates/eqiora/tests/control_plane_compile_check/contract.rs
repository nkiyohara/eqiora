use super::*;
use eqiora::api::SemanticFingerprintGeneration;
use eqiora::control::{
    COMPILE_V2_SCHEMA_JSON, MAX_COMPILE_FILENAME_BYTES_V2, MAX_COMPILE_REQUEST_BYTES_V2,
    MAX_COMPILE_RESPONSE_BYTES_V2, MAX_COMPILE_SOURCE_BYTES_V2, MAX_CONTROL_REQUEST_ID_BYTES_V2,
};
use sha2::{Digest, Sha256};

const HISTORICAL_SCHEMA: &[u8] = include_bytes!(
    "../../../../verify/interfaces/control-plane-compile-check/expected/historical/compile-v1.schema.json"
);

pub(super) fn named_fixture(name: &str) -> &'static [u8] {
    match name {
        "accepted-v2.json" => ACCEPTED,
        "rejected-source-v2.json" => REJECTED_SOURCE,
        "retired-v1.json" => RETIRED,
        "unknown-protocol-v2.json" => UNKNOWN_PROTOCOL,
        "unknown-command-v2.json" => UNKNOWN_COMMAND,
        "forbidden-model-wire-v2.json" => FORBIDDEN_MODEL_SELECTION,
        "forbidden-required-features-v2.json" => FORBIDDEN_FEATURE_LIST,
        "compile-v2.schema.json" => COMPILE_V2_SCHEMA_JSON.as_bytes(),
        "compile-v1.schema.json" => HISTORICAL_SCHEMA,
        other => panic!("the frozen contract names unknown fixture `{other}`"),
    }
}

fn raw_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn promoted_contract_is_consumed_by_schema_fixtures_and_runtime_boundaries() {
    let expected = expected();
    let schema: Value = serde_json::from_str(COMPILE_V2_SCHEMA_JSON).unwrap();
    let definitions = &schema["$defs"];
    let top_level_refs = schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .map(|branch| branch["$ref"].as_str().unwrap())
        .collect::<Vec<_>>();
    let top_level_names = top_level_refs
        .iter()
        .map(|reference| reference.rsplit('/').next().unwrap())
        .collect::<Vec<_>>();
    let diagnostic_ref =
        definitions["rejectedOutcome"]["properties"]["diagnostics"]["items"]["$ref"]
            .as_str()
            .unwrap();

    assert_eq!(expected["protocol"], CONTROL_PROTOCOL_V2);
    assert_eq!(expected["command"], COMPILE_COMMAND_V1);
    assert_eq!(expected["controlSchema"]["id"], schema["$id"]);
    assert_eq!(expected["controlSchema"]["dialect"], schema["$schema"]);
    assert_eq!(
        expected["controlSchema"]["topLevelOneOf"],
        serde_json::json!(top_level_names)
    );
    assert_eq!(
        expected["controlSchema"]["standaloneDiagnosticRef"],
        diagnostic_ref
    );
    assert_eq!(
        expected["controlSchema"]["standaloneDiagnosticIsTopLevel"],
        top_level_refs.contains(&diagnostic_ref)
    );
    assert_eq!(
        expected["controlSchema"]["inheritsV1Schema"],
        COMPILE_V2_SCHEMA_JSON.contains("compile-v1")
    );
    assert_eq!(expected["controlSchema"]["inheritsV1Schema"], false);
    for definition in expected["controlSchema"]["closedDefinitions"]
        .as_array()
        .unwrap()
    {
        assert_eq!(
            definitions[definition.as_str().unwrap()]["additionalProperties"],
            false
        );
    }

    for (contract_name, schema_name) in [
        ("request", "request"),
        ("response", "response"),
        ("acceptedOutcome", "acceptedOutcome"),
        ("rejectedOutcome", "rejectedOutcome"),
        ("model", "model"),
        ("diagnostic", "diagnostic"),
        ("sourceSpan", "sourceSpan"),
        ("patch", "patch"),
    ] {
        assert_eq!(
            expected["memberOrder"][contract_name],
            definitions[schema_name]["required"]
        );
    }
    for member in expected["removedMembers"].as_array().unwrap() {
        assert!(!COMPILE_V2_SCHEMA_JSON.contains(member.as_str().unwrap()));
    }
    assert!(
        !COMPILE_V2_SCHEMA_JSON.contains(expected["removedFeatureNamespace"].as_str().unwrap())
    );

    let request_bounds = &expected["requestResourceBounds"];
    assert_eq!(
        request_bounds["encodedRequestMaxUtf8Bytes"],
        MAX_COMPILE_REQUEST_BYTES_V2
    );
    assert_eq!(
        request_bounds["sourceMaxUtf8Bytes"],
        MAX_COMPILE_SOURCE_BYTES_V2
    );
    assert_eq!(
        request_bounds["filenameMaxUtf8Bytes"],
        MAX_COMPILE_FILENAME_BYTES_V2
    );
    assert_eq!(
        request_bounds["requestIdMaxCharacters"],
        MAX_CONTROL_REQUEST_ID_BYTES_V2
    );
    assert_eq!(
        request_bounds["requestIdPattern"],
        definitions["requestId"]["pattern"]
    );
    assert_eq!(definitions["requestId"]["pattern"], "^[A-Za-z0-9._:-]+$");
    for byte in 0_u8..=127 {
        let character = char::from(byte);
        let runtime_accepts = CompileRequestV2::new(character.to_string(), "x.eqi", "").is_ok();
        let pattern_accepts = byte.is_ascii_alphanumeric() || b"._:-".contains(&byte);
        assert_eq!(runtime_accepts, pattern_accepts, "request ID byte {byte}");
    }
    for request_id in ["é", "識別子", "full＿width"] {
        assert!(CompileRequestV2::new(request_id, "x.eqi", "").is_err());
    }
    assert_eq!(
        request_bounds["filenameAdmitsControlCharacters"],
        CompileRequestV2::new("request", "bad\nname.eqi", "").is_ok()
    );
    assert_eq!(request_bounds["boundExhaustionCode"], "EQ0901");

    let response_bounds = &expected["responseDiagnosticBounds"];
    assert_eq!(
        response_bounds["responseMaxEncodedUtf8Bytes"],
        MAX_COMPILE_RESPONSE_BYTES_V2
    );
    assert_eq!(
        response_bounds["diagnosticsMaxItems"],
        definitions["rejectedOutcome"]["properties"]["diagnostics"]["maxItems"]
    );
    assert_eq!(
        response_bounds["messageMaxUtf8Bytes"],
        definitions["diagnostic"]["properties"]["message"]["x-eqiora-maxUtf8Bytes"]
    );
    assert_eq!(
        response_bounds["graphPathMaxSegments"],
        definitions["diagnostic"]["properties"]["graphPath"]["oneOf"][0]["maxItems"]
    );

    for (name, record) in expected["fixtureDigests"].as_object().unwrap() {
        let bytes = named_fixture(name);
        assert_eq!(record["bytes"], bytes.len());
        assert_eq!(record["sha256"], raw_sha256(bytes));
    }
    let witness = &expected["witnessSource"];
    for name in witness["sharedBy"].as_array().unwrap() {
        let request: Value = serde_json::from_slice(named_fixture(name.as_str().unwrap())).unwrap();
        let source = request["source"].as_str().unwrap().as_bytes();
        assert_eq!(witness["bytes"], source.len());
        assert_eq!(witness["sha256"], raw_sha256(source));
        assert_eq!(witness["trailingLineFeed"], source.ends_with(b"\n"));
    }

    let historical = &expected["historicalCopies"];
    assert_eq!(
        historical["retired-v1.json"]["copiedFrom"],
        "verify/interfaces/control-plane-compile-check/models/accepted-v1.json"
    );
    assert_eq!(historical["retired-v1.json"]["byteForByte"], true);
    assert_eq!(
        historical["compile-v1.schema.json"]["copiedFrom"],
        "schemas/control/compile-v1.schema.json"
    );
    assert_eq!(historical["compile-v1.schema.json"]["byteForByte"], true);
    let historical_schema: Value = serde_json::from_slice(HISTORICAL_SCHEMA).unwrap();
    assert_eq!(
        historical["compile-v1.schema.json"]["id"],
        historical_schema["$id"]
    );
    assert_eq!(historical["compile-v1.schema.json"]["generated"], false);
    assert_eq!(historical["compile-v1.schema.json"]["registered"], false);
    assert_eq!(historical["compile-v1.schema.json"]["packaged"], false);
    assert_eq!(historical["compile-v1.schema.json"]["dispatched"], false);

    assert_eq!(
        expected["stagePrecedence"],
        serde_json::json!(["dispatch-prelude", "dto-admission", "compilation"])
    );
    assert_eq!(
        expected["dispatchPrelude"]["members"],
        serde_json::json!(["protocol", "command"])
    );
    assert_eq!(expected["dispatchPrelude"]["precedesDtoAdmission"], true);
    assert_eq!(expected["dispatchPrelude"]["retriesOrReinterprets"], false);
    assert_eq!(
        expected["dispatchPrelude"]["wrappedInSyntheticResponse"],
        false
    );
    assert_eq!(expected["dispatchPrelude"]["carriesRequestId"], false);
    assert_eq!(
        expected["generatedResourceBoundaries"]
            .as_array()
            .unwrap()
            .len(),
        4
    );

    assert_eq!(
        expected["sameExecutionLinkage"]["invocation"],
        "execute_compile_v2"
    );
    assert_eq!(expected["sameExecutionLinkage"]["documentPresent"], true);
    assert_eq!(expected["sameExecutionLinkage"]["echoesRequestId"], true);
    assert_eq!(
        expected["structuralRelation"]["compilations"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        expected["structuralRelation"]["fingerprintGeneration"],
        SemanticFingerprintGeneration::V3.as_str()
    );
}
