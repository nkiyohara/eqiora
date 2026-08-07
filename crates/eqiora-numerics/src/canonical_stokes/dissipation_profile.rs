//! Private exact-area profile and one-way Stokes design binding.
use super::api::{SteadyIncompressibleStokesModel2d, StokesBoundaryKey2d};
use super::geometry_realization::require_stokes_dissipation_mesh_predicates;
use super::recognize::lower_stokes_dissipation_profile_model_2d;
use crate::canonical_boundary::{PhysicalBoundaryDisposition, PhysicalBoundaryQuantity};
use crate::simplicial_ale_fsi::P1HarmonicMeshMotionAction2d;
use crate::simplicial_fsi::FixedReferenceFsiPartition2d;
use eqiora_artifact::{
    CanonicalModelArtifact, GeometryDefinitionV1, GeometryMeshCorrespondenceEnvelopeV1,
    ModelArtifactReference, ModelEnvelope, SimplicialMeshEnvelopeV1,
};
use eqiora_core::{Diagnostic, DimExponents, RawId};
use eqiora_geometry::{EDGE_DIMENSION, FACE_DIMENSION, NamedEntitySet, PlanarFace, PlanarRegion};
use eqiora_meshing::{
    CellId, FacetId, FixedTopologyGeometryState2d, MeshEntity, MeshQualityGate, MeshTopology,
    SimplicialMesh,
};
use eqiora_schema::kernel::KernelNode;
use eqiora_sem::KernelProgram;
use eqiora_solver::LinearSolveRequest;
use std::collections::{BTreeMap, BTreeSet};

const PROFILE_FORMULA_VERSION: &str = "stokes-dissipation-two-mode-exact-area-v1";
const REFERENCE_TOPOLOGY_ID: &str = "stokes-square-ring-reference-n32-m4-v1";
const REFINED_TOPOLOGY_ID: &str = "stokes-square-ring-refined-n64-m8-v1";
const LENGTH: DimExponents = DimExponents {
    length: 1,
    ..DimExponents::DIMENSIONLESS
};
const VELOCITY: DimExponents = DimExponents {
    length: 1,
    time: -1,
    ..DimExponents::DIMENSIONLESS
};
const VISCOSITY: DimExponents = DimExponents {
    mass: 1,
    length: -1,
    time: -1,
    ..DimExponents::DIMENSIONLESS
};
#[derive(Debug, Clone, PartialEq)]
pub(super) struct StokesDissipationProfileGeometry2d {
    formula_version: &'static str,
    area_radius_parameter: RawId,
    a2_parameter: RawId,
    a4_parameter: RawId,
    pub(super) area_radius_m: f64,
    a2: f64,
    a4: f64,
}
impl StokesDissipationProfileGeometry2d {
    pub(super) fn new(
        area_radius_parameter: RawId,
        a2_parameter: RawId,
        a4_parameter: RawId,
        area_radius_m: f64,
        a2: f64,
        a4: f64,
    ) -> Result<Self, Diagnostic> {
        if [area_radius_parameter, a2_parameter, a4_parameter]
            .into_iter()
            .collect::<BTreeSet<_>>()
            .len()
            != 3
        {
            return Err(invalid("profile Parameter identities must be distinct"));
        }
        if !area_radius_m.is_finite()
            || area_radius_m <= 0.0
            || [area_radius_m, a2, a4]
                .iter()
                .any(|value| !value.is_finite() || (*value == 0.0 && value.is_sign_negative()))
        {
            return Err(invalid(
                "profile values must be finite, canonical, and have positive r_A",
            ));
        }
        if a2.abs() + a4.abs() > 0.5 {
            return Err(invalid("profile coefficients violate |a_2| + |a_4| <= 1/2"));
        }
        Ok(Self {
            formula_version: PROFILE_FORMULA_VERSION,
            area_radius_parameter,
            a2_parameter,
            a4_parameter,
            area_radius_m,
            a2,
            a4,
        })
    }
    pub(super) const fn parameters(&self) -> [RawId; 3] {
        [
            self.area_radius_parameter,
            self.a2_parameter,
            self.a4_parameter,
        ]
    }
    pub(super) const fn values(&self) -> [f64; 3] {
        [self.area_radius_m, self.a2, self.a4]
    }
    pub(super) fn radius(&self, angle: f64) -> Result<f64, Diagnostic> {
        if !angle.is_finite() {
            return Err(invalid("profile angle must be finite"));
        }
        let denominator = (1.0 + 0.5 * (self.a2 * self.a2 + self.a4 * self.a4)).sqrt();
        let radius = self.area_radius_m
            * (1.0 + self.a2 * (2.0 * angle).cos() + self.a4 * (4.0 * angle).cos())
            / denominator;
        if !radius.is_finite() || radius <= 0.0 {
            return Err(invalid("profile evaluation is not finite and positive"));
        }
        Ok(radius)
    }
    pub(super) fn analytic_area(&self) -> f64 {
        std::f64::consts::PI * self.area_radius_m * self.area_radius_m
    }
    pub(super) fn bounds(&self) -> [[f64; 2]; 2] {
        let half_width = 10.0 * self.area_radius_m;
        [[-half_width, half_width], [-half_width, half_width]]
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StokesDissipationTopologyRole2d {
    Reference,
    Refined,
}
impl StokesDissipationTopologyRole2d {
    fn contract(self) -> (&'static str, usize, usize) {
        match self {
            Self::Reference => (REFERENCE_TOPOLOGY_ID, 32, 4),
            Self::Refined => (REFINED_TOPOLOGY_ID, 64, 8),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StokesDissipationVertexRecord2d {
    pub(super) id: usize,
    pub(super) ring_index: usize,
    pub(super) angle_index: usize,
    pub(super) ring_fraction: String,
    pub(super) angle_turns: String,
    pub(super) classification: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StokesDissipationCellRecord2d {
    pub(super) id: usize,
    pub(super) vertices: [usize; 3],
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StokesDissipationBoundaryFacetSource2d {
    pub(super) id: usize,
    pub(super) vertices: [usize; 2],
    pub(super) kind: String,
    pub(super) label: String,
    pub(super) orientation: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StokesDissipationBodyCorrespondence2d {
    pub(super) angle_index: usize,
    pub(super) angle_turns: String,
    pub(super) body_vertex: usize,
    pub(super) body_facet: usize,
}
#[derive(Debug, Clone, PartialEq)]
pub(super) struct StokesDissipationTopologySource2d {
    pub(super) role: StokesDissipationTopologyRole2d,
    pub(super) content_identity: String,
    pub(super) sector_count: usize,
    pub(super) radial_interval_count: usize,
    pub(super) vertex_count: usize,
    pub(super) cell_count: usize,
    pub(super) facet_count: usize,
    pub(super) membership_counts: [usize; 3],
    pub(super) vertices: Vec<StokesDissipationVertexRecord2d>,
    pub(super) cells: Vec<StokesDissipationCellRecord2d>,
    pub(super) boundary_facets: Vec<StokesDissipationBoundaryFacetSource2d>,
    pub(super) ordered_body_angles: Vec<String>,
    pub(super) correspondence: Vec<StokesDissipationBodyCorrespondence2d>,
    pub(super) quality_gate: MeshQualityGate,
    pub(super) minimum_signed_area_m2: f64,
    pub(super) minimum_body_clearance_radius_multiple: f64,
    pub(super) coordinate_tolerance_m: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub(super) struct StokesDissipationTopology2d {
    source: StokesDissipationTopologySource2d,
    role: StokesDissipationTopologyRole2d,
    content_identity: String,
    sector_count: usize,
    radial_interval_count: usize,
    reference_mesh: SimplicialMesh,
    entity_sets: BTreeMap<String, Vec<MeshEntity>>,
}
impl StokesDissipationTopology2d {
    pub(super) fn admit(
        source: StokesDissipationTopologySource2d,
        area_radius_m: f64,
    ) -> Result<Self, Diagnostic> {
        let retained_source = source.clone();
        let (identity, sectors, intervals) = source.role.contract();
        if source.content_identity != identity
            || source.sector_count != sectors
            || source.radial_interval_count != intervals
        {
            return Err(invalid(
                "topology role, content identity, or exact N/M bounds differ",
            ));
        }
        if [
            source.minimum_signed_area_m2,
            source.minimum_body_clearance_radius_multiple,
            source.coordinate_tolerance_m,
        ]
        .into_iter()
        .any(|value| !value.is_finite() || value <= 0.0)
        {
            return Err(invalid(
                "topology acceptance predicates must be finite and positive",
            ));
        }
        let vertex_count = sectors * (intervals + 1);
        if source.vertex_count != vertex_count
            || source.cell_count != 2 * sectors * intervals
            || source.facet_count != 2 * sectors
            || source.membership_counts != [sectors, sectors * (intervals - 1), sectors]
            || source.vertices.len() != vertex_count
            || source.vertices.iter().enumerate().any(|(id, record)| {
                record.id != id
                    || record.ring_index != id / sectors
                    || record.angle_index != id % sectors
                    || record.ring_fraction != format!("{}/{}", record.ring_index, intervals)
                    || record.angle_turns != format!("{}/{}", record.angle_index, sectors)
                    || record.classification
                        != match record.ring_index {
                            0 => "body_boundary",
                            ring if ring == intervals => "outer_boundary",
                            _ => "fluid_interior",
                        }
            })
        {
            return Err(invalid(
                "topology counts or vertex records differ from exact symbolic ring/angle content",
            ));
        }
        let expected_cells = expected_cells(sectors, intervals);
        if source.cells.len() != source.cell_count || source.cells != expected_cells {
            return Err(invalid(
                "topology connectivity differs from the exact ordered square-ring triangulation",
            ));
        }
        if source.ordered_body_angles
            != (0..sectors)
                .map(|angle| format!("{angle}/{sectors}"))
                .collect::<Vec<_>>()
            || source.correspondence.len() != sectors
            || source
                .correspondence
                .iter()
                .enumerate()
                .any(|(angle, entry)| {
                    entry.angle_index != angle
                        || entry.angle_turns != format!("{angle}/{sectors}")
                        || entry.body_vertex != angle
                        || entry.body_facet != angle
                })
        {
            return Err(invalid(
                "ordered body angle/correspondence identity differs from exact same-index content",
            ));
        }
        let vertices = reference_vertices(area_radius_m, sectors, intervals)?;
        let expected_facets = expected_boundary_facets(sectors, intervals)?;
        if source.boundary_facets != expected_facets {
            return Err(invalid(
                "boundary facets differ from the exact oriented five-label inventory",
            ));
        }
        let mesh = SimplicialMesh::new(
            2,
            vertices,
            source
                .cells
                .into_iter()
                .map(|cell| Vec::from(cell.vertices))
                .collect(),
            source.quality_gate,
        )?;
        let mut entity_sets = BTreeMap::<String, Vec<MeshEntity>>::new();
        for facet in &source.boundary_facets {
            let mesh_facet = mesh_facet_for_vertices(&mesh, facet.vertices)?;
            let role = semantic_role(&facet.label)?;
            entity_sets
                .entry(role.to_owned())
                .or_default()
                .push(mesh_facet);
        }
        let required = BTreeSet::from([
            "body".to_owned(),
            "outer_x_lower".to_owned(),
            "outer_x_upper".to_owned(),
            "outer_y_lower".to_owned(),
            "outer_y_upper".to_owned(),
        ]);
        if entity_sets.keys().cloned().collect::<BTreeSet<_>>() != required
            || entity_sets.values().any(Vec::is_empty)
        {
            return Err(invalid(
                "topology does not own the complete five-role boundary",
            ));
        }
        Ok(Self {
            source: retained_source,
            role: source.role,
            content_identity: source.content_identity,
            sector_count: sectors,
            radial_interval_count: intervals,
            reference_mesh: mesh,
            entity_sets,
        })
    }
    fn realize(
        &self,
        profile: &StokesDissipationProfileGeometry2d,
        solver: LinearSolveRequest<'_>,
    ) -> Result<RealizedStokesDissipationGeometry2d, Diagnostic> {
        let (augmented, partition, original_vertices) = self.harmonic_auxiliary()?;
        let action = P1HarmonicMeshMotionAction2d::new(&augmented, &partition, solver)?;
        debug_assert_eq!(original_vertices, self.reference_mesh.vertices().len());
        let state = self.harmonic_state(profile, &action)?;
        let mesh = state.reconstruct_mesh(&self.reference_mesh)?;
        require_stokes_dissipation_mesh_predicates(
            profile,
            &mesh,
            &self.reference_mesh,
            self.sector_count,
            self.source.minimum_signed_area_m2,
            self.source.minimum_body_clearance_radius_multiple,
            self.source.coordinate_tolerance_m,
        )?;
        let geometry = chordal_geometry(
            profile,
            self,
            &mesh,
            self.sector_count,
            self.source.coordinate_tolerance_m,
        )?;
        let mesh = SimplicialMeshEnvelopeV1::from_mesh(&mesh)?;
        let correspondence = GeometryMeshCorrespondenceEnvelopeV1::from_region(&geometry, &mesh)?;
        let mut entity_sets = BTreeMap::new();
        for role in [
            "fluid",
            "body",
            "outer_x_lower",
            "outer_x_upper",
            "outer_y_lower",
            "outer_y_upper",
        ] {
            entity_sets.insert(
                role.to_owned(),
                correspondence.region_entity_set_entities(&geometry, role)?,
            );
        }
        Ok(RealizedStokesDissipationGeometry2d {
            motion_action: action,
            state,
            geometry,
            mesh,
            correspondence,
            entity_sets,
        })
    }
    fn harmonic_state(
        &self,
        profile: &StokesDissipationProfileGeometry2d,
        action: &P1HarmonicMeshMotionAction2d,
    ) -> Result<FixedTopologyGeometryState2d, Diagnostic> {
        let (augmented, partition, original_vertices) = self.harmonic_auxiliary()?;
        action.validate_reference(&augmented, &partition)?;
        let mut solid_displacement = vec![[0.0; 2]; augmented.vertices().len()];
        for (angle_index, displacement) in solid_displacement[..self.sector_count]
            .iter_mut()
            .enumerate()
        {
            let angle = std::f64::consts::TAU * angle_index as f64 / self.sector_count as f64;
            let radius = profile.radius(angle)?;
            let start = &augmented.vertices()[angle_index];
            *displacement = [
                radius * angle.cos() - start[0],
                radius * angle.sin() - start[1],
            ];
        }
        let displacement = action.apply(&solid_displacement)?;
        let coordinates = augmented
            .vertices()
            .iter()
            .zip(displacement)
            .take(original_vertices)
            .map(|(reference, displacement)| {
                vec![
                    reference[0] + displacement[0],
                    reference[1] + displacement[1],
                ]
            })
            .collect();
        FixedTopologyGeometryState2d::new(&self.reference_mesh, coordinates)
    }
    fn harmonic_auxiliary(
        &self,
    ) -> Result<(SimplicialMesh, FixedReferenceFsiPartition2d, usize), Diagnostic> {
        let original_vertices = self.reference_mesh.vertices().len();
        let mut vertices = self.reference_mesh.vertices().to_vec();
        let center = vertices.len();
        vertices.push(vec![0.0, 0.0]);
        let mut cells = self.reference_mesh.cells().to_vec();
        let fluid_count = cells.len();
        for angle in 0..self.sector_count {
            cells.push(vec![center, angle, (angle + 1) % self.sector_count]);
        }
        let augmented =
            SimplicialMesh::new(2, vertices, cells, self.reference_mesh.quality_gate())?;
        let fluid_cells = (0..fluid_count).map(CellId::new).collect::<Vec<_>>();
        let solid_cells = (fluid_count..fluid_count + self.sector_count)
            .map(CellId::new)
            .collect::<Vec<_>>();
        let mut interface = (0..self.sector_count)
            .map(|angle| {
                mesh_facet_for_vertices(&augmented, [(angle + 1) % self.sector_count, angle])
                    .map(|facet| FacetId::new(facet.index()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        interface.sort_unstable();
        let partition =
            FixedReferenceFsiPartition2d::new(&augmented, fluid_cells, solid_cells, interface)?;
        Ok((augmented, partition, original_vertices))
    }
}
#[derive(Debug, Clone, PartialEq)]
struct RealizedStokesDissipationGeometry2d {
    motion_action: P1HarmonicMeshMotionAction2d,
    state: FixedTopologyGeometryState2d,
    geometry: GeometryDefinitionV1,
    mesh: SimplicialMeshEnvelopeV1,
    correspondence: GeometryMeshCorrespondenceEnvelopeV1,
    entity_sets: BTreeMap<String, Vec<MeshEntity>>,
}
#[derive(Debug, Clone, PartialEq)]
pub(super) struct StokesDissipationGeometryModelBinding2d {
    profile: StokesDissipationProfileGeometry2d,
    topology: StokesDissipationTopology2d,
    motion_action: P1HarmonicMeshMotionAction2d,
    state: FixedTopologyGeometryState2d,
    geometry: GeometryDefinitionV1,
    mesh: SimplicialMeshEnvelopeV1,
    correspondence: GeometryMeshCorrespondenceEnvelopeV1,
    entity_sets: BTreeMap<String, Vec<MeshEntity>>,
    model: SteadyIncompressibleStokesModel2d,
    model_artifact: ModelArtifactReference,
}
impl StokesDissipationGeometryModelBinding2d {
    pub(super) fn new<F>(
        profile: StokesDissipationProfileGeometry2d,
        topology: &StokesDissipationTopology2d,
        harmonic_solver: LinearSolveRequest<'_>,
        build_model: F,
    ) -> Result<(KernelProgram, Self), Diagnostic>
    where
        F: FnOnce(&GeometryDefinitionV1) -> Result<KernelProgram, Diagnostic>,
    {
        let realized = topology.realize(&profile, harmonic_solver)?;
        let program = build_model(&realized.geometry)?;
        require_profile_parameters(&program, &profile)?;
        let model = lower_stokes_dissipation_profile_model_2d(
            &program,
            &realized.geometry,
            profile.bounds(),
            profile.parameters(),
        )?;
        require_complete_boundary_model(&program, &model, &profile)?;
        let model_artifact = ModelEnvelope::from_program(&program)?.artifact_reference()?;
        if model_artifact.semantic_revision().get() != program.revision().0
            || model_artifact.model() != program.model()
        {
            return Err(invalid(
                "Model artifact identity differs from the exact semantic revision",
            ));
        }
        let binding = Self {
            profile,
            topology: topology.clone(),
            motion_action: realized.motion_action,
            state: realized.state,
            geometry: realized.geometry,
            mesh: realized.mesh,
            correspondence: realized.correspondence,
            entity_sets: realized.entity_sets,
            model,
            model_artifact,
        };
        Ok((program, binding))
    }
    pub(super) const fn mesh(&self) -> &SimplicialMeshEnvelopeV1 {
        &self.mesh
    }
    pub(super) const fn model(&self) -> &SteadyIncompressibleStokesModel2d {
        &self.model
    }
    pub(super) fn entities(&self, role: &str) -> Result<&[MeshEntity], Diagnostic> {
        self.entity_sets
            .get(role)
            .map(Vec::as_slice)
            .ok_or_else(|| invalid(format!("binding has no exact entity-set role `{role}`")))
    }
    pub(super) fn revalidate(&self, program: &KernelProgram) -> Result<(), Diagnostic> {
        let admitted = StokesDissipationTopology2d::admit(
            self.topology.source.clone(),
            self.profile.area_radius_m,
        )?;
        if admitted != self.topology {
            return Err(invalid(
                "binding topology differs from complete exact-content replay",
            ));
        }
        require_profile_parameters(program, &self.profile)?;
        if self
            .topology
            .harmonic_state(&self.profile, &self.motion_action)?
            != self.state
        {
            return Err(invalid(
                "binding harmonic state differs from exact fixed-reference replay",
            ));
        }
        let replayed_mesh = self.state.reconstruct_mesh(&self.topology.reference_mesh)?;
        if SimplicialMeshEnvelopeV1::from_mesh(&replayed_mesh)? != self.mesh {
            return Err(invalid(
                "binding fixed-topology state and retained Mesh identity differ",
            ));
        }
        require_stokes_dissipation_mesh_predicates(
            &self.profile,
            &replayed_mesh,
            &self.topology.reference_mesh,
            self.topology.sector_count,
            self.topology.source.minimum_signed_area_m2,
            self.topology.source.minimum_body_clearance_radius_multiple,
            self.topology.source.coordinate_tolerance_m,
        )?;
        let geometry = chordal_geometry(
            &self.profile,
            &self.topology,
            &replayed_mesh,
            self.topology.sector_count,
            self.topology.source.coordinate_tolerance_m,
        )?;
        if geometry != self.geometry {
            return Err(invalid(
                "binding analytic profile/state and retained chordal Geometry differ",
            ));
        }
        let correspondence =
            GeometryMeshCorrespondenceEnvelopeV1::from_region(&geometry, &self.mesh)?;
        if correspondence != self.correspondence {
            return Err(invalid(
                "binding chordal Geometry and retained Mesh correspondence differ",
            ));
        }
        for role in [
            "fluid",
            "body",
            "outer_x_lower",
            "outer_x_upper",
            "outer_y_lower",
            "outer_y_upper",
        ] {
            if correspondence.region_entity_set_entities(&geometry, role)?
                != *self.entities(role)?
            {
                return Err(invalid(
                    "binding retained entity membership differs from exact correspondence replay",
                ));
            }
        }
        let model = lower_stokes_dissipation_profile_model_2d(
            program,
            &geometry,
            self.profile.bounds(),
            self.profile.parameters(),
        )?;
        require_complete_boundary_model(program, &model, &self.profile)?;
        let model_artifact = ModelEnvelope::from_program(program)?.artifact_reference()?;
        if model != self.model || model_artifact != self.model_artifact {
            return Err(invalid(
                "binding retained Model meaning or exact Model artifact differs from replay",
            ));
        }
        Ok(())
    }
}
fn require_profile_parameters(
    program: &KernelProgram,
    profile: &StokesDissipationProfileGeometry2d,
) -> Result<(), Diagnostic> {
    for ((parameter, expected), dimension) in
        profile.parameters().into_iter().zip(profile.values()).zip([
            LENGTH,
            DimExponents::DIMENSIONLESS,
            DimExponents::DIMENSIONLESS,
        ])
    {
        let Some(KernelNode::Parameter(definition)) = program.node(parameter) else {
            return Err(invalid("profile identity names a non-Parameter Model node"));
        };
        let value = program.value(parameter).unwrap_or(definition.value());
        if value.dim() != dimension || value.value() != expected {
            return Err(invalid(
                "profile identity and exact Model Parameter value/dimension differ",
            ));
        }
    }
    Ok(())
}
fn require_complete_boundary_model(
    program: &KernelProgram,
    model: &SteadyIncompressibleStokesModel2d,
    profile: &StokesDissipationProfileGeometry2d,
) -> Result<(), Diagnostic> {
    if model.bounds() != &profile.bounds()
        || model.geometry_source_digest().is_none()
        || model.boundary_entries().count() != 5
    {
        return Err(invalid(
            "profile Model bounds, geometry identity, or boundary inventory differ",
        ));
    }
    let relation_by_boundary = model
        .boundary_relations()
        .iter()
        .copied()
        .map(|binding| (binding.boundary(), binding.relation()))
        .collect::<BTreeMap<_, _>>();
    if model.boundary_relations().len() != 5
        || relation_by_boundary.len() != 5
        || relation_by_boundary
            .values()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            != 5
    {
        return Err(invalid(
            "profile Model must retain five distinct exact Boundary Relation identities",
        ));
    }
    let body_key = StokesBoundaryKey2d::NamedEntitySet("body".to_owned());
    if model.boundary_entry(&body_key).is_none_or(|entry| {
        entry.disposition != PhysicalBoundaryDisposition::TraceZero
            || !relation_by_boundary.contains_key(&entry.boundary)
    }) {
        return Err(invalid("profile Model body is not exact trace zero"));
    }
    let mut common = None;
    for role in [
        "outer_x_lower",
        "outer_x_upper",
        "outer_y_lower",
        "outer_y_upper",
    ] {
        let key = StokesBoundaryKey2d::NamedEntitySet(role.to_owned());
        let entry = model
            .boundary_entry(&key)
            .ok_or_else(|| invalid("profile Model omits an exact outer Boundary"))?;
        if !matches!(
            entry.disposition,
            PhysicalBoundaryDisposition::Prescribed(law)
                if law.quantity() == PhysicalBoundaryQuantity::Trace
                    && relation_by_boundary.get(&entry.boundary) == Some(&law.relation())
        ) {
            return Err(invalid(
                "profile Model outer Boundary is not prescribed trace",
            ));
        }
        let trace = model
            .prescribed_velocity_trace(&key)
            .filter(|trace| trace.is_complete())
            .ok_or_else(|| invalid("profile Model outer trace is not complete affine potential"))?;
        if common.as_ref().is_some_and(|accepted| accepted != trace) {
            return Err(invalid(
                "outer Boundaries do not retain one exact chi/definition/U identity",
            ));
        }
        common = Some(trace.clone());
    }
    let common = common.expect("four exact outer roles produce one trace");
    let speed = common
        .speed_parameter()
        .expect("complete trace owns one speed Parameter");
    let Some(KernelNode::Parameter(speed_definition)) = program.node(speed) else {
        return Err(invalid("complete trace speed identity is not a Parameter"));
    };
    let speed_value = program.value(speed).unwrap_or(speed_definition.value());
    if speed_value.dim() != VELOCITY || speed_value.value() <= 0.0 {
        return Err(invalid(
            "complete trace speed must be finite positive velocity",
        ));
    }
    let [viscosity] = model.dynamic_viscosity_expression().parameter_fields() else {
        return Err(invalid(
            "profile Model viscosity must retain exactly one Parameter",
        ));
    };
    let viscosity = viscosity.erase();
    let Some(KernelNode::Parameter(viscosity_definition)) = program.node(viscosity) else {
        return Err(invalid("viscosity identity is not a Parameter"));
    };
    let viscosity_value = program
        .value(viscosity)
        .unwrap_or(viscosity_definition.value());
    let mut identities = profile.parameters().into_iter().collect::<BTreeSet<_>>();
    identities.insert(speed);
    identities.insert(viscosity);
    if identities.len() != 5
        || viscosity_value.dim() != VISCOSITY
        || !viscosity_value.value().is_finite()
        || viscosity_value.value() <= 0.0
    {
        return Err(invalid(
            "r_A/a_2/a_4/U/mu identities must be distinct and physically valid",
        ));
    }
    Ok(())
}
fn expected_cells(sectors: usize, intervals: usize) -> Vec<StokesDissipationCellRecord2d> {
    let mut cells = Vec::with_capacity(2 * sectors * intervals);
    for ring in 0..intervals {
        for angle in 0..sectors {
            let next = (angle + 1) % sectors;
            let inner = ring * sectors + angle;
            let inner_next = ring * sectors + next;
            let outer = (ring + 1) * sectors + angle;
            let outer_next = (ring + 1) * sectors + next;
            cells.push(StokesDissipationCellRecord2d {
                id: cells.len(),
                vertices: [inner, outer, outer_next],
            });
            cells.push(StokesDissipationCellRecord2d {
                id: cells.len(),
                vertices: [inner, outer_next, inner_next],
            });
        }
    }
    cells
}
fn reference_vertices(
    area_radius_m: f64,
    sectors: usize,
    intervals: usize,
) -> Result<Vec<Vec<f64>>, Diagnostic> {
    if !area_radius_m.is_finite() || area_radius_m <= 0.0 {
        return Err(invalid("topology requires finite positive r_A"));
    }
    let half_width = 10.0 * area_radius_m;
    let mut vertices = Vec::with_capacity(sectors * (intervals + 1));
    for ring in 0..=intervals {
        let fraction = ring as f64 / intervals as f64;
        for angle_index in 0..sectors {
            let angle = std::f64::consts::TAU * angle_index as f64 / sectors as f64;
            let direction = [angle.cos(), angle.sin()];
            let outer_scale = half_width / direction[0].abs().max(direction[1].abs());
            let body = [area_radius_m * direction[0], area_radius_m * direction[1]];
            let outer = [outer_scale * direction[0], outer_scale * direction[1]];
            vertices.push(vec![
                (1.0 - fraction) * body[0] + fraction * outer[0],
                (1.0 - fraction) * body[1] + fraction * outer[1],
            ]);
        }
    }
    Ok(vertices)
}
fn expected_boundary_facets(
    sectors: usize,
    intervals: usize,
) -> Result<Vec<StokesDissipationBoundaryFacetSource2d>, Diagnostic> {
    let mut facets = Vec::with_capacity(2 * sectors);
    for angle in 0..sectors {
        facets.push(StokesDissipationBoundaryFacetSource2d {
            id: facets.len(),
            vertices: [(angle + 1) % sectors, angle],
            kind: "body".to_owned(),
            label: "body_no_slip".to_owned(),
            orientation: "fluid_domain_boundary_clockwise".to_owned(),
        });
    }
    let outer_start = intervals * sectors;
    for angle in 0..sectors {
        let endpoints = [outer_start + angle, outer_start + (angle + 1) % sectors];
        facets.push(StokesDissipationBoundaryFacetSource2d {
            id: facets.len(),
            vertices: endpoints,
            kind: "outer".to_owned(),
            label: outer_source_label(angle, sectors)?.to_owned(),
            orientation: "fluid_domain_boundary_counterclockwise".to_owned(),
        });
    }
    Ok(facets)
}
fn outer_source_label(angle: usize, sectors: usize) -> Result<&'static str, Diagnostic> {
    if !sectors.is_multiple_of(8) || angle >= sectors {
        return Err(invalid(
            "outer facet has no exact indexed square-side label",
        ));
    }
    match 8 * angle / sectors {
        0 | 7 => Ok("outer_x_plus"),
        1 | 2 => Ok("outer_y_plus"),
        3 | 4 => Ok("outer_x_minus"),
        5 | 6 => Ok("outer_y_minus"),
        _ => unreachable!("angle is in exact sector range"),
    }
}
fn semantic_role(source: &str) -> Result<&'static str, Diagnostic> {
    match source {
        "body_no_slip" => Ok("body"),
        "outer_x_minus" => Ok("outer_x_lower"),
        "outer_x_plus" => Ok("outer_x_upper"),
        "outer_y_minus" => Ok("outer_y_lower"),
        "outer_y_plus" => Ok("outer_y_upper"),
        _ => Err(invalid(
            "topology contains an unknown source boundary label",
        )),
    }
}
fn mesh_facet_for_vertices(
    mesh: &SimplicialMesh,
    endpoints: [usize; 2],
) -> Result<MeshEntity, Diagnostic> {
    let target = BTreeSet::from(endpoints);
    let facet_count = mesh
        .entity_count(1)
        .ok_or_else(|| invalid("topology has no facet stratum"))?;
    let matches = (0..facet_count)
        .map(|index| MeshEntity::new(1, index))
        .filter(|facet| {
            mesh.entity_vertices(*facet).is_some_and(|vertices| {
                vertices
                    .into_iter()
                    .map(|vertex| vertex.index())
                    .collect::<BTreeSet<_>>()
                    == target
            })
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [facet] => Ok(*facet),
        _ => Err(invalid(
            "boundary source facet is missing or duplicated in exact topology",
        )),
    }
}
fn realized_facet_role<'a>(
    topology: &'a StokesDissipationTopology2d,
    mesh: &SimplicialMesh,
    coordinates: &[[f64; 2]],
    endpoints: [usize; 2],
) -> Result<&'a str, Diagnostic> {
    let expected = [coordinates[endpoints[0]], coordinates[endpoints[1]]];
    let facets = (0..mesh.entity_count(1).expect("2D mesh facets"))
        .map(|index| MeshEntity::new(1, index))
        .filter(|facet| {
            let vertices = mesh.entity_vertices(*facet).expect("mesh facet vertices");
            let actual = [
                mesh.vertices()[vertices[0].index()].as_slice(),
                mesh.vertices()[vertices[1].index()].as_slice(),
            ];
            (actual[0] == expected[0] && actual[1] == expected[1])
                || (actual[0] == expected[1] && actual[1] == expected[0])
        })
        .collect::<Vec<_>>();
    let [facet] = facets.as_slice() else {
        return Err(invalid(
            "chordal Geometry edge has no unique topology facet",
        ));
    };
    let roles = topology
        .entity_sets
        .iter()
        .filter_map(|(role, members)| members.contains(facet).then_some(role.as_str()))
        .collect::<Vec<_>>();
    match roles.as_slice() {
        [role] => Ok(*role),
        _ => Err(invalid(
            "chordal Geometry facet has no unique semantic role",
        )),
    }
}
fn chordal_geometry(
    profile: &StokesDissipationProfileGeometry2d,
    topology: &StokesDissipationTopology2d,
    mesh: &SimplicialMesh,
    sectors: usize,
    coordinate_tolerance_m: f64,
) -> Result<GeometryDefinitionV1, Diagnostic> {
    let outer_start = mesh.vertices().len() - sectors;
    let outer = (0..sectors).map(|index| outer_start + index);
    let body = 0..sectors;
    let mut compact = outer
        .chain(body)
        .map(|index| [mesh.vertices()[index][0], mesh.vertices()[index][1]])
        .collect::<Vec<_>>();
    let first = PlanarRegion::new(
        compact.clone(),
        vec![PlanarFace::new(
            (0..sectors).collect(),
            vec![(sectors..2 * sectors).collect()],
        )],
        Vec::new(),
        coordinate_tolerance_m,
    )?;
    compact = first.vertices().to_vec();
    let face = first.faces()[0].clone();
    let outer_loop = face.outer();
    let body_loop = &face.holes()[0];
    let mut sets = BTreeMap::<String, Vec<usize>>::new();
    for edge in 0..outer_loop.len() {
        let endpoints = [outer_loop[edge], outer_loop[(edge + 1) % outer_loop.len()]];
        let role = realized_facet_role(topology, mesh, &compact, endpoints)?;
        if role == "body" {
            return Err(invalid(
                "chordal outer loop is associated with the body role",
            ));
        }
        sets.entry(role.to_owned()).or_default().push(edge);
    }
    for edge in 0..body_loop.len() {
        let endpoints = [body_loop[edge], body_loop[(edge + 1) % body_loop.len()]];
        if realized_facet_role(topology, mesh, &compact, endpoints)? != "body" {
            return Err(invalid(
                "chordal body loop is associated with an outer role",
            ));
        }
    }
    sets.insert(
        "body".to_owned(),
        (outer_loop.len()..outer_loop.len() + body_loop.len()).collect(),
    );
    let mut named = sets
        .into_iter()
        .map(|(name, members)| NamedEntitySet::new(name, EDGE_DIMENSION, members))
        .collect::<Vec<_>>();
    named.push(NamedEntitySet::new("fluid", FACE_DIMENSION, vec![0]));
    let region = PlanarRegion::new(compact, vec![face], named, coordinate_tolerance_m)?;
    let geometry = GeometryDefinitionV1::from_region(&region);
    if profile.analytic_area() <= 0.0 {
        return Err(invalid("analytic profile area is not positive"));
    }
    Ok(geometry)
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(eqiora_core::diagnostic::codes::INVALID_REALIZATION, message)
}
