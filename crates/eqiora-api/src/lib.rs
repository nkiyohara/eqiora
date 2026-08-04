//! Shared application operations for thin Eqiora clients.
//!
//! This L4 crate composes compiler, transaction wire, graph commit, immutable
//! model artifact, and reference execution contracts without introducing a
//! second model semantics. Python and Studio adapters add language- or
//! UI-specific ergonomics around these owned Rust values.

mod cad;
pub mod control;
mod differentiation;
mod elasticity;
#[cfg(any(feature = "vtu", feature = "xdmf"))]
mod external_data;
mod fixed_mesh_trajectory;
mod fixed_reference_fsi;
mod geometry_edit;
mod ml_dataset;
pub mod package;
mod parameter_regeneration;
mod parameter_study;
mod reference_run;
mod remeshing_trajectory;
mod spatial;
mod spatial_data;
mod steady_stokes;
mod transient_fluid;
mod value_edit;

pub use cad::*;
pub use differentiation::*;
pub use elasticity::{
    LinearElasticityIntent2d, MixedBoundaryElasticityResult2d, ResolvedLinearElasticityPlan2d,
};
pub use eqiora_artifact::{SemanticFingerprintGeneration, StructuralSemanticFingerprint};
#[cfg(any(feature = "vtu", feature = "xdmf"))]
pub use external_data::*;
pub use fixed_mesh_trajectory::FixedMeshFieldTrajectoryReplay2dV1;
pub use fixed_reference_fsi::{
    FixedMeshMonolithicFsiIntent2d, FixedReferenceFsiResult2d, ResolvedFixedMeshMonolithicFsiPlan2d,
};
pub use geometry_edit::{CartesianDomainEditPlan, CartesianDomainEditResult};
pub use ml_dataset::*;
pub use parameter_regeneration::{
    ParameterGeometryRegenerationPlan, ParameterGeometryRegenerationResult,
};
pub use parameter_study::{
    CompleteParameterStudy, ParameterStudyPlan, ParameterStudyPointKey,
    ParameterStudyTerminalReport,
};
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
pub use steady_stokes::{
    CircularHoleSteadyStokesResult2d, ResolvedSteadyStokesPlan2d, SteadyStokesIntent2d,
};
pub use transient_fluid::*;
pub use value_edit::{ValueEditPlan, ValueEditResult};

use std::collections::BTreeMap;

use eqiora_artifact::{
    AcceptedModelArtifact, CanonicalModelArtifact, ModelArtifactReference, ModelDecoderLimits,
    ModelTransactionEnvelope, ReplayableCanonicalModelArtifact,
};
use eqiora_compiler::{CompiledModel, ModelSymbols};
use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, RawId};
use eqiora_graph::{GraphStore, InMemoryGraphStore, Revision};
use eqiora_lang::ModelDraft;
use eqiora_sem::KernelProgram;

/// One immutable, validated canonical model revision plus non-semantic source
/// aliases used by client presentation layers.
#[derive(Debug, Clone)]
pub struct ModelDocument {
    program: KernelProgram,
    artifact: AcceptedModelArtifact,
    aliases: BTreeMap<String, RawId>,
    store: InMemoryGraphStore,
}

impl PartialEq for ModelDocument {
    fn eq(&self, other: &Self) -> bool {
        self.program == other.program
            && self.artifact == other.artifact
            && self.aliases == other.aliases
    }
}

impl ModelDocument {
    /// Compile exactly one source model with the current semantic vocabulary.
    ///
    /// # Errors
    /// Returns all compiler/semantic diagnostics, or one artifact diagnostic,
    /// when no unique valid model revision can be constructed.
    pub fn compile(filename: &str, source: &str) -> Result<Self, Vec<Diagnostic>> {
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
        Self::accept_compiled(compiled.remove(0))
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
        Self::accept_compiled(eqiora_compiler::lower_draft(draft)?)
    }

    pub(crate) fn accept_compiled(compiled: CompiledModel) -> Result<Self, Vec<Diagnostic>> {
        let aliases = aliases(compiled.symbols());
        let model = compiled.model();

        // Every source/UI/language client crosses the same bounded,
        // versioned transaction representation before graph mutation.
        let transaction = ModelTransactionEnvelope::from_transaction(compiled.transaction())
            .and_then(|envelope| envelope.to_transaction())
            .map_err(single_diagnostic)?;

        let mut store = InMemoryGraphStore::new();
        store.commit(transaction)?;
        let program = KernelProgram::from_snapshot(&store.snapshot(), model)?;
        Self::from_store(store, program, aliases)
    }

    /// Replay one artifact through the single current Model contract.
    ///
    /// Historical schemas reject before semantic use; this path never sniffs,
    /// retries, or migrates bytes.
    ///
    /// # Errors
    /// Returns decoder, graph, or whole-Model validation diagnostics.
    pub fn replay(data: &[u8]) -> Result<Self, Vec<Diagnostic>> {
        let artifact = AcceptedModelArtifact::from_json(data, ModelDecoderLimits::default())
            .map_err(single_diagnostic)?;
        let (transaction, model) = artifact.to_transaction()?;
        let store = InMemoryGraphStore::restore_snapshot(
            transaction,
            Revision(artifact.source_revision()),
        )?;
        let program = KernelProgram::from_snapshot(&store.snapshot(), model)?;
        Ok(Self {
            program,
            artifact,
            aliases: BTreeMap::new(),
            store,
        })
    }

    fn from_store(
        store: InMemoryGraphStore,
        program: KernelProgram,
        aliases: BTreeMap<String, RawId>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let program = KernelProgram::from_snapshot(&store.snapshot(), program.model())?;
        let artifact = AcceptedModelArtifact::from_program(&program).map_err(single_diagnostic)?;
        // Reconstruct once more from the public artifact so client behavior
        // cannot accidentally depend on an in-memory compiler-only state.
        let bytes = artifact.canonical_json().map_err(single_diagnostic)?;
        let artifact = AcceptedModelArtifact::from_json(&bytes, ModelDecoderLimits::default())
            .map_err(single_diagnostic)?;
        artifact.replay_model().map_err(single_diagnostic)?;
        Ok(Self {
            program,
            artifact,
            aliases,
            store,
        })
    }

    /// Validated immutable execution input.
    #[must_use]
    pub const fn program(&self) -> &KernelProgram {
        &self.program
    }

    /// Typed identity of the current Model artifact.
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
        ModelDocument, ReferenceAcceptance, ReferenceExecutionPlacement,
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
        let bytes = document.canonical_json().unwrap();
        let digest = document.digest().unwrap();
        let reconstructed = ModelDocument::replay(&bytes).unwrap();
        assert_eq!(reconstructed.canonical_json().unwrap(), bytes);
        assert_eq!(reconstructed.digest().unwrap(), digest);

        let mut zero_revision: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        zero_revision["source_revision"] = serde_json::Value::from(0);
        let diagnostics =
            ModelDocument::replay(&serde_json::to_vec(&zero_revision).unwrap()).unwrap_err();
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
        let bytes = native.canonical_json().unwrap();
        let reconstructed = ModelDocument::replay(&bytes).unwrap();
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
    fn current_authoring_retains_tensor_vocabulary() {
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
        let document = ModelDocument::compile("elastic.eqi", source).unwrap();
        let bytes = document.canonical_json().unwrap();
        assert!(String::from_utf8_lossy(&bytes).contains("symmetric-part"));
        assert!(String::from_utf8_lossy(&bytes).contains("isotropic-lift"));
        let replay = ModelDocument::replay(&bytes).unwrap();
        assert_eq!(replay.canonical_json().unwrap(), bytes);
        assert_eq!(replay.digest().unwrap(), document.digest().unwrap());
    }

    #[test]
    fn current_authoring_closes_generic_pure_operators() {
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

        let bytes = current.canonical_json().unwrap();
        let json = String::from_utf8_lossy(&bytes);
        assert!(json.contains("pure-operator-application"));
        assert!(json.contains("eqiora.model-envelope/v8"));
        let replay = ModelDocument::replay(&bytes).unwrap();
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
    fn value_edit_retains_current_transaction_and_artifact_lineage() {
        let document = ModelDocument::compile("decay.eqi", SOURCE).unwrap();
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
        assert!(plan.key().starts_with("eqiora.value-edit-plan/v1:"));
        assert!(
            String::from_utf8(plan.transaction_json().unwrap())
                .unwrap()
                .contains("eqiora.model-transaction-envelope/v8")
        );

        let result = document.commit_value_edit(plan.clone()).unwrap();
        assert_eq!(result.plan(), &plan);
        assert_eq!(result.result_revision().0, 2);
        assert_ne!(result.result_digest(), base_digest);
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
        let replayed_child = ModelDocument::replay(&child_bytes).unwrap();
        let grandchild_plan = replayed_child.preview_value_edit(rate, 3.0).unwrap();
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
