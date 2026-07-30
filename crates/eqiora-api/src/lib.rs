//! Shared application operations for thin Eqiora clients.
//!
//! This L4 crate composes compiler, transaction wire, graph commit, immutable
//! model artifact, and reference execution contracts without introducing a
//! second model semantics. Python and Studio adapters add language- or
//! UI-specific ergonomics around these owned Rust values.

mod cad;
mod codec;
pub mod control;
mod differentiation;
#[cfg(any(feature = "vtu", feature = "xdmf"))]
mod external_data;
mod geometry_edit;
mod ml_dataset;
pub mod package;
mod reference_run;
mod remeshing_trajectory;
mod spatial;
mod spatial_data;
mod steady_stokes;
mod transient_fluid;

pub use cad::*;
pub use codec::ExactModelCodec;
pub(crate) use codec::VersionedModelTransactionEnvelope;
pub use differentiation::*;
pub use eqiora_artifact::{SemanticFingerprintGeneration, StructuralSemanticFingerprint};
#[cfg(any(feature = "vtu", feature = "xdmf"))]
pub use external_data::*;
pub use geometry_edit::{CartesianDomainEditPlan, CartesianDomainEditResult};
pub use ml_dataset::*;
pub use reference_run::*;
pub use remeshing_trajectory::RemeshingTrajectoryReplayInputV1;
#[cfg(feature = "hdf5")]
pub use remeshing_trajectory::{
    VerifiedXdmfHdf5TrajectoryExportV1, XdmfHdf5TrajectoryExportArtifactsV1,
    XdmfHdf5TrajectoryExportLimits, export_xdmf_hdf5_trajectory_v1,
    verify_xdmf_hdf5_trajectory_storage_v1,
};
pub use spatial::*;
pub use spatial_data::*;
pub use steady_stokes::CircularHoleSteadyStokesResult2d;
pub use transient_fluid::*;

use std::collections::BTreeMap;

use eqiora_artifact::{
    AcceptedModelArtifact, CanonicalModelArtifact, ModelArtifactGeneration, ModelArtifactReference,
    ModelDecoderLimits, ModelTransactionEnvelopeV1, ModelTransactionEnvelopeV2,
    ModelTransactionEnvelopeV3, ModelTransactionEnvelopeV4, ModelTransactionEnvelopeV5,
    ModelTransactionEnvelopeV6, ModelTransactionEnvelopeV7, ReplayableCanonicalModelArtifact,
};
use eqiora_compiler::{CompiledModel, ModelSymbols};
use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, DynQuantity, EntityKind, RawId};
use eqiora_graph::{GraphStore, InMemoryGraphStore, Op, Precondition, Revision, Transaction};
use eqiora_lang::ModelDraft;
use eqiora_sem::KernelProgram;
use serde::{Deserialize, Serialize};

/// One exact, optimistic-concurrency-checked quantitative model edit.
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
    transaction: VersionedModelTransactionEnvelope,
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

    /// Canonical model content identity against which the edit was prepared.
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

    /// Exact transaction codec retained by this immutable plan.
    #[must_use]
    pub const fn exact_codec(&self) -> ExactModelCodec {
        self.transaction.exact_codec()
    }

    /// Canonical bytes of the shared model-transaction envelope.
    ///
    /// # Errors
    /// Returns an artifact diagnostic if serialization unexpectedly fails.
    pub fn transaction_json(&self) -> Result<Vec<u8>, Diagnostic> {
        self.transaction.canonical_json()
    }
}

/// One accepted value edit and the immutable child model it produced.
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

    /// Canonical child model identity.
    #[must_use]
    pub fn result_digest(&self) -> &str {
        &self.result_digest
    }

    /// Child graph revision produced by the atomic commit.
    #[must_use]
    pub fn result_revision(&self) -> Revision {
        self.document.program.revision()
    }

    /// Borrow the immutable child model.
    #[must_use]
    pub const fn document(&self) -> &ModelDocument {
        &self.document
    }

    /// Transfer ownership of the child model to a cache or binding.
    #[must_use]
    pub fn into_document(self) -> ModelDocument {
        self.document
    }
}

/// One immutable, validated canonical model revision plus non-semantic source
/// aliases used by client presentation layers.
#[derive(Debug, Clone)]
pub struct ModelDocument {
    program: KernelProgram,
    artifact: AcceptedModelArtifact,
    exact_codec: ExactModelCodec,
    aliases: BTreeMap<String, RawId>,
    store: InMemoryGraphStore,
}

impl PartialEq for ModelDocument {
    fn eq(&self, other: &Self) -> bool {
        self.program == other.program
            && self.artifact == other.artifact
            && self.exact_codec == other.exact_codec
            && self.aliases == other.aliases
    }
}

impl ModelDocument {
    /// Compile exactly one source model with the current semantic vocabulary.
    ///
    /// The current profile selects its artifact codec internally. Callers that
    /// reproduce a historical artifact must use [`ExactModelCodec::compile`].
    ///
    /// # Errors
    /// Returns all compiler/semantic diagnostics, or one artifact diagnostic,
    /// when no unique valid model revision can be constructed.
    pub fn compile(filename: &str, source: &str) -> Result<Self, Vec<Diagnostic>> {
        ExactModelCodec::CURRENT.compile(filename, source)
    }

    fn compile_for_codec(
        filename: &str,
        source: &str,
        exact_codec: ExactModelCodec,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut compiled = eqiora_compiler::compile(filename, source)?;
        if compiled.len() != 1 {
            return Err(vec![Diagnostic::error(
                codes::LANGUAGE_LOWERING_ERROR,
                format!(
                    "compile requires exactly one model declaration, found {}",
                    compiled.len()
                ),
            )]);
        }
        Self::accept_compiled(compiled.remove(0), exact_codec)
    }

    /// Define exactly one model from immutable client-neutral declarations
    /// using the current semantic vocabulary.
    ///
    /// Native construction skips text parsing only. It crosses the same typed
    /// compiler lowerer, versioned transaction wire, atomic graph commit, and
    /// canonical artifact reconstruction as [`Self::compile`].
    ///
    /// # Errors
    /// Returns graph-path compiler/semantic diagnostics, or one artifact
    /// diagnostic, when no valid model revision can be constructed.
    pub fn define(draft: &ModelDraft) -> Result<Self, Vec<Diagnostic>> {
        ExactModelCodec::CURRENT.define(draft)
    }

    fn define_for_codec(
        draft: &ModelDraft,
        exact_codec: ExactModelCodec,
    ) -> Result<Self, Vec<Diagnostic>> {
        Self::accept_compiled(eqiora_compiler::lower_draft(draft)?, exact_codec)
    }

    pub(crate) fn accept_compiled(
        compiled: CompiledModel,
        exact_codec: ExactModelCodec,
    ) -> Result<Self, Vec<Diagnostic>> {
        let aliases = aliases(compiled.symbols());
        let model = compiled.model();

        // Every source/UI/language client crosses the same bounded,
        // versioned transaction representation before graph mutation.
        let transaction = exact_codec
            .replay_transaction(compiled.transaction())
            .map_err(single_diagnostic)?;

        let mut store = InMemoryGraphStore::new();
        store.commit(transaction)?;
        let program = KernelProgram::from_snapshot(&store.snapshot(), model)?;
        Self::from_store(store, program, aliases, exact_codec)
    }

    fn replay_codec(data: &[u8], exact_codec: ExactModelCodec) -> Result<Self, Vec<Diagnostic>> {
        let artifact = exact_codec.decode_model(data).map_err(single_diagnostic)?;
        let (transaction, model) = artifact.to_transaction()?;
        let store = InMemoryGraphStore::restore_snapshot(
            transaction,
            Revision(artifact.source_revision()),
        )?;
        let program = KernelProgram::from_snapshot(&store.snapshot(), model)?;
        Ok(Self {
            program,
            artifact,
            exact_codec,
            aliases: BTreeMap::new(),
            store,
        })
    }

    fn from_store(
        store: InMemoryGraphStore,
        program: KernelProgram,
        aliases: BTreeMap<String, RawId>,
        exact_codec: ExactModelCodec,
    ) -> Result<Self, Vec<Diagnostic>> {
        let program = KernelProgram::from_snapshot(&store.snapshot(), program.model())?;
        let artifact = exact_codec
            .encode_program(&program)
            .map_err(single_diagnostic)?;
        // Reconstruct once more from the public artifact so client behavior
        // cannot accidentally depend on an in-memory compiler-only state.
        let bytes = artifact.canonical_json().map_err(single_diagnostic)?;
        let artifact = exact_codec
            .decode_model(&bytes)
            .map_err(single_diagnostic)?;
        artifact.replay_model().map_err(single_diagnostic)?;
        Ok(Self {
            program,
            artifact,
            exact_codec,
            aliases,
            store,
        })
    }

    /// Validated immutable execution input.
    #[must_use]
    pub const fn program(&self) -> &KernelProgram {
        &self.program
    }

    /// Exact artifact codec retained by this immutable document.
    #[must_use]
    pub const fn exact_codec(&self) -> ExactModelCodec {
        self.exact_codec
    }

    /// Version-neutral typed identity of the explicitly selected Model
    /// artifact.
    ///
    /// The digest remains domain-separated by [`Self::exact_codec`]; this
    /// method does not auto-detect, upgrade, or erase the selected wire.
    ///
    /// # Errors
    /// Returns an artifact diagnostic only if validated envelope state cannot
    /// be decoded.
    pub fn artifact_reference(&self) -> Result<ModelArtifactReference, Diagnostic> {
        self.artifact.artifact_reference()
    }

    /// Non-semantic source aliases in deterministic lexical order.
    #[must_use]
    pub const fn aliases(&self) -> &BTreeMap<String, RawId> {
        &self.aliases
    }

    /// Canonical compact JSON bytes for this immutable model.
    ///
    /// # Errors
    /// Returns an artifact diagnostic if invariant replay fails.
    pub fn canonical_json(&self) -> Result<Vec<u8>, Diagnostic> {
        self.artifact.canonical_json()
    }

    /// Domain-separated semantic content digest.
    ///
    /// # Errors
    /// Returns an artifact diagnostic if invariant replay fails.
    pub fn digest(&self) -> Result<String, Diagnostic> {
        self.artifact.digest().map(|digest| digest.to_string())
    }

    /// Alpha-normalized structural comparison evidence for this Model.
    ///
    /// The fingerprint intentionally excludes occurrence IDs, source names,
    /// graph revision, and exact artifact codec.  It never substitutes for
    /// [`Self::artifact_reference`] in execution, replay, provenance, or
    /// mutation preconditions.
    ///
    /// # Errors
    /// Returns an artifact diagnostic when exact bounded graph
    /// canonicalization cannot represent this fingerprint generation.
    pub fn structural_fingerprint(&self) -> Result<StructuralSemanticFingerprint, Diagnostic> {
        StructuralSemanticFingerprint::from_program(&self.program)
    }

    /// Compare alpha-normalized Semantic Kernel structure without weakening
    /// either exact Model artifact identity.
    ///
    /// Equal digests are followed by exact private canonical-byte comparison,
    /// so a cryptographic collision fails closed.
    ///
    /// # Errors
    /// Returns an artifact diagnostic for unsupported meaning, exhausted
    /// canonicalization limits, or unequal projections with a colliding hash.
    pub fn structurally_equivalent(&self, other: &Self) -> Result<bool, Diagnostic> {
        eqiora_artifact::structurally_equivalent(&self.program, &other.program)
    }

    /// Resolve one finite Field/Parameter value change into the shared,
    /// versioned model-transaction wire without mutating this document.
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

        let label = self
            .aliases
            .iter()
            .find_map(|(name, &id)| (id == target).then_some(name.as_str()))
            .map_or_else(
                || format!("set model value {target}"),
                |name| format!("set model value {name}"),
            );
        let base_revision = self.store.revision();
        let mut transaction = Transaction::new(label);
        transaction
            .require(Precondition::RevisionIs(base_revision))
            .require(Precondition::ValueEquals {
                target,
                expected: before,
            })
            .push(Op::SetValue {
                target,
                value: after,
            });
        let transaction = self.exact_codec().encode_transaction(&transaction)?;
        let transaction_digest = transaction.digest()?;
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
    /// Returns structured diagnostics if model identity, transaction identity,
    /// optimistic preconditions, graph invariants, or artifact replay differ
    /// from the accepted preview.
    pub fn commit_value_edit(
        &self,
        plan: ValueEditPlan,
    ) -> Result<ValueEditResult, Vec<Diagnostic>> {
        if plan.exact_codec() != self.exact_codec()
            || plan.base_digest != self.digest().map_err(single_diagnostic)?
            || plan.base_revision != self.store.revision()
        {
            return Err(vec![Diagnostic::error(
                codes::PRECONDITION_FAILED,
                "value-edit plan no longer matches the selected model revision",
            )]);
        }
        let bytes = plan
            .transaction
            .canonical_json()
            .map_err(single_diagnostic)?;
        let replay = self
            .exact_codec()
            .decode_transaction(&bytes)
            .map_err(single_diagnostic)?;
        if replay.digest().map_err(single_diagnostic)? != plan.transaction_digest {
            return Err(vec![Diagnostic::error(
                codes::INVALID_ARTIFACT,
                "value-edit transaction identity changed during replay",
            )]);
        }

        let mut store = self.store.clone();
        store.commit(replay.to_transaction().map_err(single_diagnostic)?)?;
        let program = KernelProgram::from_snapshot(&store.snapshot(), self.program.model())?;
        let document = Self::from_store(store, program, self.aliases.clone(), self.exact_codec())?;
        let result_digest = document.digest().map_err(single_diagnostic)?;
        Ok(ValueEditResult {
            plan,
            document,
            result_digest,
        })
    }
}

fn aliases(symbols: &ModelSymbols) -> BTreeMap<String, RawId> {
    symbols
        .iter()
        .map(|(name, id)| (name.to_owned(), id))
        .collect()
}

fn single_diagnostic(diagnostic: Diagnostic) -> Vec<Diagnostic> {
    vec![diagnostic]
}

#[cfg(test)]
mod tests {
    use super::{
        ExactModelCodec, ModelDocument, ReferenceAcceptance, ReferenceExecutionPlacement,
        ReferenceIntegrationMethod, ReferenceNonlinearMethod, ReferenceRunDirective,
        ReferenceRunObserver, ReferenceRunOutcome, ReferenceRunPlan, ReferenceRunProgress,
    };
    use eqiora_artifact::ReplayableCanonicalModelArtifact;
    use eqiora_core::DimExponents;
    use eqiora_lang::{DraftExpression, DraftField, DraftParameter, DraftRelation, ModelDraft};

    const SOURCE: &str = r#"
model decay {
  field x: 1 = 1;
  parameter rate: 1 / s = 1;
  relation flow continuous {
    derivative(x) + rate * x = 0;
  }
}
"#;

    #[test]
    fn one_application_path_closes_compile_wire_artifact_and_run() {
        let document = ModelDocument::compile("decay.eqi", SOURCE).unwrap();
        assert_eq!(document.exact_codec(), ExactModelCodec::CURRENT);
        let bytes = document.canonical_json().unwrap();
        let digest = document.digest().unwrap();
        let reconstructed = ExactModelCodec::CURRENT.replay(&bytes).unwrap();
        assert_eq!(reconstructed.canonical_json().unwrap(), bytes);
        assert_eq!(reconstructed.digest().unwrap(), digest);

        let mut zero_revision: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        zero_revision["source_revision"] = serde_json::Value::from(0);
        let diagnostics = ExactModelCodec::CURRENT
            .replay(&serde_json::to_vec(&zero_revision).unwrap())
            .unwrap_err();
        assert_eq!(diagnostics[0].code().0, "EQ0901");

        let result = document.run_reference(0.2, 0.1).unwrap();
        let evidence = *result.evidence();
        assert_eq!(
            evidence.plan().key(),
            "eqiora.reference-plan/v1:3fc999999999999a:3fb999999999999a"
        );
        assert_eq!(
            evidence.plan().integration_method(),
            ReferenceIntegrationMethod::BackwardEuler
        );
        assert_eq!(
            evidence.plan().nonlinear_method(),
            ReferenceNonlinearMethod::DenseFiniteDifferenceNewton
        );
        assert_eq!(
            evidence.plan().placement(),
            ReferenceExecutionPlacement::HostSerial
        );
        assert_eq!(
            evidence.plan().acceptance(),
            ReferenceAcceptance::SemanticOracle
        );
        assert_eq!(evidence.field_count(), 1);
        assert_eq!(evidence.sample_count(), 3);
        assert_eq!(result.series().len(), 1);
        let series = &result.series()[0];
        assert_eq!(series.name(), Some("x"));
        assert_eq!(series.time(), [0.0, 0.1, 0.2]);
        assert_eq!(series.values().len(), 3);
        assert!((series.values()[2] - 1.0 / 1.1_f64.powi(2)).abs() < 1.0e-15);
    }

    #[test]
    fn native_definition_closes_the_same_artifact_and_execution_path() {
        let state = DraftField::new("x", DimExponents::DIMENSIONLESS, 1.0);
        let rate = DraftParameter::new(
            "rate",
            DimExponents {
                time: -1,
                ..DimExponents::DIMENSIONLESS
            },
            1.0,
        );
        let flow = DraftRelation::continuous(
            "flow",
            [DraftExpression::derivative(&state) + rate.expression() * state.expression()],
        );
        let draft = ModelDraft::new("decay", [state.into(), rate.into(), flow.into()]).unwrap();

        let native = ModelDocument::define(&draft).unwrap();
        let explicit_v1 = ExactModelCodec::V1.define(&draft).unwrap();
        let explicit_v2 = ExactModelCodec::V2.define(&draft).unwrap();
        let explicit_v3 = ExactModelCodec::V3.define(&draft).unwrap();
        let explicit_v4 = ExactModelCodec::V4.define(&draft).unwrap();
        let explicit_v5 = ExactModelCodec::V5.define(&draft).unwrap();
        let explicit_v6 = ExactModelCodec::V6.define(&draft).unwrap();
        assert_eq!(native.exact_codec(), ExactModelCodec::CURRENT);
        assert_eq!(explicit_v1.exact_codec(), ExactModelCodec::V1);
        assert_eq!(explicit_v2.exact_codec(), ExactModelCodec::V2);
        assert_eq!(explicit_v3.exact_codec(), ExactModelCodec::V3);
        assert_eq!(explicit_v4.exact_codec(), ExactModelCodec::V4);
        assert_eq!(explicit_v5.exact_codec(), ExactModelCodec::V5);
        assert_eq!(explicit_v6.exact_codec(), ExactModelCodec::V6);
        assert!(ExactModelCodec::V2.supports_scalar_physical());
        assert!(ExactModelCodec::V3.supports_scalar_physical());
        assert!(!ExactModelCodec::V2.supports_boundary_physical());
        assert!(ExactModelCodec::V3.supports_boundary_physical());
        assert!(ExactModelCodec::V4.supports_scalar_physical());
        assert!(ExactModelCodec::V4.supports_boundary_physical());
        assert!(!ExactModelCodec::V3.supports_tensor_operators());
        assert!(ExactModelCodec::V4.supports_tensor_operators());
        assert!(ExactModelCodec::V5.supports_tensor_operators());
        assert!(ExactModelCodec::V6.supports_tensor_operators());
        assert!(!ExactModelCodec::V4.supports_pure_operators());
        assert!(ExactModelCodec::V5.supports_pure_operators());
        assert!(ExactModelCodec::V6.supports_pure_operators());
        assert!(!ExactModelCodec::V5.supports_spatial_periodic());
        assert!(ExactModelCodec::V6.supports_spatial_periodic());
        assert!(
            String::from_utf8_lossy(&explicit_v2.canonical_json().unwrap())
                .contains("eqiora.model-envelope/v2")
        );
        assert!(
            String::from_utf8_lossy(&explicit_v3.canonical_json().unwrap())
                .contains("eqiora.model-envelope/v3")
        );
        assert!(
            String::from_utf8_lossy(&explicit_v4.canonical_json().unwrap())
                .contains("eqiora.model-envelope/v4")
        );
        let bytes = native.canonical_json().unwrap();
        let reconstructed = ExactModelCodec::CURRENT.replay(&bytes).unwrap();
        assert_eq!(reconstructed.canonical_json().unwrap(), bytes);
        assert_eq!(native.aliases().len(), 3);

        let source_values = ModelDocument::compile("decay.eqi", SOURCE)
            .unwrap()
            .run_reference(0.2, 0.1)
            .unwrap()
            .series()[0]
            .values()
            .to_vec();
        let native_values = native.run_reference(0.2, 0.1).unwrap().series()[0]
            .values()
            .to_vec();
        assert_eq!(native_values, source_values);
    }

    #[test]
    fn current_v7_authoring_retains_the_v4_tensor_vocabulary() {
        let source = r#"
model elastic_relation {
  domain body = box(0, 1, 0, 1);
  representation space = continuum;
  field displacement on body as space: m shape spatial_vector;
  parameter mu: kg / (m * s ^ 2) = 2;
  parameter lambda: kg / (m * s ^ 2) = 3;
  relation balance continuous on body {
    -div(
      2 * mu * symmetric_part(grad(displacement))
      + lambda * isotropic_lift(div(displacement))
    ) = 0;
  }
}
"#;
        assert!(ExactModelCodec::V3.compile("elastic.eqi", source).is_err());

        let document = ModelDocument::compile("elastic.eqi", source).unwrap();
        let exact_v4 = ExactModelCodec::V4.compile("elastic.eqi", source).unwrap();
        let bytes = document.canonical_json().unwrap();
        assert_eq!(document.exact_codec(), ExactModelCodec::CURRENT);
        assert_eq!(exact_v4.exact_codec(), ExactModelCodec::V4);
        assert!(String::from_utf8_lossy(&bytes).contains("symmetric-part"));
        assert!(String::from_utf8_lossy(&bytes).contains("isotropic-lift"));
        assert!(ExactModelCodec::V4.replay(&bytes).is_err());
        let replay = ExactModelCodec::CURRENT.replay(&bytes).unwrap();
        assert_eq!(replay.canonical_json().unwrap(), bytes);
        assert_eq!(replay.digest().unwrap(), document.digest().unwrap());
    }

    #[test]
    fn current_v7_closes_generic_pure_operators_while_exact_v4_rejects_them() {
        let source = r#"
public pure operator dyadic(left: spatial[1], right: spatial[1]) -> spatial[2]
  = component(left, 0) * component(right, 1);
model pure_relation {
  domain body = box(0, 1, 0, 1);
  representation space = continuum;
  field left on body as space: 1 shape spatial_vector;
  field right on body as space: 1 shape spatial_vector;
  relation balance continuous on body {
    div(div(dyadic(left, right))) = 0;
  }
}
"#;
        let current = ModelDocument::compile("pure-relation.eqi", source).unwrap();
        assert_eq!(current.exact_codec(), ExactModelCodec::CURRENT);
        assert!(
            ExactModelCodec::V4
                .compile("pure-relation.eqi", source)
                .is_err()
        );

        let bytes = current.canonical_json().unwrap();
        let json = String::from_utf8_lossy(&bytes);
        assert!(json.contains("pure-operator-application"));
        assert!(json.contains("eqiora.model-envelope/v7"));
        assert!(ExactModelCodec::V4.replay(&bytes).is_err());
        let replay = ExactModelCodec::CURRENT.replay(&bytes).unwrap();
        assert_eq!(replay.canonical_json().unwrap(), bytes);
        assert_eq!(replay.digest().unwrap(), current.digest().unwrap());
    }

    #[test]
    fn invalid_source_and_run_policy_remain_structured_diagnostics() {
        let diagnostics =
            ModelDocument::compile("broken.eqi", "model broken { field ; }").unwrap_err();
        assert!(!diagnostics.is_empty());
        assert!(diagnostics[0].code().0.starts_with("EQ"));

        let document = ModelDocument::compile("decay.eqi", SOURCE).unwrap();
        assert_eq!(
            document.run_reference(1.0, 0.0).unwrap_err()[0].code().0,
            "EQ0501"
        );
    }

    #[test]
    fn value_edit_retains_current_v7_and_explicit_v1_codec_provenance() {
        for codec in [ExactModelCodec::CURRENT, ExactModelCodec::V1] {
            let document = codec.compile("decay.eqi", SOURCE).unwrap();
            let base_digest = document.digest().unwrap();
            let rate = document.aliases()["rate"];
            let relation = document.aliases()["flow"];

            let plan = document.preview_value_edit(rate, 2.0).unwrap();
            assert_eq!(plan.base_digest(), base_digest);
            assert_eq!(plan.base_revision().0, 1);
            assert_eq!(plan.target(), rate);
            assert_eq!(plan.before().value(), 1.0);
            assert_eq!(plan.after().value(), 2.0);
            assert_eq!(plan.before().dim(), plan.after().dim());
            assert_eq!(plan.exact_codec(), codec);
            assert!(plan.key().starts_with("eqiora.value-edit-plan/v1:"));
            assert!(
                String::from_utf8(plan.transaction_json().unwrap())
                    .unwrap()
                    .contains(&format!(
                        "eqiora.model-transaction-envelope/{}",
                        codec.as_str()
                    ))
            );

            let result = document.commit_value_edit(plan.clone()).unwrap();
            assert_eq!(result.plan(), &plan);
            assert_eq!(result.result_revision().0, 2);
            assert_ne!(result.result_digest(), base_digest);
            assert_eq!(result.document().exact_codec(), codec);
            assert_eq!(document.program().revision().0, 1);
            assert_eq!(document.program().value(rate).unwrap().value(), 1.0);
            assert_eq!(
                result.document().program().value(rate).unwrap().value(),
                2.0
            );

            let public_artifact_replay = result.document().artifact.replay_model().unwrap();
            assert_eq!(public_artifact_replay.program().revision().0, 2);
            assert_eq!(
                public_artifact_replay
                    .artifact_reference()
                    .semantic_revision()
                    .get(),
                2
            );

            let child_bytes = result.document().canonical_json().unwrap();
            let replayed_child = codec.replay(&child_bytes).unwrap();
            assert_eq!(replayed_child.program().revision().0, 2);
            let grandchild_plan = replayed_child.preview_value_edit(rate, 3.0).unwrap();
            assert_eq!(grandchild_plan.base_revision().0, 2);
            let grandchild = replayed_child.commit_value_edit(grandchild_plan).unwrap();
            assert_eq!(grandchild.result_revision().0, 3);
            assert_eq!(
                grandchild.document().program().value(rate).unwrap().value(),
                3.0
            );

            assert_eq!(
                result.document().commit_value_edit(plan).unwrap_err()[0]
                    .code()
                    .0,
                "EQ0106"
            );
            assert_eq!(
                document.preview_value_edit(rate, 1.0).unwrap_err().code().0,
                "EQ0105"
            );
            assert_eq!(
                document
                    .preview_value_edit(rate, f64::NAN)
                    .unwrap_err()
                    .code()
                    .0,
                "EQ0105"
            );
            assert_eq!(
                document
                    .preview_value_edit(relation, 2.0)
                    .unwrap_err()
                    .code()
                    .0,
                "EQ0105"
            );
        }

        let v1 = ExactModelCodec::V1.compile("decay.eqi", SOURCE).unwrap();
        let v1_plan = v1.preview_value_edit(v1.aliases()["rate"], 2.0).unwrap();
        let current = ModelDocument::compile("decay.eqi", SOURCE).unwrap();
        assert_eq!(
            current.commit_value_edit(v1_plan).unwrap_err()[0].code().0,
            "EQ0106"
        );
    }

    #[test]
    fn value_edit_identity_includes_the_exact_base_artifact() {
        let base = ModelDocument::compile("decay.eqi", SOURCE).unwrap();
        let rate = base.aliases()["rate"];
        let state = base.aliases()["x"];

        let left = base
            .commit_value_edit(base.preview_value_edit(rate, 2.0).unwrap())
            .unwrap()
            .into_document();
        let right = base
            .commit_value_edit(base.preview_value_edit(rate, 3.0).unwrap())
            .unwrap()
            .into_document();
        let left_plan = left.preview_value_edit(state, 2.0).unwrap();
        let right_plan = right.preview_value_edit(state, 2.0).unwrap();

        assert_eq!(left_plan.base_revision(), right_plan.base_revision());
        assert_eq!(
            left_plan.transaction_digest(),
            right_plan.transaction_digest(),
            "the same graph-local transaction is reusable only against its exact base artifact"
        );
        assert_ne!(left_plan.base_digest(), right_plan.base_digest());
        assert_ne!(left_plan.key(), right_plan.key());
        assert_ne!(left_plan, right_plan);
        assert_eq!(
            left.commit_value_edit(right_plan).unwrap_err()[0].code().0,
            "EQ0106"
        );
    }

    #[test]
    fn plan_key_distinguishes_exact_floating_point_requests() {
        let baseline = ReferenceRunPlan::new(1.0, 0.1).unwrap();
        let same = ReferenceRunPlan::new(1.0, 1.0e-1).unwrap();
        let different = ReferenceRunPlan::new(1.0, f64::from_bits(0.1_f64.to_bits() + 1)).unwrap();

        assert_eq!(baseline.key(), same.key());
        assert_ne!(baseline.key(), different.key());
    }

    #[derive(Debug, Default)]
    struct CancelAfterThreeAcceptedSteps {
        observed: Vec<ReferenceRunProgress>,
    }

    impl ReferenceRunObserver for CancelAfterThreeAcceptedSteps {
        fn observe(&mut self, progress: ReferenceRunProgress) -> ReferenceRunDirective {
            self.observed.push(progress);
            if progress.accepted_steps() >= 3 {
                ReferenceRunDirective::Cancel
            } else {
                ReferenceRunDirective::Continue
            }
        }
    }

    #[test]
    fn controlled_run_cancels_only_at_an_accepted_boundary() {
        let document = ModelDocument::compile("decay.eqi", SOURCE).unwrap();
        let plan = ReferenceRunPlan::new(1.0, 0.1).unwrap();
        let mut observer = CancelAfterThreeAcceptedSteps::default();

        let outcome = document
            .run_reference_plan_controlled(plan, &mut observer)
            .unwrap();
        let ReferenceRunOutcome::Cancelled(cancellation) = outcome else {
            panic!("observer must cancel the run");
        };

        assert_eq!(
            observer
                .observed
                .iter()
                .map(|progress| progress.accepted_steps())
                .collect::<Vec<_>>(),
            [0, 1, 2, 3]
        );
        let progress = cancellation.progress();
        assert_eq!(cancellation.plan(), plan);
        assert_eq!(progress.accepted_steps(), 3);
        assert_eq!(progress.end_time(), 1.0);
        assert_eq!(progress.maximum_steps(), plan.config().max_steps());
        assert!((progress.model_time() - 0.3).abs() <= f64::EPSILON);
    }

    #[derive(Debug, Default)]
    struct RecordingObserver {
        observed: Vec<ReferenceRunProgress>,
    }

    impl ReferenceRunObserver for RecordingObserver {
        fn observe(&mut self, progress: ReferenceRunProgress) -> ReferenceRunDirective {
            self.observed.push(progress);
            ReferenceRunDirective::Continue
        }
    }

    #[test]
    fn controlled_completion_preserves_the_reference_result() {
        let document = ModelDocument::compile("decay.eqi", SOURCE).unwrap();
        let plan = ReferenceRunPlan::new(0.4, 0.1).unwrap();
        let expected = document.run_reference_plan(plan).unwrap();
        let mut observer = RecordingObserver::default();

        let outcome = document
            .run_reference_plan_controlled(plan, &mut observer)
            .unwrap();
        let ReferenceRunOutcome::Completed(actual) = outcome else {
            panic!("recording observer cannot cancel the run");
        };

        assert_eq!(actual.series(), expected.series());
        assert_eq!(actual.evidence().plan(), expected.evidence().plan());
        assert_eq!(
            observer
                .observed
                .iter()
                .map(|progress| progress.accepted_steps())
                .collect::<Vec<_>>(),
            [0, 1, 2, 3]
        );
        assert!(
            observer
                .observed
                .windows(2)
                .all(|pair| pair[0].model_time() < pair[1].model_time())
        );
    }
}
