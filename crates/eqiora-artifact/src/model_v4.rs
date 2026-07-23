//! Explicit model wire v4 for canonical tensor operators.

use eqiora_core::{Diagnostic, OntologyId};
use eqiora_graph::Transaction;
use eqiora_schema::Model;
use eqiora_sem::KernelProgram;

use crate::model_v2::ModelEnvelopeV2;
use crate::{ArtifactDigest, DecoderLimits};

/// Versioned canonical Semantic Model serialization with shaped boundary
/// semantics and canonical tensor operators.
///
/// V4 inherits the exact shaped-Field and boundary-interface grammar of v3,
/// and adds only `symmetric-part` and `isotropic-lift` expression nodes. The
/// explicit wrapper prevents older decoders from silently accepting the
/// enlarged expression vocabulary.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelEnvelopeV4 {
    inner: ModelEnvelopeV2,
}

impl ModelEnvelopeV4 {
    /// Encode one immutable validated Semantic Kernel program as explicit v4.
    ///
    /// # Errors
    /// Returns `EQ0901` for an unsupported kernel value or resource-limit
    /// violation.
    pub fn from_program(program: &KernelProgram) -> Result<Self, Diagnostic> {
        ModelEnvelopeV2::from_program_v4(program).map(|inner| Self { inner })
    }

    /// Decode and validate v4 bytes without mutating a graph store.
    ///
    /// # Errors
    /// Returns `EQ0901` for malformed, oversized, dangling, duplicated, or
    /// wrong-version data.
    pub fn from_json(bytes: &[u8], limits: DecoderLimits) -> Result<Self, Diagnostic> {
        ModelEnvelopeV2::from_json_v4(bytes, limits).map(|inner| Self { inner })
    }

    /// Deterministic compact canonical JSON.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn canonical_json(&self) -> Result<Vec<u8>, Diagnostic> {
        self.inner.canonical_json()
    }

    /// Domain-separated SHA-256 identity of semantic v4 content.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn digest(&self) -> Result<ArtifactDigest, Diagnostic> {
        self.inner.digest_v4()
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
