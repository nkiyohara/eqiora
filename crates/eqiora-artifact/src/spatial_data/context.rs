//! Validated spatial lineages shared by durable observations.

use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, Id};
use eqiora_geometry::{CanonicalGeometryRef, CanonicalGeometryV1, CircularHoleChordalMeshV1};
use eqiora_graph::{GraphStore, InMemoryGraphStore};
use eqiora_realization::{RepresentedPhysicalField, SemanticRevision};
use eqiora_schema::kernel::{DomainKind, KernelNode};
use eqiora_sem::KernelProgram;

use crate::{
    CanonicalModelArtifact, GeometryDefinitionV1, GeometryIdentityEnvelopeV1,
    GeometryMeshCorrespondenceEnvelopeV1, ModelArtifactReference, ModelEnvelope,
    RealizationEnvelopeV2, RealizationEnvelopeV3, ReplayableCanonicalModelArtifact,
    ReplayedCanonicalModel, SimplicialMeshEnvelopeV1, invalid_artifact,
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

/// Private proof of the one accepted exact-circle field-wise lineage.
///
/// This stays narrower than a general authored-geometry context. The opaque
/// chordal owner is required until a durable exact-source realization binding
/// exists, and its exact region must cover the complete imported mesh.
#[derive(Debug)]
pub(super) struct ValidatedCircularHoleFieldwiseContext<'a> {
    model_reference: ModelArtifactReference,
    program: KernelProgram,
    realization: &'a RealizationEnvelopeV2,
    geometry: &'a GeometryDefinitionV1,
    correspondence: &'a GeometryMeshCorrespondenceEnvelopeV1,
    mesh: &'a SimplicialMeshEnvelopeV1,
    domain: Id<kinds::Domain>,
    active_cells: Vec<usize>,
}

impl<'a> ValidatedCircularHoleFieldwiseContext<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        model: &ModelEnvelope,
        realization: &'a RealizationEnvelopeV2,
        source: &CanonicalGeometryV1,
        owner: &CircularHoleChordalMeshV1,
        geometry: &'a GeometryDefinitionV1,
        correspondence: &'a GeometryMeshCorrespondenceEnvelopeV1,
        mesh: &'a SimplicialMeshEnvelopeV1,
    ) -> Result<Self, Diagnostic> {
        let model_reference = model.artifact_reference()?;
        let (transaction, model_id) = model
            .to_transaction()
            .map_err(|diagnostics| authored_replay_error("reconstruct", diagnostics))?;
        let mut store = InMemoryGraphStore::new();
        store
            .commit(transaction)
            .map_err(|diagnostics| authored_replay_error("commit", diagnostics))?;
        let program = KernelProgram::from_snapshot_with_geometry(
            &store.snapshot(),
            model_id,
            &[CanonicalGeometryRef::from(source)],
        )
        .map_err(|diagnostics| authored_replay_error("admit exact geometry into", diagnostics))?;
        if program.model() != model_reference.model()
            || SemanticRevision::new(program.revision().0) != model_reference.semantic_revision()
        {
            return Err(invalid_artifact(
                "authored Model identity or revision differs after exact-geometry replay",
            ));
        }
        realization.validate_model_artifact(model)?;
        realization.validate_mesh_artifact(mesh)?;
        if owner.source().digest_bytes() != source.digest_bytes() {
            return Err(invalid_artifact(
                "circular-hole mesh owner belongs to another exact source revision",
            ));
        }
        if geometry != &GeometryDefinitionV1::from_region(owner.region())
            || mesh != &SimplicialMeshEnvelopeV1::from_mesh(owner.mesh())?
        {
            return Err(invalid_artifact(
                "authored field-wise geometry or mesh differs from its exact-source owner",
            ));
        }
        correspondence.validate_against_region(geometry, mesh)?;

        let domain = realization.plan()?.spatial().domain();
        let definition = match program.node(domain.erase()) {
            Some(KernelNode::Domain(definition)) => definition,
            _ => {
                return Err(invalid_artifact(
                    "field-wise Realization Domain is absent from the replayed Model",
                ));
            }
        };
        let DomainKind::GeometryRegion {
            geometry: model_source,
            entity_set,
        } = definition.kind()
        else {
            return Err(invalid_artifact(
                "authored field-wise projection requires one GeometryRegion Domain",
            ));
        };
        if model_source.bytes() != source.digest_bytes() {
            return Err(invalid_artifact(
                "Model GeometryRegion refers to another exact source revision",
            ));
        }
        let entities = correspondence.region_entity_set_entities(geometry, entity_set.as_str())?;
        if entities.is_empty()
            || entities
                .iter()
                .any(|entity| entity.dimension() != mesh.dimension())
        {
            return Err(invalid_artifact(
                "Model GeometryRegion does not select nonempty top-dimensional mesh cells",
            ));
        }
        let active_cells = entities
            .into_iter()
            .map(|entity| entity.index())
            .collect::<Vec<_>>();
        if active_cells
            .iter()
            .copied()
            .ne(0..mesh.mesh().cells().len())
        {
            return Err(invalid_artifact(
                "exact circular-hole GeometryRegion must realize every imported mesh cell",
            ));
        }

        Ok(Self {
            model_reference,
            program,
            realization,
            geometry,
            correspondence,
            mesh,
            domain,
            active_cells,
        })
    }

    pub(super) const fn model_reference(&self) -> &ModelArtifactReference {
        &self.model_reference
    }

    pub(super) const fn program(&self) -> &KernelProgram {
        &self.program
    }

    pub(super) const fn realization(&self) -> &'a RealizationEnvelopeV2 {
        self.realization
    }

    pub(super) const fn geometry(&self) -> &'a GeometryDefinitionV1 {
        self.geometry
    }

    pub(super) const fn correspondence(&self) -> &'a GeometryMeshCorrespondenceEnvelopeV1 {
        self.correspondence
    }

    pub(super) const fn mesh(&self) -> &'a SimplicialMeshEnvelopeV1 {
        self.mesh
    }

    pub(super) fn active_cells(&self, domain: Id<kinds::Domain>) -> Result<Vec<usize>, Diagnostic> {
        if domain != self.domain {
            return Err(invalid_artifact(
                "Field support differs from the exact GeometryRegion Domain",
            ));
        }
        Ok(self.active_cells.clone())
    }
}

fn authored_replay_error(action: &str, diagnostics: Vec<Diagnostic>) -> Diagnostic {
    invalid_artifact(format!(
        "cannot {action} authored Model: {}",
        diagnostics
            .iter()
            .map(Diagnostic::message)
            .collect::<Vec<_>>()
            .join("; ")
    ))
}
