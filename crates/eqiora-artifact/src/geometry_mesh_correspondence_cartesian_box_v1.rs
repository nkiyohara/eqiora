//! Direct source correspondence for exact Cartesian-box Geometry v1.

use std::collections::BTreeSet;

use eqiora_core::Diagnostic;
use eqiora_geometry::CanonicalGeometryV1;
use eqiora_meshing::{CartesianMesh, MeshEntity, MeshTopology};
use serde::{Deserialize, Serialize};

use super::{CORRESPONDENCE_SCHEMA, GeometryMeshCorrespondenceEnvelopeV1, WireCorrespondenceV1};
use crate::{
    ArtifactDigest, CANONICAL_ENCODING, CartesianMeshEnvelopeV1, GeometryDecoderLimits,
    invalid_artifact,
};

const SOURCE: &str = "cartesian-box-v1-structured-cartesian-v2";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireCartesianBoxV1CorrespondenceV1 {
    schema: String,
    encoding: String,
    source: String,
    pub(super) geometry_sha256: String,
    pub(super) mesh_sha256: String,
    dimension: u64,
    body: WireBodyAssignment,
    sides: Vec<WireSideAssignment>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireBodyAssignment {
    geometry_body: u64,
    cell_indices: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSideAssignment {
    geometry_side: u64,
    axis: u64,
    side: WireSide,
    facet_indices: Vec<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireSide {
    Lower,
    Upper,
}

impl GeometryMeshCorrespondenceEnvelopeV1 {
    /// Generate one exact uniform Cartesian Mesh and direct box correspondence.
    pub fn from_cartesian_box_v1(
        geometry: &CanonicalGeometryV1,
        cells: &[usize],
    ) -> Result<(CartesianMeshEnvelopeV1, Self), Diagnostic> {
        let bounds = require_cartesian_box_v1(geometry)?;
        if bounds.len() != cells.len() {
            return Err(invalid_artifact(
                "Cartesian box dimension differs from its cell-count policy",
            ));
        }
        let native = CartesianMesh::uniform(bounds, cells)?;
        let mesh = CartesianMeshEnvelopeV1::from_mesh(&native)?;
        let wire = wire_from_resources(geometry, &mesh, cells)?;
        let correspondence = Self {
            wire: WireCorrespondenceV1::CartesianBoxV1(wire),
        };
        correspondence.validate_local(GeometryDecoderLimits::default())?;
        Ok((mesh, correspondence))
    }

    /// Replay the complete exact Cartesian-box production relation.
    pub fn validate_against_cartesian_box_v1(
        &self,
        geometry: &CanonicalGeometryV1,
        mesh: &CartesianMeshEnvelopeV1,
        cells: &[usize],
    ) -> Result<(), Diagnostic> {
        let (expected_mesh, expected) = Self::from_cartesian_box_v1(geometry, cells)?;
        if mesh != &expected_mesh || self != &expected {
            return Err(invalid_artifact(
                "Cartesian-box resources differ from exact replay",
            ));
        }
        Ok(())
    }

    /// Mesh entities realizing one exact Cartesian-box entity set.
    pub fn cartesian_box_v1_entity_set_entities(
        &self,
        geometry: &CanonicalGeometryV1,
        name: &str,
    ) -> Result<Vec<MeshEntity>, Diagnostic> {
        let WireCorrespondenceV1::CartesianBoxV1(wire) = &self.wire else {
            return Err(invalid_artifact(
                "correspondence has no Cartesian-box entity sets",
            ));
        };
        let bounds = require_cartesian_box_v1(geometry)?;
        if wire.geometry_sha256 != ArtifactDigest::from_sha256(geometry.digest_bytes()).to_string()
        {
            return Err(invalid_artifact(
                "correspondence geometry digest differs from Cartesian-box Geometry",
            ));
        }
        let dimension = bounds.len();
        let set = geometry.entity_set(name).ok_or_else(|| {
            invalid_artifact(format!(
                "Cartesian-box Geometry has no entity set named '{name}'"
            ))
        })?;
        let mut entities = BTreeSet::new();
        if set.dimension() == dimension && set.members() == [0] {
            for &cell in &wire.body.cell_indices {
                entities.insert(MeshEntity::new(dimension, local(cell, "mesh cell")?));
            }
        } else if set.dimension() + 1 == dimension {
            for &member in set.members() {
                let side = wire
                    .sides
                    .get(member)
                    .filter(|side| usize::try_from(side.geometry_side) == Ok(member))
                    .ok_or_else(|| invalid_artifact("Cartesian source side is absent"))?;
                for &facet in &side.facet_indices {
                    entities.insert(MeshEntity::new(dimension - 1, local(facet, "mesh facet")?));
                }
            }
        } else {
            return Err(invalid_artifact(
                "Cartesian-box entity set differs from its exact source topology",
            ));
        }
        Ok(entities.into_iter().collect())
    }
}

fn wire_from_resources(
    geometry: &CanonicalGeometryV1,
    mesh: &CartesianMeshEnvelopeV1,
    cells: &[usize],
) -> Result<WireCartesianBoxV1CorrespondenceV1, Diagnostic> {
    let dimension = require_cartesian_box_v1(geometry)?.len();
    let native = mesh.mesh();
    if mesh.dimension() != dimension || native.topological_dimension() != dimension {
        return Err(invalid_artifact(
            "Cartesian-box correspondence dimension differs from its Mesh",
        ));
    }
    let cell_count = native
        .entity_count(dimension)
        .ok_or_else(|| invalid_artifact("Cartesian Mesh omits its top-cell stratum"))?;
    let facet_count = native
        .entity_count(dimension - 1)
        .ok_or_else(|| invalid_artifact("Cartesian Mesh omits its facet stratum"))?;
    let mut side_facets = vec![Vec::new(); dimension * 2];
    for facet_index in 0..facet_count {
        let facet = MeshEntity::new(dimension - 1, facet_index);
        let free_axes = native
            .entity_free_axes(facet)
            .ok_or_else(|| invalid_artifact("Cartesian facet has no structural axes"))?;
        let axis = (0..dimension)
            .find(|axis| free_axes.binary_search(axis).is_err())
            .ok_or_else(|| invalid_artifact("Cartesian facet has no fixed structural axis"))?;
        let vertex = native
            .entity_vertices(facet)
            .and_then(|vertices| vertices.first().copied())
            .ok_or_else(|| invalid_artifact("Cartesian facet has no vertex closure"))?;
        let anchor = native
            .vertex_multi_index(vertex)
            .and_then(|indices| indices.get(axis).copied())
            .ok_or_else(|| invalid_artifact("Cartesian facet has no structural anchor"))?;
        let side = if anchor == 0 {
            Some(0)
        } else if anchor == cells[axis] {
            Some(1)
        } else {
            None
        };
        if let Some(side) = side {
            side_facets[axis * 2 + side].push(portable(facet_index, "mesh facet")?);
        }
    }
    let sides = side_facets
        .into_iter()
        .enumerate()
        .map(|(member, facet_indices)| WireSideAssignment {
            geometry_side: member as u64,
            axis: (member / 2) as u64,
            side: if member % 2 == 0 {
                WireSide::Lower
            } else {
                WireSide::Upper
            },
            facet_indices,
        })
        .collect();
    Ok(WireCartesianBoxV1CorrespondenceV1 {
        schema: CORRESPONDENCE_SCHEMA.to_owned(),
        encoding: CANONICAL_ENCODING.to_owned(),
        source: SOURCE.to_owned(),
        geometry_sha256: ArtifactDigest::from_sha256(geometry.digest_bytes()).to_string(),
        mesh_sha256: mesh.digest()?.to_string(),
        dimension: portable(dimension, "dimension")?,
        body: WireBodyAssignment {
            geometry_body: 0,
            cell_indices: (0..cell_count)
                .map(|cell| portable(cell, "mesh cell"))
                .collect::<Result<Vec<_>, _>>()?,
        },
        sides,
    })
}

fn require_cartesian_box_v1(geometry: &CanonicalGeometryV1) -> Result<&[[f64; 2]], Diagnostic> {
    let replayed = CanonicalGeometryV1::decode_cartesian_box_v1_canonical(
        geometry.canonical_bytes(),
        Default::default(),
    )
    .map_err(|_| invalid_artifact("CartesianMesher requires exact Cartesian-box Geometry"))?;
    if replayed != *geometry {
        return Err(invalid_artifact(
            "Cartesian-box Geometry differs from canonical replay",
        ));
    }
    geometry
        .cartesian_box_bounds()
        .ok_or_else(|| invalid_artifact("CartesianMesher requires exact Cartesian-box Geometry"))
}

impl WireCartesianBoxV1CorrespondenceV1 {
    pub(super) fn validate_local(&self, limits: GeometryDecoderLimits) -> Result<(), Diagnostic> {
        let dimension = local(self.dimension, "dimension")?;
        if self.schema != CORRESPONDENCE_SCHEMA
            || self.encoding != CANONICAL_ENCODING
            || self.source != SOURCE
            || !(1..=3).contains(&dimension)
            || self.body.geometry_body != 0
            || self.body.cell_indices.is_empty()
            || !strictly_sorted(&self.body.cell_indices)
            || self.sides.len() != dimension * 2
        {
            return Err(invalid_artifact(
                "unsupported or incomplete Cartesian-box correspondence",
            ));
        }
        ArtifactDigest::from_hex(self.geometry_sha256.clone())?;
        ArtifactDigest::from_hex(self.mesh_sha256.clone())?;
        let canonical = self.sides.iter().enumerate().all(|(member, side)| {
            usize::try_from(side.geometry_side) == Ok(member)
                && usize::try_from(side.axis) == Ok(member / 2)
                && side.side
                    == if member % 2 == 0 {
                        WireSide::Lower
                    } else {
                        WireSide::Upper
                    }
                && !side.facet_indices.is_empty()
                && strictly_sorted(&side.facet_indices)
        });
        let frontier_memberships = self
            .sides
            .iter()
            .map(|side| side.facet_indices.len())
            .sum::<usize>();
        let unique = self
            .sides
            .iter()
            .flat_map(|side| side.facet_indices.iter().copied())
            .collect::<BTreeSet<_>>();
        let memberships = self
            .body
            .cell_indices
            .len()
            .checked_add(frontier_memberships)
            .ok_or_else(|| invalid_artifact("Cartesian membership count overflows usize"))?;
        if !canonical
            || unique.len() != frontier_memberships
            || self.sides.len() + 1 > limits.max_geometry_entities
            || memberships > limits.max_geometry_mesh_memberships
        {
            return Err(invalid_artifact(
                "Cartesian-box assignments are noncanonical or exceed limits",
            ));
        }
        Ok(())
    }
}

fn strictly_sorted(values: &[u64]) -> bool {
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
