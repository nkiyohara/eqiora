//! Validated spatial lineages shared by durable observations.

use std::num::NonZeroU16;

use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, Id};
use eqiora_geometry::CanonicalGeometryRef;
use eqiora_graph::{GraphStore, InMemoryGraphStore};
use eqiora_realization::{RepresentedPhysicalField, SemanticRevision, SpaceFamily};
use eqiora_schema::kernel::{DomainKind, KernelNode};
use eqiora_sem::KernelProgram;

use crate::{
    AcceptedCircularHoleChordalRealizationV1, ArtifactDigest, CanonicalModelArtifact,
    GeometryDefinitionV1, GeometryIdentityEnvelopeV1, GeometryMeshCorrespondenceEnvelopeV1,
    ModelArtifactReference, ModelEnvelope, PrescribedDynamicSolidRealizationEnvelopeV1,
    RealizationEnvelopeV2, RealizationEnvelopeV3, ReplayableCanonicalModelArtifact,
    ReplayedCanonicalModel, SimplicialMeshEnvelopeV1, invalid_artifact,
};

use super::field::ValidatedFieldSnapshotContext;

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

/// Private replay proof for the exact standalone prescribed-solid lineage.
#[derive(Debug)]
pub(super) struct ValidatedPrescribedDynamicSolidContext<'a> {
    replayed_model: ReplayedCanonicalModel,
    realization: &'a PrescribedDynamicSolidRealizationEnvelopeV1,
    geometry: &'a GeometryIdentityEnvelopeV1,
    correspondence: &'a GeometryMeshCorrespondenceEnvelopeV1,
    mesh: &'a SimplicialMeshEnvelopeV1,
    active_cells: Vec<usize>,
}

impl<'a> ValidatedPrescribedDynamicSolidContext<'a> {
    pub(super) fn new(
        model: &impl ReplayableCanonicalModelArtifact,
        realization: &'a PrescribedDynamicSolidRealizationEnvelopeV1,
        geometry: &'a GeometryIdentityEnvelopeV1,
        correspondence: &'a GeometryMeshCorrespondenceEnvelopeV1,
        mesh: &'a SimplicialMeshEnvelopeV1,
    ) -> Result<Self, Diagnostic> {
        realization.validate_against(model, geometry, correspondence, mesh)?;
        let replayed_model = model.replay_model()?;
        let active_cells = correspondence
            .body_cells(realization.solid_domain())
            .ok_or_else(|| {
                invalid_artifact("prescribed dynamic-solid body has no exact mesh-cell support")
            })?;
        if active_cells
            .iter()
            .copied()
            .ne(0..mesh.mesh().cells().len())
        {
            return Err(invalid_artifact(
                "prescribed dynamic-solid body does not cover the complete canonical mesh",
            ));
        }
        Ok(Self {
            replayed_model,
            realization,
            geometry,
            correspondence,
            mesh,
            active_cells,
        })
    }

    pub(super) const fn model_reference(&self) -> &ModelArtifactReference {
        self.replayed_model.artifact_reference()
    }

    pub(super) const fn program(&self) -> &KernelProgram {
        self.replayed_model.program()
    }

    pub(super) const fn realization(&self) -> &'a PrescribedDynamicSolidRealizationEnvelopeV1 {
        self.realization
    }

    pub(super) const fn geometry(&self) -> &'a GeometryIdentityEnvelopeV1 {
        self.geometry
    }

    pub(super) const fn correspondence(&self) -> &'a GeometryMeshCorrespondenceEnvelopeV1 {
        self.correspondence
    }

    pub(super) const fn mesh(&self) -> &'a SimplicialMeshEnvelopeV1 {
        self.mesh
    }

    pub(super) fn active_cells(&self, domain: Id<kinds::Domain>) -> Result<Vec<usize>, Diagnostic> {
        if domain != self.realization.solid_domain() {
            return Err(invalid_artifact(
                "prescribed dynamic-solid Field support differs from the solid Domain",
            ));
        }
        Ok(self.active_cells.clone())
    }

    pub(super) fn realized_field_space(
        &self,
        field: Id<kinds::Field>,
    ) -> Result<(Id<kinds::Domain>, SpaceFamily), Diagnostic> {
        if field != self.realization.displacement_field()
            && field != self.realization.velocity_field()
        {
            return Err(invalid_artifact(
                "Field is outside the prescribed dynamic-solid two-Field inventory",
            ));
        }
        Ok((
            self.realization.solid_domain(),
            SpaceFamily::ContinuousLagrange {
                order: NonZeroU16::MIN,
            },
        ))
    }
}

impl ValidatedFieldSnapshotContext for ValidatedPrescribedDynamicSolidContext<'_> {
    fn model_reference(&self) -> &ModelArtifactReference {
        ValidatedPrescribedDynamicSolidContext::model_reference(self)
    }

    fn program(&self) -> &KernelProgram {
        ValidatedPrescribedDynamicSolidContext::program(self)
    }

    fn realization_artifact(&self) -> Result<ArtifactDigest, Diagnostic> {
        self.realization().digest()
    }

    fn geometry_artifact(&self) -> Result<ArtifactDigest, Diagnostic> {
        self.geometry().digest()
    }

    fn correspondence(&self) -> &GeometryMeshCorrespondenceEnvelopeV1 {
        ValidatedPrescribedDynamicSolidContext::correspondence(self)
    }

    fn mesh(&self) -> &SimplicialMeshEnvelopeV1 {
        ValidatedPrescribedDynamicSolidContext::mesh(self)
    }

    fn active_cells(&self, domain: Id<kinds::Domain>) -> Result<Vec<usize>, Diagnostic> {
        ValidatedPrescribedDynamicSolidContext::active_cells(self, domain)
    }

    fn realized_field_space(
        &self,
        field: Id<kinds::Field>,
    ) -> Result<(Id<kinds::Domain>, SpaceFamily), Diagnostic> {
        ValidatedPrescribedDynamicSolidContext::realized_field_space(self, field)
    }
}

pub(super) fn realized_field_space_v3(
    realization: &RealizationEnvelopeV3,
    field: Id<kinds::Field>,
) -> Result<(Id<kinds::Domain>, SpaceFamily), Diagnostic> {
    let plan = realization.plan()?;
    for domain in plan.spatial().domains() {
        if let Some(binding) = domain
            .field_spaces()
            .iter()
            .find(|binding| binding.field() == field)
        {
            return Ok((domain.domain(), binding.space().family()));
        }
    }
    let eliminated = plan.time_step().eliminated_state();
    if eliminated.pair().state() == field {
        let rate = eliminated.pair().rate();
        let domain = plan
            .spatial()
            .domains()
            .iter()
            .find(|domain| {
                domain
                    .field_spaces()
                    .iter()
                    .any(|binding| binding.field() == rate)
            })
            .map(|domain| domain.domain())
            .ok_or_else(|| invalid_artifact("eliminated state rate has no realized Domain"))?;
        return Ok((domain, eliminated.state_space().family()));
    }
    Err(invalid_artifact(
        "Field snapshot Field is absent from the exact Realization",
    ))
}

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
