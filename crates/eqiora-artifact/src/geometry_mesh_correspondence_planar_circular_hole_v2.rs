//! Direct source correspondence for planar circular-hole Geometry v2.

use std::collections::BTreeSet;

use eqiora_core::Diagnostic;
use eqiora_geometry::{CanonicalGeometryV1, EDGE_DIMENSION, FACE_DIMENSION};
use eqiora_meshing::{MeshEntity, MeshQualityGate};
use serde::{Deserialize, Serialize};

use super::{CORRESPONDENCE_SCHEMA, GeometryMeshCorrespondenceEnvelopeV1, WireCorrespondenceV1};
use crate::circular_hole_chordal_reference::EXACT_BOUNDARY_COUNT;
use crate::{
    ArtifactDigest, CANONICAL_ENCODING, GeometryDecoderLimits, SimplicialMeshEnvelopeV1,
    invalid_artifact,
};

const PLANAR_CIRCULAR_HOLE_V2_SOURCE: &str = "planar-circular-hole-v2";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SourceParentOutward {
    LeftOfCanonicalFacet,
    RightOfCanonicalFacet,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SourceFrontierLineage {
    pub(crate) facet_index: usize,
    pub(crate) parent_outward: SourceParentOutward,
}

/// Private result of one Mesh construction pass with structural source lineage.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PlanarCircularHoleV2Reference {
    pub(crate) mesh: eqiora_meshing::SimplicialMesh,
    pub(crate) face_cells: Vec<usize>,
    pub(crate) frontiers: [Vec<SourceFrontierLineage>; EXACT_BOUNDARY_COUNT],
}

impl PlanarCircularHoleV2Reference {
    pub(super) const fn mesh(&self) -> &eqiora_meshing::SimplicialMesh {
        &self.mesh
    }

    pub(super) fn face_cells(&self) -> &[usize] {
        &self.face_cells
    }

    pub(super) fn frontiers(&self) -> &[Vec<SourceFrontierLineage>; EXACT_BOUNDARY_COUNT] {
        &self.frontiers
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WirePlanarCircularHoleV2CorrespondenceV1 {
    schema: String,
    encoding: String,
    source: String,
    pub(super) geometry_sha256: String,
    pub(super) mesh_sha256: String,
    dimension: u64,
    faces: Vec<WireFaceAssignment>,
    frontiers: Vec<WireFrontierAssignment>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireFaceAssignment {
    geometry_face: u64,
    cell_indices: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireFrontierAssignment {
    parent_face: u64,
    geometry_edge: u64,
    facet_indices: Vec<u64>,
    parent_outward: Vec<WireFacetOutward>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireFacetOutward {
    LeftOfCanonicalFacet,
    RightOfCanonicalFacet,
}

impl GeometryMeshCorrespondenceEnvelopeV1 {
    /// Generate one deterministic chordal reference Mesh and its complete
    /// direct source correspondence from planar circular-hole Geometry v2.
    ///
    /// The approximation request is replay input for the existing bounded
    /// reference producer; it is not stored in this structural correspondence.
    /// Source face/frontier assignments are emitted from topology construction,
    /// without a `PlanarRegion`, mesh labels, coordinate classification, or a
    /// classification tolerance.
    ///
    /// # Errors
    /// Returns `EQ0901` unless `geometry` is the exact v2 kind and the bounded
    /// reference request produces an accepted Mesh with complete lineage.
    pub fn from_planar_circular_hole_v2_reference(
        geometry: &CanonicalGeometryV1,
        requested_max_boundary_error_m: f64,
        max_segments: usize,
        quality_gate: MeshQualityGate,
    ) -> Result<(SimplicialMeshEnvelopeV1, Self), Diagnostic> {
        require_planar_circular_hole_v2(geometry)?;
        let reference = PlanarCircularHoleV2Reference::from_exact(
            geometry,
            requested_max_boundary_error_m,
            max_segments,
            quality_gate,
        )?;
        let mesh = SimplicialMeshEnvelopeV1::from_mesh(reference.mesh())?;
        let wire = planar_circular_hole_v2_wire_from_reference(geometry, &mesh, &reference)?;
        let envelope = Self {
            wire: WireCorrespondenceV1::PlanarCircularHoleV2(wire),
        };
        envelope.validate_local(GeometryDecoderLimits::default())?;
        Ok((mesh, envelope))
    }

    /// Generate direct source correspondence for an independently supplied
    /// Mesh only after exact deterministic reference replay.
    ///
    /// This is the resource-verification seam for callers that admit Geometry
    /// and Mesh separately. New producer paths should prefer
    /// [`Self::from_planar_circular_hole_v2_reference`] so Mesh and lineage
    /// come from the same construction pass.
    ///
    /// # Errors
    /// Returns `EQ0901` unless `geometry` is the exact v2 kind and `mesh` is
    /// byte-equivalent in content to the regenerated reference Mesh for the
    /// supplied bounded request.
    pub fn from_planar_circular_hole_v2_reference_mesh(
        geometry: &CanonicalGeometryV1,
        mesh: &SimplicialMeshEnvelopeV1,
        requested_max_boundary_error_m: f64,
        max_segments: usize,
    ) -> Result<Self, Diagnostic> {
        let wire = generate_planar_circular_hole_v2_wire(
            geometry,
            mesh,
            requested_max_boundary_error_m,
            max_segments,
        )?;
        let envelope = Self {
            wire: WireCorrespondenceV1::PlanarCircularHoleV2(wire),
        };
        envelope.validate_local(GeometryDecoderLimits::default())?;
        Ok(envelope)
    }

    /// Replay a direct v2 source correspondence against independently admitted
    /// canonical Geometry and Mesh resources and regenerated producer lineage.
    ///
    /// # Errors
    /// Returns `EQ0901` for source-kind or identity substitution, Mesh drift,
    /// incomplete or relabelled membership, orientation drift, or a reference
    /// producer replay failure.
    pub fn validate_against_planar_circular_hole_v2_reference(
        &self,
        geometry: &CanonicalGeometryV1,
        mesh: &SimplicialMeshEnvelopeV1,
        requested_max_boundary_error_m: f64,
        max_segments: usize,
    ) -> Result<(), Diagnostic> {
        self.validate_local(GeometryDecoderLimits::default())?;
        let WireCorrespondenceV1::PlanarCircularHoleV2(actual) = &self.wire else {
            return Err(invalid_artifact(
                "correspondence was not emitted from planar circular-hole Geometry v2",
            ));
        };
        let expected = generate_planar_circular_hole_v2_wire(
            geometry,
            mesh,
            requested_max_boundary_error_m,
            max_segments,
        )?;
        if actual != &expected {
            return Err(invalid_artifact(
                "planar circular-hole v2 correspondence differs from exact source and Mesh replay",
            ));
        }
        Ok(())
    }

    /// Mesh entities that exactly realize one named planar circular-hole v2
    /// source entity set.
    ///
    /// Exact resource replay remains the caller's admission step after
    /// decoding untrusted correspondence bytes.
    ///
    /// # Errors
    /// Returns `EQ0901` when this is not the v2 source variant, the Geometry
    /// identity differs, or the named set is absent or incomplete.
    pub fn planar_circular_hole_v2_entity_set_entities(
        &self,
        geometry: &CanonicalGeometryV1,
        name: &str,
    ) -> Result<Vec<MeshEntity>, Diagnostic> {
        let WireCorrespondenceV1::PlanarCircularHoleV2(wire) = &self.wire else {
            return Err(invalid_artifact(
                "correspondence has no planar circular-hole v2 entity sets",
            ));
        };
        require_planar_circular_hole_v2(geometry)?;
        if wire.geometry_sha256 != ArtifactDigest::from_sha256(geometry.digest_bytes()).to_string()
        {
            return Err(invalid_artifact(
                "correspondence geometry digest differs from planar circular-hole Geometry v2",
            ));
        }
        let set = geometry.entity_set(name).ok_or_else(|| {
            invalid_artifact(format!(
                "planar circular-hole Geometry v2 has no entity set named '{name}'"
            ))
        })?;
        resolve_planar_circular_hole_v2_entity_set(wire, set.dimension(), set.members())
    }
}

fn generate_planar_circular_hole_v2_wire(
    geometry: &CanonicalGeometryV1,
    mesh: &SimplicialMeshEnvelopeV1,
    requested_max_boundary_error_m: f64,
    max_segments: usize,
) -> Result<WirePlanarCircularHoleV2CorrespondenceV1, Diagnostic> {
    require_planar_circular_hole_v2(geometry)?;
    let reference = PlanarCircularHoleV2Reference::from_exact(
        geometry,
        requested_max_boundary_error_m,
        max_segments,
        mesh.mesh().quality_gate(),
    )?;
    if reference.mesh() != mesh.mesh() {
        return Err(invalid_artifact(
            "supplied Mesh differs from deterministic planar circular-hole v2 reference replay",
        ));
    }
    planar_circular_hole_v2_wire_from_reference(geometry, mesh, &reference)
}

fn planar_circular_hole_v2_wire_from_reference(
    geometry: &CanonicalGeometryV1,
    mesh: &SimplicialMeshEnvelopeV1,
    reference: &PlanarCircularHoleV2Reference,
) -> Result<WirePlanarCircularHoleV2CorrespondenceV1, Diagnostic> {
    let faces = vec![WireFaceAssignment {
        geometry_face: 0,
        cell_indices: reference
            .face_cells()
            .iter()
            .map(|&cell| portable(cell, "mesh cell"))
            .collect::<Result<Vec<_>, _>>()?,
    }];
    let frontiers = reference
        .frontiers()
        .iter()
        .enumerate()
        .map(|(geometry_edge, lineage)| {
            Ok(WireFrontierAssignment {
                parent_face: 0,
                geometry_edge: portable(geometry_edge, "geometry edge")?,
                facet_indices: lineage
                    .iter()
                    .map(|entry| portable(entry.facet_index, "mesh facet"))
                    .collect::<Result<Vec<_>, _>>()?,
                parent_outward: lineage
                    .iter()
                    .map(|entry| match entry.parent_outward {
                        SourceParentOutward::LeftOfCanonicalFacet => {
                            WireFacetOutward::LeftOfCanonicalFacet
                        }
                        SourceParentOutward::RightOfCanonicalFacet => {
                            WireFacetOutward::RightOfCanonicalFacet
                        }
                    })
                    .collect(),
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    Ok(WirePlanarCircularHoleV2CorrespondenceV1 {
        schema: CORRESPONDENCE_SCHEMA.to_owned(),
        encoding: CANONICAL_ENCODING.to_owned(),
        source: PLANAR_CIRCULAR_HOLE_V2_SOURCE.to_owned(),
        geometry_sha256: ArtifactDigest::from_sha256(geometry.digest_bytes()).to_string(),
        mesh_sha256: mesh.digest()?.to_string(),
        dimension: 2,
        faces,
        frontiers,
    })
}

fn require_planar_circular_hole_v2(geometry: &CanonicalGeometryV1) -> Result<(), Diagnostic> {
    let replayed = CanonicalGeometryV1::decode_planar_circular_hole_v2_canonical(
        geometry.canonical_bytes(),
        Default::default(),
    )
    .map_err(|_| invalid_artifact("correspondence requires planar circular-hole Geometry v2"))?;
    if replayed != *geometry {
        return Err(invalid_artifact(
            "planar circular-hole Geometry v2 differs from canonical replay",
        ));
    }
    Ok(())
}
impl WirePlanarCircularHoleV2CorrespondenceV1 {
    pub(super) fn validate_local(&self, limits: GeometryDecoderLimits) -> Result<(), Diagnostic> {
        if self.schema != CORRESPONDENCE_SCHEMA
            || self.encoding != CANONICAL_ENCODING
            || self.source != PLANAR_CIRCULAR_HOLE_V2_SOURCE
            || self.dimension != 2
        {
            return Err(invalid_artifact(
                "unsupported planar circular-hole v2 correspondence schema, encoding, source, or dimension",
            ));
        }
        ArtifactDigest::from_hex(self.geometry_sha256.clone())?;
        ArtifactDigest::from_hex(self.mesh_sha256.clone())?;
        if self.faces.len() != 1
            || self.faces[0].geometry_face != 0
            || self.faces[0].cell_indices.is_empty()
            || !strictly_sorted_u64(&self.faces[0].cell_indices)
            || self.frontiers.len() != 5
        {
            return Err(invalid_artifact(
                "planar circular-hole v2 correspondence requires one complete face and five source frontiers",
            ));
        }
        let canonical_frontiers = self.frontiers.iter().enumerate().all(|(edge, frontier)| {
            frontier.parent_face == 0
                && usize::try_from(frontier.geometry_edge) == Ok(edge)
                && !frontier.facet_indices.is_empty()
                && strictly_sorted_u64(&frontier.facet_indices)
                && frontier.facet_indices.len() == frontier.parent_outward.len()
        });
        let unique_facets = self
            .frontiers
            .iter()
            .flat_map(|frontier| frontier.facet_indices.iter().copied())
            .collect::<BTreeSet<_>>();
        let frontier_memberships = self
            .frontiers
            .iter()
            .map(|frontier| frontier.facet_indices.len())
            .sum::<usize>();
        if !canonical_frontiers || unique_facets.len() != frontier_memberships {
            return Err(invalid_artifact(
                "planar circular-hole v2 frontiers must be nonempty, canonical, oriented, and disjoint",
            ));
        }
        let assignment_count = self
            .faces
            .len()
            .checked_add(self.frontiers.len())
            .ok_or_else(|| invalid_artifact("source assignment count overflows usize"))?;
        let membership_count = self.faces[0]
            .cell_indices
            .len()
            .checked_add(frontier_memberships)
            .ok_or_else(|| invalid_artifact("source membership count overflows usize"))?;
        if assignment_count > limits.max_geometry_entities
            || membership_count > limits.max_geometry_mesh_memberships
        {
            return Err(invalid_artifact(
                "planar circular-hole v2 assignments or memberships exceed decoder limits",
            ));
        }
        Ok(())
    }
}
fn resolve_planar_circular_hole_v2_entity_set(
    wire: &WirePlanarCircularHoleV2CorrespondenceV1,
    dimension: usize,
    members: &[usize],
) -> Result<Vec<MeshEntity>, Diagnostic> {
    let mut entities = BTreeSet::new();
    match dimension {
        EDGE_DIMENSION => {
            for &member in members {
                let assignment = wire
                    .frontiers
                    .iter()
                    .find(|entry| usize::try_from(entry.geometry_edge) == Ok(member))
                    .ok_or_else(|| {
                        invalid_artifact(
                            "planar circular-hole v2 source edge set is not fully realized",
                        )
                    })?;
                for &facet in &assignment.facet_indices {
                    entities.insert(MeshEntity::new(EDGE_DIMENSION, local(facet, "mesh facet")?));
                }
            }
        }
        FACE_DIMENSION => {
            for &member in members {
                let assignment = wire
                    .faces
                    .iter()
                    .find(|entry| usize::try_from(entry.geometry_face) == Ok(member))
                    .ok_or_else(|| {
                        invalid_artifact(
                            "planar circular-hole v2 source face set is not fully realized",
                        )
                    })?;
                for &cell in &assignment.cell_indices {
                    entities.insert(MeshEntity::new(FACE_DIMENSION, local(cell, "mesh cell")?));
                }
            }
        }
        _ => {
            return Err(invalid_artifact(
                "planar circular-hole v2 entity set dimension is unsupported",
            ));
        }
    }
    Ok(entities.into_iter().collect())
}

fn strictly_sorted_u64(values: &[u64]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn portable(value: usize, label: &str) -> Result<u64, Diagnostic> {
    u64::try_from(value)
        .map_err(|_| invalid_artifact(format!("{label} index exceeds portable u64")))
}

fn local(value: u64, label: &str) -> Result<usize, Diagnostic> {
    usize::try_from(value)
        .map_err(|_| invalid_artifact(format!("{label} index exceeds local usize")))
}
