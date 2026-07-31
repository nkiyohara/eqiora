//! Exact quantitative edits through ordinary Model value transactions.
//!
//! This module owns optimistic value-edit planning and the private transaction
//! seam shared with geometry regeneration. It does not own geometry impact
//! discovery or permit coordinate Parameters to bypass that owner.

use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, DynQuantity, EntityKind, RawId};
use eqiora_graph::{EdgeKind, GraphStore, Op, Precondition, Revision, Transaction};
use eqiora_schema::kernel::KernelNode;

use crate::{ModelDocument, ModelTransactionEnvelope, single_diagnostic};

/// One exact, optimistic-concurrency-checked quantitative Model edit.
///
/// The plan owns the same versioned transaction wire used by language and
/// binding clients. Presentation adapters may show its before/after values and
/// replay key, but cannot bypass graph validation or silently retarget it.
#[derive(Debug, Clone, PartialEq)]
pub struct ValueEditPlan {
    base_digest: String,
    base_revision: Revision,
    target: RawId,
    before: DynQuantity,
    after: DynQuantity,
    transaction: ModelTransactionEnvelope,
    transaction_digest: String,
}

impl ValueEditPlan {
    /// Versioned exact key over the base artifact and transaction wire.
    #[must_use]
    pub fn key(&self) -> String {
        format!(
            "eqiora.value-edit-plan/v1:{}:{}",
            self.base_digest, self.transaction_digest
        )
    }

    /// Canonical Model content identity against which the edit was prepared.
    #[must_use]
    pub fn base_digest(&self) -> &str {
        &self.base_digest
    }

    /// Graph revision required by the edit transaction.
    #[must_use]
    pub const fn base_revision(&self) -> Revision {
        self.base_revision
    }

    /// Stable Field or Parameter identity targeted by the edit.
    #[must_use]
    pub const fn target(&self) -> RawId {
        self.target
    }

    /// Value required by the optimistic precondition.
    #[must_use]
    pub const fn before(&self) -> DynQuantity {
        self.before
    }

    /// Replacement value in coherent SI units with unchanged dimension.
    #[must_use]
    pub const fn after(&self) -> DynQuantity {
        self.after
    }

    /// Domain-separated identity of the exact ordered transaction wire.
    #[must_use]
    pub fn transaction_digest(&self) -> &str {
        &self.transaction_digest
    }

    /// Canonical bytes of the shared Model-transaction envelope.
    ///
    /// # Errors
    /// Returns an artifact diagnostic if serialization unexpectedly fails.
    pub fn transaction_json(&self) -> Result<Vec<u8>, Diagnostic> {
        self.transaction.canonical_json()
    }
}

/// One accepted value edit and the immutable child Model it produced.
#[derive(Debug, Clone, PartialEq)]
pub struct ValueEditResult {
    plan: ValueEditPlan,
    document: ModelDocument,
    result_digest: String,
}

impl ValueEditResult {
    /// Exact plan committed by the graph store.
    #[must_use]
    pub const fn plan(&self) -> &ValueEditPlan {
        &self.plan
    }

    /// Canonical child Model identity.
    #[must_use]
    pub fn result_digest(&self) -> &str {
        &self.result_digest
    }

    /// Child graph revision produced by the atomic commit.
    #[must_use]
    pub fn result_revision(&self) -> Revision {
        self.document.program.revision()
    }

    /// Borrow the immutable child Model.
    #[must_use]
    pub const fn document(&self) -> &ModelDocument {
        &self.document
    }

    /// Transfer ownership of the child Model to a cache or binding.
    #[must_use]
    pub fn into_document(self) -> ModelDocument {
        self.document
    }
}

impl ModelDocument {
    /// Resolve one finite Field/Parameter value change into the shared,
    /// versioned Model-transaction wire without mutating this document.
    ///
    /// The transaction requires both the current graph revision and the exact
    /// current quantity. A no-op, non-finite value, missing target, or target
    /// outside the quantitative node vocabulary is rejected before commit.
    ///
    /// # Errors
    /// Returns a structured graph or artifact diagnostic when the edit cannot
    /// be represented exactly.
    pub fn preview_value_edit(
        &self,
        target: RawId,
        new_value_si: f64,
    ) -> Result<ValueEditPlan, Diagnostic> {
        if !new_value_si.is_finite() {
            return Err(Diagnostic::error(
                codes::INVALID_OPERATION,
                "model value edits require one finite coherent-SI scalar",
            ));
        }
        let Some(node) = self.program.node(target) else {
            return Err(Diagnostic::error(
                codes::NODE_NOT_FOUND,
                format!("value-edit target {target} is outside this model revision"),
            ));
        };
        if !matches!(node.id().kind(), EntityKind::Field | EntityKind::Parameter) {
            return Err(Diagnostic::error(
                codes::INVALID_OPERATION,
                format!("value edits are not valid for {:?}", node.id().kind()),
            ));
        }
        if matches!(node.id().kind(), EntityKind::Parameter)
            && self.program.edges().iter().any(|edge| {
                edge.kind() == EdgeKind::DependsOn
                    && edge.to() == target
                    && matches!(self.program.node(edge.from()), Some(KernelNode::Domain(_)))
            })
        {
            return Err(Diagnostic::error(
                codes::INVALID_OPERATION,
                "value edit cannot target a Cartesian coordinate Parameter; the geometry regeneration owner currently accepts one 3D Domain",
            ));
        }
        let Some(before) = self.program.value(target) else {
            return Err(Diagnostic::error(
                codes::INVALID_OPERATION,
                format!("value-edit target {target} has no revision-local scalar value"),
            ));
        };
        let after = DynQuantity::new(new_value_si, before.dim());
        if before == after {
            return Err(Diagnostic::error(
                codes::INVALID_OPERATION,
                "value edit would not change canonical model content",
            ));
        }

        let label = self.value_edit_label(target);
        let base_revision = self.store.revision();
        let (transaction, transaction_digest) =
            self.prepare_value_transaction(target, before, after, label)?;
        Ok(ValueEditPlan {
            base_digest: self.digest()?,
            base_revision,
            target,
            before,
            after,
            transaction,
            transaction_digest,
        })
    }

    /// Replay and atomically commit one exact value-edit plan, returning a new
    /// immutable document while leaving this base document unchanged.
    ///
    /// # Errors
    /// Returns structured diagnostics if Model identity, transaction identity,
    /// optimistic preconditions, graph invariants, or artifact replay differ
    /// from the accepted preview.
    pub fn commit_value_edit(
        &self,
        plan: ValueEditPlan,
    ) -> Result<ValueEditResult, Vec<Diagnostic>> {
        if plan.base_digest != self.digest().map_err(single_diagnostic)?
            || plan.base_revision != self.store.revision()
        {
            return Err(single_diagnostic(Diagnostic::error(
                codes::PRECONDITION_FAILED,
                "value-edit plan no longer matches the selected Model revision",
            )));
        }
        let document = self.replay_model_transaction(
            &plan.transaction,
            &plan.transaction_digest,
            "value-edit transaction identity changed during replay",
        )?;
        let result_digest = document.digest().map_err(single_diagnostic)?;
        Ok(ValueEditResult {
            plan,
            document,
            result_digest,
        })
    }

    pub(crate) fn prepare_value_transaction(
        &self,
        target: RawId,
        before: DynQuantity,
        after: DynQuantity,
        label: String,
    ) -> Result<(ModelTransactionEnvelope, String), Diagnostic> {
        let mut transaction = Transaction::new(label);
        transaction
            .require(Precondition::RevisionIs(self.store.revision()))
            .require(Precondition::ValueEquals {
                target,
                expected: before,
            })
            .push(Op::SetValue {
                target,
                value: after,
            });
        let transaction = ModelTransactionEnvelope::from_transaction(&transaction)?;
        let transaction_digest = transaction.digest()?.to_string();
        Ok((transaction, transaction_digest))
    }

    pub(crate) fn replay_model_transaction(
        &self,
        transaction: &ModelTransactionEnvelope,
        transaction_digest: &str,
        identity_error: &'static str,
    ) -> Result<ModelDocument, Vec<Diagnostic>> {
        let bytes = transaction.canonical_json().map_err(single_diagnostic)?;
        let replay = ModelTransactionEnvelope::from_json(
            &bytes,
            eqiora_artifact::ModelDecoderLimits::default(),
        )
        .map_err(single_diagnostic)?;
        if replay.digest().map_err(single_diagnostic)?.to_string() != transaction_digest {
            return Err(single_diagnostic(Diagnostic::error(
                codes::INVALID_ARTIFACT,
                identity_error,
            )));
        }

        let mut store = self.store.clone();
        store.commit(replay.to_transaction().map_err(single_diagnostic)?)?;
        let program =
            eqiora_sem::KernelProgram::from_snapshot(&store.snapshot(), self.program.model())?;
        ModelDocument::from_store(store, program, self.aliases.clone())
    }

    pub(crate) fn value_edit_label(&self, target: RawId) -> String {
        self.aliases
            .iter()
            .find_map(|(name, &id)| (id == target).then_some(name.as_str()))
            .map_or_else(
                || format!("set model value {target}"),
                |name| format!("set model value {name}"),
            )
    }
}
