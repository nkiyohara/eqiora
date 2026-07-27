//! Which exact generation an artifact is written and replayed in.
//!
//! Selection is always explicit. A decoder never guesses a generation from
//! bytes, and an older one never retries a newer artifact under a reading it
//! cannot justify, so a Model that names meaning a decoder cannot represent is
//! refused rather than approximated.

use super::*;

/// Exact historical Model/Transaction artifact codec.
///
/// Ordinary authoring uses [`ModelDocument::compile`] or
/// [`ModelDocument::define`] and does not choose this value. Artifact replay,
/// compatibility tests, and conformance tools select one codec explicitly;
/// Eqiora never guesses from bytes or retries another generation.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ExactModelCodec {
    /// The original scalar model and transaction vocabulary.
    #[serde(rename = "v1")]
    V1,
    /// Scalar physical Domains, Ports, and across/through symbols.
    #[serde(rename = "v2")]
    V2,
    /// Exact shaped Fields and field-valued boundary physical interfaces.
    #[serde(rename = "v3")]
    V3,
    /// Canonical symmetric-part and isotropic-lift tensor operators.
    #[serde(rename = "v4")]
    V4,
    /// Expression-local, content-addressed canonical pure operators.
    #[serde(rename = "v5")]
    V5,
    /// Spatial-periodic boundary Connections.
    #[serde(rename = "v6")]
    V6,
    /// Domains that name an authored geometry by digest and entity set.
    #[serde(rename = "v7")]
    V7,
}

impl ExactModelCodec {
    /// Codec currently used by the ordinary authoring profile.
    ///
    /// This mapping is not a stability promise for the current semantic
    /// vocabulary. Exact artifacts retain the codec selected when authored.
    pub const CURRENT: Self = Self::V7;

    /// Exact generation spelling used by compatibility protocols.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V1 => "v1",
            Self::V2 => "v2",
            Self::V3 => "v3",
            Self::V4 => "v4",
            Self::V5 => "v5",
            Self::V6 => "v6",
            Self::V7 => "v7",
        }
    }

    /// Immutable artifact schema owned by this codec.
    #[must_use]
    pub const fn model_schema(self) -> &'static str {
        self.artifact_generation().schema()
    }

    const fn artifact_generation(self) -> ModelArtifactGeneration {
        match self {
            Self::V1 => ModelArtifactGeneration::V1,
            Self::V2 => ModelArtifactGeneration::V2,
            Self::V3 => ModelArtifactGeneration::V3,
            Self::V4 => ModelArtifactGeneration::V4,
            Self::V5 => ModelArtifactGeneration::V5,
            Self::V6 => ModelArtifactGeneration::V6,
            Self::V7 => ModelArtifactGeneration::V7,
        }
    }

    /// Compile source through exactly this historical Model/Transaction codec.
    ///
    /// # Errors
    /// Returns compiler, semantic, or artifact diagnostics without trying a
    /// different codec.
    pub fn compile(self, filename: &str, source: &str) -> Result<ModelDocument, Vec<Diagnostic>> {
        ModelDocument::compile_for_codec(filename, source, self)
    }

    /// Define a native draft through exactly this historical codec.
    ///
    /// # Errors
    /// Returns graph-path, semantic, or artifact diagnostics without trying a
    /// different codec.
    pub fn define(self, draft: &ModelDraft) -> Result<ModelDocument, Vec<Diagnostic>> {
        ModelDocument::define_for_codec(draft, self)
    }

    /// Replay bytes using exactly this historical Model decoder.
    ///
    /// # Errors
    /// Returns artifact or semantic diagnostics when the bytes do not belong
    /// to this codec. No sniffing or fallback occurs.
    pub fn replay(self, data: &[u8]) -> Result<ModelDocument, Vec<Diagnostic>> {
        ModelDocument::replay_codec(data, self)
    }

    /// Whether this wire admits nominal scalar physical semantics.
    #[must_use]
    pub const fn supports_scalar_physical(self) -> bool {
        matches!(self, Self::V2 | Self::V3 | Self::V4 | Self::V5 | Self::V6)
    }

    /// Whether this wire admits exact shaped values and field-valued boundary
    /// physical interfaces.
    #[must_use]
    pub const fn supports_boundary_physical(self) -> bool {
        matches!(self, Self::V3 | Self::V4 | Self::V5 | Self::V6)
    }

    /// Whether this wire admits the closed canonical tensor-operator
    /// vocabulary.
    #[must_use]
    pub const fn supports_tensor_operators(self) -> bool {
        matches!(self, Self::V4 | Self::V5 | Self::V6)
    }

    /// Whether this wire admits expression-local content-addressed pure operators.
    #[must_use]
    pub const fn supports_pure_operators(self) -> bool {
        matches!(self, Self::V5 | Self::V6)
    }

    /// Whether this wire admits spatial-periodic boundary Connections.
    #[must_use]
    pub const fn supports_spatial_periodic(self) -> bool {
        matches!(self, Self::V6)
    }

    pub(crate) fn replay_transaction(
        self,
        transaction: &Transaction,
    ) -> Result<Transaction, Diagnostic> {
        let envelope = self.encode_transaction(transaction)?;
        let bytes = envelope.canonical_json()?;
        self.decode_transaction(&bytes)?.to_transaction()
    }

    pub(crate) fn encode_transaction(
        self,
        transaction: &Transaction,
    ) -> Result<VersionedModelTransactionEnvelope, Diagnostic> {
        match self {
            Self::V1 => ModelTransactionEnvelopeV1::from_transaction(transaction)
                .map(VersionedModelTransactionEnvelope::V1),
            Self::V2 => ModelTransactionEnvelopeV2::from_transaction(transaction)
                .map(VersionedModelTransactionEnvelope::V2),
            Self::V3 => ModelTransactionEnvelopeV3::from_transaction(transaction)
                .map(VersionedModelTransactionEnvelope::V3),
            Self::V4 => ModelTransactionEnvelopeV4::from_transaction(transaction)
                .map(VersionedModelTransactionEnvelope::V4),
            Self::V5 => ModelTransactionEnvelopeV5::from_transaction(transaction)
                .map(VersionedModelTransactionEnvelope::V5),
            Self::V6 => ModelTransactionEnvelopeV6::from_transaction(transaction)
                .map(VersionedModelTransactionEnvelope::V6),
            Self::V7 => ModelTransactionEnvelopeV7::from_transaction(transaction)
                .map(VersionedModelTransactionEnvelope::V7),
        }
    }

    pub(crate) fn decode_transaction(
        self,
        bytes: &[u8],
    ) -> Result<VersionedModelTransactionEnvelope, Diagnostic> {
        match self {
            Self::V1 => ModelTransactionEnvelopeV1::from_json(bytes, ModelDecoderLimits::default())
                .map(VersionedModelTransactionEnvelope::V1),
            Self::V2 => ModelTransactionEnvelopeV2::from_json(bytes, ModelDecoderLimits::default())
                .map(VersionedModelTransactionEnvelope::V2),
            Self::V3 => ModelTransactionEnvelopeV3::from_json(bytes, ModelDecoderLimits::default())
                .map(VersionedModelTransactionEnvelope::V3),
            Self::V4 => ModelTransactionEnvelopeV4::from_json(bytes, ModelDecoderLimits::default())
                .map(VersionedModelTransactionEnvelope::V4),
            Self::V5 => ModelTransactionEnvelopeV5::from_json(bytes, ModelDecoderLimits::default())
                .map(VersionedModelTransactionEnvelope::V5),
            Self::V6 => ModelTransactionEnvelopeV6::from_json(bytes, ModelDecoderLimits::default())
                .map(VersionedModelTransactionEnvelope::V6),
            Self::V7 => ModelTransactionEnvelopeV7::from_json(bytes, ModelDecoderLimits::default())
                .map(VersionedModelTransactionEnvelope::V7),
        }
    }

    pub(crate) fn encode_program(
        self,
        program: &KernelProgram,
    ) -> Result<AcceptedModelArtifact, Diagnostic> {
        AcceptedModelArtifact::from_program(self.artifact_generation(), program)
    }

    pub(crate) fn decode_model(self, bytes: &[u8]) -> Result<AcceptedModelArtifact, Diagnostic> {
        AcceptedModelArtifact::from_json(
            self.artifact_generation(),
            bytes,
            ModelDecoderLimits::default(),
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum VersionedModelTransactionEnvelope {
    V1(ModelTransactionEnvelopeV1),
    V2(ModelTransactionEnvelopeV2),
    V3(ModelTransactionEnvelopeV3),
    V4(ModelTransactionEnvelopeV4),
    V5(ModelTransactionEnvelopeV5),
    V6(ModelTransactionEnvelopeV6),
    V7(ModelTransactionEnvelopeV7),
}

impl VersionedModelTransactionEnvelope {
    pub(crate) fn canonical_json(&self) -> Result<Vec<u8>, Diagnostic> {
        match self {
            Self::V1(envelope) => envelope.canonical_json(),
            Self::V2(envelope) => envelope.canonical_json(),
            Self::V3(envelope) => envelope.canonical_json(),
            Self::V4(envelope) => envelope.canonical_json(),
            Self::V5(envelope) => envelope.canonical_json(),
            Self::V6(envelope) => envelope.canonical_json(),
            Self::V7(envelope) => envelope.canonical_json(),
        }
    }

    pub(crate) fn digest(&self) -> Result<String, Diagnostic> {
        match self {
            Self::V1(envelope) => envelope.digest(),
            Self::V2(envelope) => envelope.digest(),
            Self::V3(envelope) => envelope.digest(),
            Self::V4(envelope) => envelope.digest(),
            Self::V5(envelope) => envelope.digest(),
            Self::V6(envelope) => envelope.digest(),
            Self::V7(envelope) => envelope.digest(),
        }
        .map(|digest| digest.to_string())
    }

    pub(crate) fn to_transaction(&self) -> Result<Transaction, Diagnostic> {
        match self {
            Self::V1(envelope) => envelope.to_transaction(),
            Self::V2(envelope) => envelope.to_transaction(),
            Self::V3(envelope) => envelope.to_transaction(),
            Self::V4(envelope) => envelope.to_transaction(),
            Self::V5(envelope) => envelope.to_transaction(),
            Self::V6(envelope) => envelope.to_transaction(),
            Self::V7(envelope) => envelope.to_transaction(),
        }
    }

    pub(crate) const fn exact_codec(&self) -> ExactModelCodec {
        match self {
            Self::V1(_) => ExactModelCodec::V1,
            Self::V2(_) => ExactModelCodec::V2,
            Self::V3(_) => ExactModelCodec::V3,
            Self::V4(_) => ExactModelCodec::V4,
            Self::V5(_) => ExactModelCodec::V5,
            Self::V6(_) => ExactModelCodec::V6,
            Self::V7(_) => ExactModelCodec::V7,
        }
    }
}
