use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{
    CONTROL_PROTOCOL_V1, ControlDiagnosticV1, MAX_COMPILE_FILENAME_BYTES_V1,
    MAX_COMPILE_REQUEST_BYTES_V1, MAX_COMPILE_REQUIRED_FEATURES_V1, MAX_COMPILE_SOURCE_BYTES_V1,
    MAX_CONTROL_REQUEST_ID_BYTES_V1,
};
use crate::ExactModelCodec;

/// Exact command identity of the bounded compile/check slice.
pub const COMPILE_COMMAND_V1: &str = "model.compile-check/v1";

/// Required feature identity for compile/check semantics.
pub const COMPILE_FEATURE_V1: &str = "model.compile-check/v1";

/// Closed required-feature vocabulary for compile/check v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CompileFeatureV1 {
    /// The exact compile/check command semantics.
    #[serde(rename = "model.compile-check/v1")]
    CompileCheck,
    /// Model and transaction wire v1.
    #[serde(rename = "model-wire/v1")]
    ModelSchemaGeneration1,
    /// Model and transaction wire v2.
    #[serde(rename = "model-wire/v2")]
    ModelSchemaGeneration2,
    /// Model and transaction wire v3.
    #[serde(rename = "model-wire/v3")]
    ModelSchemaGeneration3,
    /// Model and transaction wire v4.
    #[serde(rename = "model-wire/v4")]
    ModelSchemaGeneration4,
    /// Model and transaction wire v5.
    #[serde(rename = "model-wire/v5")]
    ModelSchemaGeneration5,
    /// Model and transaction wire v6.
    #[serde(rename = "model-wire/v6")]
    ModelSchemaGeneration6,
    /// Model and transaction wire v7.
    #[serde(rename = "model-wire/v7")]
    ModelSchemaGeneration7,
}

impl CompileFeatureV1 {
    /// Exact negotiated feature identity.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CompileCheck => COMPILE_FEATURE_V1,
            Self::ModelSchemaGeneration1 => "model-wire/v1",
            Self::ModelSchemaGeneration2 => "model-wire/v2",
            Self::ModelSchemaGeneration3 => "model-wire/v3",
            Self::ModelSchemaGeneration4 => "model-wire/v4",
            Self::ModelSchemaGeneration5 => "model-wire/v5",
            Self::ModelSchemaGeneration6 => "model-wire/v6",
            Self::ModelSchemaGeneration7 => "model-wire/v7",
        }
    }

    pub(super) fn parse(value: &str) -> Result<Self, ControlDiagnosticV1> {
        match value {
            COMPILE_FEATURE_V1 => Ok(Self::CompileCheck),
            "model-wire/v1" => Ok(Self::ModelSchemaGeneration1),
            "model-wire/v2" => Ok(Self::ModelSchemaGeneration2),
            "model-wire/v3" => Ok(Self::ModelSchemaGeneration3),
            "model-wire/v4" => Ok(Self::ModelSchemaGeneration4),
            "model-wire/v5" => Ok(Self::ModelSchemaGeneration5),
            "model-wire/v6" => Ok(Self::ModelSchemaGeneration6),
            "model-wire/v7" => Ok(Self::ModelSchemaGeneration7),
            _ => Err(ControlDiagnosticV1::unsupported(format!(
                "required control feature `{value}` is not supported by compile/check v1"
            ))),
        }
    }
}

/// Validated, normalized compile/check request.
///
/// Decoding rejects unknown fields and versions before compilation. Required
/// features are sorted and deduplicated, then checked against the explicitly
/// selected Model wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileRequestV1 {
    request_id: String,
    required_features: Vec<CompileFeatureV1>,
    model_codec: ExactModelCodec,
    filename: String,
    source: String,
}

impl CompileRequestV1 {
    /// Construct an ordinary authoring request for the current vocabulary.
    ///
    /// # Errors
    /// Returns a structured control diagnostic for an invalid request ID,
    /// filename, or source bound.
    pub fn new_current(
        request_id: impl Into<String>,
        filename: impl Into<String>,
        source: impl Into<String>,
    ) -> Result<Self, ControlDiagnosticV1> {
        Self::new_exact(request_id, ExactModelCodec::CURRENT, filename, source)
    }

    /// Construct a compatibility request with one exact artifact codec.
    ///
    /// # Errors
    /// Returns a structured control diagnostic for an invalid request ID,
    /// filename, or source bound.
    pub fn new_exact(
        request_id: impl Into<String>,
        model_codec: ExactModelCodec,
        filename: impl Into<String>,
        source: impl Into<String>,
    ) -> Result<Self, ControlDiagnosticV1> {
        Self::validate_parts(
            request_id.into(),
            vec![CompileFeatureV1::CompileCheck, feature_for(model_codec)],
            model_codec,
            filename.into(),
            source.into(),
        )
    }

    /// Decode one bounded JSON request using this exact protocol generation.
    ///
    /// No protocol sniffing or fallback occurs. Unknown fields, protocol,
    /// command, Model wire, and feature identities fail before compilation.
    ///
    /// # Errors
    /// Returns a structured control diagnostic without constructing or
    /// mutating a [`crate::ModelDocument`].
    pub fn from_json(bytes: &[u8]) -> Result<Self, ControlDiagnosticV1> {
        if bytes.len() > MAX_COMPILE_REQUEST_BYTES_V1 {
            return Err(ControlDiagnosticV1::invalid_request(format!(
                "compile/check request exceeds {MAX_COMPILE_REQUEST_BYTES_V1} encoded bytes"
            )));
        }
        let wire: WireCompileRequestV1 = serde_json::from_slice(bytes).map_err(|error| {
            ControlDiagnosticV1::invalid_request(format!(
                "invalid compile/check v1 request JSON: {error}"
            ))
        })?;
        Self::try_from_wire(wire)
    }

    /// Canonical compact JSON with normalized required features.
    ///
    /// # Errors
    /// Returns a structured control diagnostic only if serialization of this
    /// validated in-memory value unexpectedly fails.
    pub fn canonical_json(&self) -> Result<Vec<u8>, ControlDiagnosticV1> {
        serde_json::to_vec(self).map_err(|error| {
            ControlDiagnosticV1::invalid_request(format!(
                "cannot encode compile/check v1 request: {error}"
            ))
        })
    }

    /// Opaque caller-chosen identity echoed exactly by the response.
    #[must_use]
    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    /// Sorted, duplicate-free required feature set.
    #[must_use]
    pub fn required_features(&self) -> &[CompileFeatureV1] {
        &self.required_features
    }

    /// Explicit immutable Model and transaction wire generation.
    #[must_use]
    pub const fn model_codec(&self) -> ExactModelCodec {
        self.model_codec
    }

    /// Source filename used by compiler diagnostics.
    #[must_use]
    pub fn filename(&self) -> &str {
        &self.filename
    }

    /// Eqiora Language source.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    fn try_from_wire(wire: WireCompileRequestV1) -> Result<Self, ControlDiagnosticV1> {
        if wire.protocol != CONTROL_PROTOCOL_V1 {
            return Err(ControlDiagnosticV1::unsupported(format!(
                "unsupported control protocol `{}`; expected `{CONTROL_PROTOCOL_V1}`",
                wire.protocol
            )));
        }
        if wire.command != COMPILE_COMMAND_V1 {
            return Err(ControlDiagnosticV1::unsupported(format!(
                "unsupported control command `{}`; expected `{COMPILE_COMMAND_V1}`",
                wire.command
            )));
        }
        let model_codec = parse_model_codec(&wire.model_wire)?;
        if wire.required_features.len() > MAX_COMPILE_REQUIRED_FEATURES_V1 {
            return Err(ControlDiagnosticV1::invalid_request(format!(
                "compile/check v1 admits at most {MAX_COMPILE_REQUIRED_FEATURES_V1} required feature entries before normalization"
            )));
        }
        let required_features = wire
            .required_features
            .iter()
            .map(|feature| CompileFeatureV1::parse(feature))
            .collect::<Result<Vec<_>, _>>()?;
        Self::validate_parts(
            wire.request_id,
            required_features,
            model_codec,
            wire.filename,
            wire.source,
        )
    }

    fn validate_parts(
        request_id: String,
        mut required_features: Vec<CompileFeatureV1>,
        model_codec: ExactModelCodec,
        filename: String,
        source: String,
    ) -> Result<Self, ControlDiagnosticV1> {
        validate_request_id(&request_id)?;
        if filename.is_empty()
            || filename.len() > MAX_COMPILE_FILENAME_BYTES_V1
            || filename.chars().any(char::is_control)
        {
            return Err(ControlDiagnosticV1::invalid_request(format!(
                "source filename must contain 1 to {MAX_COMPILE_FILENAME_BYTES_V1} non-control UTF-8 bytes"
            )));
        }
        if source.len() > MAX_COMPILE_SOURCE_BYTES_V1 {
            return Err(ControlDiagnosticV1::invalid_request(format!(
                "source exceeds the {MAX_COMPILE_SOURCE_BYTES_V1}-byte compile/check v1 limit"
            )));
        }

        normalize_and_validate_features(&mut required_features, model_codec)?;

        Ok(Self {
            request_id,
            required_features,
            model_codec,
            filename,
            source,
        })
    }
}

impl Serialize for CompileRequestV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        WireCompileRequestRefV1 {
            protocol: CONTROL_PROTOCOL_V1,
            command: COMPILE_COMMAND_V1,
            request_id: &self.request_id,
            required_features: &self.required_features,
            model_wire: self.model_codec,
            filename: &self.filename,
            source: &self.source,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CompileRequestV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = WireCompileRequestV1::deserialize(deserializer)?;
        Self::try_from_wire(wire).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireCompileRequestV1 {
    protocol: String,
    command: String,
    request_id: String,
    required_features: Vec<String>,
    model_wire: String,
    filename: String,
    source: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WireCompileRequestRefV1<'a> {
    protocol: &'static str,
    command: &'static str,
    request_id: &'a str,
    required_features: &'a [CompileFeatureV1],
    model_wire: ExactModelCodec,
    filename: &'a str,
    source: &'a str,
}

pub(super) fn validate_request_id(request_id: &str) -> Result<(), ControlDiagnosticV1> {
    if request_id.is_empty()
        || request_id.len() > MAX_CONTROL_REQUEST_ID_BYTES_V1
        || !request_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(ControlDiagnosticV1::invalid_request(format!(
            "request ID must contain 1 to {MAX_CONTROL_REQUEST_ID_BYTES_V1} ASCII letters, digits, period, colon, underscore, or hyphen"
        )));
    }
    Ok(())
}

pub(super) fn normalize_and_validate_features(
    required_features: &mut Vec<CompileFeatureV1>,
    model_codec: ExactModelCodec,
) -> Result<(), ControlDiagnosticV1> {
    required_features.sort_unstable();
    required_features.dedup();
    let exact = [CompileFeatureV1::CompileCheck, feature_for(model_codec)];
    if required_features.as_slice() != exact {
        return Err(ControlDiagnosticV1::unsupported(format!(
            "compile/check v1 requires exactly `{}` and `{}` for Model wire `{}`",
            CompileFeatureV1::CompileCheck.as_str(),
            feature_for(model_codec).as_str(),
            model_codec.as_str(),
        )));
    }
    Ok(())
}

pub(super) const fn feature_for(model_codec: ExactModelCodec) -> CompileFeatureV1 {
    match model_codec {
        ExactModelCodec::V1 => CompileFeatureV1::ModelSchemaGeneration1,
        ExactModelCodec::V2 => CompileFeatureV1::ModelSchemaGeneration2,
        ExactModelCodec::V3 => CompileFeatureV1::ModelSchemaGeneration3,
        ExactModelCodec::V4 => CompileFeatureV1::ModelSchemaGeneration4,
        ExactModelCodec::V5 => CompileFeatureV1::ModelSchemaGeneration5,
        ExactModelCodec::V6 => CompileFeatureV1::ModelSchemaGeneration6,
        ExactModelCodec::V7 => CompileFeatureV1::ModelSchemaGeneration7,
    }
}

pub(super) fn parse_model_codec(value: &str) -> Result<ExactModelCodec, ControlDiagnosticV1> {
    match value {
        "v1" => Ok(ExactModelCodec::V1),
        "v2" => Ok(ExactModelCodec::V2),
        "v3" => Ok(ExactModelCodec::V3),
        "v4" => Ok(ExactModelCodec::V4),
        "v5" => Ok(ExactModelCodec::V5),
        "v6" => Ok(ExactModelCodec::V6),
        "v7" => Ok(ExactModelCodec::V7),
        _ => Err(ControlDiagnosticV1::unsupported(format!(
            "unsupported Model wire `{value}` for compile/check v1"
        ))),
    }
}
