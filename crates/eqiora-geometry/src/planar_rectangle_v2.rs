//! Classification-free canonical identity of one exact planar rectangle.

use std::collections::BTreeSet;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use serde::{Deserialize, Serialize};

use crate::canonical::{CANONICAL_ENCODING, WireEntitySet, WireLengthUnit, digest_with_schema};
use crate::region::canonical_entity_sets;
use crate::{CanonicalGeometryLimits, EDGE_DIMENSION, FACE_DIMENSION, NamedEntitySet};

pub(crate) const SCHEMA: &str = "eqiora.planar-rectangle-envelope/v2";
const CORNER_COUNT: usize = 4;
const BOUNDARY_COUNT: usize = 4;
const FACE_COUNT: usize = 1;

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}

/// Exact axis-aligned rectangle without producer or classification policy.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CanonicalPlanarRectangleGeometryV2 {
    bounds: [[f64; 2]; 2],
    entity_sets: Vec<NamedEntitySet>,
    bytes: Vec<u8>,
    digest: [u8; 32],
}

impl CanonicalPlanarRectangleGeometryV2 {
    pub(crate) fn new(
        mut bounds: [[f64; 2]; 2],
        entity_sets: Vec<NamedEntitySet>,
    ) -> Result<Self, Diagnostic> {
        for axis in &mut bounds {
            for value in axis.iter_mut() {
                if *value == 0.0 {
                    *value = 0.0;
                }
            }
            if !axis[0].is_finite() || !axis[1].is_finite() || axis[0] >= axis[1] {
                return Err(invalid(
                    "planar rectangle bounds must be finite strict intervals",
                ));
            }
        }
        let entity_sets =
            canonical_entity_sets(entity_sets, CORNER_COUNT, BOUNDARY_COUNT, FACE_COUNT)?;
        validate_complete_membership(&entity_sets)?;
        let wire = WirePlanarRectangleV2 {
            schema: SCHEMA.to_owned(),
            encoding: CANONICAL_ENCODING.to_owned(),
            length_unit: WireLengthUnit::Metre,
            bounds,
            entity_sets: entity_sets.iter().map(WireEntitySet::from_set).collect(),
        };
        let bytes = serde_json::to_vec(&wire).map_err(|error| {
            invalid(format!(
                "cannot serialize canonical planar rectangle geometry: {error}"
            ))
        })?;
        Ok(Self {
            bounds,
            entity_sets,
            digest: digest_with_schema(SCHEMA, &bytes),
            bytes,
        })
    }

    pub(crate) fn decode_canonical(
        bytes: &[u8],
        limits: CanonicalGeometryLimits,
    ) -> Result<Self, Diagnostic> {
        if bytes.len() > limits.max_bytes {
            return Err(invalid(
                "planar rectangle geometry exceeds the decoder byte limit",
            ));
        }
        let wire: WirePlanarRectangleV2 = serde_json::from_slice(bytes)
            .map_err(|error| invalid(format!("invalid planar rectangle geometry JSON: {error}")))?;
        if wire.schema != SCHEMA
            || wire.encoding != CANONICAL_ENCODING
            || wire.length_unit != WireLengthUnit::Metre
        {
            return Err(invalid("unsupported planar rectangle geometry contract"));
        }
        if wire.entity_sets.len() > limits.max_entity_sets
            || wire
                .entity_sets
                .iter()
                .map(|set| set.members.len())
                .sum::<usize>()
                > limits.max_entity_set_members
        {
            return Err(invalid("planar rectangle entity-set budget exceeded"));
        }
        let canonical = Self::new(
            wire.bounds,
            wire.entity_sets
                .into_iter()
                .map(|set| NamedEntitySet::new(set.name, set.dimension, set.members))
                .collect(),
        )?;
        if canonical.canonical_bytes() != bytes {
            return Err(invalid("planar rectangle JSON is not canonical"));
        }
        Ok(canonical)
    }

    pub(crate) const fn bounds(&self) -> &[[f64; 2]; 2] {
        &self.bounds
    }

    pub(crate) fn entity_sets(&self) -> &[NamedEntitySet] {
        &self.entity_sets
    }

    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) const fn digest_bytes(&self) -> [u8; 32] {
        self.digest
    }
}

fn validate_complete_membership(entity_sets: &[NamedEntitySet]) -> Result<(), Diagnostic> {
    let mut covered = BTreeSet::new();
    for set in entity_sets {
        if set.dimension() != EDGE_DIMENSION && set.dimension() != FACE_DIMENSION {
            return Err(invalid(
                "planar rectangle names may contain only edges or faces",
            ));
        }
        for &member in set.members() {
            if !covered.insert((set.dimension(), member)) {
                return Err(invalid(
                    "planar rectangle membership must be named exactly once",
                ));
            }
        }
    }
    let expected = BTreeSet::from([
        (EDGE_DIMENSION, 0),
        (EDGE_DIMENSION, 1),
        (EDGE_DIMENSION, 2),
        (EDGE_DIMENSION, 3),
        (FACE_DIMENSION, 0),
    ]);
    if covered != expected {
        return Err(invalid(
            "named planar rectangle topology must cover the result exactly once",
        ));
    }
    Ok(())
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WirePlanarRectangleV2 {
    schema: String,
    encoding: String,
    length_unit: WireLengthUnit,
    bounds: [[f64; 2]; 2],
    entity_sets: Vec<WireEntitySet>,
}
