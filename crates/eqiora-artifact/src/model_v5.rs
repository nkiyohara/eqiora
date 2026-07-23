//! Explicit model wire v5 for content-addressed pure-operator applications.

use eqiora_core::{Diagnostic, OntologyId};
use eqiora_graph::Transaction;
use eqiora_schema::Model;
use eqiora_sem::KernelProgram;

use crate::model_v2::ModelEnvelopeV2;
use crate::{ArtifactDigest, DecoderLimits};

/// Explicit v5 serialization of one validated canonical Semantic Model.
///
/// V5 inherits v4 unchanged and adds only expression-local pure-operator
/// definition tables plus digest-keyed ordered applications. Selection stays
/// explicit; no decoder sniffs, retries, or consults an ambient registry.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelEnvelopeV5 {
    inner: ModelEnvelopeV2,
}

impl ModelEnvelopeV5 {
    /// Encode one immutable validated Semantic Kernel program as explicit v5.
    ///
    /// # Errors
    /// Returns `EQ0901` for unsupported meaning or resource-limit violations.
    pub fn from_program(program: &KernelProgram) -> Result<Self, Diagnostic> {
        ModelEnvelopeV2::from_program_v5(program).map(|inner| Self { inner })
    }

    /// Decode the closed v5 grammar without graph mutation.
    ///
    /// # Errors
    /// Returns `EQ0901` for malformed, oversized, unsupported, dangling,
    /// digest-mismatched, duplicated, unused, or wrong-version data.
    pub fn from_json(bytes: &[u8], limits: DecoderLimits) -> Result<Self, Diagnostic> {
        ModelEnvelopeV2::from_json_v5(bytes, limits).map(|inner| Self { inner })
    }

    /// Deterministic compact canonical JSON.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn canonical_json(&self) -> Result<Vec<u8>, Diagnostic> {
        self.inner.canonical_json()
    }

    /// Domain-separated SHA-256 identity of semantic v5 content.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn digest(&self) -> Result<ArtifactDigest, Diagnostic> {
        self.inner.digest_v5()
    }

    /// Reconstruct one typed transaction without committing it.
    ///
    /// # Errors
    /// Returns structured diagnostics when reconstruction fails.
    pub fn to_transaction(&self) -> Result<(Transaction, OntologyId<Model>), Vec<Diagnostic>> {
        self.inner.to_transaction()
    }

    /// Reconstruct through typed definitions, atomic commit, and complete
    /// Semantic Model validation.
    ///
    /// # Errors
    /// Returns diagnostics from reconstruction, commit, or whole-model
    /// validation.
    pub fn to_program(&self) -> Result<KernelProgram, Vec<Diagnostic>> {
        self.inner.to_program()
    }

    /// Source graph revision retained as provenance.
    #[must_use]
    pub const fn source_revision(&self) -> u64 {
        self.inner.source_revision()
    }

    /// Typed Semantic Model identity.
    ///
    /// # Errors
    /// Returns `EQ0901` only if validated internal state were corrupted.
    pub fn model(&self) -> Result<OntologyId<Model>, Diagnostic> {
        self.inner.model()
    }
}
