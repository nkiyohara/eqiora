//! Exact outer-polyhedron correspondence for one conforming tetrahedral mesh.

#[path = "geometry_mesh_correspondence_polyhedra/assignments.rs"]
mod assignments;
#[path = "geometry_mesh_correspondence_polyhedra/exact.rs"]
mod exact;
#[path = "geometry_mesh_correspondence_polyhedra/wire.rs"]
mod wire;

use super::{CORRESPONDENCE_SCHEMA, GeometryMeshCorrespondenceEnvelopeV1, WireCorrespondenceV1};
use crate::{
    ArtifactDigest, CANONICAL_ENCODING, GeometryDecoderLimits, GeometryDefinitionV1,
    SimplicialMeshEnvelopeV1, invalid_artifact,
};
use eqiora_core::Diagnostic;
use eqiora_meshing::MeshEntity;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const SOURCE: &str = "convex-polyhedra-tetrahedra-v1";
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WirePolyhedra {
    schema: String,
    encoding: String,
    source: String,
    pub(super) geometry_sha256: String,
    pub(super) mesh_sha256: String,
    dimension: u64,
    vertices: Vec<Assignment>,
    edges: Vec<Assignment>,
    volumes: Vec<Assignment>,
    frontiers: Vec<Frontier>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Assignment {
    geometry_entity: u64,
    mesh_entities: Vec<u64>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Frontier {
    parent_volume: u64,
    geometry_facet: u64,
    facet_indices: Vec<u64>,
    parent_outward: Vec<Orientation>,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Orientation {
    AlongCanonicalFacetNormal,
    AgainstCanonicalFacetNormal,
}

impl GeometryMeshCorrespondenceEnvelopeV1 {
    /// Derive exact outer-polyhedron membership for a conforming tetrahedral Mesh.
    ///
    /// Each positive tetrahedron belongs to one exact convex Geometry volume;
    /// cell interiors cannot overlap. Every parent-relative frontier realizes
    /// exactly one complete source facet and retains its outward incidence.
    /// Mesh subdivision vertices never become Geometry entities. Source vertices,
    /// edges, facets, and volumes all retain the Geometry owner's canonical IDs.
    /// Classification uses exact binary-rational signs; precision cannot excuse
    /// a cell outside Geometry. Work is bounded to 4096 tetrahedra, 16384 vertices,
    /// and 262144 exact point/plane/projection predicates.
    ///
    /// # Errors
    /// Returns `EQ0901` for wrong family/dimension, incomplete or ambiguous
    /// membership, overlap, wrong incidence, or exhausted resource bounds.
    pub fn from_polyhedra(
        geometry: &GeometryDefinitionV1,
        mesh: &SimplicialMeshEnvelopeV1,
    ) -> Result<Self, Diagnostic> {
        let assignments = assignments::generate(geometry.canonical(), mesh.mesh())?;
        let result = Self {
            wire: WireCorrespondenceV1::Polyhedra(WirePolyhedra {
                schema: CORRESPONDENCE_SCHEMA.to_owned(),
                encoding: CANONICAL_ENCODING.to_owned(),
                source: SOURCE.to_owned(),
                geometry_sha256: geometry.digest()?.to_string(),
                mesh_sha256: mesh.digest()?.to_string(),
                dimension: 3,
                vertices: assignments.vertices,
                edges: assignments.edges,
                volumes: assignments.volumes,
                frontiers: assignments.frontiers,
            }),
        };
        result.validate_local(GeometryDecoderLimits::default())?;
        Ok(result)
    }
    /// Reconstruct all memberships and incidences from the exact referenced resources.
    /// # Errors
    /// Returns `EQ0901` for foreign/stale resources or any membership/orientation drift.
    pub fn validate_against_polyhedra(
        &self,
        geometry: &GeometryDefinitionV1,
        mesh: &SimplicialMeshEnvelopeV1,
    ) -> Result<(), Diagnostic> {
        self.validate_local(GeometryDecoderLimits::default())?;
        if !matches!(self.wire, WireCorrespondenceV1::Polyhedra(_))
            || self.geometry_artifact() != geometry.digest()?
            || self.mesh_artifact() != mesh.digest()?
        {
            return Err(invalid_artifact(
                "polyhedral correspondence requires the exact Geometry and Mesh digests",
            ));
        }
        if self != &Self::from_polyhedra(geometry, mesh)? {
            return Err(invalid_artifact(
                "polyhedral correspondence differs from exact membership and outward-incidence replay",
            ));
        }
        Ok(())
    }
    /// Mesh entities realizing one exact named polyhedral Geometry selection.
    /// Untrusted decoded correspondence must first pass `validate_against_polyhedra`.
    /// # Errors
    /// Returns `EQ0901` for wrong family, foreign Geometry, or missing membership.
    pub fn polyhedral_entity_set_entities(
        &self,
        geometry: &GeometryDefinitionV1,
        name: &str,
    ) -> Result<Vec<MeshEntity>, Diagnostic> {
        let WireCorrespondenceV1::Polyhedra(wire) = &self.wire else {
            return Err(invalid_artifact(
                "correspondence has no polyhedral selections",
            ));
        };
        if self.geometry_artifact() != geometry.digest()? {
            return Err(invalid_artifact(
                "polyhedral selection Geometry digest differs",
            ));
        }
        let set = geometry
            .canonical()
            .entity_set(name)
            .ok_or_else(|| invalid_artifact("polyhedral Geometry selection is absent"))?;
        let mut result = BTreeSet::new();
        for &member in set.members() {
            let member = u64::try_from(member)
                .map_err(|_| invalid_artifact("polyhedral Geometry index exceeds u64"))?;
            let ids = match set.dimension() {
                0 | 1 | 3 => {
                    let rows = match set.dimension() {
                        0 => &wire.vertices,
                        1 => &wire.edges,
                        _ => &wire.volumes,
                    };
                    rows.iter()
                        .find(|row| row.geometry_entity == member)
                        .map(|row| row.mesh_entities.clone())
                        .ok_or_else(|| {
                            invalid_artifact("polyhedral selection membership is missing")
                        })?
                }
                2 => {
                    let rows = wire
                        .frontiers
                        .iter()
                        .filter(|row| row.geometry_facet == member)
                        .collect::<Vec<_>>();
                    if rows.is_empty() {
                        return Err(invalid_artifact(
                            "polyhedral frontier membership is missing",
                        ));
                    }
                    rows.into_iter()
                        .flat_map(|row| row.facet_indices.iter().copied())
                        .collect()
                }
                _ => {
                    return Err(invalid_artifact(
                        "polyhedral selection has an invalid dimension",
                    ));
                }
            };
            for id in ids {
                result.insert(
                    usize::try_from(id)
                        .map_err(|_| invalid_artifact("mesh membership exceeds local usize"))?,
                );
            }
        }
        Ok(result
            .into_iter()
            .map(|id| MeshEntity::new(set.dimension(), id))
            .collect())
    }
}
