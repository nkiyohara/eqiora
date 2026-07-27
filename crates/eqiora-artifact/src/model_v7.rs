//! Explicit model wire v7 for Domains that name an authored geometry.

use eqiora_core::{Diagnostic, OntologyId};
use eqiora_graph::Transaction;
use eqiora_schema::Model;
use eqiora_sem::KernelProgram;

use crate::model_v2::ModelEnvelopeV2;
use crate::{ArtifactDigest, ModelDecoderLimits};

/// Explicit v7 serialization of one validated canonical Semantic Model.
///
/// V7 inherits the complete v6 grammar and adds only Domains that name an
/// authored geometry by content digest and entity set. Selection remains
/// explicit; older decoders do not retry or reinterpret the new meaning,
/// because a decoder shown a geometry reference it cannot read would have to
/// guess at a shape.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelEnvelopeV7 {
    inner: ModelEnvelopeV2,
}

impl ModelEnvelopeV7 {
    /// Encode one immutable validated Semantic Kernel program as explicit v7.
    ///
    /// # Errors
    /// Returns `EQ0901` for unsupported meaning or resource-limit violations.
    pub fn from_program(program: &KernelProgram) -> Result<Self, Diagnostic> {
        ModelEnvelopeV2::from_program_v7(program).map(|inner| Self { inner })
    }

    /// Decode the closed v7 grammar without graph mutation.
    ///
    /// # Errors
    /// Returns `EQ0901` for malformed, oversized, unsupported, dangling, or
    /// wrong-version data.
    pub fn from_json(bytes: &[u8], limits: ModelDecoderLimits) -> Result<Self, Diagnostic> {
        ModelEnvelopeV2::from_json_v7(bytes, limits).map(|inner| Self { inner })
    }

    /// Deterministic compact canonical JSON.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn canonical_json(&self) -> Result<Vec<u8>, Diagnostic> {
        self.inner.canonical_json()
    }

    /// Domain-separated SHA-256 identity of semantic v7 content.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn digest(&self) -> Result<ArtifactDigest, Diagnostic> {
        self.inner.digest_v7()
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
