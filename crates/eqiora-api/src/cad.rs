//! Application composition for the bounded CAD/semantic-selection slice.

use std::collections::BTreeMap;

use eqiora_artifact::{
    ArtifactDigest, CadBuildEvidenceEnvelopeV1, CadDesignEnvelopeV1,
    GeometryAssociationArtifactError, GeometryIdentityEnvelopeV1,
    GeometryMeshCorrespondenceEnvelopeV1, GeometryRevisionAssociationEnvelopeV1,
    ReplayableCanonicalModelArtifact, SimplicialMeshEnvelopeV1,
};
use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, Id, RawId};
use eqiora_geometry::{
    AxisAlignedBox3, BodyAssociationCandidate, CadBoxDesignV1, CadKernelAdapter,
    ConstrainedRectangleV1, GeometryEntity, StepLengthUnitV1, StepSourceDigest,
};
use eqiora_graph::EdgeKind;
use eqiora_meshing::{MeshEntity, MeshQualityGate, SimplicialMesh};
use eqiora_schema::kernel::{BoundarySide, KernelNode, PortPayload};
use sha2::{Digest, Sha256};

use crate::{ModelDocument, VersionedModelEnvelope};

/// Complete caller intent for the bounded STEP-stock/intersection workflow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CadBoxIntentV1 {
    target_body: Id<kinds::Domain>,
    source_length_unit: StepLengthUnitV1,
    imported_stock: AxisAlignedBox3,
    sketch: ConstrainedRectangleV1,
    extrusion_depth_m: f64,
    source_uncertainty_m: f64,
    modeling_tolerance_m: f64,
    geometry_classification_tolerance_m: f64,
    mesh_minimum_mean_ratio: f64,
}

impl CadBoxIntentV1 {
    /// Construct one complete CAD and geometry/mesh policy request.
    ///
    /// The pure CAD constructors validate all geometric feature values. The
    /// Geometry Identity and mesh constructors independently validate their
    /// own tolerance and quality policies during preview.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub const fn new(
        target_body: Id<kinds::Domain>,
        source_length_unit: StepLengthUnitV1,
        imported_stock: AxisAlignedBox3,
        sketch: ConstrainedRectangleV1,
        extrusion_depth_m: f64,
        source_uncertainty_m: f64,
        modeling_tolerance_m: f64,
        geometry_classification_tolerance_m: f64,
        mesh_minimum_mean_ratio: f64,
    ) -> Self {
        Self {
            target_body,
            source_length_unit,
            imported_stock,
            sketch,
            extrusion_depth_m,
            source_uncertainty_m,
            modeling_tolerance_m,
            geometry_classification_tolerance_m,
            mesh_minimum_mean_ratio,
        }
    }

    /// Exact target Semantic body.
    #[must_use]
    pub const fn target_body(self) -> Id<kinds::Domain> {
        self.target_body
    }
}

/// Semantic kind of one selectable geometry projection row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CadSemanticEntityKindV1 {
    /// Full-dimensional body.
    Body,
    /// Parent-relative boundary face.
    Boundary,
}

/// One selectable Domain projected through exact geometry and mesh revisions.
#[derive(Clone, Debug, PartialEq)]
pub struct CadSemanticEntityV1 {
    domain: Id<kinds::Domain>,
    display_name: Option<String>,
    kind: CadSemanticEntityKindV1,
    geometry: GeometryEntity,
    parent: Option<Id<kinds::Domain>>,
    axis_side: Option<(usize, BoundarySide)>,
    mesh_entities: Vec<usize>,
    relations: Vec<Id<kinds::Relation>>,
    ports: Vec<Id<kinds::Port>>,
}

impl CadSemanticEntityV1 {
    /// Exact Semantic Domain identity.
    #[must_use]
    pub const fn domain(&self) -> Id<kinds::Domain> {
        self.domain
    }

    /// Optional source alias used only for presentation.
    #[must_use]
    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }

    /// Body or boundary role.
    #[must_use]
    pub const fn kind(&self) -> CadSemanticEntityKindV1 {
        self.kind
    }

    /// Revision-local geometry entity; meaningful only with the plan's exact
    /// Geometry Identity digest.
    #[must_use]
    pub const fn geometry(&self) -> GeometryEntity {
        self.geometry
    }

    /// Exact semantic parent for a boundary.
    #[must_use]
    pub const fn parent(&self) -> Option<Id<kinds::Domain>> {
        self.parent
    }

    /// Exact Cartesian boundary role for a boundary.
    #[must_use]
    pub const fn axis_side(&self) -> Option<(usize, BoundarySide)> {
        self.axis_side
    }

    /// Canonical cell or facet membership in the exact mesh revision.
    #[must_use]
    pub fn mesh_entities(&self) -> &[usize] {
        &self.mesh_entities
    }

    /// Relations applied on this Domain.
    #[must_use]
    pub fn relations(&self) -> &[Id<kinds::Relation>] {
        &self.relations
    }

    /// Boundary-physical Ports bound to this Domain.
    #[must_use]
    pub fn ports(&self) -> &[Id<kinds::Port>] {
        &self.ports
    }
}

/// Renderer input whose primitives carry Semantic Domain identity, never
/// kernel face or renderer-local identity.
#[derive(Clone, Debug, PartialEq)]
pub struct CadRenderProjectionV1 {
    geometry: ArtifactDigest,
    mesh: ArtifactDigest,
    vertices_m: Vec<[f64; 3]>,
    boundary_triangles: Vec<CadRenderTriangleV1>,
}

impl CadRenderProjectionV1 {
    /// Exact Geometry Identity revision.
    #[must_use]
    pub const fn geometry(&self) -> &ArtifactDigest {
        &self.geometry
    }

    /// Exact mesh revision.
    #[must_use]
    pub const fn mesh(&self) -> &ArtifactDigest {
        &self.mesh
    }

    /// Bounded coherent-SI vertices.
    #[must_use]
    pub fn vertices_m(&self) -> &[[f64; 3]] {
        &self.vertices_m
    }

    /// Canonically Domain/facet-ordered boundary triangles.
    #[must_use]
    pub fn boundary_triangles(&self) -> &[CadRenderTriangleV1] {
        &self.boundary_triangles
    }
}

/// One render triangle mapped directly to its exact Semantic boundary Domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CadRenderTriangleV1 {
    domain: Id<kinds::Domain>,
    vertex_indices: [usize; 3],
}

impl CadRenderTriangleV1 {
    /// Exact boundary Domain selected by this projected triangle.
    #[must_use]
    pub const fn domain(self) -> Id<kinds::Domain> {
        self.domain
    }

    /// Render-only vertex indices; these cannot enter a selection request.
    #[must_use]
    pub const fn vertex_indices(self) -> [usize; 3] {
        self.vertex_indices
    }
}

/// Exact selection request shared by viewport and semantic table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CadSelectionRequestV1 {
    geometry: ArtifactDigest,
    domain: Id<kinds::Domain>,
}

impl CadSelectionRequestV1 {
    /// Name one Semantic Domain in one exact Geometry Identity revision.
    #[must_use]
    pub const fn new(geometry: ArtifactDigest, domain: Id<kinds::Domain>) -> Self {
        Self { geometry, domain }
    }

    /// Exact Geometry Identity artifact.
    #[must_use]
    pub const fn geometry(&self) -> &ArtifactDigest {
        &self.geometry
    }

    /// Exact Semantic Domain.
    #[must_use]
    pub const fn domain(&self) -> Id<kinds::Domain> {
        self.domain
    }
}

/// Accepted semantic selection; input modality is deliberately absent.
#[derive(Clone, Debug, PartialEq)]
pub struct CadSemanticSelectionV1 {
    geometry: ArtifactDigest,
    entity: CadSemanticEntityV1,
}

impl CadSemanticSelectionV1 {
    /// Exact Geometry Identity revision.
    #[must_use]
    pub const fn geometry(&self) -> &ArtifactDigest {
        &self.geometry
    }

    /// Exact selected semantic/geometry/mesh projection.
    #[must_use]
    pub const fn entity(&self) -> &CadSemanticEntityV1 {
        &self.entity
    }
}

/// Immutable preview closing CAD, Geometry Identity, mesh, correspondence,
/// and semantic selection over one exact Model.
#[derive(Clone, Debug)]
pub struct CadBoxPlanV1 {
    model: VersionedModelEnvelope,
    model_digest: String,
    key: String,
    design: CadDesignEnvelopeV1,
    build: CadBuildEvidenceEnvelopeV1,
    geometry: GeometryIdentityEnvelopeV1,
    mesh: SimplicialMeshEnvelopeV1,
    correspondence: GeometryMeshCorrespondenceEnvelopeV1,
    entities: Vec<CadSemanticEntityV1>,
    render: CadRenderProjectionV1,
}

impl CadBoxPlanV1 {
    /// Exact preview-to-replay key over every accepted artifact.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Exact Model artifact digest.
    #[must_use]
    pub fn model_digest(&self) -> &str {
        &self.model_digest
    }

    /// Canonical CAD design artifact.
    #[must_use]
    pub const fn design(&self) -> &CadDesignEnvelopeV1 {
        &self.design
    }

    /// Exact adapter replay evidence.
    #[must_use]
    pub const fn build(&self) -> &CadBuildEvidenceEnvelopeV1 {
        &self.build
    }

    /// Exact Geometry Identity artifact.
    #[must_use]
    pub const fn geometry(&self) -> &GeometryIdentityEnvelopeV1 {
        &self.geometry
    }

    /// Exact tetrahedral mesh artifact.
    #[must_use]
    pub const fn mesh(&self) -> &SimplicialMeshEnvelopeV1 {
        &self.mesh
    }

    /// Exact geometry-to-mesh correspondence.
    #[must_use]
    pub const fn correspondence(&self) -> &GeometryMeshCorrespondenceEnvelopeV1 {
        &self.correspondence
    }

    /// Complete selectable body and boundary inventory.
    #[must_use]
    pub fn entities(&self) -> &[CadSemanticEntityV1] {
        &self.entities
    }

    /// Bounded render projection.
    #[must_use]
    pub const fn render(&self) -> &CadRenderProjectionV1 {
        &self.render
    }

    /// Create the only admitted request shape for one projected Domain.
    ///
    /// # Errors
    /// Returns `EQ0901` when the Domain is absent from this exact revision.
    pub fn selection_request(
        &self,
        domain: Id<kinds::Domain>,
    ) -> Result<CadSelectionRequestV1, Diagnostic> {
        if !self.entities.iter().any(|entity| entity.domain == domain) {
            return Err(invalid_cad("selection Domain is absent from this CAD plan"));
        }
        Ok(CadSelectionRequestV1::new(self.geometry.digest()?, domain))
    }

    /// Resolve a viewport/table request to exact semantic and numerical meaning.
    ///
    /// # Errors
    /// Returns `EQ0901` for a stale geometry digest or unknown Domain.
    pub fn resolve_selection(
        &self,
        request: &CadSelectionRequestV1,
    ) -> Result<CadSemanticSelectionV1, Diagnostic> {
        let geometry = self.geometry.digest()?;
        if request.geometry != geometry {
            return Err(invalid_cad(
                "selection references a stale Geometry Identity revision",
            ));
        }
        let entity = self
            .entities
            .iter()
            .find(|entity| entity.domain == request.domain)
            .cloned()
            .ok_or_else(|| invalid_cad("selection Domain is absent from exact CAD projection"))?;
        Ok(CadSemanticSelectionV1 { geometry, entity })
    }

    /// Replay exact STEP bytes, adapter identity, Model, geometry, mesh, and
    /// correspondence without trusting the preview cache.
    ///
    /// # Errors
    /// Returns `EQ0901` for any replay drift.
    pub fn validate_replay(
        &self,
        adapter: &impl CadKernelAdapter,
        step_bytes: &[u8],
    ) -> Result<(), Diagnostic> {
        match &self.model {
            VersionedModelEnvelope::V1(model) => self.validate_with(model, adapter, step_bytes),
            VersionedModelEnvelope::V2(model) => self.validate_with(model, adapter, step_bytes),
            VersionedModelEnvelope::V3(model) => self.validate_with(model, adapter, step_bytes),
            VersionedModelEnvelope::V4(model) => self.validate_with(model, adapter, step_bytes),
            VersionedModelEnvelope::V5(model) => self.validate_with(model, adapter, step_bytes),
            VersionedModelEnvelope::V6(model) => self.validate_with(model, adapter, step_bytes),
        }
    }

    /// Validate a decoded build-evidence candidate against every exact
    /// resource owned by this plan.
    ///
    /// # Errors
    /// Returns `EQ0901` for Model, design, geometry, source, adapter, or
    /// normalized-observation drift.
    pub fn validate_build_evidence(
        &self,
        evidence: &CadBuildEvidenceEnvelopeV1,
        adapter: &impl CadKernelAdapter,
        step_bytes: &[u8],
    ) -> Result<(), Diagnostic> {
        match &self.model {
            VersionedModelEnvelope::V1(model) => {
                evidence.validate_replay(model, &self.design, &self.geometry, adapter, step_bytes)
            }
            VersionedModelEnvelope::V2(model) => {
                evidence.validate_replay(model, &self.design, &self.geometry, adapter, step_bytes)
            }
            VersionedModelEnvelope::V3(model) => {
                evidence.validate_replay(model, &self.design, &self.geometry, adapter, step_bytes)
            }
            VersionedModelEnvelope::V4(model) => {
                evidence.validate_replay(model, &self.design, &self.geometry, adapter, step_bytes)
            }
            VersionedModelEnvelope::V5(model) => {
                evidence.validate_replay(model, &self.design, &self.geometry, adapter, step_bytes)
            }
            VersionedModelEnvelope::V6(model) => {
                evidence.validate_replay(model, &self.design, &self.geometry, adapter, step_bytes)
            }
        }
    }

    fn validate_with(
        &self,
        model: &impl ReplayableCanonicalModelArtifact,
        adapter: &impl CadKernelAdapter,
        step_bytes: &[u8],
    ) -> Result<(), Diagnostic> {
        self.build
            .validate_replay(model, &self.design, &self.geometry, adapter, step_bytes)?;
        self.correspondence
            .validate_against(&self.geometry, model, &self.mesh)?;
        if plan_key(
            &self.design,
            &self.build,
            &self.geometry,
            &self.mesh,
            &self.correspondence,
        )? != self.key
        {
            return Err(invalid_cad(
                "CAD plan replay key differs from exact artifacts",
            ));
        }
        Ok(())
    }

    /// Close an explicit total one-to-one successor relation to another exact
    /// CAD plan.
    ///
    /// # Errors
    /// Returns a typed retention failure as `EQ0901`; no successor selection
    /// is emitted on missing, split, merged, or ambiguous topology.
    pub fn associate_regeneration(
        &self,
        target: CadBoxPlanV1,
    ) -> Result<CadRegenerationPlanV1, Diagnostic> {
        let source_body = self.design.design()?.target_body();
        let target_body = target.design.design()?.target_body();
        let association = associate_model_pair(self, &target, source_body, target_body)?;
        let key = regeneration_key(&self.key, &target.key, &association.digest()?);
        Ok(CadRegenerationPlanV1 {
            key,
            association,
            source: self.clone(),
            target,
        })
    }
}

/// Exact geometry/mesh successor plus the target CAD plan.
#[derive(Clone, Debug)]
pub struct CadRegenerationPlanV1 {
    key: String,
    association: GeometryRevisionAssociationEnvelopeV1,
    source: CadBoxPlanV1,
    target: CadBoxPlanV1,
}

impl CadRegenerationPlanV1 {
    /// Exact source plan key.
    #[must_use]
    pub fn source_key(&self) -> &str {
        &self.source.key
    }

    /// Exact source CAD plan.
    #[must_use]
    pub const fn source(&self) -> &CadBoxPlanV1 {
        &self.source
    }

    /// Exact regeneration plan key.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Explicit total one-to-one revision association.
    #[must_use]
    pub const fn association(&self) -> &GeometryRevisionAssociationEnvelopeV1 {
        &self.association
    }

    /// Exact target CAD plan.
    #[must_use]
    pub const fn target(&self) -> &CadBoxPlanV1 {
        &self.target
    }

    /// Replay both CAD plans and the exact geometry-revision association.
    ///
    /// # Errors
    /// Returns `EQ0901` for source, target, association, or key drift.
    pub fn validate_replay(
        &self,
        source_adapter: &impl CadKernelAdapter,
        source_step_bytes: &[u8],
        target_adapter: &impl CadKernelAdapter,
        target_step_bytes: &[u8],
    ) -> Result<(), Diagnostic> {
        self.source
            .validate_replay(source_adapter, source_step_bytes)?;
        self.target
            .validate_replay(target_adapter, target_step_bytes)?;
        let source_body = self.source.design.design()?.target_body();
        let target_body = self.target.design.design()?.target_body();
        let expected = associate_model_pair(&self.source, &self.target, source_body, target_body)?;
        if expected != self.association {
            return Err(invalid_cad(
                "CAD regeneration association differs from exact replay",
            ));
        }
        let expected_key = regeneration_key(
            &self.source.key,
            &self.target.key,
            &self.association.digest()?,
        );
        if expected_key != self.key {
            return Err(invalid_cad(
                "CAD regeneration key differs from exact source, target, and association",
            ));
        }
        Ok(())
    }

    /// Carry one accepted source selection only through the explicit
    /// association, then resolve it against the target revision.
    ///
    /// # Errors
    /// Returns `EQ0901` if the source Domain was not retained or the target
    /// projection does not contain the associated Domain.
    pub fn retain_selection(
        &self,
        source: &CadSemanticSelectionV1,
    ) -> Result<CadSemanticSelectionV1, Diagnostic> {
        if source.geometry != self.source.geometry.digest()? {
            return Err(invalid_cad(
                "selection does not belong to the regeneration source Geometry Identity revision",
            ));
        }
        let target_domain = match source.entity.kind {
            CadSemanticEntityKindV1::Body => {
                self.association.retained_body_target(source.entity.domain)
            }
            CadSemanticEntityKindV1::Boundary => self
                .association
                .retained_boundary_target(source.entity.domain),
        }
        .ok_or_else(|| invalid_cad("selected Domain has no exact retained successor"))?;
        let request = self.target.selection_request(target_domain)?;
        self.target.resolve_selection(&request)
    }
}

impl ModelDocument {
    /// Build the first CAD vertical slice through an exact adapter and ordinary
    /// Geometry Identity/mesh correspondence artifacts.
    ///
    /// # Errors
    /// Returns one structured diagnostic for source, adapter, Model, geometry,
    /// mesh, correspondence, or resource drift.
    pub fn preview_cad_box(
        &self,
        intent: CadBoxIntentV1,
        adapter: &impl CadKernelAdapter,
        step_bytes: &[u8],
    ) -> Result<CadBoxPlanV1, Diagnostic> {
        match &self.envelope {
            VersionedModelEnvelope::V1(model) => {
                preview_with_model(self, model, intent, adapter, step_bytes)
            }
            VersionedModelEnvelope::V2(model) => {
                preview_with_model(self, model, intent, adapter, step_bytes)
            }
            VersionedModelEnvelope::V3(model) => {
                preview_with_model(self, model, intent, adapter, step_bytes)
            }
            VersionedModelEnvelope::V4(model) => {
                preview_with_model(self, model, intent, adapter, step_bytes)
            }
            VersionedModelEnvelope::V5(model) => {
                preview_with_model(self, model, intent, adapter, step_bytes)
            }
            VersionedModelEnvelope::V6(model) => {
                preview_with_model(self, model, intent, adapter, step_bytes)
            }
        }
    }
}

fn preview_with_model(
    document: &ModelDocument,
    model: &impl ReplayableCanonicalModelArtifact,
    intent: CadBoxIntentV1,
    adapter: &impl CadKernelAdapter,
    step_bytes: &[u8],
) -> Result<CadBoxPlanV1, Diagnostic> {
    let design = CadBoxDesignV1::new(
        intent.target_body,
        StepSourceDigest::from_source_bytes(step_bytes),
        intent.source_length_unit,
        intent.imported_stock,
        intent.sketch,
        intent.extrusion_depth_m,
        intent.source_uncertainty_m,
        intent.modeling_tolerance_m,
    )?;
    let design_artifact = CadDesignEnvelopeV1::new(model, &design)?;
    let realization = adapter.realize_box_design(&design, step_bytes)?;
    let geometry = GeometryIdentityEnvelopeV1::new(
        model,
        [intent.target_body],
        intent.geometry_classification_tolerance_m,
    )?;
    let build = CadBuildEvidenceEnvelopeV1::new(
        model,
        &design_artifact,
        &geometry,
        adapter.identity(),
        realization,
    )?;
    let mesh = SimplicialMeshEnvelopeV1::from_mesh(&tetrahedral_box_mesh(
        design.output(),
        intent.mesh_minimum_mean_ratio,
    )?)?;
    let correspondence = GeometryMeshCorrespondenceEnvelopeV1::new(&geometry, model, &mesh)?;
    let entities = project_entities(document, &geometry, &correspondence)?;
    let render = render_projection(&geometry, &mesh, &correspondence)?;
    let key = plan_key(&design_artifact, &build, &geometry, &mesh, &correspondence)?;
    let plan = CadBoxPlanV1 {
        model: document.envelope.clone(),
        model_digest: document.digest()?,
        key,
        design: design_artifact,
        build,
        geometry,
        mesh,
        correspondence,
        entities,
        render,
    };
    plan.validate_replay(adapter, step_bytes)?;
    Ok(plan)
}

fn tetrahedral_box_mesh(
    bounds: AxisAlignedBox3,
    minimum_mean_ratio: f64,
) -> Result<SimplicialMesh, Diagnostic> {
    let [x, y, z] = bounds.bounds_m();
    let vertices = vec![
        vec![x.0, y.0, z.0],
        vec![x.1, y.0, z.0],
        vec![x.0, y.1, z.0],
        vec![x.1, y.1, z.0],
        vec![x.0, y.0, z.1],
        vec![x.1, y.0, z.1],
        vec![x.0, y.1, z.1],
        vec![x.1, y.1, z.1],
    ];
    let cells = vec![
        vec![0, 1, 3, 7],
        vec![0, 3, 2, 7],
        vec![0, 2, 6, 7],
        vec![0, 6, 4, 7],
        vec![0, 4, 5, 7],
        vec![0, 5, 1, 7],
    ];
    let gate = MeshQualityGate::new(minimum_mean_ratio)?;
    SimplicialMesh::new(3, vertices, cells, gate)
}

fn project_entities(
    document: &ModelDocument,
    geometry: &GeometryIdentityEnvelopeV1,
    correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
) -> Result<Vec<CadSemanticEntityV1>, Diagnostic> {
    let names = document
        .aliases
        .iter()
        .map(|(name, id)| (*id, name.clone()))
        .collect::<BTreeMap<_, _>>();
    let relations_by_domain = document
        .program
        .edges()
        .iter()
        .filter(|edge| edge.kind() == EdgeKind::AppliesOn)
        .fold(
            BTreeMap::<RawId, Vec<Id<kinds::Relation>>>::new(),
            |mut map, edge| {
                if let Some(relation) = edge.from().downcast::<kinds::Relation>() {
                    map.entry(edge.to()).or_default().push(relation);
                }
                map
            },
        );
    let ports_by_domain = document.program.nodes().fold(
        BTreeMap::<RawId, Vec<Id<kinds::Port>>>::new(),
        |mut map, node| {
            if let KernelNode::Port(port) = node
                && let PortPayload::BoundaryPhysical { boundary, .. } = port.payload()
            {
                map.entry(boundary.erase()).or_default().push(port.id());
            }
            map
        },
    );

    let mut entities = geometry
        .bodies()
        .into_iter()
        .map(|body| {
            let raw = body.domain().erase();
            Ok(CadSemanticEntityV1 {
                domain: body.domain(),
                display_name: names.get(&raw).cloned(),
                kind: CadSemanticEntityKindV1::Body,
                geometry: body.entity(),
                parent: None,
                axis_side: None,
                mesh_entities: correspondence
                    .body_cells(body.domain())
                    .ok_or_else(|| invalid_cad("CAD body has no mesh correspondence"))?,
                relations: sorted_ids(relations_by_domain.get(&raw)),
                ports: sorted_ids(ports_by_domain.get(&raw)),
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    entities.extend(
        geometry
            .boundaries()
            .into_iter()
            .map(|boundary| {
                let raw = boundary.domain().erase();
                Ok(CadSemanticEntityV1 {
                    domain: boundary.domain(),
                    display_name: names.get(&raw).cloned(),
                    kind: CadSemanticEntityKindV1::Boundary,
                    geometry: boundary.entity(),
                    parent: Some(boundary.parent()),
                    axis_side: Some((boundary.axis(), boundary.side())),
                    mesh_entities: correspondence
                        .boundary_facets(boundary.domain())
                        .ok_or_else(|| invalid_cad("CAD boundary has no mesh correspondence"))?,
                    relations: sorted_ids(relations_by_domain.get(&raw)),
                    ports: sorted_ids(ports_by_domain.get(&raw)),
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?,
    );
    entities.sort_by_key(|entity| entity.domain.ulid());
    Ok(entities)
}

fn sorted_ids<E: eqiora_core::Entity>(values: Option<&Vec<Id<E>>>) -> Vec<Id<E>> {
    let mut values = values.cloned().unwrap_or_default();
    values.sort_by_key(Id::ulid);
    values
}

fn render_projection(
    geometry: &GeometryIdentityEnvelopeV1,
    mesh: &SimplicialMeshEnvelopeV1,
    correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
) -> Result<CadRenderProjectionV1, Diagnostic> {
    let topology = mesh.mesh();
    let vertices_m = topology
        .vertices()
        .iter()
        .map(|point| {
            <[f64; 3]>::try_from(point.as_slice())
                .map_err(|_| invalid_cad("CAD renderer requires exact 3D mesh coordinates"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut boundary_triangles = Vec::new();
    for boundary in geometry.boundaries() {
        for facet_index in correspondence
            .boundary_facets(boundary.domain())
            .ok_or_else(|| invalid_cad("CAD renderer boundary has no exact facet membership"))?
        {
            let indices = topology
                .entity_vertices(MeshEntity::new(2, facet_index))
                .ok_or_else(|| invalid_cad("CAD renderer facet is outside exact mesh"))?;
            let vertex_indices = <[usize; 3]>::try_from(
                indices
                    .iter()
                    .map(|entity| entity.index())
                    .collect::<Vec<_>>(),
            )
            .map_err(|_| invalid_cad("CAD renderer admits only triangular boundary facets"))?;
            boundary_triangles.push(CadRenderTriangleV1 {
                domain: boundary.domain(),
                vertex_indices,
            });
        }
    }
    boundary_triangles.sort_by_key(|triangle| (triangle.domain.ulid(), triangle.vertex_indices));
    Ok(CadRenderProjectionV1 {
        geometry: geometry.digest()?,
        mesh: mesh.digest()?,
        vertices_m,
        boundary_triangles,
    })
}

fn plan_key(
    design: &CadDesignEnvelopeV1,
    build: &CadBuildEvidenceEnvelopeV1,
    geometry: &GeometryIdentityEnvelopeV1,
    mesh: &SimplicialMeshEnvelopeV1,
    correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
) -> Result<String, Diagnostic> {
    let mut hasher = Sha256::new();
    hasher.update(b"eqiora.cad-box-plan/v1");
    hasher.update([0]);
    for digest in [
        design.digest()?,
        build.digest()?,
        geometry.digest()?,
        mesh.digest()?,
        correspondence.digest()?,
    ] {
        hasher.update(digest.sha256_bytes());
    }
    Ok(hex_digest(hasher.finalize().into()))
}

fn regeneration_key(source: &str, target: &str, association: &ArtifactDigest) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"eqiora.cad-regeneration-plan/v1");
    hasher.update([0]);
    hasher.update(source.as_bytes());
    hasher.update(target.as_bytes());
    hasher.update(association.sha256_bytes());
    hex_digest(hasher.finalize().into())
}

fn hex_digest(bytes: [u8; 32]) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(64);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn associate_model_pair(
    source: &CadBoxPlanV1,
    target: &CadBoxPlanV1,
    source_body: Id<kinds::Domain>,
    target_body: Id<kinds::Domain>,
) -> Result<GeometryRevisionAssociationEnvelopeV1, Diagnostic> {
    macro_rules! associate {
        ($source_model:expr, $target_model:expr) => {{
            GeometryRevisionAssociationEnvelopeV1::new(
                $source_model,
                &source.geometry,
                &source.correspondence,
                &source.mesh,
                $target_model,
                &target.geometry,
                &target.correspondence,
                &target.mesh,
                vec![BodyAssociationCandidate::new(source_body, target_body)],
            )
            .map_err(association_diagnostic)
        }};
    }
    match (&source.model, &target.model) {
        (VersionedModelEnvelope::V1(source), VersionedModelEnvelope::V1(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V1(source), VersionedModelEnvelope::V2(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V1(source), VersionedModelEnvelope::V3(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V1(source), VersionedModelEnvelope::V4(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V1(source), VersionedModelEnvelope::V5(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V1(source), VersionedModelEnvelope::V6(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V2(source), VersionedModelEnvelope::V1(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V2(source), VersionedModelEnvelope::V2(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V2(source), VersionedModelEnvelope::V3(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V2(source), VersionedModelEnvelope::V4(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V2(source), VersionedModelEnvelope::V5(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V2(source), VersionedModelEnvelope::V6(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V3(source), VersionedModelEnvelope::V1(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V3(source), VersionedModelEnvelope::V2(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V3(source), VersionedModelEnvelope::V3(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V3(source), VersionedModelEnvelope::V4(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V3(source), VersionedModelEnvelope::V5(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V3(source), VersionedModelEnvelope::V6(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V4(source), VersionedModelEnvelope::V1(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V4(source), VersionedModelEnvelope::V2(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V4(source), VersionedModelEnvelope::V3(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V4(source), VersionedModelEnvelope::V4(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V4(source), VersionedModelEnvelope::V5(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V4(source), VersionedModelEnvelope::V6(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V5(source), VersionedModelEnvelope::V1(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V5(source), VersionedModelEnvelope::V2(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V5(source), VersionedModelEnvelope::V3(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V5(source), VersionedModelEnvelope::V4(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V5(source), VersionedModelEnvelope::V5(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V5(source), VersionedModelEnvelope::V6(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V6(source), VersionedModelEnvelope::V1(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V6(source), VersionedModelEnvelope::V2(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V6(source), VersionedModelEnvelope::V3(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V6(source), VersionedModelEnvelope::V4(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V6(source), VersionedModelEnvelope::V5(target)) => {
            associate!(source, target)
        }
        (VersionedModelEnvelope::V6(source), VersionedModelEnvelope::V6(target)) => {
            associate!(source, target)
        }
    }
}

fn association_diagnostic(error: GeometryAssociationArtifactError) -> Diagnostic {
    invalid_cad(format!("CAD regeneration association failed: {error}"))
}

fn invalid_cad(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}
