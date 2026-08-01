//! Exact-source authored-region admission for the unchanged Field snapshot.

use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, Id};
use eqiora_geometry::{CanonicalGeometryV1, CircularHoleChordalMeshV1};

use super::context::ValidatedCircularHoleFieldwiseContext;
use super::field::FieldSnapshotEnvelopeV1;
use crate::{
    DiscreteFieldEnvelopeV1, GeometryDefinitionV1, GeometryMeshCorrespondenceEnvelopeV1,
    ModelEnvelope, RealizationEnvelopeV2, SimplicialMeshEnvelopeV1, invalid_artifact,
};

impl FieldSnapshotEnvelopeV1 {
    /// Bind one exact circular-hole authored-region P1 snapshot.
    ///
    /// The unchanged V1 wire is admitted only after replaying the Model,
    /// source-owned chordal mesh, authored correspondence, and field-wise V2
    /// Realization as one in-process lineage.
    ///
    /// # Errors
    /// Returns `EQ0901` for any Model, exact-source, owner, geometry,
    /// correspondence, mesh, Realization, Field, support, or block drift.
    #[allow(clippy::too_many_arguments)]
    pub fn new_authored_fieldwise(
        model: &ModelEnvelope,
        realization: &RealizationEnvelopeV2,
        source: &CanonicalGeometryV1,
        owner: &CircularHoleChordalMeshV1,
        geometry: &GeometryDefinitionV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
        mesh: &SimplicialMeshEnvelopeV1,
        field: Id<kinds::Field>,
        blocks: &[DiscreteFieldEnvelopeV1],
    ) -> Result<Self, Diagnostic> {
        let context = ValidatedCircularHoleFieldwiseContext::new(
            model,
            realization,
            source,
            owner,
            geometry,
            correspondence,
            mesh,
        )?;
        Self::new_in_context(&context, field, blocks)
    }

    /// Rebuild and compare this snapshot through the exact circular-hole
    /// authored field-wise lineage.
    ///
    /// # Errors
    /// Returns `EQ0901` for any semantic, source-owner, V2 Realization,
    /// geometry, mesh, correspondence, block, or content drift.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_against_authored_fieldwise(
        &self,
        model: &ModelEnvelope,
        realization: &RealizationEnvelopeV2,
        source: &CanonicalGeometryV1,
        owner: &CircularHoleChordalMeshV1,
        geometry: &GeometryDefinitionV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
        mesh: &SimplicialMeshEnvelopeV1,
        blocks: &[DiscreteFieldEnvelopeV1],
    ) -> Result<(), Diagnostic> {
        let expected = Self::new_authored_fieldwise(
            model,
            realization,
            source,
            owner,
            geometry,
            correspondence,
            mesh,
            self.field(),
            blocks,
        )?;
        if self != &expected {
            return Err(invalid_artifact(
                "Field snapshot differs from exact authored field-wise replay",
            ));
        }
        Ok(())
    }
}
