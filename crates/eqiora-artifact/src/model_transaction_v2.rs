//! Ordered Semantic Model transaction wire v2.

use eqiora_core::Diagnostic;
use eqiora_graph::Transaction;
use serde::{Deserialize, Serialize};

use crate::model::{checked_count_sum, require_decoder_count};
use crate::model_transaction::{WireModelOp, WireModelPrecondition};
use crate::{
    ArtifactDigest, CANONICAL_ENCODING, DecoderLimits, check_wire_limits, invalid_artifact,
    validate_text,
};

const TRANSACTION_SCHEMA_V2: &str = "eqiora.model-transaction-envelope/v2";
const TRANSACTION_SCHEMA_V3: &str = "eqiora.model-transaction-envelope/v3";
const TRANSACTION_SCHEMA_V4: &str = "eqiora.model-transaction-envelope/v4";
const TRANSACTION_SCHEMA_V5: &str = "eqiora.model-transaction-envelope/v5";
const TRANSACTION_SCHEMA_V6: &str = "eqiora.model-transaction-envelope/v6";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransactionSchemaVersion {
    V2,
    V3,
    V4,
    V5,
    V6,
}

impl TransactionSchemaVersion {
    const fn schema(self) -> &'static str {
        match self {
            Self::V2 => TRANSACTION_SCHEMA_V2,
            Self::V3 => TRANSACTION_SCHEMA_V3,
            Self::V4 => TRANSACTION_SCHEMA_V4,
            Self::V5 => TRANSACTION_SCHEMA_V5,
            Self::V6 => TRANSACTION_SCHEMA_V6,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::V2 => "model transaction v2",
            Self::V3 => "model transaction v3",
            Self::V4 => "model transaction v4",
            Self::V5 => "model transaction v5",
            Self::V6 => "model transaction v6",
        }
    }

    const fn envelope_label(self) -> &'static str {
        match self {
            Self::V2 => "model transaction v2 envelope",
            Self::V3 => "model transaction v3 envelope",
            Self::V4 => "model transaction v4 envelope",
            Self::V5 => "model transaction v5 envelope",
            Self::V6 => "model transaction v6 envelope",
        }
    }
}

/// Explicit v2 serialization of one ordered Semantic Model transaction.
///
/// This is the identity of a locally typed edit, not a proof that the edit
/// alone forms a complete Semantic Model. References may resolve against the
/// selected store revision or a later operation. After atomic commit, callers
/// must construct a `KernelProgram` before exposing the candidate as valid.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelTransactionEnvelopeV2 {
    wire: WireModelTransactionEnvelopeV2,
}

impl ModelTransactionEnvelopeV2 {
    /// Encode an ordered transaction without changing operation order.
    ///
    /// # Errors
    /// Returns `EQ0901` for operations outside the Semantic Model vocabulary,
    /// unsupported kernel values, or resource-limit violations.
    pub fn from_transaction(transaction: &Transaction) -> Result<Self, Diagnostic> {
        Self::from_transaction_version(transaction, TransactionSchemaVersion::V2)
    }

    pub(crate) fn from_transaction_v3(transaction: &Transaction) -> Result<Self, Diagnostic> {
        Self::from_transaction_version(transaction, TransactionSchemaVersion::V3)
    }

    pub(crate) fn from_transaction_v4(transaction: &Transaction) -> Result<Self, Diagnostic> {
        Self::from_transaction_version(transaction, TransactionSchemaVersion::V4)
    }

    pub(crate) fn from_transaction_v5(transaction: &Transaction) -> Result<Self, Diagnostic> {
        Self::from_transaction_version(transaction, TransactionSchemaVersion::V5)
    }

    pub(crate) fn from_transaction_v6(transaction: &Transaction) -> Result<Self, Diagnostic> {
        Self::from_transaction_version(transaction, TransactionSchemaVersion::V6)
    }

    fn from_transaction_version(
        transaction: &Transaction,
        version: TransactionSchemaVersion,
    ) -> Result<Self, Diagnostic> {
        let ops = transaction
            .ops()
            .iter()
            .map(|op| match version {
                TransactionSchemaVersion::V2 => WireModelOp::encode_v2(op),
                TransactionSchemaVersion::V3 => WireModelOp::encode_v3(op),
                TransactionSchemaVersion::V4 => WireModelOp::encode_v4(op),
                TransactionSchemaVersion::V5 => WireModelOp::encode_v5(op),
                TransactionSchemaVersion::V6 => WireModelOp::encode_v6(op),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let preconditions = transaction
            .preconditions()
            .iter()
            .map(WireModelPrecondition::encode)
            .collect::<Result<Vec<_>, _>>()?;
        let mut envelope = Self {
            wire: WireModelTransactionEnvelopeV2 {
                schema: version.schema().to_owned(),
                encoding: CANONICAL_ENCODING.to_owned(),
                label: transaction.label().to_owned(),
                ops,
                preconditions,
            },
        };
        envelope.canonicalize_and_validate(DecoderLimits::default(), version)?;
        Ok(envelope)
    }

    /// Decode and validate the local edit grammar without mutating a graph
    /// store.
    ///
    /// # Errors
    /// Returns `EQ0901` for malformed, oversized, or wrong-version data.
    pub fn from_json(bytes: &[u8], limits: DecoderLimits) -> Result<Self, Diagnostic> {
        Self::from_json_version(bytes, limits, TransactionSchemaVersion::V2)
    }

    pub(crate) fn from_json_v3(bytes: &[u8], limits: DecoderLimits) -> Result<Self, Diagnostic> {
        Self::from_json_version(bytes, limits, TransactionSchemaVersion::V3)
    }

    pub(crate) fn from_json_v4(bytes: &[u8], limits: DecoderLimits) -> Result<Self, Diagnostic> {
        Self::from_json_version(bytes, limits, TransactionSchemaVersion::V4)
    }

    pub(crate) fn from_json_v5(bytes: &[u8], limits: DecoderLimits) -> Result<Self, Diagnostic> {
        Self::from_json_version(bytes, limits, TransactionSchemaVersion::V5)
    }

    pub(crate) fn from_json_v6(bytes: &[u8], limits: DecoderLimits) -> Result<Self, Diagnostic> {
        Self::from_json_version(bytes, limits, TransactionSchemaVersion::V6)
    }

    fn from_json_version(
        bytes: &[u8],
        limits: DecoderLimits,
        version: TransactionSchemaVersion,
    ) -> Result<Self, Diagnostic> {
        check_wire_limits(bytes, limits)?;
        let wire = serde_json::from_slice(bytes).map_err(|error| {
            invalid_artifact(format!("invalid {} JSON: {error}", version.schema()))
        })?;
        let mut envelope = Self { wire };
        envelope.canonicalize_and_validate(limits, version)?;
        Ok(envelope)
    }

    /// Deterministic compact JSON preserving semantic operation order.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn canonical_json(&self) -> Result<Vec<u8>, Diagnostic> {
        let version = self.schema_version();
        serde_json::to_vec(&self.wire).map_err(|error| {
            invalid_artifact(format!(
                "cannot serialize {}: {error}",
                version.envelope_label()
            ))
        })
    }

    /// Domain-separated SHA-256 identity of the exact ordered v2 transaction.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn digest(&self) -> Result<ArtifactDigest, Diagnostic> {
        self.digest_version(TransactionSchemaVersion::V2)
    }

    pub(crate) fn digest_v3(&self) -> Result<ArtifactDigest, Diagnostic> {
        self.digest_version(TransactionSchemaVersion::V3)
    }

    pub(crate) fn digest_v4(&self) -> Result<ArtifactDigest, Diagnostic> {
        self.digest_version(TransactionSchemaVersion::V4)
    }

    pub(crate) fn digest_v5(&self) -> Result<ArtifactDigest, Diagnostic> {
        self.digest_version(TransactionSchemaVersion::V5)
    }

    pub(crate) fn digest_v6(&self) -> Result<ArtifactDigest, Diagnostic> {
        self.digest_version(TransactionSchemaVersion::V6)
    }

    fn digest_version(
        &self,
        version: TransactionSchemaVersion,
    ) -> Result<ArtifactDigest, Diagnostic> {
        Ok(ArtifactDigest::compute(
            version.schema().as_bytes(),
            &self.canonical_json()?,
        ))
    }

    /// Reconstruct the locally typed in-memory transaction without committing
    /// it.
    ///
    /// # Errors
    /// Returns `EQ0901` if locally valid wire data cannot be reconstructed.
    /// Store-dependent references are admitted only when the committed
    /// snapshot is reconstructed as a `KernelProgram`.
    pub fn to_transaction(&self) -> Result<Transaction, Diagnostic> {
        let mut transaction = Transaction::new(&self.wire.label);
        for precondition in &self.wire.preconditions {
            transaction.require(precondition.decode()?);
        }
        for op in &self.wire.ops {
            transaction.push(op.decode()?);
        }
        Ok(transaction)
    }

    fn schema_version(&self) -> TransactionSchemaVersion {
        match self.wire.schema.as_str() {
            TRANSACTION_SCHEMA_V3 => TransactionSchemaVersion::V3,
            TRANSACTION_SCHEMA_V4 => TransactionSchemaVersion::V4,
            TRANSACTION_SCHEMA_V5 => TransactionSchemaVersion::V5,
            TRANSACTION_SCHEMA_V6 => TransactionSchemaVersion::V6,
            _ => TransactionSchemaVersion::V2,
        }
    }

    fn canonicalize_and_validate(
        &mut self,
        limits: DecoderLimits,
        version: TransactionSchemaVersion,
    ) -> Result<(), Diagnostic> {
        let label = version.label();
        if self.wire.schema != version.schema() || self.wire.encoding != CANONICAL_ENCODING {
            return Err(invalid_artifact(format!(
                "unsupported {} schema or canonical encoding",
                version.schema()
            )));
        }
        validate_text("transaction label", &self.wire.label)?;
        if self.wire.ops.is_empty() || self.wire.ops.len() > limits.max_transaction_ops {
            return Err(invalid_artifact(format!(
                "{label} requires 1..={} operations, found {}",
                limits.max_transaction_ops,
                self.wire.ops.len()
            )));
        }
        if self.wire.preconditions.len() > limits.max_transaction_preconditions {
            return Err(invalid_artifact(format!(
                "{label} has {} preconditions, exceeding the {} precondition limit",
                self.wire.preconditions.len(),
                limits.max_transaction_preconditions
            )));
        }
        for op in &mut self.wire.ops {
            op.canonicalize_sets()?;
        }
        let expression_nodes = self.wire.ops.iter().try_fold(0_usize, |count, op| {
            count
                .checked_add(op.expression_node_count())
                .ok_or_else(|| invalid_artifact("expression-node count overflows usize"))
        })?;
        if expression_nodes > limits.max_expression_nodes {
            return Err(invalid_artifact(format!(
                "{label} has {expression_nodes} expression nodes, exceeding the {} node limit",
                limits.max_expression_nodes
            )));
        }
        let expression_roots = checked_count_sum(
            self.wire.ops.iter().map(WireModelOp::expression_root_count),
            &format!("{label} expression-root count"),
        )?;
        require_decoder_count(
            &format!("{label} expression roots"),
            expression_roots,
            limits.max_expression_roots,
        )?;
        let pure_operator_counts = self.wire.ops.iter().try_fold(
            crate::model::PureOperatorWireCounts::default(),
            |counts, op| counts.checked_add(op.pure_operator_counts()?),
        )?;
        pure_operator_counts.ensure_limits(limits, label)?;
        let view_members = checked_count_sum(
            self.wire.ops.iter().map(|op| op.model_view_counts().0),
            &format!("{label} view-member count"),
        )?;
        require_decoder_count(
            &format!("{label} view members"),
            view_members,
            limits.max_model_view_members,
        )?;
        let boundaries = checked_count_sum(
            self.wire.ops.iter().map(|op| op.model_view_counts().1),
            &format!("{label} boundary count"),
        )?;
        require_decoder_count(
            &format!("{label} boundary Ports"),
            boundaries,
            limits.max_model_boundary,
        )?;
        if matches!(
            version,
            TransactionSchemaVersion::V5 | TransactionSchemaVersion::V6
        ) {
            for op in &self.wire.ops {
                op.validate_v5_features()?;
            }
            for op in &mut self.wire.ops {
                op.canonicalize_v5_definitions()?;
            }
        }
        for precondition in &self.wire.preconditions {
            precondition.decode()?;
        }
        for op in &self.wire.ops {
            op.ensure_value_shape_limits(limits)?;
            match version {
                TransactionSchemaVersion::V2 => op.ensure_v2()?,
                TransactionSchemaVersion::V3 => op.ensure_v3()?,
                TransactionSchemaVersion::V4 => op.ensure_v4()?,
                TransactionSchemaVersion::V5 => op.ensure_v5()?,
                TransactionSchemaVersion::V6 => op.ensure_v6()?,
            }
            op.decode()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireModelTransactionEnvelopeV2 {
    schema: String,
    encoding: String,
    label: String,
    ops: Vec<WireModelOp>,
    preconditions: Vec<WireModelPrecondition>,
}
