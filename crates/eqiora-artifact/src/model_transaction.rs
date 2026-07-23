//! Versioned wire contract for Semantic Model edit transactions.

use eqiora_core::{Diagnostic, EntityKind, GraphClass, OntologyId, RawId};
use eqiora_graph::{Op, Precondition, Revision, Transaction};
use eqiora_schema::{Model, ModelView};
use serde::{Deserialize, Serialize};

use crate::model::{
    WireEdgeKind, WireId, WireNode, WireQuantity, checked_count_sum, parse_ulid,
    require_decoder_count,
};
use crate::{ArtifactDigest, DecoderLimits, check_wire_limits, invalid_artifact, validate_text};

const TRANSACTION_SCHEMA: &str = "eqiora.model-transaction-envelope/v1";
const CANONICAL_ENCODING: &str = "eqiora.canonical-json/v1";

/// Versioned serialization of one ordered Semantic Model edit transaction.
///
/// This is intentionally narrower than a Graph Federation [`Transaction`]. It
/// admits Semantic Kernel mutations and the standard [`ModelView`] schema,
/// but rejects infrastructure graph nodes and other ontology schemas whose
/// validators cannot be reconstructed without a versioned schema registry.
///
/// The envelope is the identity of an ordered edit, not a complete Semantic
/// Model admission proof. References inside an edit may resolve against the
/// selected store revision or a later operation. After an atomic commit,
/// callers must construct a `KernelProgram` before exposing the candidate as a
/// valid Semantic Model.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelTransactionEnvelopeV1 {
    wire: WireModelTransactionEnvelopeV1,
}

impl ModelTransactionEnvelopeV1 {
    /// Encode an ordered model transaction without changing operation order.
    ///
    /// # Errors
    /// Returns `EQ0901` if an operation lies outside the Semantic Model wire
    /// scope, uses a newer kernel variant, or exceeds decoder limits.
    pub fn from_transaction(transaction: &Transaction) -> Result<Self, Diagnostic> {
        let ops = transaction
            .ops()
            .iter()
            .map(WireModelOp::encode_v1)
            .collect::<Result<Vec<_>, _>>()?;
        let preconditions = transaction
            .preconditions()
            .iter()
            .map(WireModelPrecondition::encode)
            .collect::<Result<Vec<_>, _>>()?;
        let mut envelope = Self {
            wire: WireModelTransactionEnvelopeV1 {
                schema: TRANSACTION_SCHEMA.to_owned(),
                encoding: CANONICAL_ENCODING.to_owned(),
                label: transaction.label().to_owned(),
                ops,
                preconditions,
            },
        };
        envelope.canonicalize_and_validate(DecoderLimits::default())?;
        Ok(envelope)
    }

    /// Decode and validate the local edit grammar without mutating a graph
    /// store.
    ///
    /// # Errors
    /// Returns `EQ0901` for malformed, oversized, unknown-version, or
    /// out-of-scope transaction data.
    pub fn from_json(bytes: &[u8], limits: DecoderLimits) -> Result<Self, Diagnostic> {
        check_wire_limits(bytes, limits)?;
        let wire = serde_json::from_slice(bytes).map_err(|error| {
            invalid_artifact(format!("invalid model transaction envelope JSON: {error}"))
        })?;
        let mut envelope = Self { wire };
        envelope.canonicalize_and_validate(limits)?;
        Ok(envelope)
    }

    /// Deterministic compact JSON preserving semantic operation order.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn canonical_json(&self) -> Result<Vec<u8>, Diagnostic> {
        serde_json::to_vec(&self.wire).map_err(|error| {
            invalid_artifact(format!(
                "cannot serialize model transaction envelope: {error}"
            ))
        })
    }

    /// Domain-separated SHA-256 identity of the exact ordered transaction.
    ///
    /// # Errors
    /// Returns `EQ0901` if canonical serialization unexpectedly fails.
    pub fn digest(&self) -> Result<ArtifactDigest, Diagnostic> {
        Ok(ArtifactDigest::compute(
            TRANSACTION_SCHEMA.as_bytes(),
            &self.canonical_json()?,
        ))
    }

    /// Reconstruct the locally typed in-memory transaction.
    ///
    /// This method does not commit. The caller chooses a graph revision and
    /// obtains the store's ordinary atomic validation and provenance behavior.
    /// Store-dependent semantic references are admitted only when the
    /// resulting snapshot is reconstructed as a `KernelProgram`.
    ///
    /// # Errors
    /// Returns `EQ0901` if locally valid wire data cannot be reconstructed.
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

    fn canonicalize_and_validate(&mut self, limits: DecoderLimits) -> Result<(), Diagnostic> {
        if self.wire.schema != TRANSACTION_SCHEMA || self.wire.encoding != CANONICAL_ENCODING {
            return Err(invalid_artifact(
                "unsupported model-transaction schema or canonical encoding",
            ));
        }
        validate_text("transaction label", &self.wire.label)?;
        if self.wire.ops.is_empty() || self.wire.ops.len() > limits.max_transaction_ops {
            return Err(invalid_artifact(format!(
                "model transaction requires 1..={} operations, found {}",
                limits.max_transaction_ops,
                self.wire.ops.len()
            )));
        }
        if self.wire.preconditions.len() > limits.max_transaction_preconditions {
            return Err(invalid_artifact(format!(
                "model transaction has {} preconditions, exceeding the {} precondition limit",
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
                "model transaction has {expression_nodes} expression nodes, exceeding the {} node limit",
                limits.max_expression_nodes
            )));
        }
        let expression_roots = checked_count_sum(
            self.wire.ops.iter().map(WireModelOp::expression_root_count),
            "model transaction expression-root count",
        )?;
        require_decoder_count(
            "model transaction expression roots",
            expression_roots,
            limits.max_expression_roots,
        )?;
        let view_members = checked_count_sum(
            self.wire.ops.iter().map(|op| op.model_view_counts().0),
            "model transaction view-member count",
        )?;
        require_decoder_count(
            "model transaction view members",
            view_members,
            limits.max_model_view_members,
        )?;
        let boundaries = checked_count_sum(
            self.wire.ops.iter().map(|op| op.model_view_counts().1),
            "model transaction boundary count",
        )?;
        require_decoder_count(
            "model transaction boundary Ports",
            boundaries,
            limits.max_model_boundary,
        )?;

        // Reconstruct every local value now. Store-dependent invariants are
        // deliberately checked only by the later atomic graph commit.
        for precondition in &self.wire.preconditions {
            precondition.decode()?;
        }
        for op in &self.wire.ops {
            op.ensure_v1()?;
            op.decode()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireModelTransactionEnvelopeV1 {
    schema: String,
    encoding: String,
    label: String,
    ops: Vec<WireModelOp>,
    preconditions: Vec<WireModelPrecondition>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum WireModelOp {
    DefineKernelNode {
        node: WireNode,
    },
    SetValue {
        target: WireId,
        value: WireQuantity,
    },
    Connect {
        from: WireId,
        to: WireId,
        edge: WireEdgeKind,
    },
    RemoveNode {
        id: WireId,
    },
    DefineModelView {
        view: WireModelView,
    },
    RemoveModelView {
        ulid: String,
    },
}

impl WireModelOp {
    fn encode_v1(op: &Op) -> Result<Self, Diagnostic> {
        Self::encode(op, ModelOperationWireVersion::V1)
    }

    pub(crate) fn encode_v2(op: &Op) -> Result<Self, Diagnostic> {
        Self::encode(op, ModelOperationWireVersion::V2)
    }

    pub(crate) fn encode_v3(op: &Op) -> Result<Self, Diagnostic> {
        Self::encode(op, ModelOperationWireVersion::V3)
    }

    pub(crate) fn encode_v4(op: &Op) -> Result<Self, Diagnostic> {
        Self::encode(op, ModelOperationWireVersion::V4)
    }

    pub(crate) fn encode_v5(op: &Op) -> Result<Self, Diagnostic> {
        Self::encode(op, ModelOperationWireVersion::V5)
    }

    pub(crate) fn encode_v6(op: &Op) -> Result<Self, Diagnostic> {
        Self::encode(op, ModelOperationWireVersion::V6)
    }

    fn encode(op: &Op, version: ModelOperationWireVersion) -> Result<Self, Diagnostic> {
        match op {
            Op::DefineKernelNode { node } => Ok(Self::DefineKernelNode {
                node: match version {
                    ModelOperationWireVersion::V1 => WireNode::encode_v1(node)?,
                    ModelOperationWireVersion::V2 => WireNode::encode_v2(node)?,
                    ModelOperationWireVersion::V3 => WireNode::encode_v3(node)?,
                    ModelOperationWireVersion::V4 => WireNode::encode_v4(node)?,
                    ModelOperationWireVersion::V5 => WireNode::encode_v5(node)?,
                    ModelOperationWireVersion::V6 => WireNode::encode_v6(node)?,
                },
            }),
            Op::SetValue { target, value } => {
                require_semantic_id(*target, "SetValue target")?;
                Ok(Self::SetValue {
                    target: WireId::from_raw(*target),
                    value: WireQuantity::encode(*value),
                })
            }
            Op::Connect { from, to, edge } => {
                require_semantic_id(*from, "Connect source")?;
                require_semantic_id(*to, "Connect target")?;
                Ok(Self::Connect {
                    from: WireId::from_raw(*from),
                    to: WireId::from_raw(*to),
                    edge: WireEdgeKind::encode(*edge)?,
                })
            }
            Op::RemoveNode { id } => {
                require_semantic_id(*id, "RemoveNode target")?;
                Ok(Self::RemoveNode {
                    id: WireId::from_raw(*id),
                })
            }
            Op::DefineOntologyView { view } => {
                let view = view.downcast::<Model>().ok_or_else(|| {
                    invalid_artifact(format!(
                        "model transaction cannot define ontology schema `{}`",
                        view.id().schema()
                    ))
                })?;
                Ok(Self::DefineModelView {
                    view: WireModelView::encode(&view),
                })
            }
            Op::RemoveOntologyView { id } => {
                let id = id.downcast::<Model>().ok_or_else(|| {
                    invalid_artifact(format!(
                        "model transaction cannot remove ontology schema `{}`",
                        id.schema()
                    ))
                })?;
                Ok(Self::RemoveModelView {
                    ulid: id.ulid().to_string(),
                })
            }
            Op::AddNode { kind, .. } => Err(invalid_artifact(format!(
                "infrastructure node {kind:?} cannot enter a Semantic Model transaction"
            ))),
            _ => Err(invalid_artifact(
                "operation is newer than the supported model transaction wire vocabulary",
            )),
        }
    }

    pub(crate) fn decode(&self) -> Result<Op, Diagnostic> {
        match self {
            Self::DefineKernelNode { node } => Ok(Op::DefineKernelNode {
                node: node.decode()?,
            }),
            Self::SetValue { target, value } => Ok(Op::SetValue {
                target: target.decode_raw()?,
                value: value.decode()?,
            }),
            Self::Connect { from, to, edge } => {
                let from = from.decode_raw()?;
                let to = to.decode_raw()?;
                let edge = edge.decode();
                if !edge.permits(from.kind(), to.kind()) {
                    return Err(invalid_artifact(
                        "wire edge endpoints violate the closed Semantic Model edge schema",
                    ));
                }
                Ok(Op::Connect { from, to, edge })
            }
            Self::RemoveNode { id } => Ok(Op::RemoveNode {
                id: id.decode_raw()?,
            }),
            Self::DefineModelView { view } => Ok(Op::DefineOntologyView {
                view: view.decode()?.into(),
            }),
            Self::RemoveModelView { ulid } => Ok(Op::RemoveOntologyView {
                id: OntologyId::<Model>::from_ulid(parse_ulid(ulid)?).erase(),
            }),
        }
    }

    pub(crate) fn expression_node_count(&self) -> usize {
        match self {
            Self::DefineKernelNode { node } => node.expression_node_count(),
            Self::SetValue { .. }
            | Self::Connect { .. }
            | Self::RemoveNode { .. }
            | Self::DefineModelView { .. }
            | Self::RemoveModelView { .. } => 0,
        }
    }

    pub(crate) fn expression_root_count(&self) -> usize {
        match self {
            Self::DefineKernelNode { node } => node.expression_root_count(),
            Self::SetValue { .. }
            | Self::Connect { .. }
            | Self::RemoveNode { .. }
            | Self::DefineModelView { .. }
            | Self::RemoveModelView { .. } => 0,
        }
    }

    pub(crate) fn pure_operator_counts(
        &self,
    ) -> Result<crate::model::PureOperatorWireCounts, Diagnostic> {
        match self {
            Self::DefineKernelNode { node } => node.pure_operator_counts(),
            _ => Ok(crate::model::PureOperatorWireCounts::default()),
        }
    }

    pub(crate) fn validate_v5_features(&self) -> Result<(), Diagnostic> {
        match self {
            Self::DefineKernelNode { node } => node.validate_v5_features(),
            _ => Ok(()),
        }
    }

    pub(crate) fn canonicalize_v5_definitions(&mut self) -> Result<(), Diagnostic> {
        match self {
            Self::DefineKernelNode { node } => node.canonicalize_v5_definitions(),
            _ => Ok(()),
        }
    }

    pub(crate) fn model_view_counts(&self) -> (usize, usize) {
        match self {
            Self::DefineModelView { view } => (view.members.len(), view.boundary.len()),
            _ => (0, 0),
        }
    }

    fn ensure_v1(&self) -> Result<(), Diagnostic> {
        match self {
            Self::DefineKernelNode { node } => node.ensure_v1(),
            _ => Ok(()),
        }
    }

    pub(crate) fn ensure_v2(&self) -> Result<(), Diagnostic> {
        match self {
            Self::DefineKernelNode { node } => node.ensure_v2(),
            _ => Ok(()),
        }
    }

    pub(crate) fn ensure_v3(&self) -> Result<(), Diagnostic> {
        match self {
            Self::DefineKernelNode { node } => node.ensure_v3(),
            _ => Ok(()),
        }
    }

    pub(crate) fn ensure_v4(&self) -> Result<(), Diagnostic> {
        match self {
            Self::DefineKernelNode { node } => node.ensure_v4(),
            _ => Ok(()),
        }
    }

    pub(crate) fn ensure_v5(&self) -> Result<(), Diagnostic> {
        match self {
            Self::DefineKernelNode { node } => node.ensure_v5(),
            _ => Ok(()),
        }
    }

    pub(crate) fn ensure_v6(&self) -> Result<(), Diagnostic> {
        match self {
            Self::DefineKernelNode { node } => node.ensure_v6(),
            _ => Ok(()),
        }
    }

    pub(crate) fn ensure_value_shape_limits(
        &self,
        limits: DecoderLimits,
    ) -> Result<(), Diagnostic> {
        match self {
            Self::DefineKernelNode { node } => node.ensure_value_shape_limits(limits),
            _ => Ok(()),
        }
    }

    pub(crate) fn canonicalize_sets(&mut self) -> Result<(), Diagnostic> {
        if let Self::DefineModelView { view } = self {
            view.members.sort();
            reject_duplicates(&view.members, "model-view member")?;
            view.boundary.sort();
            reject_duplicates(&view.boundary, "model-view boundary")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelOperationWireVersion {
    V1,
    V2,
    V3,
    V4,
    V5,
    V6,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "condition", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum WireModelPrecondition {
    ValueEquals {
        target: WireId,
        expected: WireQuantity,
    },
    RevisionIs {
        revision: u64,
    },
}

impl WireModelPrecondition {
    pub(crate) fn encode(precondition: &Precondition) -> Result<Self, Diagnostic> {
        match precondition {
            Precondition::ValueEquals { target, expected } => {
                require_semantic_id(*target, "ValueEquals target")?;
                Ok(Self::ValueEquals {
                    target: WireId::from_raw(*target),
                    expected: WireQuantity::encode(*expected),
                })
            }
            Precondition::RevisionIs(revision) => Ok(Self::RevisionIs {
                revision: revision.0,
            }),
            _ => Err(invalid_artifact(
                "precondition is newer than the supported model transaction wire vocabulary",
            )),
        }
    }

    pub(crate) fn decode(&self) -> Result<Precondition, Diagnostic> {
        match self {
            Self::ValueEquals { target, expected } => Ok(Precondition::ValueEquals {
                target: target.decode_raw()?,
                expected: expected.decode()?,
            }),
            Self::RevisionIs { revision } => Ok(Precondition::RevisionIs(Revision(*revision))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireModelView {
    ulid: String,
    members: Vec<WireId>,
    boundary: Vec<WireId>,
}

impl WireModelView {
    fn encode(view: &ModelView) -> Self {
        Self {
            ulid: view.id().ulid().to_string(),
            members: view
                .members()
                .iter()
                .copied()
                .map(WireId::from_raw)
                .collect(),
            boundary: view
                .boundary()
                .iter()
                .copied()
                .map(WireId::from_raw)
                .collect(),
        }
    }

    fn decode(&self) -> Result<ModelView, Diagnostic> {
        ensure_sorted_unique(&self.members, "model-view member")?;
        ensure_sorted_unique(&self.boundary, "model-view boundary")?;
        let members = self
            .members
            .iter()
            .map(WireId::decode_raw)
            .collect::<Result<Vec<_>, _>>()?;
        let boundary = self
            .boundary
            .iter()
            .map(WireId::decode_raw)
            .collect::<Result<Vec<_>, _>>()?;
        ModelView::new(
            OntologyId::from_ulid(parse_ulid(&self.ulid)?),
            members,
            boundary,
        )
        .map_err(|error| invalid_artifact(error.message()))
    }
}

fn require_semantic_id(id: RawId, label: &str) -> Result<(), Diagnostic> {
    if id.kind().graph() == GraphClass::Semantic && is_wire_kernel_kind(id.kind()) {
        Ok(())
    } else {
        Err(invalid_artifact(format!(
            "{label} {id} is outside the Semantic Model wire scope"
        )))
    }
}

const fn is_wire_kernel_kind(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Domain
            | EntityKind::Representation
            | EntityKind::Field
            | EntityKind::Parameter
            | EntityKind::Port
            | EntityKind::Relation
            | EntityKind::Activation
            | EntityKind::Connection
            | EntityKind::ClockDomain
    )
}

fn ensure_sorted_unique<T: Ord>(values: &[T], label: &str) -> Result<(), Diagnostic> {
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        Err(invalid_artifact(format!(
            "{label} IDs must be sorted and unique in canonical wire data"
        )))
    } else {
        Ok(())
    }
}

fn reject_duplicates<T: PartialEq>(values: &[T], label: &str) -> Result<(), Diagnostic> {
    if values.windows(2).any(|pair| pair[0] == pair[1]) {
        Err(invalid_artifact(format!(
            "model transaction contains a duplicate {label} ID"
        )))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_compiler::compile;
    use eqiora_core::diagnostic::codes;
    use eqiora_core::entity::kinds;
    use eqiora_core::{DimExponents, DynQuantity, Id};
    use eqiora_graph::{GraphStore, InMemoryGraphStore};
    use eqiora_sem::KernelProgram;

    const POISSON: &str =
        include_str!("../../../verify/numerics/poisson-fem-fvm/models/poisson.eqi");

    #[test]
    fn compiler_transaction_round_trips_and_commits() {
        let mut compiled = compile("poisson.eqi", POISSON).unwrap();
        let compiled = compiled.remove(0);
        let model = compiled.model();
        let envelope = ModelTransactionEnvelopeV1::from_transaction(compiled.transaction())
            .expect("compiler emits an admitted model transaction");
        let bytes = envelope.canonical_json().unwrap();
        let digest = envelope.digest().unwrap();
        let decoded =
            ModelTransactionEnvelopeV1::from_json(&bytes, DecoderLimits::default()).unwrap();
        assert_eq!(decoded.canonical_json().unwrap(), bytes);
        assert_eq!(decoded.digest().unwrap(), digest);

        let transaction = decoded.to_transaction().unwrap();
        assert_eq!(transaction.ops(), compiled.transaction().ops());
        assert_eq!(
            transaction.preconditions(),
            compiled.transaction().preconditions()
        );
        let mut store = InMemoryGraphStore::new();
        store.commit(transaction).unwrap();
        KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
    }

    #[test]
    fn model_view_sets_are_canonicalized_and_duplicates_rejected() {
        let mut compiled = compile("poisson.eqi", POISSON).unwrap();
        let compiled = compiled.remove(0);
        let canonical = ModelTransactionEnvelopeV1::from_transaction(compiled.transaction())
            .unwrap()
            .canonical_json()
            .unwrap();
        let mut reordered: serde_json::Value = serde_json::from_slice(&canonical).unwrap();
        let view = reordered["ops"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|op| op["op"] == "define-model-view")
            .unwrap();
        view["view"]["members"].as_array_mut().unwrap().reverse();
        let reordered = serde_json::to_vec(&reordered).unwrap();
        assert_eq!(
            ModelTransactionEnvelopeV1::from_json(&reordered, DecoderLimits::default())
                .unwrap()
                .canonical_json()
                .unwrap(),
            canonical
        );

        let mut duplicate: serde_json::Value = serde_json::from_slice(&canonical).unwrap();
        let view = duplicate["ops"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|op| op["op"] == "define-model-view")
            .unwrap();
        let member = view["view"]["members"][0].clone();
        view["view"]["members"].as_array_mut().unwrap().push(member);
        assert_eq!(
            ModelTransactionEnvelopeV1::from_json(
                &serde_json::to_vec(&duplicate).unwrap(),
                DecoderLimits::default(),
            )
            .unwrap_err()
            .code(),
            codes::INVALID_ARTIFACT
        );
    }

    #[test]
    fn preconditions_and_operation_order_survive_round_trip() {
        let field = Id::<kinds::Field>::new();
        let value = DynQuantity::new(2.0, DimExponents::DIMENSIONLESS);
        let mut transaction = Transaction::new("ordered edit");
        transaction
            .require(Precondition::RevisionIs(Revision(4)))
            .require(Precondition::ValueEquals {
                target: field.erase(),
                expected: value,
            })
            .push(Op::SetValue {
                target: field.erase(),
                value,
            })
            .push(Op::RemoveNode { id: field.erase() });

        let envelope = ModelTransactionEnvelopeV1::from_transaction(&transaction).unwrap();
        let decoded = envelope.to_transaction().unwrap();
        assert_eq!(decoded.ops(), transaction.ops());
        assert_eq!(decoded.preconditions(), transaction.preconditions());
    }

    #[test]
    fn infrastructure_and_unknown_ontology_are_rejected() {
        let mut infrastructure = Transaction::new("not a model edit");
        infrastructure.push(Op::AddNode {
            kind: EntityKind::Target,
            id: Id::<kinds::Target>::new().erase(),
        });
        assert_eq!(
            ModelTransactionEnvelopeV1::from_transaction(&infrastructure)
                .unwrap_err()
                .code(),
            codes::INVALID_ARTIFACT
        );

        let relation = Id::<kinds::Relation>::new().erase();
        let coupling = eqiora_schema::CouplingView::new(OntologyId::new(), [relation], []).unwrap();
        let mut other_ontology = Transaction::new("not a model view");
        other_ontology.push(Op::DefineOntologyView {
            view: coupling.into(),
        });
        assert_eq!(
            ModelTransactionEnvelopeV1::from_transaction(&other_ontology)
                .unwrap_err()
                .code(),
            codes::INVALID_ARTIFACT
        );
    }

    #[test]
    fn operation_expression_and_json_limits_fail_closed() {
        let mut compiled = compile("poisson.eqi", POISSON).unwrap();
        let compiled = compiled.remove(0);
        let bytes = ModelTransactionEnvelopeV1::from_transaction(compiled.transaction())
            .unwrap()
            .canonical_json()
            .unwrap();

        for limits in [
            DecoderLimits {
                max_transaction_ops: 1,
                ..DecoderLimits::default()
            },
            DecoderLimits {
                max_expression_nodes: 1,
                ..DecoderLimits::default()
            },
            DecoderLimits {
                max_bytes: bytes.len() - 1,
                ..DecoderLimits::default()
            },
        ] {
            assert_eq!(
                ModelTransactionEnvelopeV1::from_json(&bytes, limits)
                    .unwrap_err()
                    .code(),
                codes::INVALID_ARTIFACT
            );
        }
    }
}
