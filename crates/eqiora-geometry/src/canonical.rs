//! Canonical identity of one authored straight-edged planar geometry.
//!
//! Geometry owns the content and its identity. Artifact crates may wrap this
//! value in their own reference types, but no caller can supply a digest or an
//! entity catalog independently of the validated region from which both are
//! derived.

use core::fmt;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{CanonicalCircularHoleGeometryV1, NamedEntitySet, PlanarFace, PlanarRegion};

const GEOMETRY_DEFINITION_SCHEMA: &str = "eqiora.geometry-definition-envelope/v1";
pub(crate) const CANONICAL_ENCODING: &str = "eqiora.canonical-json/v1";

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}

/// Work budgets for decoding one canonical authored planar geometry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalGeometryLimits {
    /// Maximum encoded bytes accepted by the geometry decoder.
    pub max_bytes: usize,
    /// Maximum vertices in one geometry.
    pub max_vertices: usize,
    /// Maximum planar faces in one geometry.
    pub max_faces: usize,
    /// Maximum vertex indices across all outer and hole loops.
    pub max_loop_indices: usize,
    /// Maximum named entity sets.
    pub max_entity_sets: usize,
    /// Maximum members across all named entity sets.
    pub max_entity_set_members: usize,
}

impl Default for CanonicalGeometryLimits {
    fn default() -> Self {
        Self {
            max_bytes: 4 * 1024 * 1024,
            max_vertices: 1_000_000,
            max_faces: 100_000,
            // PlanarRegion currently validates segment intersections
            // quadratically. Bound the input to that stage directly rather
            // than relying on the encoded-byte cap to make the work finite.
            max_loop_indices: 4_096,
            max_entity_sets: 100_000,
            max_entity_set_members: 4_000_000,
        }
    }
}

/// Opaque canonical content and identity of one planar geometry revision.
///
/// This value can only be derived from a validated [`PlanarRegion`] or from
/// bounded canonical bytes that revalidate to exactly those same bytes. It has
/// no constructor accepting a caller-provided digest or entity-set facts.
#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalGeometryV1 {
    region: PlanarRegion,
    bytes: Vec<u8>,
    digest: [u8; 32],
}

/// Borrowed, kind-erased semantic facts from one canonical geometry.
///
/// This value can only be derived from a canonical geometry owned by this
/// crate. It deliberately exposes neither geometry content nor a constructor
/// from caller-supplied digest and dimension facts.
#[derive(Clone, Copy, PartialEq)]
pub struct CanonicalGeometryRef<'a> {
    kind: CanonicalGeometryKindRef<'a>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum CanonicalGeometryKindRef<'a> {
    StraightEdgedPlanarV1(&'a CanonicalGeometryV1),
    CircularHolePlanarV1(&'a CanonicalCircularHoleGeometryV1),
}

impl<'a> From<&'a CanonicalGeometryV1> for CanonicalGeometryRef<'a> {
    fn from(geometry: &'a CanonicalGeometryV1) -> Self {
        Self {
            kind: CanonicalGeometryKindRef::StraightEdgedPlanarV1(geometry),
        }
    }
}

impl<'a> From<&'a CanonicalCircularHoleGeometryV1> for CanonicalGeometryRef<'a> {
    fn from(geometry: &'a CanonicalCircularHoleGeometryV1) -> Self {
        Self {
            kind: CanonicalGeometryKindRef::CircularHolePlanarV1(geometry),
        }
    }
}

impl fmt::Debug for CanonicalGeometryRef<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CanonicalGeometryRef")
            .field("digest", &self.digest_bytes())
            .field("ambient_dimension", &self.ambient_dimension())
            .field("topological_dimension", &self.topological_dimension())
            .finish_non_exhaustive()
    }
}

impl CanonicalGeometryRef<'_> {
    /// Complete domain-separated content identity.
    #[must_use]
    pub const fn digest_bytes(self) -> [u8; 32] {
        match self.kind {
            CanonicalGeometryKindRef::StraightEdgedPlanarV1(geometry) => geometry.digest_bytes(),
            CanonicalGeometryKindRef::CircularHolePlanarV1(geometry) => geometry.digest_bytes(),
        }
    }

    /// Dimension of the physical coordinate embedding.
    #[must_use]
    pub const fn ambient_dimension(self) -> usize {
        match self.kind {
            CanonicalGeometryKindRef::StraightEdgedPlanarV1(_)
            | CanonicalGeometryKindRef::CircularHolePlanarV1(_) => 2,
        }
    }

    /// Highest topological dimension represented by the geometry.
    #[must_use]
    pub const fn topological_dimension(self) -> usize {
        match self.kind {
            CanonicalGeometryKindRef::StraightEdgedPlanarV1(_)
            | CanonicalGeometryKindRef::CircularHolePlanarV1(_) => 2,
        }
    }

    /// Topological dimension of one exact entity-set name.
    #[must_use]
    pub fn entity_set_dimension(self, name: &str) -> Option<usize> {
        match self.kind {
            CanonicalGeometryKindRef::StraightEdgedPlanarV1(geometry) => geometry
                .region()
                .entity_set(name)
                .map(NamedEntitySet::dimension),
            CanonicalGeometryKindRef::CircularHolePlanarV1(geometry) => {
                geometry.entity_set(name).map(NamedEntitySet::dimension)
            }
        }
    }
}

impl CanonicalGeometryV1 {
    /// Derive canonical bytes and identity from one validated region.
    ///
    /// # Errors
    /// Returns `EQ0901` if canonical JSON serialization unexpectedly fails.
    pub fn from_region(region: &PlanarRegion) -> Result<Self, Diagnostic> {
        let wire = WireGeometryDefinitionV1::from_region(region);
        let bytes = serde_json::to_vec(&wire).map_err(|error| {
            invalid(format!(
                "cannot serialize canonical geometry definition: {error}"
            ))
        })?;
        Ok(Self {
            region: region.clone(),
            digest: digest_with_schema(GEOMETRY_DEFINITION_SCHEMA, &bytes),
            bytes,
        })
    }

    /// Decode one bounded, byte-for-byte canonical geometry definition.
    ///
    /// The input is not retained. It is parsed, resource-checked, revalidated
    /// through [`PlanarRegion::new`], reconstructed from that region, and
    /// accepted only when the reconstructed bytes equal the input exactly.
    ///
    /// # Errors
    /// Returns `EQ0901` for malformed or unknown wire data, resource excess,
    /// invalid geometry, or any noncanonical encoding.
    pub fn decode_canonical(
        bytes: &[u8],
        limits: CanonicalGeometryLimits,
    ) -> Result<Self, Diagnostic> {
        if bytes.len() > limits.max_bytes {
            return Err(invalid(format!(
                "geometry definition has {} bytes, exceeding the {} byte decoder limit",
                bytes.len(),
                limits.max_bytes
            )));
        }
        let wire: WireGeometryDefinitionV1 = serde_json::from_slice(bytes)
            .map_err(|error| invalid(format!("invalid geometry definition JSON: {error}")))?;
        wire.check_contract()?;
        wire.check_limits(limits)?;
        let region = wire.into_region()?;
        let canonical = Self::from_region(&region)?;
        if canonical.bytes != bytes {
            return Err(invalid(
                "geometry definition JSON is not the canonical encoding of its content",
            ));
        }
        Ok(canonical)
    }

    /// Validated canonical region content.
    #[must_use]
    pub const fn region(&self) -> &PlanarRegion {
        &self.region
    }

    /// Exact compact canonical JSON bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Complete domain-separated SHA-256 identity bytes.
    #[must_use]
    pub const fn digest_bytes(&self) -> [u8; 32] {
        self.digest
    }
}

pub(crate) fn digest_with_schema(schema: &str, bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(schema.as_bytes());
    hasher.update([0]);
    hasher.update(bytes);
    hasher.finalize().into()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
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

impl WireGeometryDefinitionV1 {
    fn from_region(region: &PlanarRegion) -> Self {
        Self {
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
                .map(WireEntitySet::from_set)
                .collect(),
        }
    }

    fn check_contract(&self) -> Result<(), Diagnostic> {
        if self.schema != GEOMETRY_DEFINITION_SCHEMA
            || self.encoding != CANONICAL_ENCODING
            || self.kind != WireGeometryKind::StraightEdgedPlanarV1
            || self.length_unit != WireLengthUnit::Metre
        {
            return Err(invalid(
                "unsupported geometry definition schema, encoding, kind, or unit",
            ));
        }
        Ok(())
    }

    fn check_limits(&self, limits: CanonicalGeometryLimits) -> Result<(), Diagnostic> {
        if self.vertices.len() > limits.max_vertices {
            return Err(invalid("geometry vertex count exceeds decoder limits"));
        }
        if self.faces.len() > limits.max_faces {
            return Err(invalid("geometry face count exceeds decoder limits"));
        }
        let loop_indices = self.faces.iter().try_fold(0_usize, |total, face| {
            std::iter::once(&face.outer)
                .chain(face.holes.iter())
                .try_fold(total, |total, loop_indices| {
                    total.checked_add(loop_indices.len())
                })
        });
        if loop_indices.is_none_or(|count| count > limits.max_loop_indices) {
            return Err(invalid("geometry loop-index count exceeds decoder limits"));
        }
        if self.entity_sets.len() > limits.max_entity_sets {
            return Err(invalid("geometry entity-set count exceeds decoder limits"));
        }
        let entity_set_members = self
            .entity_sets
            .iter()
            .try_fold(0_usize, |total, set| total.checked_add(set.members.len()));
        if entity_set_members.is_none_or(|count| count > limits.max_entity_set_members) {
            return Err(invalid(
                "geometry entity-set member count exceeds decoder limits",
            ));
        }
        Ok(())
    }

    fn into_region(self) -> Result<PlanarRegion, Diagnostic> {
        PlanarRegion::new(
            self.vertices,
            self.faces
                .into_iter()
                .map(|face| PlanarFace::new(face.outer, face.holes))
                .collect(),
            self.entity_sets
                .into_iter()
                .map(|set| NamedEntitySet::new(set.name, set.dimension, set.members))
                .collect(),
            self.tolerance_m,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireGeometryKind {
    StraightEdgedPlanarV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WireLengthUnit {
    Metre,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct WireFace {
    outer: Vec<usize>,
    holes: Vec<Vec<usize>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct WireEntitySet {
    pub(crate) name: String,
    pub(crate) dimension: usize,
    pub(crate) members: Vec<usize>,
}

impl WireEntitySet {
    pub(crate) fn from_set(set: &NamedEntitySet) -> Self {
        Self {
            name: set.name().to_owned(),
            dimension: set.dimension(),
            members: set.members().to_vec(),
        }
    }
}
