//! Content identity of one authored continuous geometry.
//!
//! The region itself lives in [`eqiora_geometry::PlanarRegion`], which owns its
//! topology, embedding, canonical form and validation. This module owns only
//! what makes it an artifact: a schema, a canonical encoding, and a
//! domain-separated digest a Model can reference.
//!
//! That split follows the one already drawn by
//! [`GeometryIdentityEnvelopeV1`](crate::GeometryIdentityEnvelopeV1), whose
//! entity handle is likewise a geometry-crate type. Geometry decides what a
//! shape is; this crate decides what naming one costs.

use eqiora_core::Diagnostic;
use eqiora_geometry::{NamedEntitySet, PlanarFace, PlanarRegion};
use serde::{Deserialize, Serialize};

use crate::{ArtifactDigest, CANONICAL_ENCODING, invalid_artifact};

const GEOMETRY_DEFINITION_SCHEMA: &str = "eqiora.geometry-definition-envelope/v1";

/// A content-addressed authored geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct GeometryDefinitionV1 {
    wire: WireGeometryDefinitionV1,
}

impl GeometryDefinitionV1 {
    /// Wrap one already canonical region as an addressable artifact.
    ///
    /// The region cannot be invalid, because [`PlanarRegion`] admits nothing
    /// invalid, so this cannot fail.
    #[must_use]
    pub fn from_region(region: &PlanarRegion) -> Self {
        Self {
            wire: WireGeometryDefinitionV1 {
                schema: GEOMETRY_DEFINITION_SCHEMA.to_owned(),
                encoding: CANONICAL_ENCODING.to_owned(),
                kind: WireGeometryKind::StraightEdgedPlanarV1,
                length_unit: WireLengthUnit::Metre,
                tolerance_m: region.tolerance_m(),
                vertices: region.vertices().to_vec(),
                faces: region
                    .faces()
                    .iter()
                    .map(|face| WireFace {
                        outer: face.outer().to_vec(),
                        holes: face.holes().to_vec(),
                    })
                    .collect(),
                entity_sets: region
                    .entity_sets()
                    .iter()
                    .map(|set| WireEntitySet {
                        name: set.name().to_owned(),
                        dimension: set.dimension(),
                        members: set.members().to_vec(),
                    })
                    .collect(),
            },
        }
    }

    /// Replay the region this artifact encodes.
    ///
    /// Decoding revalidates rather than trusting the bytes, so an artifact
    /// edited after it was written cannot re-enter as a region.
    ///
    /// # Errors
    /// Returns `EQ0901` for an unsupported schema, encoding, kind or unit, or
    /// for any content [`PlanarRegion`] refuses.
    pub fn region(&self) -> Result<PlanarRegion, Diagnostic> {
        if self.wire.schema != GEOMETRY_DEFINITION_SCHEMA
            || self.wire.encoding != CANONICAL_ENCODING
            || self.wire.kind != WireGeometryKind::StraightEdgedPlanarV1
            || self.wire.length_unit != WireLengthUnit::Metre
        {
            return Err(invalid_artifact(
                "unsupported geometry definition schema, encoding, kind, or unit",
            ));
        }
        PlanarRegion::new(
            self.wire.vertices.clone(),
            self.wire
                .faces
                .iter()
                .map(|face| PlanarFace::new(face.outer.clone(), face.holes.clone()))
                .collect(),
            self.wire
                .entity_sets
                .iter()
                .map(|set| NamedEntitySet::new(&set.name, set.dimension, set.members.clone()))
                .collect(),
            self.wire.tolerance_m,
        )
    }

    /// Canonical encoding of this geometry.
    ///
    /// # Errors
    /// Returns `EQ0901` if canonical serialization fails.
    pub fn canonical_json(&self) -> Result<Vec<u8>, Diagnostic> {
        serde_json::to_vec(&self.wire).map_err(|error| {
            invalid_artifact(format!("cannot serialize geometry definition: {error}"))
        })
    }

    /// Domain-separated content identity of this exact geometry.
    ///
    /// # Errors
    /// Returns `EQ0901` if canonical serialization fails.
    pub fn digest(&self) -> Result<ArtifactDigest, Diagnostic> {
        Ok(ArtifactDigest::compute(
            GEOMETRY_DEFINITION_SCHEMA.as_bytes(),
            &self.canonical_json()?,
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct WireGeometryDefinitionV1 {
    schema: String,
    encoding: String,
    kind: WireGeometryKind,
    length_unit: WireLengthUnit,
    tolerance_m: f64,
    vertices: Vec<[f64; 2]>,
    faces: Vec<WireFace>,
    entity_sets: Vec<WireEntitySet>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireGeometryKind {
    StraightEdgedPlanarV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireLengthUnit {
    Metre,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct WireFace {
    outer: Vec<usize>,
    holes: Vec<Vec<usize>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct WireEntitySet {
    name: String,
    dimension: usize,
    members: Vec<usize>,
}

#[cfg(test)]
mod tests {
    use eqiora_geometry::{EDGE_DIMENSION, FACE_DIMENSION};

    use super::*;

    fn square_with_hole() -> PlanarRegion {
        PlanarRegion::new(
            vec![
                [0.0, 0.0],
                [1.0, 0.0],
                [1.0, 1.0],
                [0.0, 1.0],
                [0.25, 0.25],
                [0.75, 0.25],
                [0.75, 0.75],
                [0.25, 0.75],
            ],
            vec![PlanarFace::new(vec![0, 1, 2, 3], vec![vec![4, 5, 6, 7]])],
            vec![
                NamedEntitySet::new("exterior", EDGE_DIMENSION, vec![0, 1, 2, 3]),
                NamedEntitySet::new("hole", EDGE_DIMENSION, vec![4, 5, 6, 7]),
                NamedEntitySet::new("fluid", FACE_DIMENSION, vec![0]),
            ],
            1.0e-9,
        )
        .expect("a square with a square hole is a region")
    }

    fn filled_square() -> PlanarRegion {
        PlanarRegion::new(
            vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            vec![PlanarFace::new(vec![0, 1, 2, 3], Vec::new())],
            vec![NamedEntitySet::new(
                "exterior",
                EDGE_DIMENSION,
                vec![0, 1, 2, 3],
            )],
            1.0e-9,
        )
        .expect("a square is a region")
    }

    #[test]
    fn one_region_has_one_digest() {
        assert_eq!(
            GeometryDefinitionV1::from_region(&square_with_hole())
                .digest()
                .unwrap(),
            GeometryDefinitionV1::from_region(&square_with_hole())
                .digest()
                .unwrap()
        );
    }

    #[test]
    fn filling_the_hole_changes_the_digest() {
        // The hole is part of what the geometry is, not decoration on it.
        assert_ne!(
            GeometryDefinitionV1::from_region(&square_with_hole())
                .digest()
                .unwrap(),
            GeometryDefinitionV1::from_region(&filled_square())
                .digest()
                .unwrap()
        );
    }

    #[test]
    fn an_artifact_replays_to_the_region_it_encodes() {
        let region = square_with_hole();
        let replayed = GeometryDefinitionV1::from_region(&region).region().unwrap();
        assert_eq!(replayed, region);
    }

    #[test]
    fn a_tampered_artifact_is_refused_rather_than_replayed() {
        let mut artifact = GeometryDefinitionV1::from_region(&square_with_hole());
        artifact.wire.faces[0].holes.clear();
        // The entity sets still name the hole's four edges, which the region
        // no longer has, so replay must refuse rather than silently shrink.
        assert!(
            artifact
                .region()
                .unwrap_err()
                .message()
                .contains("does not exist")
        );
    }

    #[test]
    fn an_unsupported_schema_is_refused() {
        let mut artifact = GeometryDefinitionV1::from_region(&filled_square());
        artifact.wire.schema = "eqiora.geometry-definition-envelope/v2".to_owned();
        assert!(
            artifact
                .region()
                .unwrap_err()
                .message()
                .contains("unsupported geometry definition")
        );
    }
}
