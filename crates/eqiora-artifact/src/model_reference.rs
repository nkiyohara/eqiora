//! Version-neutral identity of one explicitly selected Model artifact.

use eqiora_core::{Diagnostic, OntologyId};
use eqiora_realization::SemanticRevision;
use eqiora_schema::Model;
use eqiora_sem::KernelProgram;

use crate::{
    ArtifactDigest, ModelEnvelopeV1, ModelEnvelopeV2, ModelEnvelopeV3, ModelEnvelopeV4,
    ModelEnvelopeV5, ModelEnvelopeV6, invalid_artifact,
};

mod sealed {
    pub trait Sealed {}
}

/// One explicitly versioned canonical Model artifact that can yield a closed
/// identity reference.
///
/// The trait is sealed: artifact adapters own the digest domain, Model
/// identity, and revision extraction. Callers cannot manufacture a reference
/// by implementing a permissive metadata interface.
pub trait CanonicalModelArtifact: sealed::Sealed {
    /// Construct the exact version-neutral reference for this artifact.
    ///
    /// # Errors
    /// Returns `EQ0901` only when validated artifact state cannot be decoded.
    fn artifact_reference(&self) -> Result<ModelArtifactReference, Diagnostic>;
}

/// One explicitly selected canonical Model artifact replayed through its own
/// wire decoder and the ordinary whole-model validator.
///
/// The exact artifact reference and immutable program are produced together;
/// a consumer cannot accidentally combine metadata from one wire generation
/// with semantic content replayed from another.
#[derive(Debug, Clone, PartialEq)]
pub struct ReplayedCanonicalModel {
    reference: ModelArtifactReference,
    program: KernelProgram,
}

impl ReplayedCanonicalModel {
    /// Exact wire-domain identity of the replayed artifact.
    #[must_use]
    pub const fn artifact_reference(&self) -> &ModelArtifactReference {
        &self.reference
    }

    /// Immutable, completely validated Semantic Kernel projection.
    #[must_use]
    pub const fn program(&self) -> &KernelProgram {
        &self.program
    }
}

/// A canonical Model artifact that can replay its exact selected wire into a
/// validated immutable Semantic Kernel program.
///
/// The [`CanonicalModelArtifact`] supertrait seals this extension. An
/// identity-only reference deliberately does not implement replay: having a
/// digest, Model ID, and revision does not imply possession of canonical
/// content. Each new Model codec must explicitly implement this boundary
/// beside its decoder before semantic consumers can admit it.
pub trait ReplayableCanonicalModelArtifact: CanonicalModelArtifact {
    /// Replay the explicitly selected wire generation and return its exact
    /// identity and validated semantic content as one indivisible value.
    ///
    /// # Errors
    /// Returns `EQ0901` if canonical reconstruction unexpectedly fails or if
    /// the replayed Model identity/revision contradicts the artifact reference.
    fn replay_model(&self) -> Result<ReplayedCanonicalModel, Diagnostic>;
}

/// Closed identity of one canonical Model artifact, independent of its wire
/// generation.
///
/// The content digest remains domain-separated by the selected Model schema;
/// this type does not erase wire identity. It merely gives downstream
/// artifacts one stable way to retain the exact digest, ontology identity,
/// and semantic revision without depending on a particular envelope type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelArtifactReference {
    artifact: ArtifactDigest,
    model: OntologyId<Model>,
    semantic_revision: SemanticRevision,
}

/// Compatibility alias for the former source-level name.
#[deprecated(note = "use ModelArtifactReference")]
pub type ModelArtifactReferenceV1 = ModelArtifactReference;

impl ModelArtifactReference {
    fn new(artifact: ArtifactDigest, model: OntologyId<Model>, source_revision: u64) -> Self {
        Self {
            artifact,
            model,
            semantic_revision: SemanticRevision::new(source_revision),
        }
    }

    /// Content digest in the selected Model wire's domain.
    #[must_use]
    pub const fn artifact(&self) -> &ArtifactDigest {
        &self.artifact
    }

    /// Typed Semantic Model identity.
    #[must_use]
    pub const fn model(&self) -> OntologyId<Model> {
        self.model
    }

    /// Semantic graph revision serialized by the Model artifact.
    #[must_use]
    pub const fn semantic_revision(&self) -> SemanticRevision {
        self.semantic_revision
    }

    /// Prove that another explicitly selected Model artifact is this exact
    /// artifact, including its wire-domain digest.
    ///
    /// # Errors
    /// Returns `EQ0901` for digest, Model identity, or revision drift.
    pub fn validate_artifact(
        &self,
        artifact: &(impl CanonicalModelArtifact + ?Sized),
    ) -> Result<(), Diagnostic> {
        let candidate = artifact.artifact_reference()?;
        if self != &candidate {
            return Err(invalid_artifact(
                "Model artifact digest, ontology identity, or semantic revision differs from the typed reference",
            ));
        }
        Ok(())
    }
}

impl sealed::Sealed for ModelArtifactReference {}

impl CanonicalModelArtifact for ModelArtifactReference {
    fn artifact_reference(&self) -> Result<ModelArtifactReference, Diagnostic> {
        Ok(self.clone())
    }
}

macro_rules! impl_model_artifact {
    ($envelope:ty) => {
        impl sealed::Sealed for $envelope {}

        impl CanonicalModelArtifact for $envelope {
            fn artifact_reference(&self) -> Result<ModelArtifactReference, Diagnostic> {
                Ok(ModelArtifactReference::new(
                    self.digest()?,
                    self.model()?,
                    self.source_revision(),
                ))
            }
        }

        impl ReplayableCanonicalModelArtifact for $envelope {
            fn replay_model(&self) -> Result<ReplayedCanonicalModel, Diagnostic> {
                let reference = self.artifact_reference()?;
                let program = self.to_program().map_err(|diagnostics| {
                    invalid_artifact(format!(
                        "cannot replay canonical Model artifact: {}",
                        diagnostics
                            .iter()
                            .map(Diagnostic::message)
                            .collect::<Vec<_>>()
                            .join("; ")
                    ))
                })?;
                if program.model() != reference.model()
                    || SemanticRevision::new(program.revision().0)
                        != reference.semantic_revision()
                {
                    return Err(invalid_artifact(
                        "replayed Model identity or semantic revision differs from its exact artifact reference",
                    ));
                }
                Ok(ReplayedCanonicalModel { reference, program })
            }
        }
    };
}

impl_model_artifact!(ModelEnvelopeV1);
impl_model_artifact!(ModelEnvelopeV2);
impl_model_artifact!(ModelEnvelopeV3);
impl_model_artifact!(ModelEnvelopeV4);
impl_model_artifact!(ModelEnvelopeV5);
impl_model_artifact!(ModelEnvelopeV6);
