//! Validated spatial lineages shared by durable observations.

use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, Id};
use eqiora_graph::{GraphStore, InMemoryGraphStore};
use eqiora_realization::SemanticRevision;
use eqiora_schema::kernel::{DomainKind, KernelNode};
use eqiora_sem::KernelProgram;

use crate::{
    AcceptedCircularHoleChordalRealizationV1, CanonicalModelArtifact, GeometryDefinitionV1,
    GeometryMeshCorrespondenceEnvelopeV1, ModelArtifactReference, ModelEnvelope,
    RealizationEnvelopeV2, SimplicialMeshEnvelopeV1, invalid_artifact,
};

/// Private proof of the one accepted exact-circle field-wise lineage.
///
/// This stays narrower than a general authored-geometry context. The opaque
/// accepted artifact owner replays the exact-source realization binding, and
/// its exact region must cover the complete imported mesh.
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
    pub(super) fn new(
        model: &ModelEnvelope,
        realization: &'a RealizationEnvelopeV2,
        accepted: &'a AcceptedCircularHoleChordalRealizationV1,
    ) -> Result<Self, Diagnostic> {
        accepted.revalidate()?;
        let source = accepted.source();
        let geometry = accepted.realized_geometry();
        let correspondence = accepted.correspondence();
        let mesh = accepted.mesh();
        let model_reference = model.artifact_reference()?;
        let (transaction, model_id) = model
            .to_transaction()
            .map_err(|diagnostics| authored_replay_error("reconstruct", diagnostics))?;
        let mut store = InMemoryGraphStore::new();
        store
            .commit(transaction)
            .map_err(|diagnostics| authored_replay_error("commit", diagnostics))?;
        let program =
            KernelProgram::from_snapshot_with_geometry(&store.snapshot(), model_id, &[source])
                .map_err(|diagnostics| {
                    authored_replay_error("admit exact geometry into", diagnostics)
                })?;
        if program.model() != model_reference.model()
            || SemanticRevision::new(program.revision().0) != model_reference.semantic_revision()
        {
            return Err(invalid_artifact(
                "authored Model identity or revision differs after exact-geometry replay",
            ));
        }
        realization.validate_model_artifact(model)?;
        realization.validate_mesh_artifact(mesh)?;
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
