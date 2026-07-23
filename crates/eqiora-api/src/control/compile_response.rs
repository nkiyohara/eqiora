use serde::{Deserialize, Serialize};

use super::compile::{
    COMPILE_COMMAND_V1, CompileFeatureV1, CompileRequestV1, normalize_and_validate_features,
    parse_model_codec, validate_request_id,
};
use super::{
    CONTROL_PROTOCOL_V1, ControlDiagnosticV1, MAX_COMPILE_REQUIRED_FEATURES_V1,
    MAX_COMPILE_RESPONSE_BYTES_V1,
};
use crate::{ExactModelCodec, ModelDocument};

/// Exact canonical Model identity returned by successful compile/check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompileModelDescriptorV1 {
    #[serde(rename = "wire")]
    model_codec: ExactModelCodec,
    schema: String,
    digest: String,
    model_id: String,
    semantic_revision: u64,
}

impl CompileModelDescriptorV1 {
    /// Explicit Model wire generation.
    #[must_use]
    pub const fn exact_codec(&self) -> ExactModelCodec {
        self.model_codec
    }

    /// Exact immutable Model artifact schema.
    #[must_use]
    pub fn schema(&self) -> &str {
        &self.schema
    }

    /// Domain-separated canonical content digest.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Typed Semantic Model identity.
    #[must_use]
    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    /// Semantic Graph Federation revision serialized by the Model artifact.
    #[must_use]
    pub const fn semantic_revision(&self) -> u64 {
        self.semantic_revision
    }
}

/// Terminal compile/check outcome. Scientific values never enter this DTO.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
pub enum CompileOutcomeV1 {
    /// One canonical Model was compiled through the selected immutable wire.
    Accepted {
        /// Exact typed Model identity.
        model: CompileModelDescriptorV1,
    },
    /// No Model document is admitted.
    Rejected {
        /// One or more structured Eqiora diagnostics.
        diagnostics: Vec<ControlDiagnosticV1>,
    },
}

/// Versioned response echoing the exact request negotiation identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileResponseV1 {
    protocol: String,
    command: String,
    request_id: String,
    required_features: Vec<CompileFeatureV1>,
    #[serde(rename = "modelWire")]
    model_codec: ExactModelCodec,
    outcome: CompileOutcomeV1,
}

impl CompileResponseV1 {
    /// Exact protocol generation.
    #[must_use]
    pub fn protocol(&self) -> &str {
        &self.protocol
    }

    /// Exact command identity.
    #[must_use]
    pub fn command(&self) -> &str {
        &self.command
    }

    /// Request identity echoed without rewriting.
    #[must_use]
    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    /// Normalized feature set used for dispatch.
    #[must_use]
    pub fn required_features(&self) -> &[CompileFeatureV1] {
        &self.required_features
    }

    /// Explicit Model wire selected by the caller.
    #[must_use]
    pub const fn model_codec(&self) -> ExactModelCodec {
        self.model_codec
    }

    /// Terminal typed outcome.
    #[must_use]
    pub const fn outcome(&self) -> &CompileOutcomeV1 {
        &self.outcome
    }

    /// Deterministic compact JSON for a client transport.
    ///
    /// # Errors
    /// Returns a control diagnostic only if serialization unexpectedly fails.
    pub fn canonical_json(&self) -> Result<Vec<u8>, ControlDiagnosticV1> {
        serde_json::to_vec(self).map_err(|error| {
            ControlDiagnosticV1::invalid_request(format!(
                "cannot encode compile/check v1 response: {error}"
            ))
        })
    }

    /// Decode and validate one exact compile/check v1 response.
    ///
    /// # Errors
    /// Returns a structured control diagnostic for oversized data, unknown
    /// fields or versions, malformed diagnostics, or Model identity fields
    /// that contradict the echoed request negotiation.
    pub fn from_json(bytes: &[u8]) -> Result<Self, ControlDiagnosticV1> {
        if bytes.len() > MAX_COMPILE_RESPONSE_BYTES_V1 {
            return Err(ControlDiagnosticV1::invalid_request(format!(
                "compile/check response exceeds {MAX_COMPILE_RESPONSE_BYTES_V1} encoded bytes"
            )));
        }
        let wire: WireCompileResponseV1 = serde_json::from_slice(bytes).map_err(|error| {
            ControlDiagnosticV1::invalid_request(format!(
                "invalid compile/check v1 response JSON: {error}"
            ))
        })?;
        if wire.protocol != CONTROL_PROTOCOL_V1 || wire.command != COMPILE_COMMAND_V1 {
            return Err(ControlDiagnosticV1::unsupported(
                "unsupported compile/check response protocol or command",
            ));
        }
        validate_request_id(&wire.request_id)?;
        let model_codec = parse_model_codec(&wire.model_wire)?;
        if wire.required_features.len() > MAX_COMPILE_REQUIRED_FEATURES_V1 {
            return Err(ControlDiagnosticV1::invalid_request(format!(
                "compile/check v1 admits at most {MAX_COMPILE_REQUIRED_FEATURES_V1} response feature entries"
            )));
        }
        let mut required_features = wire
            .required_features
            .iter()
            .map(|feature| CompileFeatureV1::parse(feature))
            .collect::<Result<Vec<_>, _>>()?;
        normalize_and_validate_features(&mut required_features, model_codec)?;

        match &wire.outcome {
            CompileOutcomeV1::Accepted { model } => validate_model(model, model_codec)?,
            CompileOutcomeV1::Rejected { diagnostics } => {
                if diagnostics.is_empty() {
                    return Err(ControlDiagnosticV1::invalid_request(
                        "rejected compile/check response must contain a diagnostic",
                    ));
                }
                for diagnostic in diagnostics {
                    diagnostic.validate()?;
                }
            }
        }
        Ok(Self {
            protocol: wire.protocol,
            command: wire.command,
            request_id: wire.request_id,
            required_features,
            model_codec,
            outcome: wire.outcome,
        })
    }

    fn rejected(request: &CompileRequestV1, diagnostics: Vec<ControlDiagnosticV1>) -> Self {
        debug_assert!(!diagnostics.is_empty());
        Self {
            protocol: CONTROL_PROTOCOL_V1.to_owned(),
            command: COMPILE_COMMAND_V1.to_owned(),
            request_id: request.request_id().to_owned(),
            required_features: request.required_features().to_vec(),
            model_codec: request.model_codec(),
            outcome: CompileOutcomeV1::Rejected { diagnostics },
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireCompileResponseV1 {
    protocol: String,
    command: String,
    request_id: String,
    required_features: Vec<String>,
    model_wire: String,
    outcome: CompileOutcomeV1,
}

/// Rust application result: transport response plus an optional opaque Model.
///
/// The `ModelDocument` crosses the existing immutable Model/transaction wire;
/// it is not serialized into the small control DTO. A rejected execution can
/// never expose a partially admitted document.
#[derive(Debug, Clone)]
pub struct CompileControlExecutionV1 {
    response: CompileResponseV1,
    document: Option<ModelDocument>,
}

impl CompileControlExecutionV1 {
    /// Transport-neutral command response.
    #[must_use]
    pub const fn response(&self) -> &CompileResponseV1 {
        &self.response
    }

    /// Accepted immutable Model, absent for every rejected response.
    #[must_use]
    pub const fn document(&self) -> Option<&ModelDocument> {
        self.document.as_ref()
    }

    /// Transfer the response and optional opaque Model to an adapter.
    #[must_use]
    pub fn into_parts(self) -> (CompileResponseV1, Option<ModelDocument>) {
        (self.response, self.document)
    }
}

/// Execute one fully validated compile/check request.
///
/// This is the only point in the v1 control slice that enters compilation. It
/// dispatches through the request's [`ExactModelCodec`], which replays the selected
/// immutable `ModelTransactionEnvelopeV1` through `V5` before graph commit.
#[must_use]
pub fn execute_compile_v1(request: &CompileRequestV1) -> CompileControlExecutionV1 {
    let document = match request
        .model_codec()
        .compile(request.filename(), request.source())
    {
        Ok(document) => document,
        Err(diagnostics) => {
            return CompileControlExecutionV1 {
                response: CompileResponseV1::rejected(
                    request,
                    diagnostics.into_iter().map(Into::into).collect(),
                ),
                document: None,
            };
        }
    };

    let reference = match document.artifact_reference() {
        Ok(reference) => reference,
        Err(diagnostic) => {
            return CompileControlExecutionV1 {
                response: CompileResponseV1::rejected(request, vec![diagnostic.into()]),
                document: None,
            };
        }
    };
    let model = CompileModelDescriptorV1 {
        model_codec: request.model_codec(),
        schema: request.model_codec().model_schema().to_owned(),
        digest: reference.artifact().to_string(),
        model_id: reference.model().to_string(),
        semantic_revision: reference.semantic_revision().get(),
    };
    CompileControlExecutionV1 {
        response: CompileResponseV1 {
            protocol: CONTROL_PROTOCOL_V1.to_owned(),
            command: COMPILE_COMMAND_V1.to_owned(),
            request_id: request.request_id().to_owned(),
            required_features: request.required_features().to_vec(),
            model_codec: request.model_codec(),
            outcome: CompileOutcomeV1::Accepted { model },
        },
        document: Some(document),
    }
}

fn validate_model(
    model: &CompileModelDescriptorV1,
    model_codec: ExactModelCodec,
) -> Result<(), ControlDiagnosticV1> {
    let valid_digest = model.digest.len() == 64
        && model
            .digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'));
    if model.model_codec != model_codec
        || model.schema != model_codec.model_schema()
        || !valid_digest
        || model.model_id.is_empty()
        || model.model_id.len() > 128
    {
        return Err(ControlDiagnosticV1::invalid_request(
            "accepted compile/check response contradicts its Model wire or identity",
        ));
    }
    Ok(())
}
