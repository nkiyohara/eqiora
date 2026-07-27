//! Explicit transaction wire v7 for spatial-periodic boundary Connections.

use eqiora_core::Diagnostic;
use eqiora_graph::Transaction;

use crate::model_transaction_v2::ModelTransactionEnvelopeV2;
use crate::{ArtifactDigest, ModelDecoderLimits};

/// Explicit v7 serialization of one ordered Semantic Model transaction.
///
/// V7 inherits the complete v5 edit grammar and adds only the closed
/// spatial-periodic Connection semantic. Decoding performs no version fallback
/// and never mutates a graph store.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelTransactionEnvelopeV7 {
    inner: ModelTransactionEnvelopeV2,
}

impl ModelTransactionEnvelopeV7 {
    /// Encode an ordered transaction without changing operation order.
    ///
    /// # Errors
    /// Returns `EQ0901` for unsupported operations, meanings, or resource
    /// limits.
    pub fn from_transaction(transaction: &Transaction) -> Result<Self, Diagnostic> {
        ModelTransactionEnvelopeV2::from_transaction_v7(transaction).map(|inner| Self { inner })
    }

    /// Decode and validate the local v7 edit grammar without graph mutation.
    ///
    /// # Errors
    /// Returns `EQ0901` for malformed, oversized, unsupported, inconsistent,
    /// or wrong-version data.
    pub fn from_json(bytes: &[u8], limits: ModelDecoderLimits) -> Result<Self, Diagnostic> {
        ModelTransactionEnvelopeV2::from_json_v7(bytes, limits).map(|inner| Self { inner })
    }

    /// Deterministic compact canonical JSON preserving operation order.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn canonical_json(&self) -> Result<Vec<u8>, Diagnostic> {
        self.inner.canonical_json()
    }

    /// Domain-separated SHA-256 identity of the exact ordered v7 transaction.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn digest(&self) -> Result<ArtifactDigest, Diagnostic> {
        self.inner.digest_v7()
    }

    /// Reconstruct the locally typed in-memory transaction without committing it.
    ///
    /// # Errors
    /// Returns `EQ0901` if locally valid wire data cannot be reconstructed.
    pub fn to_transaction(&self) -> Result<Transaction, Diagnostic> {
        self.inner.to_transaction()
    }
}
