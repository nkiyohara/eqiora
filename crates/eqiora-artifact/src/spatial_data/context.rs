//! One validated fixed-spatial lineage shared by durable observations.

use eqiora_core::Diagnostic;
use eqiora_realization::RepresentedPhysicalField;
use eqiora_sem::KernelProgram;

use crate::{
    GeometryIdentityEnvelopeV1, GeometryMeshCorrespondenceEnvelopeV1, ModelArtifactReference,
    RealizationEnvelopeV3, ReplayableCanonicalModelArtifact, ReplayedCanonicalModel,
    SimplicialMeshEnvelopeV1,
};

/// Runtime proof that one fixed-spatial artifact lineage has been replayed.
///
/// This is deliberately a borrowed, non-serializable token rather than a new
/// universal context artifact. Model replay and geometry/correspondence checks
/// run once; every Field and state constructed through the token inherits the
/// same exact Model, Realization, geometry, correspondence, mesh, and physical
/// observation inventory.
#[derive(Debug)]
pub struct ValidatedFixedSpatialContextV1<'a> {
    replayed_model: ReplayedCanonicalModel,
    realization: &'a RealizationEnvelopeV3,
    geometry: &'a GeometryIdentityEnvelopeV1,
    correspondence: &'a GeometryMeshCorrespondenceEnvelopeV1,
    mesh: &'a SimplicialMeshEnvelopeV1,
    represented_fields: Vec<RepresentedPhysicalField>,
}

impl<'a> ValidatedFixedSpatialContextV1<'a> {
    /// Replay and cross-validate one exact fixed-spatial lineage.
    ///
    /// # Errors
    /// Returns `EQ0901` for any Model, Realization, geometry,
    /// correspondence, mesh, or represented-Field drift.
    pub fn new(
        model: &impl ReplayableCanonicalModelArtifact,
        realization: &'a RealizationEnvelopeV3,
        geometry: &'a GeometryIdentityEnvelopeV1,
        correspondence: &'a GeometryMeshCorrespondenceEnvelopeV1,
        mesh: &'a SimplicialMeshEnvelopeV1,
    ) -> Result<Self, Diagnostic> {
        let replayed_model = model.replay_model()?;
        realization.validate_model_artifact(model)?;
        realization.validate_mesh_artifact(mesh)?;
        geometry.validate_against(model)?;
        correspondence.validate_against(geometry, model, mesh)?;
        let represented_fields = realization.plan()?.represented_physical_fields()?;
        Ok(Self {
            replayed_model,
            realization,
            geometry,
            correspondence,
            mesh,
            represented_fields,
        })
    }

    /// Exact selected Model wire identity and semantic revision.
    #[must_use]
    pub const fn model_reference(&self) -> &ModelArtifactReference {
        self.replayed_model.artifact_reference()
    }

    /// Completely validated immutable Semantic Kernel projection.
    #[must_use]
    pub const fn program(&self) -> &KernelProgram {
        self.replayed_model.program()
    }

    /// Exact coupled fixed-spatial Realization.
    #[must_use]
    pub const fn realization(&self) -> &'a RealizationEnvelopeV3 {
        self.realization
    }

    /// Exact geometry revision.
    #[must_use]
    pub const fn geometry(&self) -> &'a GeometryIdentityEnvelopeV1 {
        self.geometry
    }

    /// Exact geometry-to-mesh correspondence.
    #[must_use]
    pub const fn correspondence(&self) -> &'a GeometryMeshCorrespondenceEnvelopeV1 {
        self.correspondence
    }

    /// Exact immutable mesh revision.
    #[must_use]
    pub const fn mesh(&self) -> &'a SimplicialMeshEnvelopeV1 {
        self.mesh
    }

    /// Complete physics-neutral physical Field inventory selected by the
    /// Realization, including represented-but-eliminated state Fields.
    #[must_use]
    pub fn represented_fields(&self) -> &[RepresentedPhysicalField] {
        &self.represented_fields
    }
}
