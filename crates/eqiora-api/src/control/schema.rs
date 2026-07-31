use serde_json::{Value, json};

use super::compile::feature_for;
use super::{
    COMPILE_COMMAND_V1, COMPILE_FEATURE_V1, CONTROL_PROTOCOL_V1, MAX_COMPILE_FILENAME_BYTES_V1,
    MAX_COMPILE_REQUEST_BYTES_V1, MAX_COMPILE_REQUIRED_FEATURES_V1, MAX_COMPILE_RESPONSE_BYTES_V1,
    MAX_COMPILE_SOURCE_BYTES_V1, MAX_CONTROL_REQUEST_ID_BYTES_V1,
};
use crate::ExactModelCodec;

/// Committed JSON Schema for the complete compile/check v1 request/response
/// wire.
pub const COMPILE_V1_SCHEMA_JSON: &str =
    include_str!("../../../../schemas/control/compile-v1.schema.json");

const EXACT_MODEL_CODECS: [ExactModelCodec; 8] = [
    ExactModelCodec::V1,
    ExactModelCodec::V2,
    ExactModelCodec::V3,
    ExactModelCodec::V4,
    ExactModelCodec::V5,
    ExactModelCodec::V6,
    ExactModelCodec::V7,
    ExactModelCodec::V8,
];

/// Generate the deterministic committed compile/check v1 JSON Schema.
///
/// Keeping generation beside the Rust DTO constants makes protocol, command,
/// feature, and resource-bound drift a test failure. No runtime schema
/// reflection dependency is required.
///
/// # Errors
/// Returns a JSON serialization error if the in-memory schema document cannot
/// be encoded.
pub fn generated_compile_v1_schema_json() -> Result<String, serde_json::Error> {
    let mut encoded = serde_json::to_string_pretty(&compile_v1_schema_value())?;
    encoded.push('\n');
    Ok(encoded)
}

fn compile_v1_schema_value() -> Value {
    let model_wires = EXACT_MODEL_CODECS.map(ExactModelCodec::as_str);
    let mut features = Vec::with_capacity(EXACT_MODEL_CODECS.len() + 1);
    features.push(COMPILE_FEATURE_V1);
    features.extend(EXACT_MODEL_CODECS.map(|codec| feature_for(codec).as_str()));
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "urn:eqiora:schema:control:compile-v1",
        "title": "Eqiora compile/check control protocol v1",
        "description": "Closed control-plane request and response. Scientific arrays, meshes, Fields, and trajectories are excluded.",
        "oneOf": [
            { "$ref": "#/$defs/request" },
            { "$ref": "#/$defs/response" }
        ],
        "$defs": {
            "requestId": {
                "type": "string",
                "minLength": 1,
                "maxLength": MAX_CONTROL_REQUEST_ID_BYTES_V1,
                "pattern": "^[A-Za-z0-9._:-]+$"
            },
            "modelWire": {
                "type": "string",
                "enum": model_wires
            },
            "requiredFeatures": {
                "type": "array",
                "maxItems": MAX_COMPILE_REQUIRED_FEATURES_V1,
                "items": {
                    "type": "string",
                    "enum": features
                },
                "description": "Input order and duplicates are normalized; an admitted request contains exactly compile/check v1 and its selected Model wire feature."
            },
            "request": {
                "type": "object",
                "additionalProperties": false,
                "x-eqiora-maxEncodedUtf8Bytes": MAX_COMPILE_REQUEST_BYTES_V1,
                "required": [
                    "protocol",
                    "command",
                    "requestId",
                    "requiredFeatures",
                    "modelWire",
                    "filename",
                    "source"
                ],
                "properties": {
                    "protocol": { "const": CONTROL_PROTOCOL_V1 },
                    "command": { "const": COMPILE_COMMAND_V1 },
                    "requestId": { "$ref": "#/$defs/requestId" },
                    "requiredFeatures": { "$ref": "#/$defs/requiredFeatures" },
                    "modelWire": { "$ref": "#/$defs/modelWire" },
                    "filename": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": MAX_COMPILE_FILENAME_BYTES_V1,
                        "x-eqiora-maxUtf8Bytes": MAX_COMPILE_FILENAME_BYTES_V1
                    },
                    "source": {
                        "type": "string",
                        "maxLength": MAX_COMPILE_SOURCE_BYTES_V1,
                        "x-eqiora-maxUtf8Bytes": MAX_COMPILE_SOURCE_BYTES_V1
                    }
                }
            },
            "sourceSpan": {
                "type": "object",
                "additionalProperties": false,
                "required": ["file", "start", "end"],
                "properties": {
                    "file": {
                        "type": "string",
                        "maxLength": MAX_COMPILE_FILENAME_BYTES_V1,
                        "x-eqiora-maxUtf8Bytes": MAX_COMPILE_FILENAME_BYTES_V1
                    },
                    "start": { "type": "integer", "minimum": 0, "maximum": 4294967295_u64 },
                    "end": { "type": "integer", "minimum": 0, "maximum": 4294967295_u64 }
                }
            },
            "patch": {
                "type": "object",
                "additionalProperties": false,
                "required": ["summary"],
                "properties": {
                    "summary": { "type": "string", "minLength": 1 }
                }
            },
            "diagnostic": {
                "type": "object",
                "additionalProperties": false,
                "required": ["source", "severity", "code", "message", "graphPath", "span", "patch"],
                "properties": {
                    "source": { "enum": ["control", "kernel"] },
                    "severity": { "enum": ["error", "warning", "note"] },
                    "code": { "type": "string", "pattern": "^[A-Z]{2}[0-9]{4}$" },
                    "message": { "type": "string", "minLength": 1 },
                    "graphPath": {
                        "oneOf": [
                            { "type": "array", "items": { "type": "string" } },
                            { "type": "null" }
                        ]
                    },
                    "span": {
                        "oneOf": [
                            { "$ref": "#/$defs/sourceSpan" },
                            { "type": "null" }
                        ]
                    },
                    "patch": {
                        "oneOf": [
                            { "$ref": "#/$defs/patch" },
                            { "type": "null" }
                        ]
                    }
                }
            },
            "model": {
                "type": "object",
                "additionalProperties": false,
                "required": ["wire", "schema", "digest", "modelId", "semanticRevision"],
                "properties": {
                    "wire": { "$ref": "#/$defs/modelWire" },
                    "schema": {
                        "enum": [
                            "eqiora.model-envelope/v1",
                            "eqiora.model-envelope/v2",
                            "eqiora.model-envelope/v3",
                            "eqiora.model-envelope/v4",
                            "eqiora.model-envelope/v5",
                            "eqiora.model-envelope/v6"
                        ]
                    },
                    "digest": { "type": "string", "pattern": "^[0-9a-f]{64}$" },
                    "modelId": { "type": "string", "minLength": 1, "maxLength": 128 },
                    "semanticRevision": { "type": "integer", "minimum": 0 }
                }
            },
            "acceptedOutcome": {
                "type": "object",
                "additionalProperties": false,
                "required": ["status", "model"],
                "properties": {
                    "status": { "const": "accepted" },
                    "model": { "$ref": "#/$defs/model" }
                }
            },
            "rejectedOutcome": {
                "type": "object",
                "additionalProperties": false,
                "required": ["status", "diagnostics"],
                "properties": {
                    "status": { "const": "rejected" },
                    "diagnostics": {
                        "type": "array",
                        "minItems": 1,
                        "items": { "$ref": "#/$defs/diagnostic" }
                    }
                }
            },
            "response": {
                "type": "object",
                "additionalProperties": false,
                "x-eqiora-maxEncodedUtf8Bytes": MAX_COMPILE_RESPONSE_BYTES_V1,
                "required": ["protocol", "command", "requestId", "requiredFeatures", "modelWire", "outcome"],
                "properties": {
                    "protocol": { "const": CONTROL_PROTOCOL_V1 },
                    "command": { "const": COMPILE_COMMAND_V1 },
                    "requestId": { "$ref": "#/$defs/requestId" },
                    "requiredFeatures": { "$ref": "#/$defs/requiredFeatures" },
                    "modelWire": { "$ref": "#/$defs/modelWire" },
                    "outcome": {
                        "oneOf": [
                            { "$ref": "#/$defs/acceptedOutcome" },
                            { "$ref": "#/$defs/rejectedOutcome" }
                        ]
                    }
                }
            }
        }
    })
}
