//! Exact, topology-preserving edits of Cartesian Semantic Domains.
//!
//! This module owns the narrow mutation seam between immutable Model revisions.
//! It does not own CAD features, source rewriting, meshing, or a second geometry
//! meaning. An edit is one ordinary versioned Model transaction whose complete
//! child Model is replayed during preview and again during commit.

use std::collections::BTreeSet;

use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, Id};
use eqiora_graph::{GraphStore, Op, Precondition, Revision, Transaction};
use eqiora_schema::kernel::{
    AxisBounds, CartesianCoordinateSource, DomainDef, DomainKind, KernelNode,
};
use sha2::{Digest, Sha256};

use crate::{ExactModelCodec, ModelDocument, VersionedModelTransactionEnvelope, single_diagnostic};

const CARTESIAN_DOMAIN_EDIT_PLAN: &[u8] = b"eqiora.cartesian-domain-edit-plan/v1";

/// One exact, topology-preserving Cartesian Domain edit set.
///
/// The plan owns the same versioned transaction wire used by other Model
/// clients. The expected child digest is computed by a complete atomic replay
/// during preview, so commit cannot silently admit a different Model.
#[derive(Debug, Clone, PartialEq)]
pub struct CartesianDomainEditPlan {
    key: String,
    base_digest: String,
    base_revision: Revision,
    target: Id<kinds::Domain>,
    edits: Vec<(usize, AxisBounds, AxisBounds)>,
    transaction: VersionedModelTransactionEnvelope,
    transaction_digest: String,
    expected_child_digest: String,
}

impl CartesianDomainEditPlan {
    /// Content key over the exact base, transaction, and expected child.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
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

    /// Stable Cartesian body Domain identity retained by the child Model.
    #[must_use]
    pub const fn target(&self) -> Id<kinds::Domain> {
        self.target
    }

    /// Canonical axis-keyed edits as `(axis, before, after)`, ordered by axis.
    #[must_use]
    pub fn edits(&self) -> &[(usize, AxisBounds, AxisBounds)] {
        &self.edits
    }

    /// Domain-separated identity of the exact ordered transaction wire.
    #[must_use]
    pub fn transaction_digest(&self) -> &str {
        &self.transaction_digest
    }

    /// Canonical child Model digest proved during preview.
    #[must_use]
    pub fn expected_child_digest(&self) -> &str {
        &self.expected_child_digest
    }

    /// Exact transaction codec retained by this immutable plan.
    #[must_use]
    pub const fn exact_codec(&self) -> ExactModelCodec {
        self.transaction.exact_codec()
    }

    /// Canonical bytes of the shared Model transaction envelope.
    ///
    /// # Errors
    /// Returns an artifact diagnostic if serialization unexpectedly fails.
    pub fn transaction_json(&self) -> Result<Vec<u8>, Diagnostic> {
        self.transaction.canonical_json()
    }
}

/// One accepted Cartesian Domain edit and its immutable child Model.
#[derive(Debug, Clone, PartialEq)]
pub struct CartesianDomainEditResult {
    plan: CartesianDomainEditPlan,
    document: ModelDocument,
}

impl CartesianDomainEditResult {
    /// Exact plan committed by the graph store.
    #[must_use]
    pub const fn plan(&self) -> &CartesianDomainEditPlan {
        &self.plan
    }

    /// Canonical child Model identity.
    #[must_use]
    pub fn result_digest(&self) -> &str {
        &self.plan.expected_child_digest
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

    /// Transfer ownership of the child Model to a cache or client adapter.
    #[must_use]
    pub fn into_document(self) -> ModelDocument {
        self.document
    }
}

impl ModelDocument {
    /// Resolve one non-empty axis-keyed interval edit set on the sole three-dimensional
    /// Cartesian body without mutating this Model.
    ///
    /// The preview preserves the body and oriented boundary Domain identities,
    /// Model membership, and every incident edge. It fully replays the
    /// versioned transaction and records the resulting child Model digest.
    ///
    /// # Errors
    /// Returns structured diagnostics when the selected Model is not the
    /// current v8 profile, does not contain exactly one 3D Cartesian body, the
    /// target or any axis is wrong, any edit is duplicated or a no-op, or
    /// complete child replay violates a graph, semantic, or artifact invariant.
    pub fn preview_cartesian_domain_edit<I>(
        &self,
        target: Id<kinds::Domain>,
        edits: I,
    ) -> Result<CartesianDomainEditPlan, Vec<Diagnostic>>
    where
        I: IntoIterator<Item = (usize, AxisBounds)>,
    {
        if self.exact_codec() != ExactModelCodec::CURRENT {
            return Err(single_diagnostic(invalid_edit(
                "Cartesian Domain edit v1 requires the current Model wire",
            )));
        }

        let body_count = self
            .program
            .nodes()
            .filter(|node| {
                matches!(
                    node,
                    KernelNode::Domain(domain)
                        if matches!(domain.kind(), DomainKind::CartesianBox { .. })
                )
            })
            .count();
        if body_count != 1 {
            return Err(single_diagnostic(invalid_edit(format!(
                "Cartesian Domain edit v1 requires exactly one body, found {body_count}",
            ))));
        }

        let target_raw = target.erase();
        let Some(KernelNode::Domain(domain)) = self.program.node(target_raw) else {
            return Err(single_diagnostic(Diagnostic::error(
                codes::NODE_NOT_FOUND,
                format!("Cartesian Domain edit target {target_raw} is outside this Model"),
            )));
        };
        let DomainKind::CartesianBox { coordinates } = domain.kind() else {
            return Err(single_diagnostic(invalid_edit(
                "Cartesian Domain edit target is not a Cartesian body",
            )));
        };
        if coordinates.iter().any(|axis| {
            !matches!(axis.lower(), CartesianCoordinateSource::Fixed(_))
                || !matches!(axis.upper(), CartesianCoordinateSource::Fixed(_))
        }) {
            return Err(single_diagnostic(invalid_edit(
                "direct Cartesian Domain edit does not admit a Parameter-backed coordinate",
            )));
        }
        let bounds = self
            .program
            .resolved_cartesian_bounds(target)
            .map_err(|diagnostic| vec![diagnostic])?;
        if bounds.len() != 3 {
            return Err(single_diagnostic(invalid_edit(format!(
                "Cartesian Domain edit v1 requires three dimensions, found {}",
                bounds.len(),
            ))));
        }
        let mut requested = edits.into_iter().collect::<Vec<_>>();
        if requested.is_empty() {
            return Err(single_diagnostic(invalid_edit(
                "Cartesian Domain edit set must not be empty",
            )));
        }
        requested.sort_by_key(|(axis, _)| *axis);

        let mut validation_diagnostics = Vec::new();
        let mut seen_axes = BTreeSet::new();
        let mut replacement_bounds = bounds.to_vec();
        let mut canonical_edits = Vec::with_capacity(requested.len());
        for (axis, after) in requested {
            if !seen_axes.insert(axis) {
                validation_diagnostics.push(invalid_edit(format!(
                    "Cartesian Domain edit axis {axis} occurs more than once",
                )));
                continue;
            }
            let Some(&before) = bounds.get(axis) else {
                validation_diagnostics.push(invalid_edit(format!(
                    "Cartesian Domain edit axis {axis} is outside three-dimensional geometry",
                )));
                continue;
            };
            if before == after {
                validation_diagnostics.push(invalid_edit(format!(
                    "Cartesian Domain edit axis {axis} would not change canonical Model content",
                )));
                continue;
            }
            replacement_bounds[axis] = after;
            canonical_edits.push((axis, before, after));
        }
        if !validation_diagnostics.is_empty() {
            return Err(validation_diagnostics);
        }

        let replacement_domain =
            DomainDef::cartesian_box(target, replacement_bounds).map_err(single_diagnostic)?;

        let mut incident_edges = self
            .program
            .edges()
            .iter()
            .filter(|edge| edge.from() == target_raw || edge.to() == target_raw)
            .copied()
            .collect::<Vec<_>>();
        incident_edges.sort_unstable();
        let base_revision = self.store.revision();
        let summary = if canonical_edits.len() == 1 {
            "edit Cartesian Domain interval"
        } else {
            "edit Cartesian Domain intervals"
        };
        let mut transaction = Transaction::new(summary);
        transaction
            .require(Precondition::RevisionIs(base_revision))
            .push(Op::RemoveNode { id: target_raw })
            .push(Op::DefineKernelNode {
                node: KernelNode::from(replacement_domain),
            });
        for edge in incident_edges {
            transaction.push(Op::Connect {
                from: edge.from(),
                to: edge.to(),
                edge: edge.kind(),
            });
        }

        let transaction = self
            .exact_codec()
            .encode_transaction(&transaction)
            .map_err(single_diagnostic)?;
        let transaction_digest = transaction.digest().map_err(single_diagnostic)?;
        let child = replay_edit(self, &transaction, &transaction_digest)?;
        let base_digest = self.digest().map_err(single_diagnostic)?;
        let expected_child_digest = child.digest().map_err(single_diagnostic)?;
        let key = plan_key(&base_digest, &transaction_digest, &expected_child_digest);

        Ok(CartesianDomainEditPlan {
            key,
            base_digest,
            base_revision,
            target,
            edits: canonical_edits,
            transaction,
            transaction_digest,
            expected_child_digest,
        })
    }

    /// Replay and atomically commit one exact Cartesian Domain edit.
    ///
    /// The base document remains immutable. A plan prepared from a stale or
    /// same-revision foreign Model fails before graph mutation.
    ///
    /// # Errors
    /// Returns structured diagnostics if Model identity, transaction identity,
    /// graph/semantic invariants, or the previewed child digest differ.
    pub fn commit_cartesian_domain_edit(
        &self,
        plan: CartesianDomainEditPlan,
    ) -> Result<CartesianDomainEditResult, Vec<Diagnostic>> {
        if plan.exact_codec() != self.exact_codec()
            || plan.base_digest != self.digest().map_err(single_diagnostic)?
            || plan.base_revision != self.store.revision()
        {
            return Err(single_diagnostic(Diagnostic::error(
                codes::PRECONDITION_FAILED,
                "Cartesian Domain edit plan no longer matches the selected Model revision",
            )));
        }

        let document = replay_edit(self, &plan.transaction, &plan.transaction_digest)?;
        let result_digest = document.digest().map_err(single_diagnostic)?;
        if result_digest != plan.expected_child_digest {
            return Err(single_diagnostic(Diagnostic::error(
                codes::INVALID_ARTIFACT,
                "Cartesian Domain edit child differs from the exact preview",
            )));
        }

        Ok(CartesianDomainEditResult { plan, document })
    }
}

fn replay_edit(
    base: &ModelDocument,
    transaction: &VersionedModelTransactionEnvelope,
    transaction_digest: &str,
) -> Result<ModelDocument, Vec<Diagnostic>> {
    let bytes = transaction.canonical_json().map_err(single_diagnostic)?;
    let replay = base
        .exact_codec()
        .decode_transaction(&bytes)
        .map_err(single_diagnostic)?;
    if replay.digest().map_err(single_diagnostic)? != transaction_digest {
        return Err(single_diagnostic(Diagnostic::error(
            codes::INVALID_ARTIFACT,
            "Cartesian Domain edit transaction identity changed during replay",
        )));
    }

    let mut store = base.store.clone();
    store.commit(replay.to_transaction().map_err(single_diagnostic)?)?;
    let program =
        eqiora_sem::KernelProgram::from_snapshot(&store.snapshot(), base.program.model())?;
    ModelDocument::from_store(store, program, base.aliases.clone(), base.exact_codec())
}

fn plan_key(base_digest: &str, transaction_digest: &str, child_digest: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CARTESIAN_DOMAIN_EDIT_PLAN);
    for value in [base_digest, transaction_digest, child_digest] {
        hasher.update([0]);
        hasher.update(value.as_bytes());
    }
    format!(
        "eqiora.cartesian-domain-edit-plan/v1:{}",
        hex_digest(hasher.finalize().into())
    )
}

fn hex_digest(bytes: [u8; 32]) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(64);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn invalid_edit(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_OPERATION, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_core::{DimExponents, DynQuantity};

    const BASE: &str =
        include_str!("../../../verify/geometry/cad-semantic-selection-box/models/base.eqi");
    const BASE_BOX: &str = "box(-0.5, 0.5, -0.5, 0.5, -0.5, 0.5)";
    const TARGET_BOX: &str = "box(-0.6, 0.6, -0.5, 0.5, -0.5, 0.5)";
    const TWO_DIMENSIONAL: &str = r"
model Plane {
  domain body = box(-0.5, 0.5, -0.5, 0.5);
  representation scalar_space = continuum;
  field witness on body as scalar_space: 1 = 0;
  relation retain_body continuous on body { witness = 0; }
}
";
    const MULTI_BODY: &str = r"
model Pair {
  domain body = box(-0.5, 0.5, -0.5, 0.5, -0.5, 0.5);
  domain peer = box(1.0, 2.0, -0.5, 0.5, -0.5, 0.5);
  representation scalar_space = continuum;
  field witness on body as scalar_space: 1 = 0;
  relation retain_body continuous on body { witness = 0; }
}
";

    #[test]
    fn edit_is_exact_immutable_and_topology_preserving() {
        let base = ModelDocument::compile("base.eqi", BASE).unwrap();
        let independent_target = target_document("target.eqi");
        let body = domain(&base, "body");
        let before_edges = base.program().edges().to_vec();
        let before_ids = base
            .program()
            .nodes()
            .map(KernelNode::id)
            .collect::<Vec<_>>();
        let replacement = axis_bounds(-0.6, 0.6);

        let plan = base
            .preview_cartesian_domain_edit(body, [(0, replacement)])
            .unwrap();
        let repeated = base
            .preview_cartesian_domain_edit(body, [(0, replacement)])
            .unwrap();
        let distinct = base
            .preview_cartesian_domain_edit(body, [(0, axis_bounds(-0.7, 0.7))])
            .unwrap();
        assert_eq!(plan.target(), body);
        assert_eq!(plan.edits(), &[(0, axis_bounds(-0.5, 0.5), replacement)]);
        assert_ne!(plan.base_digest(), plan.expected_child_digest());
        assert_eq!(plan.exact_codec(), ExactModelCodec::CURRENT);
        assert_eq!(plan, repeated);
        assert_eq!(plan.key(), repeated.key());
        assert_eq!(plan.transaction_digest(), repeated.transaction_digest());
        assert_eq!(
            plan.transaction_json().unwrap(),
            repeated.transaction_json().unwrap()
        );
        assert_ne!(plan.key(), distinct.key());
        assert_ne!(plan.transaction_digest(), distinct.transaction_digest());
        assert_ne!(
            plan.expected_child_digest(),
            distinct.expected_child_digest()
        );
        let replay = plan
            .exact_codec()
            .decode_transaction(&plan.transaction_json().unwrap())
            .unwrap();
        assert_eq!(replay, plan.transaction);
        assert_eq!(replay.digest().unwrap(), plan.transaction_digest());

        let result = base.commit_cartesian_domain_edit(plan).unwrap();
        let child = result.document();
        assert_eq!(
            child.program().revision().0,
            base.program().revision().0 + 1
        );
        assert_eq!(
            child
                .program()
                .nodes()
                .map(KernelNode::id)
                .collect::<Vec<_>>(),
            before_ids
        );
        assert_eq!(child.program().edges(), before_edges);
        assert!(child.structurally_equivalent(&independent_target).unwrap());
        assert_eq!(result.result_digest(), child.digest().unwrap());

        let bounds = base.program().resolved_cartesian_bounds(body).unwrap();
        assert_eq!(bounds[0], axis_bounds(-0.5, 0.5));
    }

    #[test]
    fn stale_foreign_noop_and_wrong_axis_plans_fail_closed() {
        let base = ModelDocument::compile("base.eqi", BASE).unwrap();
        let base_digest = base.digest().unwrap();
        let body = domain(&base, "body");
        let boundary = domain(&base, "x_lower");
        let accepted = base
            .preview_cartesian_domain_edit(body, [(0, axis_bounds(-0.6, 0.6))])
            .unwrap();
        assert!(
            base.preview_cartesian_domain_edit(body, [(0, axis_bounds(-0.5, 0.5))])
                .is_err()
        );
        assert!(
            base.preview_cartesian_domain_edit(body, [(3, axis_bounds(-0.6, 0.6))])
                .is_err()
        );
        assert!(
            base.preview_cartesian_domain_edit(boundary, [(0, axis_bounds(-0.6, 0.6))])
                .is_err()
        );
        assert!(
            base.preview_cartesian_domain_edit(body, std::iter::empty())
                .is_err()
        );
        assert!(
            base.preview_cartesian_domain_edit(
                body,
                [(0, axis_bounds(-0.6, 0.6)), (0, axis_bounds(-0.7, 0.7)),],
            )
            .is_err()
        );

        let sibling_source = BASE.replacen("model Main", "model Sibling", 1);
        assert_ne!(sibling_source, BASE);
        let sibling = ModelDocument::compile("sibling.eqi", &sibling_source).unwrap();
        assert!(sibling.structurally_equivalent(&base).unwrap());
        assert_ne!(sibling.digest().unwrap(), base_digest);
        assert!(
            sibling
                .commit_cartesian_domain_edit(accepted.clone())
                .is_err()
        );

        let child = base
            .commit_cartesian_domain_edit(accepted.clone())
            .unwrap()
            .into_document();
        assert!(child.commit_cartesian_domain_edit(accepted).is_err());
        assert_eq!(base.digest().unwrap(), base_digest);
    }

    #[test]
    fn unsupported_codec_dimension_and_body_multiplicity_fail_closed() {
        let v5 = ExactModelCodec::V5.compile("v5.eqi", BASE).unwrap();
        let v5_body = domain(&v5, "body");
        assert!(
            v5.preview_cartesian_domain_edit(v5_body, [(0, axis_bounds(-0.6, 0.6))])
                .is_err()
        );

        let plane = ModelDocument::compile("plane.eqi", TWO_DIMENSIONAL).unwrap();
        let plane_body = domain(&plane, "body");
        assert!(
            plane
                .preview_cartesian_domain_edit(plane_body, [(0, axis_bounds(-0.6, 0.6))])
                .is_err()
        );

        let pair = ModelDocument::compile("pair.eqi", MULTI_BODY).unwrap();
        let pair_body = domain(&pair, "body");
        assert!(
            pair.preview_cartesian_domain_edit(pair_body, [(0, axis_bounds(-0.6, 0.6))])
                .is_err()
        );
    }

    fn domain(document: &ModelDocument, name: &str) -> Id<kinds::Domain> {
        document.aliases()[name].downcast().unwrap()
    }

    fn target_document(filename: &str) -> ModelDocument {
        let source = BASE.replacen(BASE_BOX, TARGET_BOX, 1);
        assert_ne!(source, BASE);
        ModelDocument::compile(filename, &source).unwrap()
    }

    fn axis_bounds(lower: f64, upper: f64) -> AxisBounds {
        let length = DimExponents {
            length: 1,
            ..DimExponents::DIMENSIONLESS
        };
        AxisBounds::new(
            DynQuantity::new(lower, length),
            DynQuantity::new(upper, length),
        )
        .unwrap()
    }
}
