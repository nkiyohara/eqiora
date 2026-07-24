//! Version-neutral identity of one explicitly selected Model artifact.

use eqiora_core::{Diagnostic, OntologyId};
use eqiora_graph::Transaction;
use eqiora_realization::SemanticRevision;
use eqiora_schema::Model;
use eqiora_sem::KernelProgram;

use crate::{
    ArtifactDigest, DecoderLimits, ModelEnvelopeV1, ModelEnvelopeV2, ModelEnvelopeV3,
    ModelEnvelopeV4, ModelEnvelopeV5, ModelEnvelopeV6, invalid_artifact,
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

macro_rules! define_model_artifact_registry {
    ($(($variant:ident, $envelope:ty, $schema:literal)),+ $(,)?) => {
        /// Explicit Model artifact generation selected by an owning caller policy.
        ///
        /// This is not a wire discriminator and is never inferred from bytes.
        /// It selects one exact decoder from the closed artifact-owner registry.
        #[non_exhaustive]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum ModelArtifactGeneration {
            $(
                #[doc = concat!("Exact `", $schema, "` artifact generation.")]
                $variant,
            )+
        }

        impl ModelArtifactGeneration {
            /// Every generation registered by the Model artifact owner.
            ///
            /// Adding a generation extends the registry invocation below; the
            /// owned envelope and all encode/decode/replay/reference dispatch
            /// are generated from that one declaration.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// Exact schema selected by this generation.
            #[must_use]
            pub const fn schema(self) -> &'static str {
                match self {
                    $(Self::$variant => $schema,)+
                }
            }
        }

        #[derive(Debug, Clone, PartialEq)]
        enum AcceptedModelEnvelope {
            $($variant($envelope),)+
        }

        /// One owned, explicitly selected canonical Model artifact.
        ///
        /// Historical envelope types remain private implementation details of
        /// this value. Consumers retain exact bytes, the schema-domain digest,
        /// and validated replay without matching Model generations.
        #[derive(Debug, Clone, PartialEq)]
        pub struct AcceptedModelArtifact {
            envelope: AcceptedModelEnvelope,
        }

        impl AcceptedModelArtifact {
            /// Encode one validated Kernel Program through exactly one selected
            /// Model generation.
            ///
            /// # Errors
            /// Returns `EQ0901` when that generation cannot represent the
            /// program or its bounded canonical artifact.
            pub fn from_program(
                generation: ModelArtifactGeneration,
                program: &KernelProgram,
            ) -> Result<Self, Diagnostic> {
                let envelope = match generation {
                    $(
                        ModelArtifactGeneration::$variant => {
                            <$envelope>::from_program(program).map(AcceptedModelEnvelope::$variant)
                        }
                    )+
                }?;
                Ok(Self { envelope })
            }

            /// Decode bytes through exactly one caller-selected Model
            /// generation.
            ///
            /// No schema sniffing, retry, migration, or compatibility fallback
            /// occurs.
            ///
            /// # Errors
            /// Returns `EQ0901` for malformed, oversized, noncanonical,
            /// unsupported, or wrong-generation data.
            pub fn from_json(
                generation: ModelArtifactGeneration,
                bytes: &[u8],
                limits: DecoderLimits,
            ) -> Result<Self, Diagnostic> {
                let envelope = match generation {
                    $(
                        ModelArtifactGeneration::$variant => {
                            <$envelope>::from_json(bytes, limits)
                                .map(AcceptedModelEnvelope::$variant)
                        }
                    )+
                }?;
                Ok(Self { envelope })
            }

            /// Exact generation selected for this artifact.
            #[must_use]
            pub const fn generation(&self) -> ModelArtifactGeneration {
                match &self.envelope {
                    $(AcceptedModelEnvelope::$variant(_) => ModelArtifactGeneration::$variant,)+
                }
            }

            /// Deterministic compact canonical JSON.
            ///
            /// # Errors
            /// Returns `EQ0901` if serialization unexpectedly fails.
            pub fn canonical_json(&self) -> Result<Vec<u8>, Diagnostic> {
                match &self.envelope {
                    $(AcceptedModelEnvelope::$variant(envelope) => envelope.canonical_json(),)+
                }
            }

            /// Domain-separated identity in the selected Model schema.
            ///
            /// # Errors
            /// Returns `EQ0901` if canonical serialization fails.
            pub fn digest(&self) -> Result<ArtifactDigest, Diagnostic> {
                match &self.envelope {
                    $(AcceptedModelEnvelope::$variant(envelope) => envelope.digest(),)+
                }
            }

            /// Reconstruct the exact transaction and typed Model identity
            /// without committing them.
            ///
            /// # Errors
            /// Returns structured reconstruction diagnostics.
            pub fn to_transaction(
                &self,
            ) -> Result<(Transaction, OntologyId<Model>), Vec<Diagnostic>> {
                match &self.envelope {
                    $(AcceptedModelEnvelope::$variant(envelope) => envelope.to_transaction(),)+
                }
            }

            /// Source graph revision retained by the selected artifact.
            #[must_use]
            pub const fn source_revision(&self) -> u64 {
                match &self.envelope {
                    $(AcceptedModelEnvelope::$variant(envelope) => envelope.source_revision(),)+
                }
            }
        }

        $(
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
        )+

        impl sealed::Sealed for AcceptedModelArtifact {}

        impl CanonicalModelArtifact for AcceptedModelArtifact {
            fn artifact_reference(&self) -> Result<ModelArtifactReference, Diagnostic> {
                match &self.envelope {
                    $(AcceptedModelEnvelope::$variant(envelope) => envelope.artifact_reference(),)+
                }
            }
        }

        impl ReplayableCanonicalModelArtifact for AcceptedModelArtifact {
            fn replay_model(&self) -> Result<ReplayedCanonicalModel, Diagnostic> {
                match &self.envelope {
                    $(AcceptedModelEnvelope::$variant(envelope) => envelope.replay_model(),)+
                }
            }
        }
    };
}

// This is the only registration point for accepted Model artifact
// generations. All owned dispatch above is generated from this list.
define_model_artifact_registry!(
    (V1, ModelEnvelopeV1, "eqiora.model-envelope/v1"),
    (V2, ModelEnvelopeV2, "eqiora.model-envelope/v2"),
    (V3, ModelEnvelopeV3, "eqiora.model-envelope/v3"),
    (V4, ModelEnvelopeV4, "eqiora.model-envelope/v4"),
    (V5, ModelEnvelopeV5, "eqiora.model-envelope/v5"),
    (V6, ModelEnvelopeV6, "eqiora.model-envelope/v6"),
);
