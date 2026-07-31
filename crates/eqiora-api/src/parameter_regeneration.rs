//! Atomic regeneration of Parameter-backed Cartesian geometry.
//!
//! The coordinate recipe remains Semantic Model meaning. This owner changes
//! one exact root length Parameter through an ordinary value transaction,
//! proves the complete resolved Cartesian impact during preview, and replays
//! the same immutable child during commit.

use std::collections::BTreeSet;

use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, DimExponents, DynQuantity, Id, RawId};
use eqiora_graph::{EdgeKind, GraphStore, Revision};
use eqiora_schema::kernel::{AxisBounds, CartesianCoordinateSource, DomainKind, KernelNode};
use sha2::{Digest, Sha256};

use crate::geometry_edit::canonical_axis_differences;
use crate::{ExactModelCodec, ModelDocument, VersionedModelTransactionEnvelope, single_diagnostic};

const PARAMETER_GEOMETRY_REGENERATION_PLAN: &[u8] =
    b"eqiora.parameter-geometry-regeneration-plan/v1";

/// One exact root-Parameter change and its complete Cartesian geometry impact.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterGeometryRegenerationPlan {
    key: String,
    base_digest: String,
    base_revision: Revision,
    parameter: Id<kinds::Parameter>,
    before: DynQuantity,
    after: DynQuantity,
    domain: Id<kinds::Domain>,
    edits: Vec<(usize, AxisBounds, AxisBounds)>,
    transaction: VersionedModelTransactionEnvelope,
    transaction_digest: String,
    expected_child_digest: String,
}

impl ParameterGeometryRegenerationPlan {
    /// Content key over the exact base, transaction, and expected child.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Canonical Model content identity against which regeneration was prepared.
    #[must_use]
    pub fn base_digest(&self) -> &str {
        &self.base_digest
    }

    /// Graph revision required by the value transaction.
    #[must_use]
    pub const fn base_revision(&self) -> Revision {
        self.base_revision
    }

    /// Exact root length Parameter changed by this plan.
    #[must_use]
    pub const fn parameter(&self) -> Id<kinds::Parameter> {
        self.parameter
    }

    /// Parameter value required by the optimistic precondition.
    #[must_use]
    pub const fn before(&self) -> DynQuantity {
        self.before
    }

    /// Replacement Parameter value in coherent SI units.
    #[must_use]
    pub const fn after(&self) -> DynQuantity {
        self.after
    }

    /// Sole Cartesian Domain affected by the first bounded regeneration owner.
    #[must_use]
    pub const fn domain(&self) -> Id<kinds::Domain> {
        self.domain
    }

    /// Complete canonical axis differences as `(axis, before, after)`.
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

    /// Canonical bytes of the ordinary Model-transaction envelope.
    ///
    /// # Errors
    /// Returns an artifact diagnostic if serialization unexpectedly fails.
    pub fn transaction_json(&self) -> Result<Vec<u8>, Diagnostic> {
        self.transaction.canonical_json()
    }
}

/// One accepted Parameter-driven regeneration and its immutable child Model.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterGeometryRegenerationResult {
    plan: ParameterGeometryRegenerationPlan,
    document: ModelDocument,
}

impl ParameterGeometryRegenerationResult {
    /// Exact plan committed by the graph store.
    #[must_use]
    pub const fn plan(&self) -> &ParameterGeometryRegenerationPlan {
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

    /// Transfer ownership of the child Model to a client or cache.
    #[must_use]
    pub fn into_document(self) -> ModelDocument {
        self.document
    }
}

impl ModelDocument {
    /// Preview one atomic value transaction and its complete Cartesian impact.
    ///
    /// The first owner accepts one current-v8 root length Parameter referenced
    /// by exactly one three-dimensional Cartesian Domain. The Domain recipe,
    /// every node and edge, and all semantic identities remain unchanged.
    /// The axis edit set is complete over Cartesian geometry; the ordinary
    /// transaction and expected child Model digest also close any other exact
    /// semantic references to the same Parameter.
    ///
    /// # Errors
    /// Returns structured diagnostics for a stale vocabulary, absent or
    /// wrong-kind target, non-length or invalid value, absent or ambiguous
    /// geometry dependency, invalid successor bounds, or incomplete impact.
    pub fn preview_parameter_geometry_regeneration(
        &self,
        target: RawId,
        new_value_si: f64,
    ) -> Result<ParameterGeometryRegenerationPlan, Vec<Diagnostic>> {
        if self.exact_codec() != ExactModelCodec::CURRENT {
            return Err(single_diagnostic(invalid_regeneration(
                "Parameter geometry regeneration v1 requires the current Model wire",
            )));
        }
        if !new_value_si.is_finite() {
            return Err(single_diagnostic(invalid_regeneration(
                "Parameter geometry regeneration requires one finite coherent-SI scalar",
            )));
        }
        let Some(node) = self.program.node(target) else {
            return Err(single_diagnostic(Diagnostic::error(
                codes::NODE_NOT_FOUND,
                format!("regeneration target {target} is outside this Model revision"),
            )));
        };
        let KernelNode::Parameter(parameter) = node else {
            return Err(single_diagnostic(invalid_regeneration(format!(
                "Parameter geometry regeneration cannot target {:?}",
                node.id().kind(),
            ))));
        };
        let parameter = parameter.id();
        let Some(before) = self.program.value(target) else {
            return Err(single_diagnostic(invalid_regeneration(format!(
                "regeneration Parameter {target} has no revision-local scalar value",
            ))));
        };
        if before.dim() != length_dimension() {
            return Err(single_diagnostic(invalid_regeneration(
                "Parameter geometry regeneration requires one physical length Parameter",
            )));
        }
        let after = DynQuantity::new(canonical_zero(new_value_si), before.dim());
        if before == after {
            return Err(single_diagnostic(invalid_regeneration(
                "Parameter geometry regeneration would not change canonical Model content",
            )));
        }

        let mut domains = self
            .program
            .edges()
            .iter()
            .filter(|edge| edge.kind() == EdgeKind::DependsOn && edge.to() == target)
            .filter_map(|edge| edge.from().downcast::<kinds::Domain>())
            .collect::<Vec<_>>();
        domains.sort_by_key(|domain| domain.ulid());
        domains.dedup_by_key(|domain| domain.ulid());
        if domains.len() != 1 {
            return Err(single_diagnostic(invalid_regeneration(format!(
                "Parameter geometry regeneration v1 requires one affected Domain, found {}",
                domains.len(),
            ))));
        }
        let domain = domains[0];
        let affected_axes = parameter_axes(&self.program, domain, parameter)?;
        let before_bounds = self
            .program
            .resolved_cartesian_bounds(domain)
            .map_err(single_diagnostic)?;
        if before_bounds.len() != 3 {
            return Err(single_diagnostic(invalid_regeneration(format!(
                "Parameter geometry regeneration v1 requires three dimensions, found {}",
                before_bounds.len(),
            ))));
        }

        // The regeneration wire is independent of source aliases and caller
        // traversal order. RawId is the exact occurrence identity already
        // carried by the operation and its optimistic precondition.
        let label = format!("regenerate geometry parameter {target}");
        let (transaction, transaction_digest) = self
            .prepare_value_transaction(target, before, after, label)
            .map_err(single_diagnostic)?;
        let child = self.replay_model_transaction(
            &transaction,
            &transaction_digest,
            "Parameter geometry regeneration transaction identity changed during preview",
        )?;
        let after_bounds = child
            .program
            .resolved_cartesian_bounds(domain)
            .map_err(single_diagnostic)?;
        let edits =
            canonical_axis_differences(before_bounds, after_bounds).map_err(single_diagnostic)?;
        require_complete_impact(&edits, &affected_axes)?;

        let base_digest = self.digest().map_err(single_diagnostic)?;
        let expected_child_digest = child.digest().map_err(single_diagnostic)?;
        let key = plan_key(&base_digest, &transaction_digest, &expected_child_digest);
        Ok(ParameterGeometryRegenerationPlan {
            key,
            base_digest,
            base_revision: self.store.revision(),
            parameter,
            before,
            after,
            domain,
            edits,
            transaction,
            transaction_digest,
            expected_child_digest,
        })
    }

    /// Replay and atomically commit one exact Parameter geometry regeneration.
    ///
    /// The base document remains immutable. Commit repeats the complete
    /// Cartesian impact check and rejects any child that differs from preview.
    ///
    /// # Errors
    /// Returns structured diagnostics for stale/foreign plans, transaction
    /// drift, semantic replay failure, incomplete impact, or child mismatch.
    pub fn commit_parameter_geometry_regeneration(
        &self,
        plan: ParameterGeometryRegenerationPlan,
    ) -> Result<ParameterGeometryRegenerationResult, Vec<Diagnostic>> {
        if plan.exact_codec() != self.exact_codec()
            || plan.base_digest != self.digest().map_err(single_diagnostic)?
            || plan.base_revision != self.store.revision()
        {
            return Err(single_diagnostic(Diagnostic::error(
                codes::PRECONDITION_FAILED,
                "Parameter geometry regeneration plan no longer matches the selected Model revision",
            )));
        }
        let before_domain = self.program.node(plan.domain.erase()).cloned();
        let before_bounds = self
            .program
            .resolved_cartesian_bounds(plan.domain)
            .map_err(single_diagnostic)?;
        let document = self.replay_model_transaction(
            &plan.transaction,
            &plan.transaction_digest,
            "Parameter geometry regeneration transaction identity changed during replay",
        )?;
        let result_digest = document.digest().map_err(single_diagnostic)?;
        if result_digest != plan.expected_child_digest {
            return Err(single_diagnostic(Diagnostic::error(
                codes::INVALID_ARTIFACT,
                "Parameter geometry regeneration child differs from the exact preview",
            )));
        }
        if document.program.node(plan.domain.erase()).cloned() != before_domain
            || document.program.value(plan.parameter.erase()) != Some(plan.after)
        {
            return Err(single_diagnostic(Diagnostic::error(
                codes::INVALID_ARTIFACT,
                "Parameter geometry regeneration changed the recipe or wrong Parameter value",
            )));
        }
        let after_bounds = document
            .program
            .resolved_cartesian_bounds(plan.domain)
            .map_err(single_diagnostic)?;
        let edits =
            canonical_axis_differences(before_bounds, after_bounds).map_err(single_diagnostic)?;
        if edits != plan.edits {
            return Err(single_diagnostic(Diagnostic::error(
                codes::INVALID_ARTIFACT,
                "Parameter geometry regeneration impact differs from the exact preview",
            )));
        }

        Ok(ParameterGeometryRegenerationResult { plan, document })
    }
}

fn parameter_axes(
    program: &eqiora_sem::KernelProgram,
    domain: Id<kinds::Domain>,
    parameter: Id<kinds::Parameter>,
) -> Result<BTreeSet<usize>, Vec<Diagnostic>> {
    let Some(KernelNode::Domain(definition)) = program.node(domain.erase()) else {
        return Err(single_diagnostic(invalid_regeneration(
            "geometry dependency source is not a Domain",
        )));
    };
    let DomainKind::CartesianBox { coordinates } = definition.kind() else {
        return Err(single_diagnostic(invalid_regeneration(
            "geometry dependency source is not a Cartesian body",
        )));
    };
    let axes = coordinates
        .iter()
        .enumerate()
        .filter_map(|(axis, definition)| {
            [
                CartesianCoordinateSource::Parameter(parameter) == definition.lower(),
                CartesianCoordinateSource::Parameter(parameter) == definition.upper(),
            ]
            .into_iter()
            .any(|matches| matches)
            .then_some(axis)
        })
        .collect::<BTreeSet<_>>();
    if axes.is_empty() {
        return Err(single_diagnostic(invalid_regeneration(
            "geometry dependency does not name the selected Parameter in any endpoint",
        )));
    }
    Ok(axes)
}

fn require_complete_impact(
    edits: &[(usize, AxisBounds, AxisBounds)],
    affected_axes: &BTreeSet<usize>,
) -> Result<(), Vec<Diagnostic>> {
    let observed = edits
        .iter()
        .map(|(axis, _, _)| *axis)
        .collect::<BTreeSet<_>>();
    if &observed != affected_axes {
        return Err(single_diagnostic(invalid_regeneration(
            "resolved Cartesian edit set omits or adds an affected Parameter axis",
        )));
    }
    Ok(())
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn length_dimension() -> DimExponents {
    DimExponents {
        length: 1,
        ..DimExponents::DIMENSIONLESS
    }
}

fn plan_key(base_digest: &str, transaction_digest: &str, child_digest: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(PARAMETER_GEOMETRY_REGENERATION_PLAN);
    for value in [base_digest, transaction_digest, child_digest] {
        hasher.update([0]);
        hasher.update(value.as_bytes());
    }
    format!(
        "eqiora.parameter-geometry-regeneration-plan/v1:{}",
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

fn invalid_regeneration(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_OPERATION, message)
}
