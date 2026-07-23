//! Explicit model wire v3 for shaped Fields and boundary physical interfaces.

use eqiora_core::{Diagnostic, OntologyId};
use eqiora_graph::Transaction;
use eqiora_schema::Model;
use eqiora_sem::KernelProgram;

use crate::model_v2::ModelEnvelopeV2;
use crate::{ArtifactDigest, DecoderLimits};

/// Versioned canonical Semantic Model serialization with boundary physical
/// interface semantics.
///
/// The implementation deliberately shares the bounded graph-envelope
/// machinery with v2. Selection remains explicit: no decoder sniffs or retries
/// another schema, and the v3 validator rejects the legacy Field encoding.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelEnvelopeV3 {
    inner: ModelEnvelopeV2,
}

impl ModelEnvelopeV3 {
    /// Encode one immutable validated Semantic Kernel program as explicit v3.
    ///
    /// # Errors
    /// Returns `EQ0901` for an unsupported kernel value or resource-limit
    /// violation.
    pub fn from_program(program: &KernelProgram) -> Result<Self, Diagnostic> {
        ModelEnvelopeV2::from_program_v3(program).map(|inner| Self { inner })
    }

    /// Decode and validate v3 bytes without mutating a graph store.
    ///
    /// # Errors
    /// Returns `EQ0901` for malformed, oversized, dangling, duplicated, or
    /// wrong-version data.
    pub fn from_json(bytes: &[u8], limits: DecoderLimits) -> Result<Self, Diagnostic> {
        ModelEnvelopeV2::from_json_v3(bytes, limits).map(|inner| Self { inner })
    }

    /// Deterministic compact canonical JSON.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn canonical_json(&self) -> Result<Vec<u8>, Diagnostic> {
        self.inner.canonical_json()
    }

    /// Domain-separated SHA-256 identity of semantic v3 content.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn digest(&self) -> Result<ArtifactDigest, Diagnostic> {
        self.inner.digest_v3()
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
