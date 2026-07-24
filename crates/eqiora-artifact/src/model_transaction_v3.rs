//! Explicit transaction wire v3 for shaped Fields and boundary interfaces.

use eqiora_core::Diagnostic;
use eqiora_graph::Transaction;

use crate::model_transaction_v2::ModelTransactionEnvelopeV2;
use crate::{ArtifactDigest, ModelDecoderLimits};

/// Explicit v3 serialization of one ordered Semantic Model transaction.
///
/// This wrapper shares ordered-edit machinery with v2 while selecting the v3
/// operation vocabulary before encoding or decoding. Selection is explicit;
/// no input sniffing or version fallback occurs.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelTransactionEnvelopeV3 {
    inner: ModelTransactionEnvelopeV2,
}

impl ModelTransactionEnvelopeV3 {
    /// Encode an ordered transaction without changing operation order.
    ///
    /// # Errors
    /// Returns `EQ0901` for operations outside the Semantic Model vocabulary,
    /// unsupported values, or resource-limit violations.
    pub fn from_transaction(transaction: &Transaction) -> Result<Self, Diagnostic> {
        ModelTransactionEnvelopeV2::from_transaction_v3(transaction).map(|inner| Self { inner })
    }

    /// Decode and validate the local v3 edit grammar without graph mutation.
    ///
    /// # Errors
    /// Returns `EQ0901` for malformed, oversized, or wrong-version data.
    pub fn from_json(bytes: &[u8], limits: ModelDecoderLimits) -> Result<Self, Diagnostic> {
        ModelTransactionEnvelopeV2::from_json_v3(bytes, limits).map(|inner| Self { inner })
    }

    /// Deterministic compact JSON preserving semantic operation order.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn canonical_json(&self) -> Result<Vec<u8>, Diagnostic> {
        self.inner.canonical_json()
    }

    /// Domain-separated SHA-256 identity of the exact ordered v3 transaction.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn digest(&self) -> Result<ArtifactDigest, Diagnostic> {
        self.inner.digest_v3()
    }

    /// Reconstruct the locally typed in-memory transaction without commit.
    ///
    /// # Errors
    /// Returns `EQ0901` if locally valid wire data cannot be reconstructed.
    pub fn to_transaction(&self) -> Result<Transaction, Diagnostic> {
        self.inner.to_transaction()
    }
}
